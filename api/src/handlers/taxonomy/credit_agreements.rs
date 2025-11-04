//! Credit Agreements API Handlers
//!
//! Provides REST API endpoints for credit agreement CRUD operations.

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
    create_credit_agreement, create_credit_agreements_batch, delete_credit_agreement,
    get_credit_agreement_by_id, list_credit_agreements, update_credit_agreement,
    CreateCreditAgreementRequest as DbCreateRequest,
    UpdateCreditAgreementRequest as DbUpdateRequest,
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

/// Credit Agreement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "CA-2025-001",
    "borrower_legal_name": "Acme Corp",
    "agent_or_lender": "Bank of America",
    "facility_type": "Term Loan",
    "committed_amount": "50000000.00",
    "deal_state": "executed"
}))]
pub struct CreditAgreementResponse {
    pub id: String,
    pub document_id: String,
    pub version: String,
    pub execution_status: String,
    pub executed_pdf_hash: String,
    pub bundle_hash: Option<String>,
    pub effective_date: NaiveDate,
    pub maturity_date: NaiveDate,
    pub closing_date: Option<NaiveDate>,
    pub created_at: String,
    pub updated_at: String,
    pub governing_law: String,
    pub borrower_legal_name: String,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: String,
    pub collateral_agent: Option<String>,
    pub guarantors_present: bool,
    pub guarantor_names: Option<Vec<String>>,
    pub facility_type: String,
    #[schema(value_type = String)]
    pub committed_amount: Decimal,
    pub currency: String,
    pub draw_rules: Option<String>,
    pub repayment_type: String,
    pub amortization_schedule: Option<serde_json::Value>,
    pub reference_rate: String,
    pub margin_bps: i32,
    pub floor_bps: Option<i32>,
    pub secured_flag: bool,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: String,
    pub metadata: Option<serde_json::Value>,
}

/// Create credit agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateCreditAgreementApiRequest {
    #[schema(example = "CA-2025-001")]
    pub document_id: String,

    #[schema(example = "1.0")]
    pub version: Option<String>,

    #[schema(example = "executed")]
    pub execution_status: Option<String>,

    #[schema(example = "sha256:abcdef123456...")]
    pub executed_pdf_hash: String,

    pub bundle_hash: Option<String>,

    #[schema(value_type = String, example = "2025-01-01")]
    pub effective_date: NaiveDate,

    #[schema(value_type = String, example = "2030-01-01")]
    pub maturity_date: NaiveDate,

    #[schema(value_type = String)]
    pub closing_date: Option<NaiveDate>,

    #[schema(example = "New York")]
    pub governing_law: String,

    #[schema(example = "Acme Corporation")]
    pub borrower_legal_name: String,

    pub borrower_external_id: Option<String>,

    #[schema(example = "Bank of America, N.A.")]
    pub agent_or_lender: String,

    pub collateral_agent: Option<String>,

    #[schema(example = true)]
    pub guarantors_present: bool,

    pub guarantor_names: Option<Vec<String>>,

    #[schema(example = "Term Loan")]
    pub facility_type: String,

    #[schema(value_type = String, example = "50000000.00")]
    pub committed_amount: Decimal,

    #[schema(example = "USD")]
    pub currency: Option<String>,

    pub draw_rules: Option<String>,

    #[schema(example = "Amortizing")]
    pub repayment_type: String,

    pub amortization_schedule: Option<serde_json::Value>,

    #[schema(example = "SOFR")]
    pub reference_rate: String,

    #[schema(example = 250)]
    pub margin_bps: i32,

    #[schema(example = 0)]
    pub floor_bps: Option<i32>,

    #[schema(example = true)]
    pub secured_flag: bool,

    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,

    #[schema(example = "executed")]
    pub deal_state: Option<String>,

    pub metadata: Option<serde_json::Value>,
}

/// Update credit agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateCreditAgreementApiRequest {
    pub document_id: Option<String>,
    pub version: Option<String>,
    pub execution_status: Option<String>,
    pub executed_pdf_hash: Option<String>,
    pub bundle_hash: Option<String>,
    #[schema(value_type = String)]
    pub effective_date: Option<NaiveDate>,
    #[schema(value_type = String)]
    pub maturity_date: Option<NaiveDate>,
    #[schema(value_type = String)]
    pub closing_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub borrower_legal_name: Option<String>,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: Option<String>,
    pub collateral_agent: Option<String>,
    pub guarantors_present: Option<bool>,
    pub guarantor_names: Option<Vec<String>>,
    pub facility_type: Option<String>,
    #[schema(value_type = String)]
    pub committed_amount: Option<Decimal>,
    pub currency: Option<String>,
    pub draw_rules: Option<String>,
    pub repayment_type: Option<String>,
    pub amortization_schedule: Option<serde_json::Value>,
    pub reference_rate: Option<String>,
    pub margin_bps: Option<i32>,
    pub floor_bps: Option<i32>,
    pub secured_flag: Option<bool>,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing credit agreements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct CreditAgreementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by deal state
    #[param(example = "executed")]
    pub deal_state: Option<String>,

    /// Filter by execution status
    #[param(example = "executed")]
    pub execution_status: Option<String>,

    /// Filter by facility type
    #[param(example = "Term Loan")]
    pub facility_type: Option<String>,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// List credit agreements with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/credit-agreements",
    params(CreditAgreementListParams),
    responses(
        (status = 200, description = "Successfully retrieved credit agreements", body = PaginatedResponse<CreditAgreementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state), level = "info")]
pub async fn list_credit_agreements_handler(
    State(state): State<ApiState>,
    Query(params): Query<CreditAgreementListParams>,
) -> Result<Json<PaginatedResponse<CreditAgreementResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing credit agreements");
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();
    let sort_by = params.pagination.sort_by.clone();
    let sort_order = Some(params.pagination.get_sort_order().to_string());

    let (records, total) = list_credit_agreements(
        &state.pool,
        limit,
        offset,
        params.deal_state,
        params.execution_status,
        params.facility_type,
        sort_by,
        sort_order,
    )
    .await
    .map_err(|e| internal_error("List credit agreements", e))?;

    let data: Vec<CreditAgreementResponse> = records
        .into_iter()
        .map(|r| CreditAgreementResponse {
            id: r.id.to_string(),
            document_id: r.document_id,
            version: r.version,
            execution_status: r.execution_status,
            executed_pdf_hash: r.executed_pdf_hash,
            bundle_hash: r.bundle_hash,
            effective_date: r.effective_date,
            maturity_date: r.maturity_date,
            closing_date: r.closing_date,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
            governing_law: r.governing_law,
            borrower_legal_name: r.borrower_legal_name,
            borrower_external_id: r.borrower_external_id,
            agent_or_lender: r.agent_or_lender,
            collateral_agent: r.collateral_agent,
            guarantors_present: r.guarantors_present,
            guarantor_names: r.guarantor_names,
            facility_type: r.facility_type,
            committed_amount: r.committed_amount,
            currency: r.currency,
            draw_rules: r.draw_rules,
            repayment_type: r.repayment_type,
            amortization_schedule: r.amortization_schedule,
            reference_rate: r.reference_rate,
            margin_bps: r.margin_bps,
            floor_bps: r.floor_bps,
            secured_flag: r.secured_flag,
            collateral_scope_summary: r.collateral_scope_summary,
            financial_covenants: r.financial_covenants,
            use_of_proceeds: r.use_of_proceeds,
            accordion_provision: r.accordion_provision,
            prepayment_terms: r.prepayment_terms,
            default_provisions: r.default_provisions,
            deal_state: r.deal_state,
            metadata: r.metadata,
        })
        .collect();

    let count = data.len() as i64;
    let response = PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    };

    info!(
        "✅ API: Listed {} credit agreements (total: {})",
        count, total
    );
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "list")
        .record(elapsed.as_secs_f64());

    Ok(Json(response))
}

/// Get credit agreement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/credit-agreements/{id}",
    params(
        ("id" = String, Path, description = "Credit agreement UUID")
    ),
    responses(
        (status = 200, description = "Successfully retrieved credit agreement", body = CreditAgreementResponse),
        (status = 400, description = "Invalid UUID format", body = ErrorResponse),
        (status = 404, description = "Credit agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state), level = "info")]
pub async fn get_credit_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CreditAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting credit agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_credit_agreement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get credit agreement", e))?
        .ok_or_else(|| not_found_error("Credit agreement", &id))?;

    let response = CreditAgreementResponse {
        id: record.id.to_string(),
        document_id: record.document_id,
        version: record.version,
        execution_status: record.execution_status,
        executed_pdf_hash: record.executed_pdf_hash,
        bundle_hash: record.bundle_hash,
        effective_date: record.effective_date,
        maturity_date: record.maturity_date,
        closing_date: record.closing_date,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
        governing_law: record.governing_law,
        borrower_legal_name: record.borrower_legal_name,
        borrower_external_id: record.borrower_external_id,
        agent_or_lender: record.agent_or_lender,
        collateral_agent: record.collateral_agent,
        guarantors_present: record.guarantors_present,
        guarantor_names: record.guarantor_names,
        facility_type: record.facility_type,
        committed_amount: record.committed_amount,
        currency: record.currency,
        draw_rules: record.draw_rules,
        repayment_type: record.repayment_type,
        amortization_schedule: record.amortization_schedule,
        reference_rate: record.reference_rate,
        margin_bps: record.margin_bps,
        floor_bps: record.floor_bps,
        secured_flag: record.secured_flag,
        collateral_scope_summary: record.collateral_scope_summary,
        financial_covenants: record.financial_covenants,
        use_of_proceeds: record.use_of_proceeds,
        accordion_provision: record.accordion_provision,
        prepayment_terms: record.prepayment_terms,
        default_provisions: record.default_provisions,
        deal_state: record.deal_state,
        metadata: record.metadata,
    };

    info!("✅ API: Retrieved credit agreement {}", id);
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "get")
        .record(elapsed.as_secs_f64());

    Ok(Json(response))
}

/// Create a new credit agreement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/credit-agreements",
    request_body = CreateCreditAgreementApiRequest,
    responses(
        (status = 201, description = "Credit agreement created successfully", body = CreditAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state, headers), level = "info")]
pub async fn create_credit_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateCreditAgreementApiRequest>,
) -> Result<(StatusCode, Json<CreditAgreementResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating credit agreement {}", request.document_id);
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        version: request.version,
        execution_status: request.execution_status,
        executed_pdf_hash: request.executed_pdf_hash,
        bundle_hash: request.bundle_hash,
        effective_date: request.effective_date,
        maturity_date: request.maturity_date,
        closing_date: request.closing_date,
        governing_law: request.governing_law,
        borrower_legal_name: request.borrower_legal_name,
        borrower_external_id: request.borrower_external_id,
        agent_or_lender: request.agent_or_lender,
        collateral_agent: request.collateral_agent,
        guarantors_present: request.guarantors_present,
        guarantor_names: request.guarantor_names,
        facility_type: request.facility_type,
        committed_amount: request.committed_amount,
        currency: request.currency,
        draw_rules: request.draw_rules,
        repayment_type: request.repayment_type,
        amortization_schedule: request.amortization_schedule,
        reference_rate: request.reference_rate,
        margin_bps: request.margin_bps,
        floor_bps: request.floor_bps,
        secured_flag: request.secured_flag,
        collateral_scope_summary: request.collateral_scope_summary,
        financial_covenants: request.financial_covenants,
        use_of_proceeds: request.use_of_proceeds,
        accordion_provision: request.accordion_provision,
        prepayment_terms: request.prepayment_terms,
        default_provisions: request.default_provisions,
        deal_state: request.deal_state,
        metadata: request.metadata,
        created_by,
    };

    let record = create_credit_agreement(&state.pool, db_request)
        .await
        .map_err(|e| {
            error!("❌ Failed to create credit agreement: {}", e);
            metrics::counter!("taxonomy_errors_total", "table" => "credit_agreements", "operation" => "create").increment(1);
            internal_error("Create credit agreement", e)
        })?;

    let response = CreditAgreementResponse {
        id: record.id.to_string(),
        document_id: record.document_id,
        version: record.version,
        execution_status: record.execution_status,
        executed_pdf_hash: record.executed_pdf_hash,
        bundle_hash: record.bundle_hash,
        effective_date: record.effective_date,
        maturity_date: record.maturity_date,
        closing_date: record.closing_date,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
        governing_law: record.governing_law,
        borrower_legal_name: record.borrower_legal_name,
        borrower_external_id: record.borrower_external_id,
        agent_or_lender: record.agent_or_lender,
        collateral_agent: record.collateral_agent,
        guarantors_present: record.guarantors_present,
        guarantor_names: record.guarantor_names,
        facility_type: record.facility_type,
        committed_amount: record.committed_amount,
        currency: record.currency,
        draw_rules: record.draw_rules,
        repayment_type: record.repayment_type,
        amortization_schedule: record.amortization_schedule,
        reference_rate: record.reference_rate,
        margin_bps: record.margin_bps,
        floor_bps: record.floor_bps,
        secured_flag: record.secured_flag,
        collateral_scope_summary: record.collateral_scope_summary,
        financial_covenants: record.financial_covenants,
        use_of_proceeds: record.use_of_proceeds,
        accordion_provision: record.accordion_provision,
        prepayment_terms: record.prepayment_terms,
        default_provisions: record.default_provisions,
        deal_state: record.deal_state,
        metadata: record.metadata,
    };

    info!(
        "✅ API: Created credit agreement {} ({})",
        response.id, response.document_id
    );
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "create")
        .record(elapsed.as_secs_f64());

    Ok((StatusCode::CREATED, Json(response)))
}

/// Create multiple credit agreements in a batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/credit-agreements/batch",
    request_body = Vec<CreateCreditAgreementApiRequest>,
    responses(
        (status = 200, description = "Batch operation completed", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state, headers, requests), level = "info")]
pub async fn create_credit_agreements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(requests): Json<Vec<CreateCreditAgreementApiRequest>>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();
    let total = requests.len();

    info!("🔍 API: Creating batch of {} credit agreements", total);
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "batch_create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let db_requests: Vec<DbCreateRequest> = requests
        .into_iter()
        .map(|req| DbCreateRequest {
            document_id: req.document_id,
            version: req.version,
            execution_status: req.execution_status,
            executed_pdf_hash: req.executed_pdf_hash,
            bundle_hash: req.bundle_hash,
            effective_date: req.effective_date,
            maturity_date: req.maturity_date,
            closing_date: req.closing_date,
            governing_law: req.governing_law,
            borrower_legal_name: req.borrower_legal_name,
            borrower_external_id: req.borrower_external_id,
            agent_or_lender: req.agent_or_lender,
            collateral_agent: req.collateral_agent,
            guarantors_present: req.guarantors_present,
            guarantor_names: req.guarantor_names,
            facility_type: req.facility_type,
            committed_amount: req.committed_amount,
            currency: req.currency,
            draw_rules: req.draw_rules,
            repayment_type: req.repayment_type,
            amortization_schedule: req.amortization_schedule,
            reference_rate: req.reference_rate,
            margin_bps: req.margin_bps,
            floor_bps: req.floor_bps,
            secured_flag: req.secured_flag,
            collateral_scope_summary: req.collateral_scope_summary,
            financial_covenants: req.financial_covenants,
            use_of_proceeds: req.use_of_proceeds,
            accordion_provision: req.accordion_provision,
            prepayment_terms: req.prepayment_terms,
            default_provisions: req.default_provisions,
            deal_state: req.deal_state,
            metadata: req.metadata,
            created_by,
        })
        .collect();

    let results = create_credit_agreements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create credit agreements", e))?;

    let mut successful_ids = Vec::new();
    let mut errors = Vec::new();

    for (index, result) in results.into_iter().enumerate() {
        match result {
            Ok(record) => {
                successful_ids.push(record.id.to_string());
            }
            Err(e) => {
                errors.push(BatchError {
                    index,
                    error_code: "create_failed".to_string(),
                    error_message: e.to_string(),
                });
            }
        }
    }

    let success_count = successful_ids.len();
    let error_count = errors.len();

    let response = BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    };

    info!(
        "✅ API: Batch created {} credit agreements ({} succeeded, {} failed)",
        total, success_count, error_count
    );
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "batch_create")
        .record(elapsed.as_secs_f64());

    Ok(Json(response))
}

/// Update credit agreement (full replacement)
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/credit-agreements/{id}",
    params(
        ("id" = String, Path, description = "Credit agreement UUID")
    ),
    request_body = UpdateCreditAgreementApiRequest,
    responses(
        (status = 200, description = "Credit agreement updated successfully", body = CreditAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Credit agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state, headers), level = "info")]
pub async fn update_credit_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCreditAgreementApiRequest>,
) -> Result<Json<CreditAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating credit agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        version: request.version,
        execution_status: request.execution_status,
        executed_pdf_hash: request.executed_pdf_hash,
        bundle_hash: request.bundle_hash,
        effective_date: request.effective_date,
        maturity_date: request.maturity_date,
        closing_date: request.closing_date,
        governing_law: request.governing_law,
        borrower_legal_name: request.borrower_legal_name,
        borrower_external_id: request.borrower_external_id,
        agent_or_lender: request.agent_or_lender,
        collateral_agent: request.collateral_agent,
        guarantors_present: request.guarantors_present,
        guarantor_names: request.guarantor_names,
        facility_type: request.facility_type,
        committed_amount: request.committed_amount,
        currency: request.currency,
        draw_rules: request.draw_rules,
        repayment_type: request.repayment_type,
        amortization_schedule: request.amortization_schedule,
        reference_rate: request.reference_rate,
        margin_bps: request.margin_bps,
        floor_bps: request.floor_bps,
        secured_flag: request.secured_flag,
        collateral_scope_summary: request.collateral_scope_summary,
        financial_covenants: request.financial_covenants,
        use_of_proceeds: request.use_of_proceeds,
        accordion_provision: request.accordion_provision,
        prepayment_terms: request.prepayment_terms,
        default_provisions: request.default_provisions,
        deal_state: request.deal_state,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_credit_agreement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| {
            error!("❌ Failed to update credit agreement: {}", e);
            metrics::counter!("taxonomy_errors_total", "table" => "credit_agreements", "operation" => "update").increment(1);
            internal_error("Update credit agreement", e)
        })?;

    let response = CreditAgreementResponse {
        id: record.id.to_string(),
        document_id: record.document_id,
        version: record.version,
        execution_status: record.execution_status,
        executed_pdf_hash: record.executed_pdf_hash,
        bundle_hash: record.bundle_hash,
        effective_date: record.effective_date,
        maturity_date: record.maturity_date,
        closing_date: record.closing_date,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
        governing_law: record.governing_law,
        borrower_legal_name: record.borrower_legal_name,
        borrower_external_id: record.borrower_external_id,
        agent_or_lender: record.agent_or_lender,
        collateral_agent: record.collateral_agent,
        guarantors_present: record.guarantors_present,
        guarantor_names: record.guarantor_names,
        facility_type: record.facility_type,
        committed_amount: record.committed_amount,
        currency: record.currency,
        draw_rules: record.draw_rules,
        repayment_type: record.repayment_type,
        amortization_schedule: record.amortization_schedule,
        reference_rate: record.reference_rate,
        margin_bps: record.margin_bps,
        floor_bps: record.floor_bps,
        secured_flag: record.secured_flag,
        collateral_scope_summary: record.collateral_scope_summary,
        financial_covenants: record.financial_covenants,
        use_of_proceeds: record.use_of_proceeds,
        accordion_provision: record.accordion_provision,
        prepayment_terms: record.prepayment_terms,
        default_provisions: record.default_provisions,
        deal_state: record.deal_state,
        metadata: record.metadata,
    };

    info!("✅ API: Updated credit agreement {}", id);
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "update")
        .record(elapsed.as_secs_f64());

    Ok(Json(response))
}

/// Delete credit agreement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/credit-agreements/{id}",
    params(
        ("id" = String, Path, description = "Credit agreement UUID")
    ),
    responses(
        (status = 200, description = "Credit agreement deleted successfully", body = DeleteResponse),
        (status = 400, description = "Invalid UUID format", body = ErrorResponse),
        (status = 404, description = "Credit agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Credit Agreements",
    security(("session_token" = []))
)]
#[instrument(skip(state), level = "info")]
pub async fn delete_credit_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting credit agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "credit_agreements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_credit_agreement(&state.pool, uuid)
        .await
        .map_err(|e| {
            error!("❌ Failed to delete credit agreement: {}", e);
            metrics::counter!("taxonomy_errors_total", "table" => "credit_agreements", "operation" => "delete").increment(1);
            internal_error("Delete credit agreement", e)
        })?;

    if !deleted {
        return Err(not_found_error("Credit agreement", &id));
    }

    let response = DeleteResponse {
        success: true,
        message: "Credit agreement deleted successfully".to_string(),
        id,
    };

    info!("✅ API: Deleted credit agreement {}", response.id);
    let elapsed = start_time.elapsed();
    metrics::histogram!("taxonomy_duration_seconds", "table" => "credit_agreements", "operation" => "delete")
        .record(elapsed.as_secs_f64());

    Ok(Json(response))
}
