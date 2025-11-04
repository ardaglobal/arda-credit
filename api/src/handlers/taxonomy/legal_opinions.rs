//! Legal Opinions API Handlers
//!
//! Provides REST API endpoints for legal opinion CRUD operations.

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
    create_legal_opinion, create_legal_opinions_batch, delete_legal_opinion,
    get_legal_opinion_by_id, list_legal_opinions, update_legal_opinion,
    CreateLegalOpinionRequest as DbCreateRequest, UpdateLegalOpinionRequest as DbUpdateRequest,
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

/// Legal Opinion API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LegalOpinionResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_legal_opinion: Vec<String>,
    pub opinion_firms: Vec<String>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Vec<String>,
    pub governing_laws: Vec<String>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: bool,
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: NaiveDate,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: bool,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<String>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<String>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create legal opinion request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLegalOpinionApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_legal_opinion: Vec<String>,
    pub opinion_firms: Vec<String>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Vec<String>,
    pub governing_laws: Vec<String>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: Option<bool>,
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: NaiveDate,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: Option<bool>,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<String>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<String>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update legal opinion request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateLegalOpinionApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub doc_hash_legal_opinion: Option<Vec<String>>,
    pub opinion_firms: Option<Vec<String>>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub governing_laws: Option<Vec<String>>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: Option<bool>,
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: Option<bool>,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<String>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<String>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing legal opinions
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct LegalOpinionListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    pub credit_agreement_id: Option<String>,

    /// Filter by coverage status
    pub coverage_status: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateLegalOpinionsRequest {
    pub legal_opinions: Vec<CreateLegalOpinionApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::LegalOpinion) -> LegalOpinionResponse {
    LegalOpinionResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        doc_hash_legal_opinion: db.doc_hash_legal_opinion,
        opinion_firms: db.opinion_firms,
        lead_opinion_firm: db.lead_opinion_firm,
        scopes: db.scopes,
        governing_laws: db.governing_laws,
        primary_governing_law: db.primary_governing_law,
        reliance_language_present: db.reliance_language_present,
        beneficiaries: db.beneficiaries,
        opinion_date: db.opinion_date,
        effective_date: db.effective_date,
        limitations: db.limitations,
        assumptions: db.assumptions,
        full_opinion_delivered: db.full_opinion_delivered,
        disclosure_schedules_attached: db.disclosure_schedules_attached,
        bring_down_opinion_id: db.bring_down_opinion_id.map(|id| id.to_string()),
        is_bring_down: db.is_bring_down,
        original_opinion_id: db.original_opinion_id.map(|id| id.to_string()),
        coverage_status: db.coverage_status,
        gaps_or_qualifications: db.gaps_or_qualifications,
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

/// List legal opinions
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/legal-opinions",
    params(LegalOpinionListParams),
    responses(
        (status = 200, description = "List of legal opinions", body = PaginatedResponse<LegalOpinionResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn list_legal_opinions_handler(
    State(state): State<ApiState>,
    Query(params): Query<LegalOpinionListParams>,
) -> Result<Json<PaginatedResponse<LegalOpinionResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing legal opinions");
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_legal_opinions(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.coverage_status,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List legal opinions", e))?;

    let data: Vec<LegalOpinionResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get legal opinion by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/legal-opinions/{id}",
    params(
        ("id" = String, Path, description = "Legal opinion UUID")
    ),
    responses(
        (status = 200, description = "Legal opinion details", body = LegalOpinionResponse),
        (status = 404, description = "Legal opinion not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn get_legal_opinion_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<LegalOpinionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting legal opinion {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_legal_opinion_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get legal opinion", e))?
        .ok_or_else(|| not_found_error("Legal opinion", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create legal opinion
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/legal-opinions",
    request_body = CreateLegalOpinionApiRequest,
    responses(
        (status = 201, description = "Legal opinion created", body = LegalOpinionResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn create_legal_opinion_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateLegalOpinionApiRequest>,
) -> Result<(StatusCode, Json<LegalOpinionResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating legal opinion {}", request.document_id);
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "create").increment(1);

    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let bring_down_opinion_id = if let Some(id_str) = request.bring_down_opinion_id {
        Some(parse_uuid(&id_str, "bring_down_opinion_id")?)
    } else {
        None
    };

    let original_opinion_id = if let Some(id_str) = request.original_opinion_id {
        Some(parse_uuid(&id_str, "original_opinion_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash_legal_opinion: request.doc_hash_legal_opinion,
        opinion_firms: request.opinion_firms,
        lead_opinion_firm: request.lead_opinion_firm,
        scopes: request.scopes,
        governing_laws: request.governing_laws,
        primary_governing_law: request.primary_governing_law,
        reliance_language_present: request.reliance_language_present,
        beneficiaries: request.beneficiaries,
        opinion_date: request.opinion_date,
        effective_date: request.effective_date,
        limitations: request.limitations,
        assumptions: request.assumptions,
        full_opinion_delivered: request.full_opinion_delivered,
        disclosure_schedules_attached: request.disclosure_schedules_attached,
        bring_down_opinion_id,
        is_bring_down: request.is_bring_down,
        original_opinion_id,
        coverage_status: request.coverage_status,
        gaps_or_qualifications: request.gaps_or_qualifications,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_legal_opinion(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create legal opinion", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create legal opinions in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/legal-opinions/batch",
    request_body = BatchCreateLegalOpinionsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn create_legal_opinions_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateLegalOpinionsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} legal opinions",
        request.legal_opinions.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "batch_create")
        .increment(1);

    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.legal_opinions {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let bring_down_opinion_id = if let Some(id_str) = req.bring_down_opinion_id {
            match parse_uuid(&id_str, "bring_down_opinion_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let original_opinion_id = if let Some(id_str) = req.original_opinion_id {
            match parse_uuid(&id_str, "original_opinion_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            document_id: req.document_id,
            credit_agreement_id,
            doc_hash_legal_opinion: req.doc_hash_legal_opinion,
            opinion_firms: req.opinion_firms,
            lead_opinion_firm: req.lead_opinion_firm,
            scopes: req.scopes,
            governing_laws: req.governing_laws,
            primary_governing_law: req.primary_governing_law,
            reliance_language_present: req.reliance_language_present,
            beneficiaries: req.beneficiaries,
            opinion_date: req.opinion_date,
            effective_date: req.effective_date,
            limitations: req.limitations,
            assumptions: req.assumptions,
            full_opinion_delivered: req.full_opinion_delivered,
            disclosure_schedules_attached: req.disclosure_schedules_attached,
            bring_down_opinion_id,
            is_bring_down: req.is_bring_down,
            original_opinion_id,
            coverage_status: req.coverage_status,
            gaps_or_qualifications: req.gaps_or_qualifications,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_legal_opinions_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create legal opinions", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update legal opinion
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/legal-opinions/{id}",
    params(
        ("id" = String, Path, description = "Legal opinion UUID")
    ),
    request_body = UpdateLegalOpinionApiRequest,
    responses(
        (status = 200, description = "Legal opinion updated", body = LegalOpinionResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Legal opinion not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn update_legal_opinion_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateLegalOpinionApiRequest>,
) -> Result<Json<LegalOpinionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating legal opinion {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let bring_down_opinion_id = if let Some(id_str) = request.bring_down_opinion_id {
        Some(parse_uuid(&id_str, "bring_down_opinion_id")?)
    } else {
        None
    };

    let original_opinion_id = if let Some(id_str) = request.original_opinion_id {
        Some(parse_uuid(&id_str, "original_opinion_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        doc_hash_legal_opinion: request.doc_hash_legal_opinion,
        opinion_firms: request.opinion_firms,
        lead_opinion_firm: request.lead_opinion_firm,
        scopes: request.scopes,
        governing_laws: request.governing_laws,
        primary_governing_law: request.primary_governing_law,
        reliance_language_present: request.reliance_language_present,
        beneficiaries: request.beneficiaries,
        opinion_date: request.opinion_date,
        effective_date: request.effective_date,
        limitations: request.limitations,
        assumptions: request.assumptions,
        full_opinion_delivered: request.full_opinion_delivered,
        disclosure_schedules_attached: request.disclosure_schedules_attached,
        bring_down_opinion_id,
        is_bring_down: request.is_bring_down,
        original_opinion_id,
        coverage_status: request.coverage_status,
        gaps_or_qualifications: request.gaps_or_qualifications,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_legal_opinion(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update legal opinion", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete legal opinion
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/legal-opinions/{id}",
    params(
        ("id" = String, Path, description = "Legal opinion UUID")
    ),
    responses(
        (status = 200, description = "Legal opinion deleted", body = DeleteResponse),
        (status = 404, description = "Legal opinion not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Legal Opinions"
)]
#[instrument(skip(state))]
pub async fn delete_legal_opinion_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting legal opinion {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "legal_opinions", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_legal_opinion(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete legal opinion", e))?;

    if !deleted {
        return Err(not_found_error("Legal opinion", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "legal_opinions", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Legal opinion {} deleted successfully", id),
    }))
}
