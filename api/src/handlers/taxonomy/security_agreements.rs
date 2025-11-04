//! Security Agreements API Handlers
//!
//! Provides REST API endpoints for security agreement CRUD operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_security_agreement, create_security_agreements_batch, delete_security_agreement,
    get_security_agreement_by_id, list_security_agreements, update_security_agreement,
    CreateSecurityAgreementRequest as DbCreateRequest,
    UpdateSecurityAgreementRequest as DbUpdateRequest,
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

/// Security Agreement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "SEC-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "grantor": "Acme Corp",
    "secured_party": "Bank of Example",
    "perfection_status": "perfected",
    "lien_priority": "first_lien"
}))]
pub struct SecurityAgreementResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub grantor: String,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: String,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: String,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Vec<String>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<String>,
    pub governing_law: String,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create security agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSecurityAgreementApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub grantor: String,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: String,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: String,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Vec<String>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<String>,
    pub governing_law: String,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update security agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSecurityAgreementApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: Option<String>,
    pub grantor: Option<String>,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: Option<String>,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: Option<String>,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Option<Vec<String>>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<String>,
    pub governing_law: Option<String>,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing security agreements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SecurityAgreementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by perfection status
    #[param(example = "perfected")]
    pub perfection_status: Option<String>,

    /// Filter by lien priority
    #[param(example = "first_lien")]
    pub lien_priority: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateSecurityAgreementsRequest {
    pub security_agreements: Vec<CreateSecurityAgreementApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::SecurityAgreement) -> SecurityAgreementResponse {
    SecurityAgreementResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        grantor: db.grantor,
        grantor_jurisdiction: db.grantor_jurisdiction,
        secured_party: db.secured_party,
        collateral_agent: db.collateral_agent,
        collateral_scope_summary: db.collateral_scope_summary,
        collateral_description: db.collateral_description,
        includes_accounts_receivable: db.includes_accounts_receivable,
        includes_inventory: db.includes_inventory,
        includes_equipment: db.includes_equipment,
        includes_intellectual_property: db.includes_intellectual_property,
        includes_real_estate: db.includes_real_estate,
        includes_deposit_accounts: db.includes_deposit_accounts,
        includes_investment_property: db.includes_investment_property,
        includes_general_intangibles: db.includes_general_intangibles,
        collateral_details: db.collateral_details,
        perfection_methods: db.perfection_methods,
        perfection_status: db.perfection_status,
        perfection_date: db.perfection_date,
        lien_priority: db.lien_priority,
        intercreditor_agreement_id: db.intercreditor_agreement_id.map(|id| id.to_string()),
        governing_law: db.governing_law,
        filing_jurisdictions: db.filing_jurisdictions,
        excluded_collateral: db.excluded_collateral,
        permitted_liens: db.permitted_liens,
        effective_date: db.effective_date,
        termination_date: db.termination_date,
        release_conditions: db.release_conditions,
        partial_release_permitted: db.partial_release_permitted,
        maintenance_covenants: db.maintenance_covenants,
        insurance_requirements: db.insurance_requirements,
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

/// List security agreements
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/security-agreements",
    params(SecurityAgreementListParams),
    responses(
        (status = 200, description = "List of security agreements", body = PaginatedResponse<SecurityAgreementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn list_security_agreements_handler(
    State(state): State<ApiState>,
    Query(params): Query<SecurityAgreementListParams>,
) -> Result<Json<PaginatedResponse<SecurityAgreementResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing security agreements");
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_security_agreements(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.perfection_status.clone(),
        params.lien_priority.clone(),
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List security agreements", e))?;

    let data: Vec<SecurityAgreementResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get security agreement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/security-agreements/{id}",
    params(
        ("id" = String, Path, description = "Security agreement UUID")
    ),
    responses(
        (status = 200, description = "Security agreement details", body = SecurityAgreementResponse),
        (status = 404, description = "Security agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn get_security_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<SecurityAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting security agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_security_agreement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get security agreement", e))?
        .ok_or_else(|| not_found_error("Security agreement", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create security agreement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/security-agreements",
    request_body = CreateSecurityAgreementApiRequest,
    responses(
        (status = 201, description = "Security agreement created", body = SecurityAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn create_security_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateSecurityAgreementApiRequest>,
) -> Result<(StatusCode, Json<SecurityAgreementResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Creating security agreement {}",
        request.document_id
    );
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let intercreditor_agreement_id = if let Some(id_str) = request.intercreditor_agreement_id {
        Some(parse_uuid(&id_str, "intercreditor_agreement_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash: request.doc_hash,
        grantor: request.grantor,
        grantor_jurisdiction: request.grantor_jurisdiction,
        secured_party: request.secured_party,
        collateral_agent: request.collateral_agent,
        collateral_scope_summary: request.collateral_scope_summary,
        collateral_description: request.collateral_description,
        includes_accounts_receivable: request.includes_accounts_receivable,
        includes_inventory: request.includes_inventory,
        includes_equipment: request.includes_equipment,
        includes_intellectual_property: request.includes_intellectual_property,
        includes_real_estate: request.includes_real_estate,
        includes_deposit_accounts: request.includes_deposit_accounts,
        includes_investment_property: request.includes_investment_property,
        includes_general_intangibles: request.includes_general_intangibles,
        collateral_details: request.collateral_details,
        perfection_methods: request.perfection_methods,
        perfection_status: request.perfection_status,
        perfection_date: request.perfection_date,
        lien_priority: request.lien_priority,
        intercreditor_agreement_id,
        governing_law: request.governing_law,
        filing_jurisdictions: request.filing_jurisdictions,
        excluded_collateral: request.excluded_collateral,
        permitted_liens: request.permitted_liens,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        release_conditions: request.release_conditions,
        partial_release_permitted: request.partial_release_permitted,
        maintenance_covenants: request.maintenance_covenants,
        insurance_requirements: request.insurance_requirements,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_security_agreement(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create security agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create security agreements in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/security-agreements/batch",
    request_body = BatchCreateSecurityAgreementsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn create_security_agreements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateSecurityAgreementsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} security agreements",
        request.security_agreements.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.security_agreements {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let intercreditor_agreement_id = if let Some(id_str) = req.intercreditor_agreement_id {
            match parse_uuid(&id_str, "intercreditor_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            document_id: req.document_id,
            credit_agreement_id,
            doc_hash: req.doc_hash,
            grantor: req.grantor,
            grantor_jurisdiction: req.grantor_jurisdiction,
            secured_party: req.secured_party,
            collateral_agent: req.collateral_agent,
            collateral_scope_summary: req.collateral_scope_summary,
            collateral_description: req.collateral_description,
            includes_accounts_receivable: req.includes_accounts_receivable,
            includes_inventory: req.includes_inventory,
            includes_equipment: req.includes_equipment,
            includes_intellectual_property: req.includes_intellectual_property,
            includes_real_estate: req.includes_real_estate,
            includes_deposit_accounts: req.includes_deposit_accounts,
            includes_investment_property: req.includes_investment_property,
            includes_general_intangibles: req.includes_general_intangibles,
            collateral_details: req.collateral_details,
            perfection_methods: req.perfection_methods,
            perfection_status: req.perfection_status,
            perfection_date: req.perfection_date,
            lien_priority: req.lien_priority,
            intercreditor_agreement_id,
            governing_law: req.governing_law,
            filing_jurisdictions: req.filing_jurisdictions,
            excluded_collateral: req.excluded_collateral,
            permitted_liens: req.permitted_liens,
            effective_date: req.effective_date,
            termination_date: req.termination_date,
            release_conditions: req.release_conditions,
            partial_release_permitted: req.partial_release_permitted,
            maintenance_covenants: req.maintenance_covenants,
            insurance_requirements: req.insurance_requirements,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_security_agreements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create security agreements", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update security agreement
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/security-agreements/{id}",
    params(
        ("id" = String, Path, description = "Security agreement UUID")
    ),
    request_body = UpdateSecurityAgreementApiRequest,
    responses(
        (status = 200, description = "Security agreement updated", body = SecurityAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Security agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn update_security_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateSecurityAgreementApiRequest>,
) -> Result<Json<SecurityAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating security agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let intercreditor_agreement_id = if let Some(id_str) = request.intercreditor_agreement_id {
        Some(parse_uuid(&id_str, "intercreditor_agreement_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash: request.doc_hash,
        grantor: request.grantor,
        grantor_jurisdiction: request.grantor_jurisdiction,
        secured_party: request.secured_party,
        collateral_agent: request.collateral_agent,
        collateral_scope_summary: request.collateral_scope_summary,
        collateral_description: request.collateral_description,
        includes_accounts_receivable: request.includes_accounts_receivable,
        includes_inventory: request.includes_inventory,
        includes_equipment: request.includes_equipment,
        includes_intellectual_property: request.includes_intellectual_property,
        includes_real_estate: request.includes_real_estate,
        includes_deposit_accounts: request.includes_deposit_accounts,
        includes_investment_property: request.includes_investment_property,
        includes_general_intangibles: request.includes_general_intangibles,
        collateral_details: request.collateral_details,
        perfection_methods: request.perfection_methods,
        perfection_status: request.perfection_status,
        perfection_date: request.perfection_date,
        lien_priority: request.lien_priority,
        intercreditor_agreement_id,
        governing_law: request.governing_law,
        filing_jurisdictions: request.filing_jurisdictions,
        excluded_collateral: request.excluded_collateral,
        permitted_liens: request.permitted_liens,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        release_conditions: request.release_conditions,
        partial_release_permitted: request.partial_release_permitted,
        maintenance_covenants: request.maintenance_covenants,
        insurance_requirements: request.insurance_requirements,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_security_agreement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update security agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete security agreement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/security-agreements/{id}",
    params(
        ("id" = String, Path, description = "Security agreement UUID")
    ),
    responses(
        (status = 200, description = "Security agreement deleted", body = DeleteResponse),
        (status = 404, description = "Security agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Security Agreements"
)]
#[instrument(skip(state))]
pub async fn delete_security_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting security agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "security_agreements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_security_agreement(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete security agreement", e))?;

    if !deleted {
        return Err(not_found_error("Security agreement", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "security_agreements", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Security agreement {} deleted successfully", id),
    }))
}
