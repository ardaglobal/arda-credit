//! SSI Records API Handlers
//!
//! Provides REST API endpoints for SSI record CRUD operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_ssi_record, create_ssi_records_batch, delete_ssi_record, get_ssi_record_by_id,
    list_ssi_records, update_ssi_record, CreateSsiRecordRequest as DbCreateRequest,
    UpdateSsiRecordRequest as DbUpdateRequest,
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

/// SSI Record API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "SSI-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "ssi_type": "full_bilateral",
    "ssi_status": "active",
    "settlement_method": "wire"
}))]
pub struct SsiRecordResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub deal_id: Option<String>,
    pub doc_hash: String,
    pub ssi_type: Option<String>,

    // Seller SSI Details
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>,
    pub seller_ssi_verified_at: Option<String>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>,
    pub buyer_ssi_verified_at: Option<String>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>,
    pub effective_date: NaiveDate,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>,

    // Compliance & Security
    pub sanctions_screened: Option<bool>,
    pub sanctions_screen_date: Option<String>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>,
    pub aml_check_date: Option<String>,
    pub confidential: Option<bool>,
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create SSI record request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSsiRecordApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub deal_id: Option<String>,
    pub doc_hash: String,
    pub ssi_type: Option<String>,

    // Seller SSI Details
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>,
    pub seller_ssi_verified_at: Option<DateTime<Utc>>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>,
    pub buyer_ssi_verified_at: Option<DateTime<Utc>>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>,
    pub effective_date: NaiveDate,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>,

    // Compliance & Security
    pub sanctions_screened: Option<bool>,
    pub sanctions_screen_date: Option<DateTime<Utc>>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>,
    pub aml_check_date: Option<DateTime<Utc>>,
    pub confidential: Option<bool>,
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update SSI record request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSsiRecordApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub deal_id: Option<String>,
    pub doc_hash: Option<String>,
    pub ssi_type: Option<String>,

    // Seller SSI Details
    pub seller_ssi_id: Option<String>,
    pub seller_beneficiary_name: Option<String>,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: Option<String>,
    pub buyer_beneficiary_name: Option<String>,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>,
    pub seller_ssi_verified_at: Option<DateTime<Utc>>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>,
    pub buyer_ssi_verified_at: Option<DateTime<Utc>>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>,

    // Compliance & Security
    pub sanctions_screened: Option<bool>,
    pub sanctions_screen_date: Option<DateTime<Utc>>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>,
    pub aml_check_date: Option<DateTime<Utc>>,
    pub confidential: Option<bool>,
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing SSI records
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SsiRecordListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by SSI status
    #[param(example = "active")]
    pub ssi_status: Option<String>,

    /// Filter by SSI type
    #[param(example = "full_bilateral")]
    pub ssi_type: Option<String>,

    /// Filter by settlement method
    #[param(example = "wire")]
    pub settlement_method: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateSsiRecordsRequest {
    pub ssi_records: Vec<CreateSsiRecordApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::SsiRecord) -> SsiRecordResponse {
    SsiRecordResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        deal_id: db.deal_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        ssi_type: db.ssi_type,
        seller_ssi_id: db.seller_ssi_id,
        seller_beneficiary_name: db.seller_beneficiary_name,
        seller_bank_name: db.seller_bank_name,
        seller_bank_identifier: db.seller_bank_identifier,
        seller_swift_code: db.seller_swift_code,
        seller_aba_routing: db.seller_aba_routing,
        seller_iban: db.seller_iban,
        seller_account_number: db.seller_account_number,
        seller_reference_text: db.seller_reference_text,
        seller_bank_address: db.seller_bank_address,
        seller_intermediary_bank: db.seller_intermediary_bank,
        seller_intermediary_swift: db.seller_intermediary_swift,
        seller_custody_type: db.seller_custody_type,
        seller_custodian_name: db.seller_custodian_name,
        seller_custodian_account_ref: db.seller_custodian_account_ref,
        seller_wallet_address: db.seller_wallet_address,
        seller_wallet_chain: db.seller_wallet_chain,
        seller_wallet_chain_id: db.seller_wallet_chain_id,
        seller_wallet_type: db.seller_wallet_type,
        buyer_ssi_id: db.buyer_ssi_id,
        buyer_beneficiary_name: db.buyer_beneficiary_name,
        buyer_bank_name: db.buyer_bank_name,
        buyer_bank_identifier: db.buyer_bank_identifier,
        buyer_swift_code: db.buyer_swift_code,
        buyer_aba_routing: db.buyer_aba_routing,
        buyer_iban: db.buyer_iban,
        buyer_account_number: db.buyer_account_number,
        buyer_reference_text: db.buyer_reference_text,
        buyer_bank_address: db.buyer_bank_address,
        buyer_intermediary_bank: db.buyer_intermediary_bank,
        buyer_intermediary_swift: db.buyer_intermediary_swift,
        buyer_custody_type: db.buyer_custody_type,
        buyer_custodian_name: db.buyer_custodian_name,
        buyer_custodian_account_ref: db.buyer_custodian_account_ref,
        buyer_wallet_address: db.buyer_wallet_address,
        buyer_wallet_chain: db.buyer_wallet_chain,
        buyer_wallet_chain_id: db.buyer_wallet_chain_id,
        buyer_wallet_type: db.buyer_wallet_type,
        seller_ssi_verified: db.seller_ssi_verified,
        seller_ssi_verified_at: db.seller_ssi_verified_at.map(|t| t.to_rfc3339()),
        seller_ssi_verified_by: db.seller_ssi_verified_by,
        buyer_ssi_verified: db.buyer_ssi_verified,
        buyer_ssi_verified_at: db.buyer_ssi_verified_at.map(|t| t.to_rfc3339()),
        buyer_ssi_verified_by: db.buyer_ssi_verified_by,
        ssi_status: db.ssi_status,
        effective_date: db.effective_date,
        expiry_date: db.expiry_date,
        last_used_date: db.last_used_date,
        usage_count: db.usage_count,
        sanctions_screened: db.sanctions_screened,
        sanctions_screen_date: db.sanctions_screen_date.map(|t| t.to_rfc3339()),
        sanctions_screen_result: db.sanctions_screen_result,
        aml_check_passed: db.aml_check_passed,
        aml_check_date: db.aml_check_date.map(|t| t.to_rfc3339()),
        confidential: db.confidential,
        access_restricted_to: db.access_restricted_to,
        settlement_currency: db.settlement_currency,
        settlement_method: db.settlement_method,
        special_instructions: db.special_instructions,
        internal_notes: db.internal_notes,
        metadata: db.metadata,
        created_at: db.created_at.map(|t| t.to_rfc3339()),
        updated_at: db.updated_at.map(|t| t.to_rfc3339()),
        created_by: db.created_by.map(|id| id.to_string()),
        updated_by: db.updated_by.map(|id| id.to_string()),
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// List SSI records
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/ssi-records",
    params(SsiRecordListParams),
    responses(
        (status = 200, description = "List of SSI records", body = PaginatedResponse<SsiRecordResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn list_ssi_records_handler(
    State(state): State<ApiState>,
    Query(params): Query<SsiRecordListParams>,
) -> Result<Json<PaginatedResponse<SsiRecordResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing SSI records");
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "list")
        .increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_ssi_records(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.ssi_status.clone(),
        params.ssi_type.clone(),
        params.settlement_method.clone(),
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List SSI records", e))?;

    let data: Vec<SsiRecordResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get SSI record by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/ssi-records/{id}",
    params(
        ("id" = String, Path, description = "SSI record UUID")
    ),
    responses(
        (status = 200, description = "SSI record details", body = SsiRecordResponse),
        (status = 404, description = "SSI record not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn get_ssi_record_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<SsiRecordResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting SSI record {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "get")
        .increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_ssi_record_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get SSI record", e))?
        .ok_or_else(|| not_found_error("SSI record", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create SSI record
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/ssi-records",
    request_body = CreateSsiRecordApiRequest,
    responses(
        (status = 201, description = "SSI record created", body = SsiRecordResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn create_ssi_record_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateSsiRecordApiRequest>,
) -> Result<(StatusCode, Json<SsiRecordResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating SSI record {}", request.document_id);
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let deal_id = if let Some(id_str) = request.deal_id {
        Some(parse_uuid(&id_str, "deal_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        deal_id,
        doc_hash: request.doc_hash,
        ssi_type: request.ssi_type,
        seller_ssi_id: request.seller_ssi_id,
        seller_beneficiary_name: request.seller_beneficiary_name,
        seller_bank_name: request.seller_bank_name,
        seller_bank_identifier: request.seller_bank_identifier,
        seller_swift_code: request.seller_swift_code,
        seller_aba_routing: request.seller_aba_routing,
        seller_iban: request.seller_iban,
        seller_account_number: request.seller_account_number,
        seller_reference_text: request.seller_reference_text,
        seller_bank_address: request.seller_bank_address,
        seller_intermediary_bank: request.seller_intermediary_bank,
        seller_intermediary_swift: request.seller_intermediary_swift,
        seller_custody_type: request.seller_custody_type,
        seller_custodian_name: request.seller_custodian_name,
        seller_custodian_account_ref: request.seller_custodian_account_ref,
        seller_wallet_address: request.seller_wallet_address,
        seller_wallet_chain: request.seller_wallet_chain,
        seller_wallet_chain_id: request.seller_wallet_chain_id,
        seller_wallet_type: request.seller_wallet_type,
        buyer_ssi_id: request.buyer_ssi_id,
        buyer_beneficiary_name: request.buyer_beneficiary_name,
        buyer_bank_name: request.buyer_bank_name,
        buyer_bank_identifier: request.buyer_bank_identifier,
        buyer_swift_code: request.buyer_swift_code,
        buyer_aba_routing: request.buyer_aba_routing,
        buyer_iban: request.buyer_iban,
        buyer_account_number: request.buyer_account_number,
        buyer_reference_text: request.buyer_reference_text,
        buyer_bank_address: request.buyer_bank_address,
        buyer_intermediary_bank: request.buyer_intermediary_bank,
        buyer_intermediary_swift: request.buyer_intermediary_swift,
        buyer_custody_type: request.buyer_custody_type,
        buyer_custodian_name: request.buyer_custodian_name,
        buyer_custodian_account_ref: request.buyer_custodian_account_ref,
        buyer_wallet_address: request.buyer_wallet_address,
        buyer_wallet_chain: request.buyer_wallet_chain,
        buyer_wallet_chain_id: request.buyer_wallet_chain_id,
        buyer_wallet_type: request.buyer_wallet_type,
        seller_ssi_verified: request.seller_ssi_verified,
        seller_ssi_verified_at: request.seller_ssi_verified_at,
        seller_ssi_verified_by: request.seller_ssi_verified_by,
        buyer_ssi_verified: request.buyer_ssi_verified,
        buyer_ssi_verified_at: request.buyer_ssi_verified_at,
        buyer_ssi_verified_by: request.buyer_ssi_verified_by,
        ssi_status: request.ssi_status,
        effective_date: request.effective_date,
        expiry_date: request.expiry_date,
        last_used_date: request.last_used_date,
        usage_count: request.usage_count,
        sanctions_screened: request.sanctions_screened,
        sanctions_screen_date: request.sanctions_screen_date,
        sanctions_screen_result: request.sanctions_screen_result,
        aml_check_passed: request.aml_check_passed,
        aml_check_date: request.aml_check_date,
        confidential: request.confidential,
        access_restricted_to: request.access_restricted_to,
        settlement_currency: request.settlement_currency,
        settlement_method: request.settlement_method,
        special_instructions: request.special_instructions,
        internal_notes: request.internal_notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_ssi_record(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create SSI record", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create SSI records in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/ssi-records/batch",
    request_body = BatchCreateSsiRecordsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn create_ssi_records_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateSsiRecordsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} SSI records",
        request.ssi_records.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.ssi_records {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let deal_id = if let Some(id_str) = req.deal_id {
            match parse_uuid(&id_str, "deal_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            document_id: req.document_id,
            credit_agreement_id,
            deal_id,
            doc_hash: req.doc_hash,
            ssi_type: req.ssi_type,
            seller_ssi_id: req.seller_ssi_id,
            seller_beneficiary_name: req.seller_beneficiary_name,
            seller_bank_name: req.seller_bank_name,
            seller_bank_identifier: req.seller_bank_identifier,
            seller_swift_code: req.seller_swift_code,
            seller_aba_routing: req.seller_aba_routing,
            seller_iban: req.seller_iban,
            seller_account_number: req.seller_account_number,
            seller_reference_text: req.seller_reference_text,
            seller_bank_address: req.seller_bank_address,
            seller_intermediary_bank: req.seller_intermediary_bank,
            seller_intermediary_swift: req.seller_intermediary_swift,
            seller_custody_type: req.seller_custody_type,
            seller_custodian_name: req.seller_custodian_name,
            seller_custodian_account_ref: req.seller_custodian_account_ref,
            seller_wallet_address: req.seller_wallet_address,
            seller_wallet_chain: req.seller_wallet_chain,
            seller_wallet_chain_id: req.seller_wallet_chain_id,
            seller_wallet_type: req.seller_wallet_type,
            buyer_ssi_id: req.buyer_ssi_id,
            buyer_beneficiary_name: req.buyer_beneficiary_name,
            buyer_bank_name: req.buyer_bank_name,
            buyer_bank_identifier: req.buyer_bank_identifier,
            buyer_swift_code: req.buyer_swift_code,
            buyer_aba_routing: req.buyer_aba_routing,
            buyer_iban: req.buyer_iban,
            buyer_account_number: req.buyer_account_number,
            buyer_reference_text: req.buyer_reference_text,
            buyer_bank_address: req.buyer_bank_address,
            buyer_intermediary_bank: req.buyer_intermediary_bank,
            buyer_intermediary_swift: req.buyer_intermediary_swift,
            buyer_custody_type: req.buyer_custody_type,
            buyer_custodian_name: req.buyer_custodian_name,
            buyer_custodian_account_ref: req.buyer_custodian_account_ref,
            buyer_wallet_address: req.buyer_wallet_address,
            buyer_wallet_chain: req.buyer_wallet_chain,
            buyer_wallet_chain_id: req.buyer_wallet_chain_id,
            buyer_wallet_type: req.buyer_wallet_type,
            seller_ssi_verified: req.seller_ssi_verified,
            seller_ssi_verified_at: req.seller_ssi_verified_at,
            seller_ssi_verified_by: req.seller_ssi_verified_by,
            buyer_ssi_verified: req.buyer_ssi_verified,
            buyer_ssi_verified_at: req.buyer_ssi_verified_at,
            buyer_ssi_verified_by: req.buyer_ssi_verified_by,
            ssi_status: req.ssi_status,
            effective_date: req.effective_date,
            expiry_date: req.expiry_date,
            last_used_date: req.last_used_date,
            usage_count: req.usage_count,
            sanctions_screened: req.sanctions_screened,
            sanctions_screen_date: req.sanctions_screen_date,
            sanctions_screen_result: req.sanctions_screen_result,
            aml_check_passed: req.aml_check_passed,
            aml_check_date: req.aml_check_date,
            confidential: req.confidential,
            access_restricted_to: req.access_restricted_to,
            settlement_currency: req.settlement_currency,
            settlement_method: req.settlement_method,
            special_instructions: req.special_instructions,
            internal_notes: req.internal_notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_ssi_records_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create SSI records", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update SSI record
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/ssi-records/{id}",
    params(
        ("id" = String, Path, description = "SSI record UUID")
    ),
    request_body = UpdateSsiRecordApiRequest,
    responses(
        (status = 200, description = "SSI record updated", body = SsiRecordResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "SSI record not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn update_ssi_record_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateSsiRecordApiRequest>,
) -> Result<Json<SsiRecordResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating SSI record {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let deal_id = if let Some(id_str) = request.deal_id {
        Some(parse_uuid(&id_str, "deal_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        deal_id,
        doc_hash: request.doc_hash,
        ssi_type: request.ssi_type,
        seller_ssi_id: request.seller_ssi_id,
        seller_beneficiary_name: request.seller_beneficiary_name,
        seller_bank_name: request.seller_bank_name,
        seller_bank_identifier: request.seller_bank_identifier,
        seller_swift_code: request.seller_swift_code,
        seller_aba_routing: request.seller_aba_routing,
        seller_iban: request.seller_iban,
        seller_account_number: request.seller_account_number,
        seller_reference_text: request.seller_reference_text,
        seller_bank_address: request.seller_bank_address,
        seller_intermediary_bank: request.seller_intermediary_bank,
        seller_intermediary_swift: request.seller_intermediary_swift,
        seller_custody_type: request.seller_custody_type,
        seller_custodian_name: request.seller_custodian_name,
        seller_custodian_account_ref: request.seller_custodian_account_ref,
        seller_wallet_address: request.seller_wallet_address,
        seller_wallet_chain: request.seller_wallet_chain,
        seller_wallet_chain_id: request.seller_wallet_chain_id,
        seller_wallet_type: request.seller_wallet_type,
        buyer_ssi_id: request.buyer_ssi_id,
        buyer_beneficiary_name: request.buyer_beneficiary_name,
        buyer_bank_name: request.buyer_bank_name,
        buyer_bank_identifier: request.buyer_bank_identifier,
        buyer_swift_code: request.buyer_swift_code,
        buyer_aba_routing: request.buyer_aba_routing,
        buyer_iban: request.buyer_iban,
        buyer_account_number: request.buyer_account_number,
        buyer_reference_text: request.buyer_reference_text,
        buyer_bank_address: request.buyer_bank_address,
        buyer_intermediary_bank: request.buyer_intermediary_bank,
        buyer_intermediary_swift: request.buyer_intermediary_swift,
        buyer_custody_type: request.buyer_custody_type,
        buyer_custodian_name: request.buyer_custodian_name,
        buyer_custodian_account_ref: request.buyer_custodian_account_ref,
        buyer_wallet_address: request.buyer_wallet_address,
        buyer_wallet_chain: request.buyer_wallet_chain,
        buyer_wallet_chain_id: request.buyer_wallet_chain_id,
        buyer_wallet_type: request.buyer_wallet_type,
        seller_ssi_verified: request.seller_ssi_verified,
        seller_ssi_verified_at: request.seller_ssi_verified_at,
        seller_ssi_verified_by: request.seller_ssi_verified_by,
        buyer_ssi_verified: request.buyer_ssi_verified,
        buyer_ssi_verified_at: request.buyer_ssi_verified_at,
        buyer_ssi_verified_by: request.buyer_ssi_verified_by,
        ssi_status: request.ssi_status,
        effective_date: request.effective_date,
        expiry_date: request.expiry_date,
        last_used_date: request.last_used_date,
        usage_count: request.usage_count,
        sanctions_screened: request.sanctions_screened,
        sanctions_screen_date: request.sanctions_screen_date,
        sanctions_screen_result: request.sanctions_screen_result,
        aml_check_passed: request.aml_check_passed,
        aml_check_date: request.aml_check_date,
        confidential: request.confidential,
        access_restricted_to: request.access_restricted_to,
        settlement_currency: request.settlement_currency,
        settlement_method: request.settlement_method,
        special_instructions: request.special_instructions,
        internal_notes: request.internal_notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_ssi_record(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update SSI record", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete SSI record
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/ssi-records/{id}",
    params(
        ("id" = String, Path, description = "SSI record UUID")
    ),
    responses(
        (status = 200, description = "SSI record deleted", body = DeleteResponse),
        (status = 404, description = "SSI record not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - SSI Records"
)]
#[instrument(skip(state))]
pub async fn delete_ssi_record_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting SSI record {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ssi_records", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_ssi_record(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete SSI record", e))?;

    if !deleted {
        return Err(not_found_error("SSI record", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ssi_records", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("SSI record {} deleted successfully", id),
    }))
}
