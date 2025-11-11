//! UCC Financing Statements API Handlers
//!
//! Provides REST API endpoints for UCC financing statement CRUD operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_ucc_financing_statement, create_ucc_financing_statements_batch,
    delete_ucc_financing_statement, get_ucc_financing_statement_by_id,
    list_ucc_financing_statements, update_ucc_financing_statement,
    CreateUccFinancingStatementRequest as DbCreateRequest,
    UpdateUccFinancingStatementRequest as DbUpdateRequest,
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

/// UCC Financing Statement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "UCC-2025-DE-001",
    "security_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "file_number": "2025-1234567-89",
    "jurisdiction": "DE",
    "filing_status": "active"
}))]
pub struct UccFinancingStatementResponse {
    pub id: String,
    pub document_id: String,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub file_number: String,
    pub filing_office: String,
    pub jurisdiction: String,
    pub file_datetime: String,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: String,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: String,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: String,
    pub secured_party_address: Option<String>,
    pub additional_debtors: serde_json::Value,
    pub additional_secured_parties: serde_json::Value,
    pub collateral_description: String,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: bool,
    pub specific_collateral: bool,
    pub filing_status: String,
    pub continuation_filed: bool,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: bool,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: bool,
    pub is_fixture_filing: bool,
    pub is_manufactured_home: bool,
    pub real_property_description: Option<String>,
    pub filing_fee: Option<String>,
    pub search_verified: bool,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create UCC financing statement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateUccFinancingStatementApiRequest {
    pub document_id: String,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub file_number: String,
    pub filing_office: String,
    pub jurisdiction: String,
    pub file_datetime: DateTime<Utc>,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: Option<String>,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: String,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: String,
    pub secured_party_address: Option<String>,
    pub additional_debtors: Option<serde_json::Value>,
    pub additional_secured_parties: Option<serde_json::Value>,
    pub collateral_description: String,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: Option<bool>,
    pub specific_collateral: Option<bool>,
    pub filing_status: Option<String>,
    pub continuation_filed: Option<bool>,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: Option<bool>,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: Option<bool>,
    pub is_fixture_filing: Option<bool>,
    pub is_manufactured_home: Option<bool>,
    pub real_property_description: Option<String>,
    #[schema(value_type = Option<String>)]
    pub filing_fee: Option<Decimal>,
    pub search_verified: Option<bool>,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update UCC financing statement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUccFinancingStatementApiRequest {
    pub document_id: Option<String>,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: Option<String>,
    pub file_number: Option<String>,
    pub filing_office: Option<String>,
    pub jurisdiction: Option<String>,
    pub file_datetime: Option<DateTime<Utc>>,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: Option<String>,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: Option<String>,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: Option<String>,
    pub secured_party_address: Option<String>,
    pub additional_debtors: Option<serde_json::Value>,
    pub additional_secured_parties: Option<serde_json::Value>,
    pub collateral_description: Option<String>,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: Option<bool>,
    pub specific_collateral: Option<bool>,
    pub filing_status: Option<String>,
    pub continuation_filed: Option<bool>,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: Option<bool>,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: Option<bool>,
    pub is_fixture_filing: Option<bool>,
    pub is_manufactured_home: Option<bool>,
    pub real_property_description: Option<String>,
    #[schema(value_type = Option<String>)]
    pub filing_fee: Option<Decimal>,
    pub search_verified: Option<bool>,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing UCC financing statements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct UccFinancingStatementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by security agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub security_agreement_id: Option<String>,

    /// Filter by jurisdiction
    #[param(example = "DE")]
    pub jurisdiction: Option<String>,

    /// Filter by filing status
    #[param(example = "active")]
    pub filing_status: Option<String>,

    /// Filter by filing type
    #[param(example = "initial")]
    pub filing_type: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateUccFinancingStatementsRequest {
    pub ucc_financing_statements: Vec<CreateUccFinancingStatementApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::UccFinancingStatement) -> UccFinancingStatementResponse {
    UccFinancingStatementResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        security_agreement_id: db.security_agreement_id.map(|id| id.to_string()),
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        file_number: db.file_number,
        filing_office: db.filing_office,
        jurisdiction: db.jurisdiction,
        file_datetime: db.file_datetime.to_rfc3339(),
        expiration_date: db.expiration_date,
        lapse_date: db.lapse_date,
        filing_type: db.filing_type,
        original_file_number: db.original_file_number,
        amends_file_number: db.amends_file_number,
        debtor_name: db.debtor_name,
        debtor_type: db.debtor_type,
        debtor_jurisdiction: db.debtor_jurisdiction,
        debtor_organization_id: db.debtor_organization_id,
        debtor_address: db.debtor_address,
        secured_party: db.secured_party,
        secured_party_address: db.secured_party_address,
        additional_debtors: db.additional_debtors,
        additional_secured_parties: db.additional_secured_parties,
        collateral_description: db.collateral_description,
        collateral_desc_short: db.collateral_desc_short,
        all_assets_indicator: db.all_assets_indicator,
        specific_collateral: db.specific_collateral,
        filing_status: db.filing_status,
        continuation_filed: db.continuation_filed,
        continuation_file_number: db.continuation_file_number,
        continuation_date: db.continuation_date,
        terminated: db.terminated,
        termination_date: db.termination_date,
        termination_reason: db.termination_reason,
        is_agricultural_lien: db.is_agricultural_lien,
        is_fixture_filing: db.is_fixture_filing,
        is_manufactured_home: db.is_manufactured_home,
        real_property_description: db.real_property_description,
        filing_fee: db.filing_fee.map(|d| d.to_string()),
        search_verified: db.search_verified,
        search_verification_date: db.search_verification_date,
        search_results: db.search_results,
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

/// List UCC financing statements
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/ucc-financing-statements",
    params(UccFinancingStatementListParams),
    responses(
        (status = 200, description = "List of UCC financing statements", body = PaginatedResponse<UccFinancingStatementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn list_ucc_financing_statements_handler(
    State(state): State<ApiState>,
    Query(params): Query<UccFinancingStatementListParams>,
) -> Result<Json<PaginatedResponse<UccFinancingStatementResponse>>, (StatusCode, Json<ErrorResponse>)>
{
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing UCC financing statements");
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let security_agreement_id = if let Some(id_str) = params.security_agreement_id {
        Some(parse_uuid(&id_str, "security_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_ucc_financing_statements(
        &state.pool,
        limit,
        offset,
        security_agreement_id,
        params.jurisdiction.clone(),
        params.filing_status.clone(),
        params.filing_type.clone(),
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List UCC financing statements", e))?;

    let data: Vec<UccFinancingStatementResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get UCC financing statement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/ucc-financing-statements/{id}",
    params(
        ("id" = String, Path, description = "UCC financing statement UUID")
    ),
    responses(
        (status = 200, description = "UCC financing statement details", body = UccFinancingStatementResponse),
        (status = 404, description = "UCC financing statement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn get_ucc_financing_statement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<UccFinancingStatementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting UCC financing statement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_ucc_financing_statement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get UCC financing statement", e))?
        .ok_or_else(|| not_found_error("UCC financing statement", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create UCC financing statement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/ucc-financing-statements",
    request_body = CreateUccFinancingStatementApiRequest,
    responses(
        (status = 201, description = "UCC financing statement created", body = UccFinancingStatementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn create_ucc_financing_statement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateUccFinancingStatementApiRequest>,
) -> Result<(StatusCode, Json<UccFinancingStatementResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Creating UCC financing statement {}",
        request.document_id
    );
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let security_agreement_id = if let Some(id_str) = request.security_agreement_id {
        Some(parse_uuid(&id_str, "security_agreement_id")?)
    } else {
        None
    };

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        security_agreement_id,
        credit_agreement_id,
        doc_hash: request.doc_hash,
        file_number: request.file_number,
        filing_office: request.filing_office,
        jurisdiction: request.jurisdiction,
        file_datetime: request.file_datetime,
        expiration_date: request.expiration_date,
        lapse_date: request.lapse_date,
        filing_type: request.filing_type,
        original_file_number: request.original_file_number,
        amends_file_number: request.amends_file_number,
        debtor_name: request.debtor_name,
        debtor_type: request.debtor_type,
        debtor_jurisdiction: request.debtor_jurisdiction,
        debtor_organization_id: request.debtor_organization_id,
        debtor_address: request.debtor_address,
        secured_party: request.secured_party,
        secured_party_address: request.secured_party_address,
        additional_debtors: request.additional_debtors,
        additional_secured_parties: request.additional_secured_parties,
        collateral_description: request.collateral_description,
        collateral_desc_short: request.collateral_desc_short,
        all_assets_indicator: request.all_assets_indicator,
        specific_collateral: request.specific_collateral,
        filing_status: request.filing_status,
        continuation_filed: request.continuation_filed,
        continuation_file_number: request.continuation_file_number,
        continuation_date: request.continuation_date,
        terminated: request.terminated,
        termination_date: request.termination_date,
        termination_reason: request.termination_reason,
        is_agricultural_lien: request.is_agricultural_lien,
        is_fixture_filing: request.is_fixture_filing,
        is_manufactured_home: request.is_manufactured_home,
        real_property_description: request.real_property_description,
        filing_fee: request.filing_fee,
        search_verified: request.search_verified,
        search_verification_date: request.search_verification_date,
        search_results: request.search_results,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_ucc_financing_statement(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create UCC financing statement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create UCC financing statements in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/ucc-financing-statements/batch",
    request_body = BatchCreateUccFinancingStatementsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn create_ucc_financing_statements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateUccFinancingStatementsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} UCC financing statements",
        request.ucc_financing_statements.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.ucc_financing_statements {
        let security_agreement_id = if let Some(id_str) = req.security_agreement_id {
            match parse_uuid(&id_str, "security_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

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
            security_agreement_id,
            credit_agreement_id,
            doc_hash: req.doc_hash,
            file_number: req.file_number,
            filing_office: req.filing_office,
            jurisdiction: req.jurisdiction,
            file_datetime: req.file_datetime,
            expiration_date: req.expiration_date,
            lapse_date: req.lapse_date,
            filing_type: req.filing_type,
            original_file_number: req.original_file_number,
            amends_file_number: req.amends_file_number,
            debtor_name: req.debtor_name,
            debtor_type: req.debtor_type,
            debtor_jurisdiction: req.debtor_jurisdiction,
            debtor_organization_id: req.debtor_organization_id,
            debtor_address: req.debtor_address,
            secured_party: req.secured_party,
            secured_party_address: req.secured_party_address,
            additional_debtors: req.additional_debtors,
            additional_secured_parties: req.additional_secured_parties,
            collateral_description: req.collateral_description,
            collateral_desc_short: req.collateral_desc_short,
            all_assets_indicator: req.all_assets_indicator,
            specific_collateral: req.specific_collateral,
            filing_status: req.filing_status,
            continuation_filed: req.continuation_filed,
            continuation_file_number: req.continuation_file_number,
            continuation_date: req.continuation_date,
            terminated: req.terminated,
            termination_date: req.termination_date,
            termination_reason: req.termination_reason,
            is_agricultural_lien: req.is_agricultural_lien,
            is_fixture_filing: req.is_fixture_filing,
            is_manufactured_home: req.is_manufactured_home,
            real_property_description: req.real_property_description,
            filing_fee: req.filing_fee,
            search_verified: req.search_verified,
            search_verification_date: req.search_verification_date,
            search_results: req.search_results,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_ucc_financing_statements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create UCC financing statements", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update UCC financing statement
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/ucc-financing-statements/{id}",
    params(
        ("id" = String, Path, description = "UCC financing statement UUID")
    ),
    request_body = UpdateUccFinancingStatementApiRequest,
    responses(
        (status = 200, description = "UCC financing statement updated", body = UccFinancingStatementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "UCC financing statement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn update_ucc_financing_statement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateUccFinancingStatementApiRequest>,
) -> Result<Json<UccFinancingStatementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating UCC financing statement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let security_agreement_id = if let Some(id_str) = request.security_agreement_id {
        Some(parse_uuid(&id_str, "security_agreement_id")?)
    } else {
        None
    };

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        security_agreement_id,
        credit_agreement_id,
        doc_hash: request.doc_hash,
        file_number: request.file_number,
        filing_office: request.filing_office,
        jurisdiction: request.jurisdiction,
        file_datetime: request.file_datetime,
        expiration_date: request.expiration_date,
        lapse_date: request.lapse_date,
        filing_type: request.filing_type,
        original_file_number: request.original_file_number,
        amends_file_number: request.amends_file_number,
        debtor_name: request.debtor_name,
        debtor_type: request.debtor_type,
        debtor_jurisdiction: request.debtor_jurisdiction,
        debtor_organization_id: request.debtor_organization_id,
        debtor_address: request.debtor_address,
        secured_party: request.secured_party,
        secured_party_address: request.secured_party_address,
        additional_debtors: request.additional_debtors,
        additional_secured_parties: request.additional_secured_parties,
        collateral_description: request.collateral_description,
        collateral_desc_short: request.collateral_desc_short,
        all_assets_indicator: request.all_assets_indicator,
        specific_collateral: request.specific_collateral,
        filing_status: request.filing_status,
        continuation_filed: request.continuation_filed,
        continuation_file_number: request.continuation_file_number,
        continuation_date: request.continuation_date,
        terminated: request.terminated,
        termination_date: request.termination_date,
        termination_reason: request.termination_reason,
        is_agricultural_lien: request.is_agricultural_lien,
        is_fixture_filing: request.is_fixture_filing,
        is_manufactured_home: request.is_manufactured_home,
        real_property_description: request.real_property_description,
        filing_fee: request.filing_fee,
        search_verified: request.search_verified,
        search_verification_date: request.search_verification_date,
        search_results: request.search_results,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_ucc_financing_statement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update UCC financing statement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete UCC financing statement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/ucc-financing-statements/{id}",
    params(
        ("id" = String, Path, description = "UCC financing statement UUID")
    ),
    responses(
        (status = 200, description = "UCC financing statement deleted", body = DeleteResponse),
        (status = 404, description = "UCC financing statement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - UCC Financing Statements"
)]
#[instrument(skip(state))]
pub async fn delete_ucc_financing_statement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting UCC financing statement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "ucc_financing_statements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_ucc_financing_statement(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete UCC financing statement", e))?;

    if !deleted {
        return Err(not_found_error("UCC financing statement", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "ucc_financing_statements", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("UCC financing statement {} deleted successfully", id),
    }))
}
