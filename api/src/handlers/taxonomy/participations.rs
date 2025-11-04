//! Participation Agreements API Handlers
//!
//! Provides REST API endpoints for participation agreement CRUD operations.

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
    create_participation_agreement, create_participation_agreements_batch,
    delete_participation_agreement, get_participation_agreement_by_id,
    list_participation_agreements, update_participation_agreement,
    CreateParticipationAgreementRequest as DbCreateRequest,
    UpdateParticipationAgreementRequest as DbUpdateRequest,
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

/// Participation Agreement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "PART-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "participation_type": "funded_participation",
    "grantor": "Original Lender LLC",
    "participant": "Participant Bank",
    "participation_amount": "1000000.00",
    "agreement_status": "active"
}))]
pub struct ParticipationAgreementResponse {
    pub id: String,
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: String,
    pub participation_type: String,
    pub grantor: String,
    pub grantor_entity_id: Option<String>,
    pub grantor_remains_lender_of_record: Option<bool>,
    pub participant: String,
    pub participant_entity_id: Option<String>,
    pub borrower_name: String,
    pub facility_reference: Option<String>,
    pub participation_percentage: Option<String>,
    pub participation_amount: Option<String>,
    pub grantor_total_commitment: Option<String>,
    pub grantor_outstanding_principal: Option<String>,
    pub purchase_price: Option<String>,
    pub purchase_price_percentage: Option<String>,
    pub funded_at_closing: Option<bool>,
    pub funding_mechanism: Option<String>,
    pub payments_flow: String,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>,
    pub no_privity_with_borrower: Option<bool>,
    pub participant_has_voting_rights: Option<bool>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_knowledge_of_participation: Option<bool>,
    pub agent_notification_required: Option<bool>,
    pub agent_notified: Option<bool>,
    pub pro_rata_sharing: Option<bool>,
    pub principal_sharing_percentage: Option<String>,
    pub interest_sharing_percentage: Option<String>,
    pub fee_sharing_percentage: Option<String>,
    pub grantor_pays_participant: Option<bool>,
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>,
    pub participant_funding_risk: Option<bool>,
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>,
    pub participant_representations: Option<serde_json::Value>,
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>,
    pub is_sub_participation: Option<bool>,
    pub parent_participation_id: Option<String>,
    pub confidential_agreement: Option<bool>,
    pub disclosure_restrictions: Option<String>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>,
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub participation_active: Option<bool>,
    pub total_payments_received: Option<String>,
    pub total_payments_to_participant: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create participation agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateParticipationAgreementApiRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: String,
    pub participation_type: String,
    pub grantor: String,
    pub grantor_entity_id: Option<String>,
    pub grantor_remains_lender_of_record: Option<bool>,
    pub participant: String,
    pub participant_entity_id: Option<String>,
    pub borrower_name: String,
    pub facility_reference: Option<String>,
    #[schema(value_type = Option<String>)]
    pub participation_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub participation_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub grantor_total_commitment: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub grantor_outstanding_principal: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price_percentage: Option<Decimal>,
    pub funded_at_closing: Option<bool>,
    pub funding_mechanism: Option<String>,
    pub payments_flow: String,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>,
    pub no_privity_with_borrower: Option<bool>,
    pub participant_has_voting_rights: Option<bool>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_knowledge_of_participation: Option<bool>,
    pub agent_notification_required: Option<bool>,
    pub agent_notified: Option<bool>,
    pub pro_rata_sharing: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub principal_sharing_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub interest_sharing_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub fee_sharing_percentage: Option<Decimal>,
    pub grantor_pays_participant: Option<bool>,
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>,
    pub participant_funding_risk: Option<bool>,
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>,
    pub participant_representations: Option<serde_json::Value>,
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>,
    pub is_sub_participation: Option<bool>,
    pub parent_participation_id: Option<String>,
    pub confidential_agreement: Option<bool>,
    pub disclosure_restrictions: Option<String>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>,
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub participation_active: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub total_payments_received: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub total_payments_to_participant: Option<Decimal>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update participation agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateParticipationAgreementApiRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub trade_id: Option<String>,
    pub doc_hash: Option<String>,
    pub participation_type: Option<String>,
    pub grantor: Option<String>,
    pub grantor_entity_id: Option<String>,
    pub grantor_remains_lender_of_record: Option<bool>,
    pub participant: Option<String>,
    pub participant_entity_id: Option<String>,
    pub borrower_name: Option<String>,
    pub facility_reference: Option<String>,
    #[schema(value_type = Option<String>)]
    pub participation_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub participation_amount: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub grantor_total_commitment: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub grantor_outstanding_principal: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub purchase_price_percentage: Option<Decimal>,
    pub funded_at_closing: Option<bool>,
    pub funding_mechanism: Option<String>,
    pub payments_flow: Option<String>,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>,
    pub no_privity_with_borrower: Option<bool>,
    pub participant_has_voting_rights: Option<bool>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_knowledge_of_participation: Option<bool>,
    pub agent_notification_required: Option<bool>,
    pub agent_notified: Option<bool>,
    pub pro_rata_sharing: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub principal_sharing_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub interest_sharing_percentage: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub fee_sharing_percentage: Option<Decimal>,
    pub grantor_pays_participant: Option<bool>,
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>,
    pub participant_funding_risk: Option<bool>,
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>,
    pub participant_representations: Option<serde_json::Value>,
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>,
    pub is_sub_participation: Option<bool>,
    pub parent_participation_id: Option<String>,
    pub confidential_agreement: Option<bool>,
    pub disclosure_restrictions: Option<String>,
    pub trade_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>,
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub participation_active: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub total_payments_received: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub total_payments_to_participant: Option<Decimal>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing participation agreements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ParticipationAgreementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by participation type
    #[param(example = "funded_participation")]
    pub participation_type: Option<String>,

    /// Filter by agreement status
    #[param(example = "active")]
    pub agreement_status: Option<String>,

    /// Filter by participation active flag
    #[param(example = "true")]
    pub participation_active: Option<bool>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateParticipationAgreementsRequest {
    pub participation_agreements: Vec<CreateParticipationAgreementApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::ParticipationAgreement) -> ParticipationAgreementResponse {
    ParticipationAgreementResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        trade_id: db.trade_id.map(|id| id.to_string()),
        doc_hash: db.doc_hash,
        participation_type: db.participation_type,
        grantor: db.grantor,
        grantor_entity_id: db.grantor_entity_id.map(|id| id.to_string()),
        grantor_remains_lender_of_record: db.grantor_remains_lender_of_record,
        participant: db.participant,
        participant_entity_id: db.participant_entity_id.map(|id| id.to_string()),
        borrower_name: db.borrower_name,
        facility_reference: db.facility_reference,
        participation_percentage: db.participation_percentage.map(|d| d.to_string()),
        participation_amount: db.participation_amount.map(|d| d.to_string()),
        grantor_total_commitment: db.grantor_total_commitment.map(|d| d.to_string()),
        grantor_outstanding_principal: db.grantor_outstanding_principal.map(|d| d.to_string()),
        purchase_price: db.purchase_price.map(|d| d.to_string()),
        purchase_price_percentage: db.purchase_price_percentage.map(|d| d.to_string()),
        funded_at_closing: db.funded_at_closing,
        funding_mechanism: db.funding_mechanism,
        payments_flow: db.payments_flow,
        payment_terms: db.payment_terms,
        participant_rights: db.participant_rights,
        no_privity_with_borrower: db.no_privity_with_borrower,
        participant_has_voting_rights: db.participant_has_voting_rights,
        borrower_consent_required: db.borrower_consent_required,
        borrower_consent_obtained: db.borrower_consent_obtained,
        borrower_knowledge_of_participation: db.borrower_knowledge_of_participation,
        agent_notification_required: db.agent_notification_required,
        agent_notified: db.agent_notified,
        pro_rata_sharing: db.pro_rata_sharing,
        principal_sharing_percentage: db.principal_sharing_percentage.map(|d| d.to_string()),
        interest_sharing_percentage: db.interest_sharing_percentage.map(|d| d.to_string()),
        fee_sharing_percentage: db.fee_sharing_percentage.map(|d| d.to_string()),
        grantor_pays_participant: db.grantor_pays_participant,
        settlement_frequency: db.settlement_frequency,
        participant_credit_risk: db.participant_credit_risk,
        participant_funding_risk: db.participant_funding_risk,
        setoff_rights: db.setoff_rights,
        netting_provisions: db.netting_provisions,
        grantor_representations: db.grantor_representations,
        participant_representations: db.participant_representations,
        participant_assignment_rights: db.participant_assignment_rights,
        requires_grantor_consent_to_assign: db.requires_grantor_consent_to_assign,
        is_sub_participation: db.is_sub_participation,
        parent_participation_id: db.parent_participation_id.map(|id| id.to_string()),
        confidential_agreement: db.confidential_agreement,
        disclosure_restrictions: db.disclosure_restrictions,
        trade_date: db.trade_date,
        settlement_date: db.settlement_date,
        effective_date: db.effective_date,
        termination_date: db.termination_date,
        termination_conditions: db.termination_conditions,
        participant_kyc_completed: db.participant_kyc_completed,
        participant_kyc_date: db.participant_kyc_date,
        participant_eligible: db.participant_eligible,
        agreement_status: db.agreement_status,
        cash_settled: db.cash_settled,
        participation_active: db.participation_active,
        total_payments_received: db.total_payments_received.map(|d| d.to_string()),
        total_payments_to_participant: db.total_payments_to_participant.map(|d| d.to_string()),
        notes: db.notes,
        special_terms: db.special_terms,
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

/// List participation agreements
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/participation-agreements",
    params(ParticipationAgreementListParams),
    responses(
        (status = 200, description = "List of participation agreements", body = PaginatedResponse<ParticipationAgreementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn list_participation_agreements_handler(
    State(state): State<ApiState>,
    Query(params): Query<ParticipationAgreementListParams>,
) -> Result<
    Json<PaginatedResponse<ParticipationAgreementResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing participation agreements");
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_participation_agreements(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.participation_type.clone(),
        params.agreement_status.clone(),
        params.participation_active,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List participation agreements", e))?;

    let data: Vec<ParticipationAgreementResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get participation agreement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/participation-agreements/{id}",
    params(
        ("id" = String, Path, description = "Participation agreement UUID")
    ),
    responses(
        (status = 200, description = "Participation agreement details", body = ParticipationAgreementResponse),
        (status = 404, description = "Participation agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn get_participation_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ParticipationAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting participation agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_participation_agreement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get participation agreement", e))?
        .ok_or_else(|| not_found_error("Participation agreement", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create participation agreement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/participation-agreements",
    request_body = CreateParticipationAgreementApiRequest,
    responses(
        (status = 201, description = "Participation agreement created", body = ParticipationAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn create_participation_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateParticipationAgreementApiRequest>,
) -> Result<(StatusCode, Json<ParticipationAgreementResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Creating participation agreement {}",
        request.document_id
    );
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "create").increment(1);

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

    let grantor_entity_id = if let Some(id_str) = request.grantor_entity_id {
        Some(parse_uuid(&id_str, "grantor_entity_id")?)
    } else {
        None
    };

    let participant_entity_id = if let Some(id_str) = request.participant_entity_id {
        Some(parse_uuid(&id_str, "participant_entity_id")?)
    } else {
        None
    };

    let parent_participation_id = if let Some(id_str) = request.parent_participation_id {
        Some(parse_uuid(&id_str, "parent_participation_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        trade_id,
        doc_hash: request.doc_hash,
        participation_type: request.participation_type,
        grantor: request.grantor,
        grantor_entity_id,
        grantor_remains_lender_of_record: request.grantor_remains_lender_of_record,
        participant: request.participant,
        participant_entity_id,
        borrower_name: request.borrower_name,
        facility_reference: request.facility_reference,
        participation_percentage: request.participation_percentage,
        participation_amount: request.participation_amount,
        grantor_total_commitment: request.grantor_total_commitment,
        grantor_outstanding_principal: request.grantor_outstanding_principal,
        purchase_price: request.purchase_price,
        purchase_price_percentage: request.purchase_price_percentage,
        funded_at_closing: request.funded_at_closing,
        funding_mechanism: request.funding_mechanism,
        payments_flow: request.payments_flow,
        payment_terms: request.payment_terms,
        participant_rights: request.participant_rights,
        no_privity_with_borrower: request.no_privity_with_borrower,
        participant_has_voting_rights: request.participant_has_voting_rights,
        borrower_consent_required: request.borrower_consent_required,
        borrower_consent_obtained: request.borrower_consent_obtained,
        borrower_knowledge_of_participation: request.borrower_knowledge_of_participation,
        agent_notification_required: request.agent_notification_required,
        agent_notified: request.agent_notified,
        pro_rata_sharing: request.pro_rata_sharing,
        principal_sharing_percentage: request.principal_sharing_percentage,
        interest_sharing_percentage: request.interest_sharing_percentage,
        fee_sharing_percentage: request.fee_sharing_percentage,
        grantor_pays_participant: request.grantor_pays_participant,
        settlement_frequency: request.settlement_frequency,
        participant_credit_risk: request.participant_credit_risk,
        participant_funding_risk: request.participant_funding_risk,
        setoff_rights: request.setoff_rights,
        netting_provisions: request.netting_provisions,
        grantor_representations: request.grantor_representations,
        participant_representations: request.participant_representations,
        participant_assignment_rights: request.participant_assignment_rights,
        requires_grantor_consent_to_assign: request.requires_grantor_consent_to_assign,
        is_sub_participation: request.is_sub_participation,
        parent_participation_id,
        confidential_agreement: request.confidential_agreement,
        disclosure_restrictions: request.disclosure_restrictions,
        trade_date: request.trade_date,
        settlement_date: request.settlement_date,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        termination_conditions: request.termination_conditions,
        participant_kyc_completed: request.participant_kyc_completed,
        participant_kyc_date: request.participant_kyc_date,
        participant_eligible: request.participant_eligible,
        agreement_status: request.agreement_status,
        cash_settled: request.cash_settled,
        participation_active: request.participation_active,
        total_payments_received: request.total_payments_received,
        total_payments_to_participant: request.total_payments_to_participant,
        notes: request.notes,
        special_terms: request.special_terms,
        metadata: request.metadata,
        created_by,
    };

    let record = create_participation_agreement(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create participation agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create participation agreements in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/participation-agreements/batch",
    request_body = BatchCreateParticipationAgreementsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn create_participation_agreements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateParticipationAgreementsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} participation agreements",
        request.participation_agreements.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.participation_agreements {
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

        let grantor_entity_id = if let Some(id_str) = req.grantor_entity_id {
            match parse_uuid(&id_str, "grantor_entity_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let participant_entity_id = if let Some(id_str) = req.participant_entity_id {
            match parse_uuid(&id_str, "participant_entity_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let parent_participation_id = if let Some(id_str) = req.parent_participation_id {
            match parse_uuid(&id_str, "parent_participation_id") {
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
            participation_type: req.participation_type,
            grantor: req.grantor,
            grantor_entity_id,
            grantor_remains_lender_of_record: req.grantor_remains_lender_of_record,
            participant: req.participant,
            participant_entity_id,
            borrower_name: req.borrower_name,
            facility_reference: req.facility_reference,
            participation_percentage: req.participation_percentage,
            participation_amount: req.participation_amount,
            grantor_total_commitment: req.grantor_total_commitment,
            grantor_outstanding_principal: req.grantor_outstanding_principal,
            purchase_price: req.purchase_price,
            purchase_price_percentage: req.purchase_price_percentage,
            funded_at_closing: req.funded_at_closing,
            funding_mechanism: req.funding_mechanism,
            payments_flow: req.payments_flow,
            payment_terms: req.payment_terms,
            participant_rights: req.participant_rights,
            no_privity_with_borrower: req.no_privity_with_borrower,
            participant_has_voting_rights: req.participant_has_voting_rights,
            borrower_consent_required: req.borrower_consent_required,
            borrower_consent_obtained: req.borrower_consent_obtained,
            borrower_knowledge_of_participation: req.borrower_knowledge_of_participation,
            agent_notification_required: req.agent_notification_required,
            agent_notified: req.agent_notified,
            pro_rata_sharing: req.pro_rata_sharing,
            principal_sharing_percentage: req.principal_sharing_percentage,
            interest_sharing_percentage: req.interest_sharing_percentage,
            fee_sharing_percentage: req.fee_sharing_percentage,
            grantor_pays_participant: req.grantor_pays_participant,
            settlement_frequency: req.settlement_frequency,
            participant_credit_risk: req.participant_credit_risk,
            participant_funding_risk: req.participant_funding_risk,
            setoff_rights: req.setoff_rights,
            netting_provisions: req.netting_provisions,
            grantor_representations: req.grantor_representations,
            participant_representations: req.participant_representations,
            participant_assignment_rights: req.participant_assignment_rights,
            requires_grantor_consent_to_assign: req.requires_grantor_consent_to_assign,
            is_sub_participation: req.is_sub_participation,
            parent_participation_id,
            confidential_agreement: req.confidential_agreement,
            disclosure_restrictions: req.disclosure_restrictions,
            trade_date: req.trade_date,
            settlement_date: req.settlement_date,
            effective_date: req.effective_date,
            termination_date: req.termination_date,
            termination_conditions: req.termination_conditions,
            participant_kyc_completed: req.participant_kyc_completed,
            participant_kyc_date: req.participant_kyc_date,
            participant_eligible: req.participant_eligible,
            agreement_status: req.agreement_status,
            cash_settled: req.cash_settled,
            participation_active: req.participation_active,
            total_payments_received: req.total_payments_received,
            total_payments_to_participant: req.total_payments_to_participant,
            notes: req.notes,
            special_terms: req.special_terms,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_participation_agreements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create participation agreements", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update participation agreement
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/participation-agreements/{id}",
    params(
        ("id" = String, Path, description = "Participation agreement UUID")
    ),
    request_body = UpdateParticipationAgreementApiRequest,
    responses(
        (status = 200, description = "Participation agreement updated", body = ParticipationAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Participation agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn update_participation_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateParticipationAgreementApiRequest>,
) -> Result<Json<ParticipationAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating participation agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "update").increment(1);

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

    let grantor_entity_id = if let Some(id_str) = request.grantor_entity_id {
        Some(parse_uuid(&id_str, "grantor_entity_id")?)
    } else {
        None
    };

    let participant_entity_id = if let Some(id_str) = request.participant_entity_id {
        Some(parse_uuid(&id_str, "participant_entity_id")?)
    } else {
        None
    };

    let parent_participation_id = if let Some(id_str) = request.parent_participation_id {
        Some(parse_uuid(&id_str, "parent_participation_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        document_id: request.document_id,
        credit_agreement_id,
        trade_id,
        doc_hash: request.doc_hash,
        participation_type: request.participation_type,
        grantor: request.grantor,
        grantor_entity_id,
        grantor_remains_lender_of_record: request.grantor_remains_lender_of_record,
        participant: request.participant,
        participant_entity_id,
        borrower_name: request.borrower_name,
        facility_reference: request.facility_reference,
        participation_percentage: request.participation_percentage,
        participation_amount: request.participation_amount,
        grantor_total_commitment: request.grantor_total_commitment,
        grantor_outstanding_principal: request.grantor_outstanding_principal,
        purchase_price: request.purchase_price,
        purchase_price_percentage: request.purchase_price_percentage,
        funded_at_closing: request.funded_at_closing,
        funding_mechanism: request.funding_mechanism,
        payments_flow: request.payments_flow,
        payment_terms: request.payment_terms,
        participant_rights: request.participant_rights,
        no_privity_with_borrower: request.no_privity_with_borrower,
        participant_has_voting_rights: request.participant_has_voting_rights,
        borrower_consent_required: request.borrower_consent_required,
        borrower_consent_obtained: request.borrower_consent_obtained,
        borrower_knowledge_of_participation: request.borrower_knowledge_of_participation,
        agent_notification_required: request.agent_notification_required,
        agent_notified: request.agent_notified,
        pro_rata_sharing: request.pro_rata_sharing,
        principal_sharing_percentage: request.principal_sharing_percentage,
        interest_sharing_percentage: request.interest_sharing_percentage,
        fee_sharing_percentage: request.fee_sharing_percentage,
        grantor_pays_participant: request.grantor_pays_participant,
        settlement_frequency: request.settlement_frequency,
        participant_credit_risk: request.participant_credit_risk,
        participant_funding_risk: request.participant_funding_risk,
        setoff_rights: request.setoff_rights,
        netting_provisions: request.netting_provisions,
        grantor_representations: request.grantor_representations,
        participant_representations: request.participant_representations,
        participant_assignment_rights: request.participant_assignment_rights,
        requires_grantor_consent_to_assign: request.requires_grantor_consent_to_assign,
        is_sub_participation: request.is_sub_participation,
        parent_participation_id,
        confidential_agreement: request.confidential_agreement,
        disclosure_restrictions: request.disclosure_restrictions,
        trade_date: request.trade_date,
        settlement_date: request.settlement_date,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        termination_conditions: request.termination_conditions,
        participant_kyc_completed: request.participant_kyc_completed,
        participant_kyc_date: request.participant_kyc_date,
        participant_eligible: request.participant_eligible,
        agreement_status: request.agreement_status,
        cash_settled: request.cash_settled,
        participation_active: request.participation_active,
        total_payments_received: request.total_payments_received,
        total_payments_to_participant: request.total_payments_to_participant,
        notes: request.notes,
        special_terms: request.special_terms,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_participation_agreement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update participation agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete participation agreement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/participation-agreements/{id}",
    params(
        ("id" = String, Path, description = "Participation agreement UUID")
    ),
    responses(
        (status = 200, description = "Participation agreement deleted", body = DeleteResponse),
        (status = 404, description = "Participation agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Participation Agreements"
)]
#[instrument(skip(state))]
pub async fn delete_participation_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting participation agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "participation_agreements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_participation_agreement(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete participation agreement", e))?;

    if !deleted {
        return Err(not_found_error("Participation agreement", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "participation_agreements", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Participation agreement {} deleted successfully", id),
    }))
}
