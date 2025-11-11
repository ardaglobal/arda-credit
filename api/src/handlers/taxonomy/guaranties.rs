//! Guaranties API Handlers
//!
//! Provides REST API endpoints for guaranty CRUD operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_guaranties_batch, create_guaranty, delete_guaranty, get_guaranty_by_id, list_guaranties,
    update_guaranty, CreateGuarantyRequest as DbCreateRequest,
    UpdateGuarantyRequest as DbUpdateRequest,
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

/// Guaranty API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "GUA-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "guarantor_name": "Parent Company LLC",
    "guarantor_type": "parent",
    "guarantee_type": "full_unconditional",
    "guaranty_status": "effective"
}))]
pub struct GuarantyResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub guarantor_name: String,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: String,
    pub guarantee_type: String,
    pub coverage_scope: String,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    pub cap_amount: Option<String>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: String,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    pub amount_claimed: Option<String>,
    pub amount_recovered: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create guaranty request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateGuarantyApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: String,
    pub guarantor_name: String,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: String,
    pub guarantee_type: String,
    pub coverage_scope: String,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: String,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub amount_claimed: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update guaranty request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateGuarantyApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash: Option<String>,
    pub guarantor_name: Option<String>,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: Option<String>,
    pub guarantee_type: Option<String>,
    pub coverage_scope: Option<String>,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub amount_claimed: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing guaranties
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GuarantyListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by guarantor type
    #[param(example = "parent")]
    pub guarantor_type: Option<String>,

    /// Filter by guarantee type
    #[param(example = "full_unconditional")]
    pub guarantee_type: Option<String>,

    /// Filter by guaranty status
    #[param(example = "effective")]
    pub guaranty_status: Option<String>,

    /// Filter by is_capped flag
    #[param(example = "true")]
    pub is_capped: Option<bool>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateGuarantiesRequest {
    pub guaranties: Vec<CreateGuarantyApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::Guaranty) -> GuarantyResponse {
    GuarantyResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        guarantor_name: db.guarantor_name,
        guarantor_type: db.guarantor_type,
        guarantor_jurisdiction: db.guarantor_jurisdiction,
        guarantor_relationship: db.guarantor_relationship,
        is_joint_guaranty: db.is_joint_guaranty,
        other_guarantors: db.other_guarantors,
        beneficiary: db.beneficiary,
        guarantee_type: db.guarantee_type,
        coverage_scope: db.coverage_scope,
        covers_principal: db.covers_principal,
        covers_interest: db.covers_interest,
        covers_fees: db.covers_fees,
        covers_expenses: db.covers_expenses,
        covers_amendments: db.covers_amendments,
        is_capped: db.is_capped,
        cap_amount: db.cap_amount.map(|d| d.to_string()),
        cap_currency: db.cap_currency,
        cap_type: db.cap_type,
        cap_formula: db.cap_formula,
        is_payment_guaranty: db.is_payment_guaranty,
        is_collection_guaranty: db.is_collection_guaranty,
        is_continuing: db.is_continuing,
        revolving_guaranty: db.revolving_guaranty,
        waivers_of_defenses: db.waivers_of_defenses,
        suretyship_defenses_waived: db.suretyship_defenses_waived,
        notice_of_default_waived: db.notice_of_default_waived,
        notice_of_acceptance_waived: db.notice_of_acceptance_waived,
        subrogation_rights: db.subrogation_rights,
        subrogation_subordinated: db.subrogation_subordinated,
        subordination_provisions: db.subordination_provisions,
        secured_guaranty: db.secured_guaranty,
        guarantor_security_description: db.guarantor_security_description,
        guarantor_covenants: db.guarantor_covenants,
        release_conditions: db.release_conditions,
        automatic_release_trigger: db.automatic_release_trigger,
        partial_release_permitted: db.partial_release_permitted,
        effective_date: db.effective_date,
        termination_date: db.termination_date,
        governing_law: db.governing_law,
        enforceability_opinion_received: db.enforceability_opinion_received,
        opinion_reference: db.opinion_reference,
        corporate_authority_documents: db.corporate_authority_documents,
        guaranty_status: db.guaranty_status,
        claims_filed: db.claims_filed,
        amount_claimed: db.amount_claimed.map(|d| d.to_string()),
        amount_recovered: db.amount_recovered.map(|d| d.to_string()),
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

/// List guaranties
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/guaranties",
    params(GuarantyListParams),
    responses(
        (status = 200, description = "List of guaranties", body = PaginatedResponse<GuarantyResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn list_guaranties_handler(
    State(state): State<ApiState>,
    Query(params): Query<GuarantyListParams>,
) -> Result<Json<PaginatedResponse<GuarantyResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing guaranties");
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "list")
        .increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_guaranties(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.guarantor_type.clone(),
        params.guarantee_type.clone(),
        params.guaranty_status.clone(),
        params.is_capped,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List guaranties", e))?;

    let data: Vec<GuarantyResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get guaranty by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/guaranties/{id}",
    params(
        ("id" = String, Path, description = "Guaranty UUID")
    ),
    responses(
        (status = 200, description = "Guaranty details", body = GuarantyResponse),
        (status = 404, description = "Guaranty not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn get_guaranty_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<GuarantyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting guaranty {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "get")
        .increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_guaranty_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get guaranty", e))?
        .ok_or_else(|| not_found_error("Guaranty", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create guaranty
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/guaranties",
    request_body = CreateGuarantyApiRequest,
    responses(
        (status = 201, description = "Guaranty created", body = GuarantyResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn create_guaranty_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateGuarantyApiRequest>,
) -> Result<(StatusCode, Json<GuarantyResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating guaranty {}", request.document_id);
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "create").increment(1);

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
        doc_hash: request.doc_hash,
        guarantor_name: request.guarantor_name,
        guarantor_type: request.guarantor_type,
        guarantor_jurisdiction: request.guarantor_jurisdiction,
        guarantor_relationship: request.guarantor_relationship,
        is_joint_guaranty: request.is_joint_guaranty,
        other_guarantors: request.other_guarantors,
        beneficiary: request.beneficiary,
        guarantee_type: request.guarantee_type,
        coverage_scope: request.coverage_scope,
        covers_principal: request.covers_principal,
        covers_interest: request.covers_interest,
        covers_fees: request.covers_fees,
        covers_expenses: request.covers_expenses,
        covers_amendments: request.covers_amendments,
        is_capped: request.is_capped,
        cap_amount: request.cap_amount,
        cap_currency: request.cap_currency,
        cap_type: request.cap_type,
        cap_formula: request.cap_formula,
        is_payment_guaranty: request.is_payment_guaranty,
        is_collection_guaranty: request.is_collection_guaranty,
        is_continuing: request.is_continuing,
        revolving_guaranty: request.revolving_guaranty,
        waivers_of_defenses: request.waivers_of_defenses,
        suretyship_defenses_waived: request.suretyship_defenses_waived,
        notice_of_default_waived: request.notice_of_default_waived,
        notice_of_acceptance_waived: request.notice_of_acceptance_waived,
        subrogation_rights: request.subrogation_rights,
        subrogation_subordinated: request.subrogation_subordinated,
        subordination_provisions: request.subordination_provisions,
        secured_guaranty: request.secured_guaranty,
        guarantor_security_description: request.guarantor_security_description,
        guarantor_covenants: request.guarantor_covenants,
        release_conditions: request.release_conditions,
        automatic_release_trigger: request.automatic_release_trigger,
        partial_release_permitted: request.partial_release_permitted,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        governing_law: request.governing_law,
        enforceability_opinion_received: request.enforceability_opinion_received,
        opinion_reference: request.opinion_reference,
        corporate_authority_documents: request.corporate_authority_documents,
        guaranty_status: request.guaranty_status,
        claims_filed: request.claims_filed,
        amount_claimed: request.amount_claimed,
        amount_recovered: request.amount_recovered,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_guaranty(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create guaranty", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create guaranties in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/guaranties/batch",
    request_body = BatchCreateGuarantiesRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn create_guaranties_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateGuarantiesRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} guaranties",
        request.guaranties.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.guaranties {
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
            doc_hash: req.doc_hash,
            guarantor_name: req.guarantor_name,
            guarantor_type: req.guarantor_type,
            guarantor_jurisdiction: req.guarantor_jurisdiction,
            guarantor_relationship: req.guarantor_relationship,
            is_joint_guaranty: req.is_joint_guaranty,
            other_guarantors: req.other_guarantors,
            beneficiary: req.beneficiary,
            guarantee_type: req.guarantee_type,
            coverage_scope: req.coverage_scope,
            covers_principal: req.covers_principal,
            covers_interest: req.covers_interest,
            covers_fees: req.covers_fees,
            covers_expenses: req.covers_expenses,
            covers_amendments: req.covers_amendments,
            is_capped: req.is_capped,
            cap_amount: req.cap_amount,
            cap_currency: req.cap_currency,
            cap_type: req.cap_type,
            cap_formula: req.cap_formula,
            is_payment_guaranty: req.is_payment_guaranty,
            is_collection_guaranty: req.is_collection_guaranty,
            is_continuing: req.is_continuing,
            revolving_guaranty: req.revolving_guaranty,
            waivers_of_defenses: req.waivers_of_defenses,
            suretyship_defenses_waived: req.suretyship_defenses_waived,
            notice_of_default_waived: req.notice_of_default_waived,
            notice_of_acceptance_waived: req.notice_of_acceptance_waived,
            subrogation_rights: req.subrogation_rights,
            subrogation_subordinated: req.subrogation_subordinated,
            subordination_provisions: req.subordination_provisions,
            secured_guaranty: req.secured_guaranty,
            guarantor_security_description: req.guarantor_security_description,
            guarantor_covenants: req.guarantor_covenants,
            release_conditions: req.release_conditions,
            automatic_release_trigger: req.automatic_release_trigger,
            partial_release_permitted: req.partial_release_permitted,
            effective_date: req.effective_date,
            termination_date: req.termination_date,
            governing_law: req.governing_law,
            enforceability_opinion_received: req.enforceability_opinion_received,
            opinion_reference: req.opinion_reference,
            corporate_authority_documents: req.corporate_authority_documents,
            guaranty_status: req.guaranty_status,
            claims_filed: req.claims_filed,
            amount_claimed: req.amount_claimed,
            amount_recovered: req.amount_recovered,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_guaranties_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create guaranties", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update guaranty
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/guaranties/{id}",
    params(
        ("id" = String, Path, description = "Guaranty UUID")
    ),
    request_body = UpdateGuarantyApiRequest,
    responses(
        (status = 200, description = "Guaranty updated", body = GuarantyResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Guaranty not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn update_guaranty_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateGuarantyApiRequest>,
) -> Result<Json<GuarantyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating guaranty {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "update").increment(1);

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
        doc_hash: request.doc_hash,
        guarantor_name: request.guarantor_name,
        guarantor_type: request.guarantor_type,
        guarantor_jurisdiction: request.guarantor_jurisdiction,
        guarantor_relationship: request.guarantor_relationship,
        is_joint_guaranty: request.is_joint_guaranty,
        other_guarantors: request.other_guarantors,
        beneficiary: request.beneficiary,
        guarantee_type: request.guarantee_type,
        coverage_scope: request.coverage_scope,
        covers_principal: request.covers_principal,
        covers_interest: request.covers_interest,
        covers_fees: request.covers_fees,
        covers_expenses: request.covers_expenses,
        covers_amendments: request.covers_amendments,
        is_capped: request.is_capped,
        cap_amount: request.cap_amount,
        cap_currency: request.cap_currency,
        cap_type: request.cap_type,
        cap_formula: request.cap_formula,
        is_payment_guaranty: request.is_payment_guaranty,
        is_collection_guaranty: request.is_collection_guaranty,
        is_continuing: request.is_continuing,
        revolving_guaranty: request.revolving_guaranty,
        waivers_of_defenses: request.waivers_of_defenses,
        suretyship_defenses_waived: request.suretyship_defenses_waived,
        notice_of_default_waived: request.notice_of_default_waived,
        notice_of_acceptance_waived: request.notice_of_acceptance_waived,
        subrogation_rights: request.subrogation_rights,
        subrogation_subordinated: request.subrogation_subordinated,
        subordination_provisions: request.subordination_provisions,
        secured_guaranty: request.secured_guaranty,
        guarantor_security_description: request.guarantor_security_description,
        guarantor_covenants: request.guarantor_covenants,
        release_conditions: request.release_conditions,
        automatic_release_trigger: request.automatic_release_trigger,
        partial_release_permitted: request.partial_release_permitted,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        governing_law: request.governing_law,
        enforceability_opinion_received: request.enforceability_opinion_received,
        opinion_reference: request.opinion_reference,
        corporate_authority_documents: request.corporate_authority_documents,
        guaranty_status: request.guaranty_status,
        claims_filed: request.claims_filed,
        amount_claimed: request.amount_claimed,
        amount_recovered: request.amount_recovered,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_guaranty(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update guaranty", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete guaranty
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/guaranties/{id}",
    params(
        ("id" = String, Path, description = "Guaranty UUID")
    ),
    responses(
        (status = 200, description = "Guaranty deleted", body = DeleteResponse),
        (status = 404, description = "Guaranty not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Guaranties"
)]
#[instrument(skip(state))]
pub async fn delete_guaranty_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting guaranty {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "guaranties", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_guaranty(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete guaranty", e))?;

    if !deleted {
        return Err(not_found_error("Guaranty", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "guaranties", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Guaranty {} deleted successfully", id),
    }))
}
