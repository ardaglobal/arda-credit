///! Loan Tape API handlers
///!
///! This module provides endpoints for loan tape operations, which combine
///! deal data with payment statistics, company information, and computed metrics.
///!
///! Endpoints:
///! - POST /api/v1/loan-tape - Create single loan tape record
///! - POST /api/v1/loan-tape/bulk - Create multiple loan tape records
///! - GET /api/v1/loan-tape - List loan tape records with pagination
///! - GET /api/v1/loan-tape/{id} - Get single loan tape record
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use crate::dto::loan_tape_dto::{
    BulkCreateLoanTapeRequest, CreateLoanTapeRequest, LoanTapeListResponse, LoanTapeRecord,
};
use crate::middleware::session_middleware::extract_current_user;
use arda_credit_db::{
    count_loan_tape_records, create_loan_tape_record, get_loan_tape_record_by_id,
    get_loan_tape_records,
};

// ============================================================================
// REQUEST / RESPONSE TYPES
// ============================================================================

/// Query parameters for listing loan tape records
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListLoanTapeQuery {
    /// Filter by deal status (optional)
    #[param(example = "active")]
    pub status: Option<String>,

    /// Maximum number of records to return
    #[param(example = 20)]
    pub limit: Option<i64>,

    /// Number of records to skip for pagination
    #[param(example = 0)]
    pub offset: Option<i64>,
}

/// Generic error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Single loan tape record response (wrapper for consistency)
#[derive(Debug, Serialize, ToSchema)]
pub struct LoanTapeResponse {
    pub record: LoanTapeRecord,
}

/// Bulk create response
#[derive(Debug, Serialize, ToSchema)]
pub struct BulkLoanTapeResponse {
    pub success: bool,
    pub created_count: usize,
    pub records: Vec<LoanTapeRecord>,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Create a single loan tape record
///
/// Creates a new deal with comprehensive loan tape fields including
/// collateral information, credit metrics, and blockchain tracking.
#[utoipa::path(
    post,
    path = "/api/v1/loan-tape",
    request_body = CreateLoanTapeRequest,
    responses(
        (status = 201, description = "Loan tape record created successfully", body = LoanTapeResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state))]
pub async fn create_loan_tape_handler(
    State(state): State<ApiState>,
    Json(req): Json<CreateLoanTapeRequest>,
) -> Result<(StatusCode, Json<LoanTapeResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Creating loan tape record: originator={}, investor={:?}, amount={}",
        req.originator_id, req.investor_id, req.principal_amount
    );

    // Parse state transition hash from hex string if provided
    let state_hash = req
        .state_transition_hash
        .as_ref()
        .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok());

    // Create loan tape record in database
    let deal = create_loan_tape_record(
        &state.pool,
        req.originator_id,
        req.investor_id,
        req.principal_amount,
        req.outstanding_balance,
        req.interest_rate_bps,
        &req.deal_status,
        req.originated_at,
        req.maturity_date,
        req.custom_deal_id.as_deref(),
        req.title.as_deref(),
        req.description.as_deref(),
        req.payment_frequency.as_deref(),
        req.collateral_type.as_deref(),
        req.collateral_value,
        req.ltv_ratio,
        req.credit_score,
        req.dscr,
        req.risk_level.as_deref(),
        req.asset_class.as_deref(),
        req.region.as_deref(),
        req.industry.as_deref(),
        state_hash,
        req.proof_included.unwrap_or(false),
    )
    .await
    .map_err(|e| {
        error!("Failed to create loan tape record: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to create loan tape record".to_string(),
                details: Some(e.to_string()),
            }),
        )
    })?;

    // Get full loan tape record with joined data
    let record = get_loan_tape_record_by_id(&state.pool, deal.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch created loan tape record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Record created but failed to fetch details".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?
        .ok_or_else(|| {
            error!("Created loan tape record not found");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Record created but not found".to_string(),
                    details: None,
                }),
            )
        })?;

    // Convert to DTO
    let dto_record = convert_db_record_to_dto(record);

    info!(
        "Loan tape record created successfully: id={}",
        dto_record.loan_id
    );

    Ok((
        StatusCode::CREATED,
        Json(LoanTapeResponse { record: dto_record }),
    ))
}

/// Create multiple loan tape records in bulk
///
/// Creates multiple deal records with comprehensive loan tape fields.
/// All records are created in a single transaction for atomicity.
#[utoipa::path(
    post,
    path = "/api/v1/loan-tape/bulk",
    request_body = BulkCreateLoanTapeRequest,
    responses(
        (status = 201, description = "Loan tape records created successfully", body = BulkLoanTapeResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state))]
pub async fn create_bulk_loan_tape_handler(
    State(state): State<ApiState>,
    Json(req): Json<BulkCreateLoanTapeRequest>,
) -> Result<(StatusCode, Json<BulkLoanTapeResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.records.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No records provided".to_string(),
                details: Some("Request must contain at least one record".to_string()),
            }),
        ));
    }

    info!("Creating {} loan tape records in bulk", req.records.len());

    // TODO: Consider wrapping in a transaction for true atomicity
    let mut created_records = Vec::new();

    for (idx, record_req) in req.records.iter().enumerate() {
        let state_hash = record_req
            .state_transition_hash
            .as_ref()
            .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok());

        let deal = create_loan_tape_record(
            &state.pool,
            record_req.originator_id,
            record_req.investor_id,
            record_req.principal_amount,
            record_req.outstanding_balance,
            record_req.interest_rate_bps,
            &record_req.deal_status,
            record_req.originated_at,
            record_req.maturity_date,
            record_req.custom_deal_id.as_deref(),
            record_req.title.as_deref(),
            record_req.description.as_deref(),
            record_req.payment_frequency.as_deref(),
            record_req.collateral_type.as_deref(),
            record_req.collateral_value,
            record_req.ltv_ratio,
            record_req.credit_score,
            record_req.dscr,
            record_req.risk_level.as_deref(),
            record_req.asset_class.as_deref(),
            record_req.region.as_deref(),
            record_req.industry.as_deref(),
            state_hash,
            record_req.proof_included.unwrap_or(false),
        )
        .await
        .map_err(|e| {
            error!("Failed to create loan tape record at index {}: {}", idx, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create record at index {}", idx),
                    details: Some(e.to_string()),
                }),
            )
        })?;

        if let Some(record) = get_loan_tape_record_by_id(&state.pool, deal.id)
            .await
            .ok()
            .flatten()
        {
            created_records.push(convert_db_record_to_dto(record));
        }
    }

    info!(
        "Successfully created {} loan tape records",
        created_records.len()
    );

    Ok((
        StatusCode::CREATED,
        Json(BulkLoanTapeResponse {
            success: true,
            created_count: created_records.len(),
            records: created_records,
        }),
    ))
}

/// List loan tape records with pagination and filtering
///
/// Retrieves loan tape records with full detail including:
/// - Company information (originator/investor names and KYB status)
/// - Computed metrics (APR, days to maturity)
/// - Payment statistics (aggregated from deal_payments table)
#[utoipa::path(
    get,
    path = "/api/v1/loan-tape",
    params(ListLoanTapeQuery),
    responses(
        (status = 200, description = "Loan tape records retrieved successfully", body = LoanTapeListResponse),
        (status = 401, description = "Unauthorized - authentication required", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    security(
        ("bearer" = [])
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state, headers))]
pub async fn list_loan_tape_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ListLoanTapeQuery>,
) -> Result<Json<LoanTapeListResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Authenticate user
    let _user = extract_current_user(&headers).ok_or_else(|| {
        error!("Authentication failed: missing or invalid session");
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                details: Some("Missing or invalid session token".to_string()),
            }),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100); // Cap at 100
    let offset = query.offset.unwrap_or(0);

    info!(
        "Listing loan tape records: status={:?}, limit={}, offset={}",
        query.status, limit, offset
    );

    // Get total count
    let total = count_loan_tape_records(&state.pool, query.status.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to count loan tape records: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to count records".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Get records
    let records = get_loan_tape_records(&state.pool, query.status.as_deref(), limit, offset)
        .await
        .map_err(|e| {
            error!("Failed to fetch loan tape records: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch records".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let dto_records: Vec<LoanTapeRecord> =
        records.into_iter().map(convert_db_record_to_dto).collect();

    Ok(Json(LoanTapeListResponse {
        records: dto_records.clone(),
        total,
        count: dto_records.len(),
        offset,
        limit,
    }))
}

/// Get a single loan tape record by ID
///
/// Retrieves a complete loan tape record with all joined and computed fields.
#[utoipa::path(
    get,
    path = "/api/v1/loan-tape/{id}",
    params(
        ("id" = Uuid, Path, description = "Deal UUID")
    ),
    responses(
        (status = 200, description = "Loan tape record retrieved successfully", body = LoanTapeResponse),
        (status = 401, description = "Unauthorized - authentication required", body = ErrorResponse),
        (status = 404, description = "Loan tape record not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    security(
        ("bearer" = [])
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state, headers))]
pub async fn get_loan_tape_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<LoanTapeResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Authenticate user
    let _user = extract_current_user(&headers).ok_or_else(|| {
        error!("Authentication failed: missing or invalid session");
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                details: Some("Missing or invalid session token".to_string()),
            }),
        )
    })?;

    info!("Fetching loan tape record: id={}", id);

    let record = get_loan_tape_record_by_id(&state.pool, id)
        .await
        .map_err(|e| {
            error!("Failed to fetch loan tape record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch record".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Loan tape record not found".to_string(),
                    details: Some(format!("No record found with ID: {}", id)),
                }),
            )
        })?;

    let dto_record = convert_db_record_to_dto(record);

    Ok(Json(LoanTapeResponse { record: dto_record }))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert database LoanTapeRecord to DTO LoanTapeRecord
fn convert_db_record_to_dto(record: arda_credit_db::LoanTapeRecord) -> LoanTapeRecord {
    LoanTapeRecord {
        loan_id: record.loan_id,
        deal_id: record.deal_id,
        originator_id: record.originator_id,
        originator_name: record.originator_name,
        originator_kyb_status: record.originator_kyb_status,
        investor_id: record.investor_id,
        investor_name: record.investor_name,
        investor_kyb_status: record.investor_kyb_status,
        principal_amount: record.principal_amount,
        outstanding_balance: record.outstanding_balance,
        interest_rate_bps: record.interest_rate_bps,
        interest_rate_apr: record.interest_rate_apr,
        loan_status: record.loan_status,
        originated_at: record.originated_at,
        maturity_date: record.maturity_date,
        days_to_maturity: record.days_to_maturity,
        payment_frequency: record.payment_frequency,
        last_payment_date: record.last_payment_date,
        last_payment_amount: record.last_payment_amount,
        total_payments_made: record.total_payments_made,
        total_interest_paid: record.total_interest_paid,
        collateral_type: record.collateral_type,
        collateral_value: record.collateral_value,
        ltv_ratio: record.ltv_ratio,
        risk_level: record.risk_level,
        asset_class: record.asset_class,
        region: record.region,
        borrower_industry: record.borrower_industry,
        credit_score: record.credit_score,
        dscr: record.dscr,
        deal_title: record.deal_title,
        state_transition_hash: record
            .state_transition_hash
            .map(|h| format!("0x{}", hex::encode(h))),
        proof_included: record.proof_included,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}
