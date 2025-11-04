//! Fee Letters API Handlers
//!
//! Provides REST API endpoints for fee letter CRUD operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_fee_letter, create_fee_letters_batch, delete_fee_letter, get_fee_letter_by_id,
    list_fee_letters, update_fee_letter, CreateFeeLetterRequest as DbCreateRequest,
    UpdateFeeLetterRequest as DbUpdateRequest,
};

use crate::config::types::{ApiState, ErrorResponse};
use crate::handlers::taxonomy::common::{
    internal_error, not_found_error, parse_uuid, BatchError, BatchOperationResponse,
    DeleteResponse, ListQueryParams, PaginatedResponse,
};
use crate::middleware::session_middleware::extract_current_user;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Fee Letter API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "FEE-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "confidentiality_flag": true,
    "upfront_fee_pct": "1.5",
    "agency_fee_amount": "50000.00"
}))]
pub struct FeeLetterResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_fee_letter: String,
    pub fee_components: serde_json::Value,
    pub confidentiality_flag: bool,
    pub upfront_fee_pct: Option<String>,
    pub original_issue_discount_pct: Option<String>,
    pub unused_commitment_fee_bps: Option<i32>,
    pub agency_fee_amount: Option<String>,
    pub amendment_fee_amount: Option<String>,
    pub fee_payment_schedule: Option<String>,
    pub fee_calculation_basis: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create fee letter request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateFeeLetterApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_fee_letter: String,
    pub fee_components: Option<serde_json::Value>,
    pub confidentiality_flag: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub upfront_fee_pct: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub original_issue_discount_pct: Option<Decimal>,
    pub unused_commitment_fee_bps: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub agency_fee_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub amendment_fee_amount: Option<Decimal>,
    pub fee_payment_schedule: Option<String>,
    pub fee_calculation_basis: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update fee letter request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateFeeLetterApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_fee_letter: Option<String>,
    pub fee_components: Option<serde_json::Value>,
    pub confidentiality_flag: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub upfront_fee_pct: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub original_issue_discount_pct: Option<Decimal>,
    pub unused_commitment_fee_bps: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub agency_fee_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub amendment_fee_amount: Option<Decimal>,
    pub fee_payment_schedule: Option<String>,
    pub fee_calculation_basis: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing fee letters
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct FeeLetterListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateFeeLettersRequest {
    pub fee_letters: Vec<CreateFeeLetterApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::FeeLetter) -> FeeLetterResponse {
    FeeLetterResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        doc_hash_fee_letter: db.doc_hash_fee_letter,
        fee_components: db.fee_components,
        confidentiality_flag: db.confidentiality_flag,
        upfront_fee_pct: db.upfront_fee_pct.map(|d| d.to_string()),
        original_issue_discount_pct: db.original_issue_discount_pct.map(|d| d.to_string()),
        unused_commitment_fee_bps: db.unused_commitment_fee_bps,
        agency_fee_amount: db.agency_fee_amount.map(|d| d.to_string()),
        amendment_fee_amount: db.amendment_fee_amount.map(|d| d.to_string()),
        fee_payment_schedule: db.fee_payment_schedule,
        fee_calculation_basis: db.fee_calculation_basis,
        effective_date: db.effective_date,
        expiry_date: db.expiry_date,
        notes: db.notes,
        metadata: db.metadata,
        created_at: db.created_at.to_rfc3339(),
        updated_at: db.updated_at.to_rfc3339(),
        created_by: db.created_by.map(|id| id.to_string()),
        updated_by: db.updated_by.map(|id| id.to_string()),
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// List fee letters
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/fee-letters",
    params(FeeLetterListParams),
    responses(
        (status = 200, description = "List of fee letters", body = PaginatedResponse<FeeLetterResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn list_fee_letters_handler(
    State(state): State<ApiState>,
    Query(params): Query<FeeLetterListParams>,
) -> Result<Json<PaginatedResponse<FeeLetterResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing fee letters");
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "list")
        .increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_fee_letters(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List fee letters", e))?;

    let data: Vec<FeeLetterResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get fee letter by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/fee-letters/{id}",
    params(
        ("id" = String, Path, description = "Fee letter UUID")
    ),
    responses(
        (status = 200, description = "Fee letter details", body = FeeLetterResponse),
        (status = 404, description = "Fee letter not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn get_fee_letter_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<FeeLetterResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting fee letter {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "get")
        .increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_fee_letter_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get fee letter", e))?
        .ok_or_else(|| not_found_error("Fee letter", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create fee letter
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/fee-letters",
    request_body = CreateFeeLetterApiRequest,
    responses(
        (status = 201, description = "Fee letter created", body = FeeLetterResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn create_fee_letter_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateFeeLetterApiRequest>,
) -> Result<(StatusCode, Json<FeeLetterResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating fee letter {}", request.document_id);
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash_fee_letter: request.doc_hash_fee_letter,
        fee_components: request.fee_components,
        confidentiality_flag: request.confidentiality_flag,
        upfront_fee_pct: request.upfront_fee_pct,
        original_issue_discount_pct: request.original_issue_discount_pct,
        unused_commitment_fee_bps: request.unused_commitment_fee_bps,
        agency_fee_amount: request.agency_fee_amount,
        amendment_fee_amount: request.amendment_fee_amount,
        fee_payment_schedule: request.fee_payment_schedule,
        fee_calculation_basis: request.fee_calculation_basis,
        effective_date: request.effective_date,
        expiry_date: request.expiry_date,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_fee_letter(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create fee letter", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create fee letters in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/fee-letters/batch",
    request_body = BatchCreateFeeLettersRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn create_fee_letters_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateFeeLettersRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} fee letters",
        request.fee_letters.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.fee_letters {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            document_id: req.document_id,
            credit_agreement_id,
            doc_hash_fee_letter: req.doc_hash_fee_letter,
            fee_components: req.fee_components,
            confidentiality_flag: req.confidentiality_flag,
            upfront_fee_pct: req.upfront_fee_pct,
            original_issue_discount_pct: req.original_issue_discount_pct,
            unused_commitment_fee_bps: req.unused_commitment_fee_bps,
            agency_fee_amount: req.agency_fee_amount,
            amendment_fee_amount: req.amendment_fee_amount,
            fee_payment_schedule: req.fee_payment_schedule,
            fee_calculation_basis: req.fee_calculation_basis,
            effective_date: req.effective_date,
            expiry_date: req.expiry_date,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_fee_letters_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create fee letters", e))?;

    let mut successful_ids = Vec::new();
    let mut errors = Vec::new();

    for (idx, result) in results.into_iter().enumerate() {
        match result {
            Ok(record) => successful_ids.push(record.id.to_string()),
            Err(e) => errors.push(BatchError {
                index: idx,
                error_code: "create_failed".to_string(),
                error_message: e.to_string(),
            }),
        }
    }

    let success_count = successful_ids.len();
    let error_count = errors.len();

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update fee letter
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/fee-letters/{id}",
    params(
        ("id" = String, Path, description = "Fee letter UUID")
    ),
    request_body = UpdateFeeLetterApiRequest,
    responses(
        (status = 200, description = "Fee letter updated", body = FeeLetterResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Fee letter not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn update_fee_letter_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateFeeLetterApiRequest>,
) -> Result<Json<FeeLetterResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating fee letter {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash_fee_letter: request.doc_hash_fee_letter,
        fee_components: request.fee_components,
        confidentiality_flag: request.confidentiality_flag,
        upfront_fee_pct: request.upfront_fee_pct,
        original_issue_discount_pct: request.original_issue_discount_pct,
        unused_commitment_fee_bps: request.unused_commitment_fee_bps,
        agency_fee_amount: request.agency_fee_amount,
        amendment_fee_amount: request.amendment_fee_amount,
        fee_payment_schedule: request.fee_payment_schedule,
        fee_calculation_basis: request.fee_calculation_basis,
        effective_date: request.effective_date,
        expiry_date: request.expiry_date,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_fee_letter(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update fee letter", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete fee letter
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/fee-letters/{id}",
    params(
        ("id" = String, Path, description = "Fee letter UUID")
    ),
    responses(
        (status = 200, description = "Fee letter deleted", body = DeleteResponse),
        (status = 404, description = "Fee letter not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Fee Letters"
)]
#[instrument(skip(state))]
pub async fn delete_fee_letter_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting fee letter {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "fee_letters", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_fee_letter(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete fee letter", e))?;

    if !deleted {
        return Err(not_found_error("Fee letter", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "fee_letters", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Fee letter {} deleted successfully", id),
    }))
}
