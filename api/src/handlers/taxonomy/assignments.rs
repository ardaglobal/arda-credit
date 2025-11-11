//! Assignment & Assumption Agreements API Handlers
//!
//! Provides REST API endpoints for assignment & assumption agreement CRUD operations.

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
    create_assignment_assumption_agreement, create_assignment_assumption_agreements_batch,
    delete_assignment_assumption_agreement, get_assignment_assumption_agreement_by_id,
    list_assignment_assumption_agreements, update_assignment_assumption_agreement,
    CreateAssignmentAssumptionAgreementRequest as DbCreateRequest,
    UpdateAssignmentAssumptionAgreementRequest as DbUpdateRequest,
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

/// Assignment & Assumption Agreement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "A&A-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "assignment_type": "partial_assignment",
    "assignor": "Bank of America",
    "assignee": "JPMorgan Chase",
    "purchase_price": "47500000.00",
    "trade_date": "2025-01-15",
    "agreement_status": "executed"
}))]
pub struct AssignmentAssumptionAgreementResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: String,
    pub assignment_type: String,
    pub assignor: String,
    pub assignor_entity_id: Option<String>,
    pub assignee: String,
    pub assignee_entity_id: Option<String>,
    pub administrative_agent: Option<String>,
    pub borrower_name: String,
    pub facility_assigned: Option<String>,
    pub assigned_commitment_amount: Option<String>,
    pub assigned_outstanding_principal: Option<String>,
    pub assigned_percentage: Option<String>,
    pub pro_rata_share_before: Option<String>,
    pub pro_rata_share_after_assignor: Option<String>,
    pub pro_rata_share_assignee: Option<String>,
    pub purchase_price: Option<String>,
    pub price_per_unit: Option<String>,
    pub accrued_interest: Option<String>,
    pub fees_assigned: Option<String>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    pub minimum_assignment_amount: Option<String>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    pub processing_fee: Option<String>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create assignment & assumption agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateAssignmentAssumptionAgreementApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: String,
    pub assignment_type: String,
    pub assignor: String,
    pub assignor_entity_id: Option<String>,
    pub assignee: String,
    pub assignee_entity_id: Option<String>,
    pub administrative_agent: Option<String>,
    pub borrower_name: String,
    pub facility_assigned: Option<String>,
    #[schema(value_type = Option<String>)]
    pub assigned_commitment_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub assigned_outstanding_principal: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub assigned_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_before: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_after_assignor: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_assignee: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub price_per_unit: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub accrued_interest: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub fees_assigned: Option<Decimal>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub minimum_assignment_amount: Option<Decimal>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    #[schema(value_type = Option<String>)]
    pub processing_fee: Option<Decimal>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update assignment & assumption agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAssignmentAssumptionAgreementApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: Option<String>,
    pub assignment_type: Option<String>,
    pub assignor: Option<String>,
    pub assignor_entity_id: Option<String>,
    pub assignee: Option<String>,
    pub assignee_entity_id: Option<String>,
    pub administrative_agent: Option<String>,
    pub borrower_name: Option<String>,
    pub facility_assigned: Option<String>,
    #[schema(value_type = Option<String>)]
    pub assigned_commitment_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub assigned_outstanding_principal: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub assigned_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_before: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_after_assignor: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub pro_rata_share_assignee: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub price_per_unit: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub accrued_interest: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub fees_assigned: Option<Decimal>,
    pub trade_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub minimum_assignment_amount: Option<Decimal>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    #[schema(value_type = Option<String>)]
    pub processing_fee: Option<Decimal>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing assignment & assumption agreements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AssignmentAssumptionAgreementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by assignment type
    #[param(example = "partial_assignment")]
    pub assignment_type: Option<String>,

    /// Filter by agreement status
    #[param(example = "executed")]
    pub agreement_status: Option<String>,

    /// Filter by trade date
    #[param(example = "2025-01-15")]
    pub trade_date: Option<NaiveDate>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateAssignmentAssumptionAgreementsRequest {
    pub agreements: Vec<CreateAssignmentAssumptionAgreementApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(
    db: arda_credit_db::AssignmentAssumptionAgreement,
) -> AssignmentAssumptionAgreementResponse {
    AssignmentAssumptionAgreementResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        trade_id: db.trade_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        assignment_type: db.assignment_type,
        assignor: db.assignor,
        assignor_entity_id: db.assignor_entity_id.map(|id| id.to_string()),
        assignee: db.assignee,
        assignee_entity_id: db.assignee_entity_id.map(|id| id.to_string()),
        administrative_agent: db.administrative_agent,
        borrower_name: db.borrower_name,
        facility_assigned: db.facility_assigned,
        assigned_commitment_amount: db.assigned_commitment_amount.map(|d| d.to_string()),
        assigned_outstanding_principal: db.assigned_outstanding_principal.map(|d| d.to_string()),
        assigned_percentage: db.assigned_percentage.map(|d| d.to_string()),
        pro_rata_share_before: db.pro_rata_share_before.map(|d| d.to_string()),
        pro_rata_share_after_assignor: db.pro_rata_share_after_assignor.map(|d| d.to_string()),
        pro_rata_share_assignee: db.pro_rata_share_assignee.map(|d| d.to_string()),
        purchase_price: db.purchase_price.map(|d| d.to_string()),
        price_per_unit: db.price_per_unit.map(|d| d.to_string()),
        accrued_interest: db.accrued_interest.map(|d| d.to_string()),
        fees_assigned: db.fees_assigned.map(|d| d.to_string()),
        trade_date: db.trade_date,
        settlement_date: db.settlement_date,
        effective_date: db.effective_date,
        borrower_consent_required: db.borrower_consent_required,
        borrower_consent_obtained: db.borrower_consent_obtained,
        borrower_consent_date: db.borrower_consent_date,
        agent_consent_required: db.agent_consent_required,
        agent_consent_obtained: db.agent_consent_obtained,
        agent_consent_date: db.agent_consent_date,
        other_consents_required: db.other_consents_required,
        all_consents_obtained: db.all_consents_obtained,
        minimum_assignment_amount: db.minimum_assignment_amount.map(|d| d.to_string()),
        meets_minimum_threshold: db.meets_minimum_threshold,
        eligible_assignee: db.eligible_assignee,
        eligibility_criteria_met: db.eligibility_criteria_met,
        disqualified_lender_check: db.disqualified_lender_check,
        not_on_dql: db.not_on_dql,
        assignor_representations: db.assignor_representations,
        assignee_representations: db.assignee_representations,
        processing_fee: db.processing_fee.map(|d| d.to_string()),
        processing_fee_paid_by: db.processing_fee_paid_by,
        recorded_in_register: db.recorded_in_register,
        register_update_date: db.register_update_date,
        register_notation: db.register_notation,
        assignee_kyc_completed: db.assignee_kyc_completed,
        assignee_kyc_date: db.assignee_kyc_date,
        assignee_tax_forms_received: db.assignee_tax_forms_received,
        is_assignment: db.is_assignment,
        changes_lender_of_record: db.changes_lender_of_record,
        joinder_agreement_required: db.joinder_agreement_required,
        joinder_executed: db.joinder_executed,
        agreement_status: db.agreement_status,
        cash_settled: db.cash_settled,
        asset_transferred: db.asset_transferred,
        settlement_confirmation: db.settlement_confirmation,
        notes: db.notes,
        special_terms: db.special_terms,
        metadata: db.metadata,
        created_at: db.created_at.map(|dt| dt.to_rfc3339()),
        updated_at: db.updated_at.map(|dt| dt.to_rfc3339()),
        created_by: db.created_by.map(|id| id.to_string()),
        updated_by: db.updated_by.map(|id| id.to_string()),
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// List assignment & assumption agreements
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/assignment-assumption-agreements",
    params(AssignmentAssumptionAgreementListParams),
    responses(
        (status = 200, description = "List of assignment & assumption agreements", body = PaginatedResponse<AssignmentAssumptionAgreementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn list_assignment_assumption_agreements_handler(
    State(state): State<ApiState>,
    Query(params): Query<AssignmentAssumptionAgreementListParams>,
) -> Result<
    Json<PaginatedResponse<AssignmentAssumptionAgreementResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing assignment & assumption agreements");
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_assignment_assumption_agreements(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.assignment_type.clone(),
        params.agreement_status.clone(),
        params.trade_date,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List assignment & assumption agreements", e))?;

    let data: Vec<AssignmentAssumptionAgreementResponse> =
        records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get assignment & assumption agreement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/assignment-assumption-agreements/{id}",
    params(
        ("id" = String, Path, description = "Assignment & Assumption Agreement UUID")
    ),
    responses(
        (status = 200, description = "Assignment & assumption agreement details", body = AssignmentAssumptionAgreementResponse),
        (status = 404, description = "Assignment & assumption agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn get_assignment_assumption_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<AssignmentAssumptionAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting assignment & assumption agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_assignment_assumption_agreement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get assignment & assumption agreement", e))?
        .ok_or_else(|| not_found_error("Assignment & assumption agreement", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create assignment & assumption agreement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/assignment-assumption-agreements",
    request_body = CreateAssignmentAssumptionAgreementApiRequest,
    responses(
        (status = 201, description = "Assignment & assumption agreement created", body = AssignmentAssumptionAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn create_assignment_assumption_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateAssignmentAssumptionAgreementApiRequest>,
) -> Result<
    (StatusCode, Json<AssignmentAssumptionAgreementResponse>),
    (StatusCode, Json<ErrorResponse>),
> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Creating assignment & assumption agreement {}",
        request.document_id
    );
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let trade_id = if let Some(id_str) = request.trade_id {
        Some(parse_uuid(&id_str, "trade_id")?)
    } else {
        None
    };

    let assignor_entity_id = if let Some(id_str) = request.assignor_entity_id {
        Some(parse_uuid(&id_str, "assignor_entity_id")?)
    } else {
        None
    };

    let assignee_entity_id = if let Some(id_str) = request.assignee_entity_id {
        Some(parse_uuid(&id_str, "assignee_entity_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        trade_id,
        doc_hash: request.doc_hash,
        assignment_type: request.assignment_type,
        assignor: request.assignor,
        assignor_entity_id,
        assignee: request.assignee,
        assignee_entity_id,
        administrative_agent: request.administrative_agent,
        borrower_name: request.borrower_name,
        facility_assigned: request.facility_assigned,
        assigned_commitment_amount: request.assigned_commitment_amount,
        assigned_outstanding_principal: request.assigned_outstanding_principal,
        assigned_percentage: request.assigned_percentage,
        pro_rata_share_before: request.pro_rata_share_before,
        pro_rata_share_after_assignor: request.pro_rata_share_after_assignor,
        pro_rata_share_assignee: request.pro_rata_share_assignee,
        purchase_price: request.purchase_price,
        price_per_unit: request.price_per_unit,
        accrued_interest: request.accrued_interest,
        fees_assigned: request.fees_assigned,
        trade_date: request.trade_date,
        settlement_date: request.settlement_date,
        effective_date: request.effective_date,
        borrower_consent_required: request.borrower_consent_required,
        borrower_consent_obtained: request.borrower_consent_obtained,
        borrower_consent_date: request.borrower_consent_date,
        agent_consent_required: request.agent_consent_required,
        agent_consent_obtained: request.agent_consent_obtained,
        agent_consent_date: request.agent_consent_date,
        other_consents_required: request.other_consents_required,
        all_consents_obtained: request.all_consents_obtained,
        minimum_assignment_amount: request.minimum_assignment_amount,
        meets_minimum_threshold: request.meets_minimum_threshold,
        eligible_assignee: request.eligible_assignee,
        eligibility_criteria_met: request.eligibility_criteria_met,
        disqualified_lender_check: request.disqualified_lender_check,
        not_on_dql: request.not_on_dql,
        assignor_representations: request.assignor_representations,
        assignee_representations: request.assignee_representations,
        processing_fee: request.processing_fee,
        processing_fee_paid_by: request.processing_fee_paid_by,
        recorded_in_register: request.recorded_in_register,
        register_update_date: request.register_update_date,
        register_notation: request.register_notation,
        assignee_kyc_completed: request.assignee_kyc_completed,
        assignee_kyc_date: request.assignee_kyc_date,
        assignee_tax_forms_received: request.assignee_tax_forms_received,
        is_assignment: request.is_assignment,
        changes_lender_of_record: request.changes_lender_of_record,
        joinder_agreement_required: request.joinder_agreement_required,
        joinder_executed: request.joinder_executed,
        agreement_status: request.agreement_status,
        cash_settled: request.cash_settled,
        asset_transferred: request.asset_transferred,
        settlement_confirmation: request.settlement_confirmation,
        notes: request.notes,
        special_terms: request.special_terms,
        metadata: request.metadata,
        created_by,
    };

    let record = create_assignment_assumption_agreement(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create assignment & assumption agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create assignment & assumption agreements in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/assignment-assumption-agreements/batch",
    request_body = BatchCreateAssignmentAssumptionAgreementsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn create_assignment_assumption_agreements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateAssignmentAssumptionAgreementsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} assignment & assumption agreements",
        request.agreements.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.agreements {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let trade_id = if let Some(id_str) = req.trade_id {
            match parse_uuid(&id_str, "trade_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let assignor_entity_id = if let Some(id_str) = req.assignor_entity_id {
            match parse_uuid(&id_str, "assignor_entity_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let assignee_entity_id = if let Some(id_str) = req.assignee_entity_id {
            match parse_uuid(&id_str, "assignee_entity_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            document_id: req.document_id,
            credit_agreement_id,
            trade_id,
            doc_hash: req.doc_hash,
            assignment_type: req.assignment_type,
            assignor: req.assignor,
            assignor_entity_id,
            assignee: req.assignee,
            assignee_entity_id,
            administrative_agent: req.administrative_agent,
            borrower_name: req.borrower_name,
            facility_assigned: req.facility_assigned,
            assigned_commitment_amount: req.assigned_commitment_amount,
            assigned_outstanding_principal: req.assigned_outstanding_principal,
            assigned_percentage: req.assigned_percentage,
            pro_rata_share_before: req.pro_rata_share_before,
            pro_rata_share_after_assignor: req.pro_rata_share_after_assignor,
            pro_rata_share_assignee: req.pro_rata_share_assignee,
            purchase_price: req.purchase_price,
            price_per_unit: req.price_per_unit,
            accrued_interest: req.accrued_interest,
            fees_assigned: req.fees_assigned,
            trade_date: req.trade_date,
            settlement_date: req.settlement_date,
            effective_date: req.effective_date,
            borrower_consent_required: req.borrower_consent_required,
            borrower_consent_obtained: req.borrower_consent_obtained,
            borrower_consent_date: req.borrower_consent_date,
            agent_consent_required: req.agent_consent_required,
            agent_consent_obtained: req.agent_consent_obtained,
            agent_consent_date: req.agent_consent_date,
            other_consents_required: req.other_consents_required,
            all_consents_obtained: req.all_consents_obtained,
            minimum_assignment_amount: req.minimum_assignment_amount,
            meets_minimum_threshold: req.meets_minimum_threshold,
            eligible_assignee: req.eligible_assignee,
            eligibility_criteria_met: req.eligibility_criteria_met,
            disqualified_lender_check: req.disqualified_lender_check,
            not_on_dql: req.not_on_dql,
            assignor_representations: req.assignor_representations,
            assignee_representations: req.assignee_representations,
            processing_fee: req.processing_fee,
            processing_fee_paid_by: req.processing_fee_paid_by,
            recorded_in_register: req.recorded_in_register,
            register_update_date: req.register_update_date,
            register_notation: req.register_notation,
            assignee_kyc_completed: req.assignee_kyc_completed,
            assignee_kyc_date: req.assignee_kyc_date,
            assignee_tax_forms_received: req.assignee_tax_forms_received,
            is_assignment: req.is_assignment,
            changes_lender_of_record: req.changes_lender_of_record,
            joinder_agreement_required: req.joinder_agreement_required,
            joinder_executed: req.joinder_executed,
            agreement_status: req.agreement_status,
            cash_settled: req.cash_settled,
            asset_transferred: req.asset_transferred,
            settlement_confirmation: req.settlement_confirmation,
            notes: req.notes,
            special_terms: req.special_terms,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_assignment_assumption_agreements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create assignment & assumption agreements", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update assignment & assumption agreement
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/assignment-assumption-agreements/{id}",
    params(
        ("id" = String, Path, description = "Assignment & Assumption Agreement UUID")
    ),
    request_body = UpdateAssignmentAssumptionAgreementApiRequest,
    responses(
        (status = 200, description = "Assignment & assumption agreement updated", body = AssignmentAssumptionAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Assignment & assumption agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn update_assignment_assumption_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateAssignmentAssumptionAgreementApiRequest>,
) -> Result<Json<AssignmentAssumptionAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating assignment & assumption agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let trade_id = if let Some(id_str) = request.trade_id {
        Some(parse_uuid(&id_str, "trade_id")?)
    } else {
        None
    };

    let assignor_entity_id = if let Some(id_str) = request.assignor_entity_id {
        Some(parse_uuid(&id_str, "assignor_entity_id")?)
    } else {
        None
    };

    let assignee_entity_id = if let Some(id_str) = request.assignee_entity_id {
        Some(parse_uuid(&id_str, "assignee_entity_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        trade_id,
        doc_hash: request.doc_hash,
        assignment_type: request.assignment_type,
        assignor: request.assignor,
        assignor_entity_id,
        assignee: request.assignee,
        assignee_entity_id,
        administrative_agent: request.administrative_agent,
        borrower_name: request.borrower_name,
        facility_assigned: request.facility_assigned,
        assigned_commitment_amount: request.assigned_commitment_amount,
        assigned_outstanding_principal: request.assigned_outstanding_principal,
        assigned_percentage: request.assigned_percentage,
        pro_rata_share_before: request.pro_rata_share_before,
        pro_rata_share_after_assignor: request.pro_rata_share_after_assignor,
        pro_rata_share_assignee: request.pro_rata_share_assignee,
        purchase_price: request.purchase_price,
        price_per_unit: request.price_per_unit,
        accrued_interest: request.accrued_interest,
        fees_assigned: request.fees_assigned,
        trade_date: request.trade_date,
        settlement_date: request.settlement_date,
        effective_date: request.effective_date,
        borrower_consent_required: request.borrower_consent_required,
        borrower_consent_obtained: request.borrower_consent_obtained,
        borrower_consent_date: request.borrower_consent_date,
        agent_consent_required: request.agent_consent_required,
        agent_consent_obtained: request.agent_consent_obtained,
        agent_consent_date: request.agent_consent_date,
        other_consents_required: request.other_consents_required,
        all_consents_obtained: request.all_consents_obtained,
        minimum_assignment_amount: request.minimum_assignment_amount,
        meets_minimum_threshold: request.meets_minimum_threshold,
        eligible_assignee: request.eligible_assignee,
        eligibility_criteria_met: request.eligibility_criteria_met,
        disqualified_lender_check: request.disqualified_lender_check,
        not_on_dql: request.not_on_dql,
        assignor_representations: request.assignor_representations,
        assignee_representations: request.assignee_representations,
        processing_fee: request.processing_fee,
        processing_fee_paid_by: request.processing_fee_paid_by,
        recorded_in_register: request.recorded_in_register,
        register_update_date: request.register_update_date,
        register_notation: request.register_notation,
        assignee_kyc_completed: request.assignee_kyc_completed,
        assignee_kyc_date: request.assignee_kyc_date,
        assignee_tax_forms_received: request.assignee_tax_forms_received,
        is_assignment: request.is_assignment,
        changes_lender_of_record: request.changes_lender_of_record,
        joinder_agreement_required: request.joinder_agreement_required,
        joinder_executed: request.joinder_executed,
        agreement_status: request.agreement_status,
        cash_settled: request.cash_settled,
        asset_transferred: request.asset_transferred,
        settlement_confirmation: request.settlement_confirmation,
        notes: request.notes,
        special_terms: request.special_terms,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_assignment_assumption_agreement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update assignment & assumption agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete assignment & assumption agreement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/assignment-assumption-agreements/{id}",
    params(
        ("id" = String, Path, description = "Assignment & Assumption Agreement UUID")
    ),
    responses(
        (status = 200, description = "Assignment & assumption agreement deleted", body = DeleteResponse),
        (status = 404, description = "Assignment & assumption agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Assignment & Assumption Agreements"
)]
#[instrument(skip(state))]
pub async fn delete_assignment_assumption_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting assignment & assumption agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "assignment_assumption_agreements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_assignment_assumption_agreement(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete assignment & assumption agreement", e))?;

    if !deleted {
        return Err(not_found_error("Assignment & assumption agreement", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "assignment_assumption_agreements", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!(
            "Assignment & assumption agreement {} deleted successfully",
            id
        ),
    }))
}
