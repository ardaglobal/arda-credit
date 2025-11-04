//! Account Control Agreements API Handlers
//!
//! Provides REST API endpoints for account control agreement CRUD operations.

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
    create_account_control_agreement, create_account_control_agreements_batch,
    delete_account_control_agreement, get_account_control_agreement_by_id,
    list_account_control_agreements, update_account_control_agreement,
    CreateAccountControlAgreementRequest as DbCreateRequest,
    UpdateAccountControlAgreementRequest as DbUpdateRequest,
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

/// Account Control Agreement API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "document_id": "DACA-2025-001",
    "security_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "agreement_type": "DACA",
    "control_level": "springing_control",
    "bank_name": "JPMorgan Chase Bank, N.A.",
    "control_activated": null
}))]
pub struct AccountControlAgreementResponse {
    pub id: String,
    pub document_id: String,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub agreement_type: String,
    pub doc_hash: String,
    pub debtor_name: String,
    pub secured_party: String,
    pub bank_name: String,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>,
    pub total_accounts_count: Option<i32>,
    pub control_level: String,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>,
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>,
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>,
    pub notice_required_for_control: Option<bool>,
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>,
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>,
    pub bank_acknowledgment_received: Option<bool>,
    pub bank_compliance_confirmed: Option<bool>,
    pub bank_fees: Option<String>,
    pub prior_liens_exist: Option<bool>,
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,
    pub lockbox_account: Option<bool>,
    pub collections_account: Option<bool>,
    pub linked_to_cash_management: Option<bool>,
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

/// Create account control agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateAccountControlAgreementApiRequest {
    pub document_id: String,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub agreement_type: String,
    pub doc_hash: String,
    pub debtor_name: String,
    pub secured_party: String,
    pub bank_name: String,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>,
    pub total_accounts_count: Option<i32>,
    pub control_level: String,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>,
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>,
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>,
    pub notice_required_for_control: Option<bool>,
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>,
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>,
    pub bank_acknowledgment_received: Option<bool>,
    pub bank_compliance_confirmed: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub bank_fees: Option<Decimal>,
    pub prior_liens_exist: Option<bool>,
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,
    pub lockbox_account: Option<bool>,
    pub collections_account: Option<bool>,
    pub linked_to_cash_management: Option<bool>,
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update account control agreement request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAccountControlAgreementApiRequest {
    pub document_id: Option<String>,
    pub security_agreement_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub agreement_type: Option<String>,
    pub doc_hash: Option<String>,
    pub debtor_name: Option<String>,
    pub secured_party: Option<String>,
    pub bank_name: Option<String>,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>,
    pub total_accounts_count: Option<i32>,
    pub control_level: Option<String>,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>,
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>,
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>,
    pub notice_required_for_control: Option<bool>,
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>,
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>,
    pub bank_acknowledgment_received: Option<bool>,
    pub bank_compliance_confirmed: Option<bool>,
    #[schema(value_type = Option<String>)]
    pub bank_fees: Option<Decimal>,
    pub prior_liens_exist: Option<bool>,
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,
    pub lockbox_account: Option<bool>,
    pub collections_account: Option<bool>,
    pub linked_to_cash_management: Option<bool>,
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing account control agreements
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AccountControlAgreementListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by security agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub security_agreement_id: Option<String>,

    /// Filter by agreement type (DACA, ACA, SACA)
    #[param(example = "DACA")]
    pub agreement_type: Option<String>,

    /// Filter by control level
    #[param(example = "springing_control")]
    pub control_level: Option<String>,

    /// Filter by control activated status
    #[param(example = "false")]
    pub control_activated: Option<bool>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateAccountControlAgreementsRequest {
    pub account_control_agreements: Vec<CreateAccountControlAgreementApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::AccountControlAgreement) -> AccountControlAgreementResponse {
    AccountControlAgreementResponse {
        id: db.id.to_string(),
        document_id: db.document_id,
        security_agreement_id: db.security_agreement_id.map(|id| id.to_string()),
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        agreement_type: db.agreement_type,
        doc_hash: db.doc_hash,
        debtor_name: db.debtor_name,
        secured_party: db.secured_party,
        bank_name: db.bank_name,
        bank_identifier: db.bank_identifier,
        accounts_covered: db.accounts_covered,
        total_accounts_count: db.total_accounts_count,
        control_level: db.control_level,
        control_trigger_event: db.control_trigger_event,
        control_activated: db.control_activated,
        activation_date: db.activation_date,
        activation_trigger: db.activation_trigger,
        debtor_withdrawal_rights: db.debtor_withdrawal_rights,
        sweeps_permitted: db.sweeps_permitted,
        sweep_terms: db.sweep_terms,
        setoff_rights_waived: db.setoff_rights_waived,
        notice_required_for_control: db.notice_required_for_control,
        notice_period_days: db.notice_period_days,
        notice_delivery_method: db.notice_delivery_method,
        ucc9_control_perfected: db.ucc9_control_perfected,
        perfection_date: db.perfection_date,
        control_method: db.control_method,
        effective_date: db.effective_date,
        termination_date: db.termination_date,
        termination_conditions: db.termination_conditions,
        automatic_termination: db.automatic_termination,
        bank_acknowledgment_received: db.bank_acknowledgment_received,
        bank_compliance_confirmed: db.bank_compliance_confirmed,
        bank_fees: db.bank_fees.map(|d| d.to_string()),
        prior_liens_exist: db.prior_liens_exist,
        prior_liens_description: db.prior_liens_description,
        subordination_agreements: db.subordination_agreements,
        collateral_account: db.collateral_account,
        lockbox_account: db.lockbox_account,
        collections_account: db.collections_account,
        linked_to_cash_management: db.linked_to_cash_management,
        cash_management_system: db.cash_management_system,
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

/// List account control agreements
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/account-control-agreements",
    params(AccountControlAgreementListParams),
    responses(
        (status = 200, description = "List of account control agreements", body = PaginatedResponse<AccountControlAgreementResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn list_account_control_agreements_handler(
    State(state): State<ApiState>,
    Query(params): Query<AccountControlAgreementListParams>,
) -> Result<
    Json<PaginatedResponse<AccountControlAgreementResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing account control agreements");
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let security_agreement_id = if let Some(id_str) = params.security_agreement_id {
        Some(parse_uuid(&id_str, "security_agreement_id")?)
    } else {
        None
    };

    let (records, total) = list_account_control_agreements(
        &state.pool,
        limit,
        offset,
        security_agreement_id,
        params.agreement_type.clone(),
        params.control_level.clone(),
        params.control_activated,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List account control agreements", e))?;

    let data: Vec<AccountControlAgreementResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get account control agreement by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/account-control-agreements/{id}",
    params(
        ("id" = String, Path, description = "Account control agreement UUID")
    ),
    responses(
        (status = 200, description = "Account control agreement details", body = AccountControlAgreementResponse),
        (status = 404, description = "Account control agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn get_account_control_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<AccountControlAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting account control agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_account_control_agreement_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get account control agreement", e))?
        .ok_or_else(|| not_found_error("Account control agreement", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create account control agreement
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/account-control-agreements",
    request_body = CreateAccountControlAgreementApiRequest,
    responses(
        (status = 201, description = "Account control agreement created", body = AccountControlAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn create_account_control_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateAccountControlAgreementApiRequest>,
) -> Result<(StatusCode, Json<AccountControlAgreementResponse>), (StatusCode, Json<ErrorResponse>)>
{
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Creating account control agreement {}",
        request.document_id
    );
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "create").increment(1);

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
        agreement_type: request.agreement_type,
        doc_hash: request.doc_hash,
        debtor_name: request.debtor_name,
        secured_party: request.secured_party,
        bank_name: request.bank_name,
        bank_identifier: request.bank_identifier,
        accounts_covered: request.accounts_covered,
        total_accounts_count: request.total_accounts_count,
        control_level: request.control_level,
        control_trigger_event: request.control_trigger_event,
        control_activated: request.control_activated,
        activation_date: request.activation_date,
        activation_trigger: request.activation_trigger,
        debtor_withdrawal_rights: request.debtor_withdrawal_rights,
        sweeps_permitted: request.sweeps_permitted,
        sweep_terms: request.sweep_terms,
        setoff_rights_waived: request.setoff_rights_waived,
        notice_required_for_control: request.notice_required_for_control,
        notice_period_days: request.notice_period_days,
        notice_delivery_method: request.notice_delivery_method,
        ucc9_control_perfected: request.ucc9_control_perfected,
        perfection_date: request.perfection_date,
        control_method: request.control_method,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        termination_conditions: request.termination_conditions,
        automatic_termination: request.automatic_termination,
        bank_acknowledgment_received: request.bank_acknowledgment_received,
        bank_compliance_confirmed: request.bank_compliance_confirmed,
        bank_fees: request.bank_fees,
        prior_liens_exist: request.prior_liens_exist,
        prior_liens_description: request.prior_liens_description,
        subordination_agreements: request.subordination_agreements,
        collateral_account: request.collateral_account,
        lockbox_account: request.lockbox_account,
        collections_account: request.collections_account,
        linked_to_cash_management: request.linked_to_cash_management,
        cash_management_system: request.cash_management_system,
        notes: request.notes,
        metadata: request.metadata,
        created_by,
    };

    let record = create_account_control_agreement(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create account control agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create account control agreements in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/account-control-agreements/batch",
    request_body = BatchCreateAccountControlAgreementsRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn create_account_control_agreements_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateAccountControlAgreementsRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} account control agreements",
        request.account_control_agreements.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.account_control_agreements {
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
            agreement_type: req.agreement_type,
            doc_hash: req.doc_hash,
            debtor_name: req.debtor_name,
            secured_party: req.secured_party,
            bank_name: req.bank_name,
            bank_identifier: req.bank_identifier,
            accounts_covered: req.accounts_covered,
            total_accounts_count: req.total_accounts_count,
            control_level: req.control_level,
            control_trigger_event: req.control_trigger_event,
            control_activated: req.control_activated,
            activation_date: req.activation_date,
            activation_trigger: req.activation_trigger,
            debtor_withdrawal_rights: req.debtor_withdrawal_rights,
            sweeps_permitted: req.sweeps_permitted,
            sweep_terms: req.sweep_terms,
            setoff_rights_waived: req.setoff_rights_waived,
            notice_required_for_control: req.notice_required_for_control,
            notice_period_days: req.notice_period_days,
            notice_delivery_method: req.notice_delivery_method,
            ucc9_control_perfected: req.ucc9_control_perfected,
            perfection_date: req.perfection_date,
            control_method: req.control_method,
            effective_date: req.effective_date,
            termination_date: req.termination_date,
            termination_conditions: req.termination_conditions,
            automatic_termination: req.automatic_termination,
            bank_acknowledgment_received: req.bank_acknowledgment_received,
            bank_compliance_confirmed: req.bank_compliance_confirmed,
            bank_fees: req.bank_fees,
            prior_liens_exist: req.prior_liens_exist,
            prior_liens_description: req.prior_liens_description,
            subordination_agreements: req.subordination_agreements,
            collateral_account: req.collateral_account,
            lockbox_account: req.lockbox_account,
            collections_account: req.collections_account,
            linked_to_cash_management: req.linked_to_cash_management,
            cash_management_system: req.cash_management_system,
            notes: req.notes,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_account_control_agreements_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create account control agreements", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update account control agreement
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/account-control-agreements/{id}",
    params(
        ("id" = String, Path, description = "Account control agreement UUID")
    ),
    request_body = UpdateAccountControlAgreementApiRequest,
    responses(
        (status = 200, description = "Account control agreement updated", body = AccountControlAgreementResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Account control agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn update_account_control_agreement_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateAccountControlAgreementApiRequest>,
) -> Result<Json<AccountControlAgreementResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating account control agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "update").increment(1);

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
        agreement_type: request.agreement_type,
        doc_hash: request.doc_hash,
        debtor_name: request.debtor_name,
        secured_party: request.secured_party,
        bank_name: request.bank_name,
        bank_identifier: request.bank_identifier,
        accounts_covered: request.accounts_covered,
        total_accounts_count: request.total_accounts_count,
        control_level: request.control_level,
        control_trigger_event: request.control_trigger_event,
        control_activated: request.control_activated,
        activation_date: request.activation_date,
        activation_trigger: request.activation_trigger,
        debtor_withdrawal_rights: request.debtor_withdrawal_rights,
        sweeps_permitted: request.sweeps_permitted,
        sweep_terms: request.sweep_terms,
        setoff_rights_waived: request.setoff_rights_waived,
        notice_required_for_control: request.notice_required_for_control,
        notice_period_days: request.notice_period_days,
        notice_delivery_method: request.notice_delivery_method,
        ucc9_control_perfected: request.ucc9_control_perfected,
        perfection_date: request.perfection_date,
        control_method: request.control_method,
        effective_date: request.effective_date,
        termination_date: request.termination_date,
        termination_conditions: request.termination_conditions,
        automatic_termination: request.automatic_termination,
        bank_acknowledgment_received: request.bank_acknowledgment_received,
        bank_compliance_confirmed: request.bank_compliance_confirmed,
        bank_fees: request.bank_fees,
        prior_liens_exist: request.prior_liens_exist,
        prior_liens_description: request.prior_liens_description,
        subordination_agreements: request.subordination_agreements,
        collateral_account: request.collateral_account,
        lockbox_account: request.lockbox_account,
        collections_account: request.collections_account,
        linked_to_cash_management: request.linked_to_cash_management,
        cash_management_system: request.cash_management_system,
        notes: request.notes,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_account_control_agreement(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update account control agreement", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete account control agreement
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/account-control-agreements/{id}",
    params(
        ("id" = String, Path, description = "Account control agreement UUID")
    ),
    responses(
        (status = 200, description = "Account control agreement deleted", body = DeleteResponse),
        (status = 404, description = "Account control agreement not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Account Control Agreements"
)]
#[instrument(skip(state))]
pub async fn delete_account_control_agreement_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting account control agreement {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "account_control_agreements", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_account_control_agreement(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete account control agreement", e))?;

    if !deleted {
        return Err(not_found_error("Account control agreement", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "account_control_agreements", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Account control agreement {} deleted successfully", id),
    }))
}
