///! Loan Tape API handlers
///!
///! This module provides endpoints for loan tape operations, which combine
///! deal data with payment statistics, company information, and computed metrics.
///!
///! Endpoints:
///! - POST /api/v1/loan-tape/bulk - Upload loan tape portfolio (CSV → JSON)
///! - GET /api/v1/loan-tape - List loan tape records with pagination
///! - GET /api/v1/loan-tape/{id} - Get single loan tape record
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use crate::dto::loan_tape_dto::{BulkCreateLoanTapeRequest, LoanTapeListResponse, LoanTapeRecord};
use crate::middleware::session_middleware::extract_current_user;
use arda_credit_db::{
    business::company::{
        create_company_with_membership, get_company_by_name, get_user_membership, ProfileType,
    },
    count_loan_tape_records, create_loan_tape_record, get_identity_by_id,
    get_loan_tape_record_by_id, get_loan_tape_records,
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

/// Upload loan tape portfolio (bulk create)
///
/// Uploads a portfolio of deals from a CSV file (converted to JSON by client).
/// The system auto-detects the consistent party (originator or investor) and:
/// - Validates the user exists and is associated with the consistent company
/// - Creates companies as needed (consistent + counterparties)
/// - Creates all deal records
///
/// Requirements:
/// - All records must have the same originator OR same investor (single-party portfolio)
/// - User must be a member of the consistent company (or it will be created)
#[utoipa::path(
    post,
    path = "/api/v1/loan-tape/bulk",
    request_body = BulkCreateLoanTapeRequest,
    responses(
        (status = 201, description = "Loan tape records created successfully", body = BulkLoanTapeResponse),
        (status = 400, description = "Invalid request - multi-party portfolio or validation failed", body = ErrorResponse),
        (status = 403, description = "Forbidden - user not associated with consistent company", body = ErrorResponse),
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

    info!(
        "Processing loan tape upload: user_id={}, record_count={}",
        req.user_id,
        req.records.len()
    );

    // Step 1: Validate user exists
    let user = get_identity_by_id(&state.pool, req.user_id)
        .await
        .map_err(|e| {
            error!(
                "User validation failed: user_id={}, error={}",
                req.user_id, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup user".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?
        .ok_or_else(|| {
            error!("User not found: user_id={}", req.user_id);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid user ID".to_string(),
                    details: Some(format!("User {} not found", req.user_id)),
                }),
            )
        })?;

    info!("User validated: email={}", user.email);

    // Step 2: Auto-detect consistent party
    let mut originator_names: HashSet<String> = HashSet::new();
    let mut investor_names: HashSet<String> = HashSet::new();

    for record in &req.records {
        originator_names.insert(record.originator_name.clone());
        investor_names.insert(record.investor_name.clone());
    }

    let (consistent_role, consistent_name) = if originator_names.len() == 1
        && investor_names.len() > 1
    {
        (
            "originator",
            originator_names.iter().next().unwrap().clone(),
        )
    } else if investor_names.len() == 1 && originator_names.len() > 1 {
        ("investor", investor_names.iter().next().unwrap().clone())
    } else if originator_names.len() == 1 && investor_names.len() == 1 {
        // Edge case: both are consistent (single deal or same counterparty)
        // Default to originator as consistent party
        (
            "originator",
            originator_names.iter().next().unwrap().clone(),
        )
    } else {
        error!(
            "Multi-party portfolio detected: originator_count={}, investor_count={}",
            originator_names.len(),
            investor_names.len()
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Multi-party portfolio not supported".to_string(),
                details: Some(format!(
                    "Upload must contain portfolio for a single party. Found {} unique originators and {} unique investors.",
                    originator_names.len(),
                    investor_names.len()
                )),
            }),
        ));
    };

    let profile_type = if consistent_role == "originator" {
        ProfileType::Originator
    } else {
        ProfileType::Investor
    };

    info!(
        "Detected consistent party: role={}, name={}, profile_type={:?}",
        consistent_role, consistent_name, profile_type
    );

    // Step 3: Get or create consistent company and validate/create user membership
    let consistent_company = match get_company_by_name(&state.pool, &consistent_name)
        .await
        .map_err(|e| {
            error!("Failed to query company by name: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup company".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })? {
        Some(company) => {
            info!(
                "Consistent company exists: id={}, name={}",
                company.id, company.name
            );

            // Verify user has membership with correct profile type
            match get_user_membership(&state.pool, company.id, req.user_id, profile_type).await {
                Ok(membership) => {
                    info!(
                        "User has valid membership: role={:?}, status={:?}",
                        membership.role, membership.status
                    );
                }
                Err(_) => {
                    error!(
                        "User {} not a member of company {} with profile {:?}",
                        req.user_id, company.id, profile_type
                    );
                    return Err((
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: "User not authorized for this company".to_string(),
                            details: Some(format!(
                                "User must be a member of company '{}' with {} profile to upload its portfolio",
                                consistent_name, consistent_role
                            )),
                        }),
                    ));
                }
            }

            company
        }
        None => {
            info!(
                "Consistent company does not exist, creating: name={}",
                consistent_name
            );

            // Create company and add user as admin member
            let (company, membership) = create_company_with_membership(
                &state.pool,
                &consistent_name,
                req.user_id,
                profile_type,
            )
            .await
            .map_err(|e| {
                error!("Failed to create company with membership: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create company".to_string(),
                        details: Some(e.to_string()),
                    }),
                )
            })?;

            info!(
                "Created company and membership: company_id={}, membership_id={}, role={:?}",
                company.id, membership.id, membership.role
            );

            company
        }
    };

    // Step 4: Collect all counterparty company names and get or create them
    let counterparty_names: HashSet<String> = if consistent_role == "originator" {
        investor_names
    } else {
        originator_names
    };

    let mut company_cache: HashMap<String, Uuid> = HashMap::new();
    company_cache.insert(consistent_name.clone(), consistent_company.id);

    for name in counterparty_names {
        if name == consistent_name {
            continue; // Skip if it's the same as consistent company
        }

        let company = match get_company_by_name(&state.pool, &name).await.map_err(|e| {
            error!("Failed to query counterparty company: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup counterparty company".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })? {
            Some(company) => {
                info!(
                    "Counterparty company exists: id={}, name={}",
                    company.id, company.name
                );
                company
            }
            None => {
                info!("Creating counterparty company: name={}", name);

                // Create company without user membership (orphaned company)
                let company = arda_credit_db::business::company::create_company(
                    &state.pool,
                    &arda_credit_db::business::company::CreateCompanyRequest {
                        name: name.clone(),
                        description: Some(format!(
                            "Auto-created from loan tape upload as counterparty"
                        )),
                        metadata: Some(serde_json::json!({
                            "auto_created": true,
                            "created_via": "loan_tape_counterparty"
                        })),
                    },
                )
                .await
                .map_err(|e| {
                    error!("Failed to create counterparty company: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to create counterparty company".to_string(),
                            details: Some(e.to_string()),
                        }),
                    )
                })?;

                info!(
                    "Created counterparty company: id={}, name={}",
                    company.id, company.name
                );
                company
            }
        };

        company_cache.insert(name, company.id);
    }

    info!("All companies ready: total_count={}", company_cache.len());

    // Step 5: Create all deal records
    let mut created_records = Vec::new();

    for (idx, record_req) in req.records.iter().enumerate() {
        let originator_id = company_cache
            .get(&record_req.originator_name)
            .ok_or_else(|| {
                error!(
                    "Originator company not found in cache: {}",
                    record_req.originator_name
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Internal error".to_string(),
                        details: Some(format!(
                            "Originator company '{}' not in cache",
                            record_req.originator_name
                        )),
                    }),
                )
            })?;

        let investor_id = company_cache
            .get(&record_req.investor_name)
            .ok_or_else(|| {
                error!(
                    "Investor company not found in cache: {}",
                    record_req.investor_name
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Internal error".to_string(),
                        details: Some(format!(
                            "Investor company '{}' not in cache",
                            record_req.investor_name
                        )),
                    }),
                )
            })?;

        let state_hash = record_req
            .state_transition_hash
            .as_ref()
            .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok());

        let deal = create_loan_tape_record(
            &state.pool,
            *originator_id,
            Some(*investor_id),
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
        } else {
            warn!(
                "Created deal but failed to fetch full record: deal_id={}",
                deal.id
            );
        }
    }

    info!(
        "Successfully created {} loan tape records for user {} ({})",
        created_records.len(),
        user.email,
        consistent_role
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
        (status = 200, description = "Loan tape record retrieved successfully", body = LoanTapeRecord),
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
) -> Result<Json<LoanTapeRecord>, (StatusCode, Json<ErrorResponse>)> {
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

    Ok(Json(dto_record))
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
