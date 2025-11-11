//! Secondary Trades API Handlers
//!
//! Provides REST API endpoints for secondary trade CRUD operations.

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
    create_secondary_trade, create_secondary_trades_batch, delete_secondary_trade,
    get_secondary_trade_by_id, list_secondary_trades, update_secondary_trade,
    CreateSecondaryTradeRequest as DbCreateRequest, UpdateSecondaryTradeRequest as DbUpdateRequest,
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

/// Secondary Trade API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "trade_id": "TRD-2025-001",
    "credit_agreement_id": "650e8400-e29b-41d4-a716-446655440000",
    "instrument_type": "term_loan",
    "trade_type": "par_trade",
    "trade_status": "pending",
    "notional_amount": "1000000.00",
    "price": "100.00"
}))]
pub struct SecondaryTradeResponse {
    pub id: String,
    pub trade_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_confirm_hash: String,
    pub instrument_id: String,
    pub instrument_type: Option<String>,
    pub seller_party_id: String,
    pub seller_legal_name: String,
    pub seller_role: Option<String>,
    pub buyer_party_id: String,
    pub buyer_legal_name: String,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<String>,
    pub broker_name: Option<String>,
    pub broker_fee_amount: Option<String>,
    pub broker_fee_currency: Option<String>,
    pub trade_datetime: String,
    pub trade_date: NaiveDate,
    pub trade_type: String,
    pub trade_status: Option<String>,
    pub effective_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub settlement_days: Option<i32>,
    pub actual_settlement_date: Option<NaiveDate>,
    pub notional_amount: String,
    pub notional_currency: Option<String>,
    pub price_type: String,
    pub price: String,
    pub price_in_dollars: Option<String>,
    pub accrued_interest_amount: Option<String>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    pub total_consideration: String,
    pub total_consideration_currency: Option<String>,
    pub transfer_fee_amount: Option<String>,
    pub transfer_fee_currency: Option<String>,
    pub transfer_fee_paid_by: Option<String>,
    pub other_fees: Option<serde_json::Value>,
    pub eligibility_checks: Option<serde_json::Value>,
    pub kyc_checks_passed: Option<bool>,
    pub aml_checks_passed: Option<bool>,
    pub sanctions_screening_passed: Option<bool>,
    pub transfer_restrictions_apply: Option<bool>,
    pub transfer_consent_required: Option<bool>,
    pub transfer_consent_obtained: Option<bool>,
    pub transfer_consent_date: Option<NaiveDate>,
    pub transfer_consent_obtained_from: Option<String>,
    pub allocation_id: Option<String>,
    pub allocation_percentage: Option<String>,
    pub pro_rata_share: Option<bool>,
    pub docs_required: Option<serde_json::Value>,
    pub docs_received_count: Option<i32>,
    pub docs_required_count: Option<i32>,
    pub docs_complete: Option<bool>,
    pub trade_confirm_sent: Option<bool>,
    pub trade_confirm_sent_at: Option<String>,
    pub trade_confirm_received: Option<bool>,
    pub trade_confirm_received_at: Option<String>,
    pub special_terms: Option<String>,
    pub representations_and_warranties: Option<String>,
    pub conditions_precedent: Option<String>,
    pub post_closing_obligations: Option<String>,
    pub internal_notes: Option<String>,
    pub trader_notes: Option<String>,
    pub compliance_notes: Option<String>,
    pub market_segment: Option<String>,
    pub credit_rating: Option<String>,
    pub sector: Option<String>,
    pub yield_to_maturity: Option<String>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<String>,
    pub buyer_ssi_id: Option<String>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub confirmed_by: Option<String>,
    pub settled_by: Option<String>,
}

/// Create secondary trade request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSecondaryTradeApiRequest {
    pub trade_id: String,
    pub credit_agreement_id: Option<String>,
    pub trade_confirm_hash: String,
    pub instrument_id: String,
    pub instrument_type: Option<String>,
    pub seller_party_id: String,
    pub seller_legal_name: String,
    pub seller_role: Option<String>,
    pub buyer_party_id: String,
    pub buyer_legal_name: String,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<String>,
    pub broker_name: Option<String>,
    #[schema(value_type = Option<String>)]
    pub broker_fee_amount: Option<Decimal>,
    pub broker_fee_currency: Option<String>,
    pub trade_datetime: DateTime<Utc>,
    pub trade_date: NaiveDate,
    pub trade_type: String,
    pub trade_status: Option<String>,
    pub effective_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub settlement_days: Option<i32>,
    pub actual_settlement_date: Option<NaiveDate>,
    #[schema(value_type = String)]
    pub notional_amount: Decimal,
    pub notional_currency: Option<String>,
    pub price_type: String,
    #[schema(value_type = String)]
    pub price: Decimal,
    #[schema(value_type = Option<String>)]
    pub price_in_dollars: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    #[schema(value_type = String)]
    pub total_consideration: Decimal,
    pub total_consideration_currency: Option<String>,
    #[schema(value_type = Option<String>)]
    pub transfer_fee_amount: Option<Decimal>,
    pub transfer_fee_currency: Option<String>,
    pub transfer_fee_paid_by: Option<String>,
    pub other_fees: Option<serde_json::Value>,
    pub eligibility_checks: Option<serde_json::Value>,
    pub kyc_checks_passed: Option<bool>,
    pub aml_checks_passed: Option<bool>,
    pub sanctions_screening_passed: Option<bool>,
    pub transfer_restrictions_apply: Option<bool>,
    pub transfer_consent_required: Option<bool>,
    pub transfer_consent_obtained: Option<bool>,
    pub transfer_consent_date: Option<NaiveDate>,
    pub transfer_consent_obtained_from: Option<String>,
    pub allocation_id: Option<String>,
    #[schema(value_type = Option<String>)]
    pub allocation_percentage: Option<Decimal>,
    pub pro_rata_share: Option<bool>,
    pub docs_required: Option<serde_json::Value>,
    pub docs_received_count: Option<i32>,
    pub docs_required_count: Option<i32>,
    pub docs_complete: Option<bool>,
    pub trade_confirm_sent: Option<bool>,
    pub trade_confirm_sent_at: Option<DateTime<Utc>>,
    pub trade_confirm_received: Option<bool>,
    pub trade_confirm_received_at: Option<DateTime<Utc>>,
    pub special_terms: Option<String>,
    pub representations_and_warranties: Option<String>,
    pub conditions_precedent: Option<String>,
    pub post_closing_obligations: Option<String>,
    pub internal_notes: Option<String>,
    pub trader_notes: Option<String>,
    pub compliance_notes: Option<String>,
    pub market_segment: Option<String>,
    pub credit_rating: Option<String>,
    pub sector: Option<String>,
    #[schema(value_type = Option<String>)]
    pub yield_to_maturity: Option<Decimal>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<String>,
    pub buyer_ssi_id: Option<String>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Update secondary trade request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSecondaryTradeApiRequest {
    pub trade_id: Option<String>,
    pub credit_agreement_id: Option<String>,
    pub trade_confirm_hash: Option<String>,
    pub instrument_id: Option<String>,
    pub instrument_type: Option<String>,
    pub seller_party_id: Option<String>,
    pub seller_legal_name: Option<String>,
    pub seller_role: Option<String>,
    pub buyer_party_id: Option<String>,
    pub buyer_legal_name: Option<String>,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<String>,
    pub broker_name: Option<String>,
    #[schema(value_type = Option<String>)]
    pub broker_fee_amount: Option<Decimal>,
    pub broker_fee_currency: Option<String>,
    pub trade_datetime: Option<DateTime<Utc>>,
    pub trade_date: Option<NaiveDate>,
    pub trade_type: Option<String>,
    pub trade_status: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub settlement_days: Option<i32>,
    pub actual_settlement_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>)]
    pub notional_amount: Option<Decimal>,
    pub notional_currency: Option<String>,
    pub price_type: Option<String>,
    #[schema(value_type = Option<String>)]
    pub price: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub price_in_dollars: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    #[schema(value_type = Option<String>)]
    pub total_consideration: Option<Decimal>,
    pub total_consideration_currency: Option<String>,
    #[schema(value_type = Option<String>)]
    pub transfer_fee_amount: Option<Decimal>,
    pub transfer_fee_currency: Option<String>,
    pub transfer_fee_paid_by: Option<String>,
    pub other_fees: Option<serde_json::Value>,
    pub eligibility_checks: Option<serde_json::Value>,
    pub kyc_checks_passed: Option<bool>,
    pub aml_checks_passed: Option<bool>,
    pub sanctions_screening_passed: Option<bool>,
    pub transfer_restrictions_apply: Option<bool>,
    pub transfer_consent_required: Option<bool>,
    pub transfer_consent_obtained: Option<bool>,
    pub transfer_consent_date: Option<NaiveDate>,
    pub transfer_consent_obtained_from: Option<String>,
    pub allocation_id: Option<String>,
    #[schema(value_type = Option<String>)]
    pub allocation_percentage: Option<Decimal>,
    pub pro_rata_share: Option<bool>,
    pub docs_required: Option<serde_json::Value>,
    pub docs_received_count: Option<i32>,
    pub docs_required_count: Option<i32>,
    pub docs_complete: Option<bool>,
    pub trade_confirm_sent: Option<bool>,
    pub trade_confirm_sent_at: Option<DateTime<Utc>>,
    pub trade_confirm_received: Option<bool>,
    pub trade_confirm_received_at: Option<DateTime<Utc>>,
    pub special_terms: Option<String>,
    pub representations_and_warranties: Option<String>,
    pub conditions_precedent: Option<String>,
    pub post_closing_obligations: Option<String>,
    pub internal_notes: Option<String>,
    pub trader_notes: Option<String>,
    pub compliance_notes: Option<String>,
    pub market_segment: Option<String>,
    pub credit_rating: Option<String>,
    pub sector: Option<String>,
    #[schema(value_type = Option<String>)]
    pub yield_to_maturity: Option<Decimal>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<String>,
    pub buyer_ssi_id: Option<String>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing secondary trades
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SecondaryTradeListParams {
    #[serde(flatten)]
    pub pagination: ListQueryParams,

    /// Filter by credit agreement ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub credit_agreement_id: Option<String>,

    /// Filter by trade status
    #[param(example = "pending")]
    pub trade_status: Option<String>,

    /// Filter by trade type
    #[param(example = "par_trade")]
    pub trade_type: Option<String>,

    /// Filter by seller party ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub seller_party_id: Option<String>,

    /// Filter by buyer party ID
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub buyer_party_id: Option<String>,
}

/// Batch create request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateSecondaryTradesRequest {
    pub secondary_trades: Vec<CreateSecondaryTradeApiRequest>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn db_to_api(db: arda_credit_db::SecondaryTrade) -> SecondaryTradeResponse {
    SecondaryTradeResponse {
        id: db.id.to_string(),
        trade_id: db.trade_id,
        credit_agreement_id: db.credit_agreement_id.map(|id| id.to_string()),
        trade_confirm_hash: db.trade_confirm_hash,
        instrument_id: db.instrument_id,
        instrument_type: db.instrument_type,
        seller_party_id: db.seller_party_id.to_string(),
        seller_legal_name: db.seller_legal_name,
        seller_role: db.seller_role,
        buyer_party_id: db.buyer_party_id.to_string(),
        buyer_legal_name: db.buyer_legal_name,
        buyer_role: db.buyer_role,
        broker_party_id: db.broker_party_id.map(|id| id.to_string()),
        broker_name: db.broker_name,
        broker_fee_amount: db.broker_fee_amount.map(|d| d.to_string()),
        broker_fee_currency: db.broker_fee_currency,
        trade_datetime: db.trade_datetime.to_rfc3339(),
        trade_date: db.trade_date,
        trade_type: db.trade_type,
        trade_status: db.trade_status,
        effective_date: db.effective_date,
        settlement_date: db.settlement_date,
        settlement_days: db.settlement_days,
        actual_settlement_date: db.actual_settlement_date,
        notional_amount: db.notional_amount.to_string(),
        notional_currency: db.notional_currency,
        price_type: db.price_type,
        price: db.price.to_string(),
        price_in_dollars: db.price_in_dollars.map(|d| d.to_string()),
        accrued_interest_amount: db.accrued_interest_amount.map(|d| d.to_string()),
        accrued_interest_currency: db.accrued_interest_currency,
        accrued_interest_calculation_method: db.accrued_interest_calculation_method,
        accrued_interest_from_date: db.accrued_interest_from_date,
        accrued_interest_to_date: db.accrued_interest_to_date,
        total_consideration: db.total_consideration.to_string(),
        total_consideration_currency: db.total_consideration_currency,
        transfer_fee_amount: db.transfer_fee_amount.map(|d| d.to_string()),
        transfer_fee_currency: db.transfer_fee_currency,
        transfer_fee_paid_by: db.transfer_fee_paid_by,
        other_fees: db.other_fees,
        eligibility_checks: db.eligibility_checks,
        kyc_checks_passed: db.kyc_checks_passed,
        aml_checks_passed: db.aml_checks_passed,
        sanctions_screening_passed: db.sanctions_screening_passed,
        transfer_restrictions_apply: db.transfer_restrictions_apply,
        transfer_consent_required: db.transfer_consent_required,
        transfer_consent_obtained: db.transfer_consent_obtained,
        transfer_consent_date: db.transfer_consent_date,
        transfer_consent_obtained_from: db.transfer_consent_obtained_from,
        allocation_id: db.allocation_id,
        allocation_percentage: db.allocation_percentage.map(|d| d.to_string()),
        pro_rata_share: db.pro_rata_share,
        docs_required: db.docs_required,
        docs_received_count: db.docs_received_count,
        docs_required_count: db.docs_required_count,
        docs_complete: db.docs_complete,
        trade_confirm_sent: db.trade_confirm_sent,
        trade_confirm_sent_at: db.trade_confirm_sent_at.map(|dt| dt.to_rfc3339()),
        trade_confirm_received: db.trade_confirm_received,
        trade_confirm_received_at: db.trade_confirm_received_at.map(|dt| dt.to_rfc3339()),
        special_terms: db.special_terms,
        representations_and_warranties: db.representations_and_warranties,
        conditions_precedent: db.conditions_precedent,
        post_closing_obligations: db.post_closing_obligations,
        internal_notes: db.internal_notes,
        trader_notes: db.trader_notes,
        compliance_notes: db.compliance_notes,
        market_segment: db.market_segment,
        credit_rating: db.credit_rating,
        sector: db.sector,
        yield_to_maturity: db.yield_to_maturity.map(|d| d.to_string()),
        spread_to_benchmark_bps: db.spread_to_benchmark_bps,
        seller_ssi_id: db.seller_ssi_id.map(|id| id.to_string()),
        buyer_ssi_id: db.buyer_ssi_id.map(|id| id.to_string()),
        settlement_method: db.settlement_method,
        settlement_account_seller: db.settlement_account_seller,
        settlement_account_buyer: db.settlement_account_buyer,
        metadata: db.metadata,
        created_at: db.created_at.map(|dt| dt.to_rfc3339()),
        updated_at: db.updated_at.map(|dt| dt.to_rfc3339()),
        created_by: db.created_by.map(|id| id.to_string()),
        updated_by: db.updated_by.map(|id| id.to_string()),
        confirmed_by: db.confirmed_by.map(|id| id.to_string()),
        settled_by: db.settled_by.map(|id| id.to_string()),
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// List secondary trades
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/secondary-trades",
    params(SecondaryTradeListParams),
    responses(
        (status = 200, description = "List of secondary trades", body = PaginatedResponse<SecondaryTradeResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn list_secondary_trades_handler(
    State(state): State<ApiState>,
    Query(params): Query<SecondaryTradeListParams>,
) -> Result<Json<PaginatedResponse<SecondaryTradeResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Listing secondary trades");
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "list").increment(1);

    let limit = params.pagination.get_limit();
    let offset = params.pagination.get_offset();

    let credit_agreement_id = if let Some(id_str) = params.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let seller_party_id = if let Some(id_str) = params.seller_party_id {
        Some(parse_uuid(&id_str, "seller_party_id")?)
    } else {
        None
    };

    let buyer_party_id = if let Some(id_str) = params.buyer_party_id {
        Some(parse_uuid(&id_str, "buyer_party_id")?)
    } else {
        None
    };

    let (records, total) = list_secondary_trades(
        &state.pool,
        limit,
        offset,
        credit_agreement_id,
        params.trade_status,
        params.trade_type,
        seller_party_id,
        buyer_party_id,
        params.pagination.sort_by.clone(),
        params.pagination.sort_order.clone(),
    )
    .await
    .map_err(|e| internal_error("List secondary trades", e))?;

    let data: Vec<SecondaryTradeResponse> = records.into_iter().map(db_to_api).collect();
    let count = data.len() as i64;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "list")
        .record(duration.as_secs_f64());

    Ok(Json(PaginatedResponse {
        data,
        total,
        count,
        limit,
        offset,
    }))
}

/// Get secondary trade by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/secondary-trades/{id}",
    params(
        ("id" = String, Path, description = "Secondary trade UUID")
    ),
    responses(
        (status = 200, description = "Secondary trade details", body = SecondaryTradeResponse),
        (status = 404, description = "Secondary trade not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn get_secondary_trade_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<SecondaryTradeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Getting secondary trade {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "get").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let record = get_secondary_trade_by_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Get secondary trade", e))?
        .ok_or_else(|| not_found_error("Secondary trade", &id))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "get")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Create secondary trade
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/secondary-trades",
    request_body = CreateSecondaryTradeApiRequest,
    responses(
        (status = 201, description = "Secondary trade created", body = SecondaryTradeResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn create_secondary_trade_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateSecondaryTradeApiRequest>,
) -> Result<(StatusCode, Json<SecondaryTradeResponse>), (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Creating secondary trade {}", request.trade_id);
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "create").increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let seller_party_id = parse_uuid(&request.seller_party_id, "seller_party_id")?;
    let buyer_party_id = parse_uuid(&request.buyer_party_id, "buyer_party_id")?;

    let broker_party_id = if let Some(id_str) = request.broker_party_id {
        Some(parse_uuid(&id_str, "broker_party_id")?)
    } else {
        None
    };

    let seller_ssi_id = if let Some(id_str) = request.seller_ssi_id {
        Some(parse_uuid(&id_str, "seller_ssi_id")?)
    } else {
        None
    };

    let buyer_ssi_id = if let Some(id_str) = request.buyer_ssi_id {
        Some(parse_uuid(&id_str, "buyer_ssi_id")?)
    } else {
        None
    };

    let db_request = DbCreateRequest {
        trade_id: request.trade_id,
        credit_agreement_id,
        trade_confirm_hash: request.trade_confirm_hash,
        instrument_id: request.instrument_id,
        instrument_type: request.instrument_type,
        seller_party_id,
        seller_legal_name: request.seller_legal_name,
        seller_role: request.seller_role,
        buyer_party_id,
        buyer_legal_name: request.buyer_legal_name,
        buyer_role: request.buyer_role,
        broker_party_id,
        broker_name: request.broker_name,
        broker_fee_amount: request.broker_fee_amount,
        broker_fee_currency: request.broker_fee_currency,
        trade_datetime: request.trade_datetime,
        trade_date: request.trade_date,
        trade_type: request.trade_type,
        trade_status: request.trade_status,
        effective_date: request.effective_date,
        settlement_date: request.settlement_date,
        settlement_days: request.settlement_days,
        actual_settlement_date: request.actual_settlement_date,
        notional_amount: request.notional_amount,
        notional_currency: request.notional_currency,
        price_type: request.price_type,
        price: request.price,
        price_in_dollars: request.price_in_dollars,
        accrued_interest_amount: request.accrued_interest_amount,
        accrued_interest_currency: request.accrued_interest_currency,
        accrued_interest_calculation_method: request.accrued_interest_calculation_method,
        accrued_interest_from_date: request.accrued_interest_from_date,
        accrued_interest_to_date: request.accrued_interest_to_date,
        total_consideration: request.total_consideration,
        total_consideration_currency: request.total_consideration_currency,
        transfer_fee_amount: request.transfer_fee_amount,
        transfer_fee_currency: request.transfer_fee_currency,
        transfer_fee_paid_by: request.transfer_fee_paid_by,
        other_fees: request.other_fees,
        eligibility_checks: request.eligibility_checks,
        kyc_checks_passed: request.kyc_checks_passed,
        aml_checks_passed: request.aml_checks_passed,
        sanctions_screening_passed: request.sanctions_screening_passed,
        transfer_restrictions_apply: request.transfer_restrictions_apply,
        transfer_consent_required: request.transfer_consent_required,
        transfer_consent_obtained: request.transfer_consent_obtained,
        transfer_consent_date: request.transfer_consent_date,
        transfer_consent_obtained_from: request.transfer_consent_obtained_from,
        allocation_id: request.allocation_id,
        allocation_percentage: request.allocation_percentage,
        pro_rata_share: request.pro_rata_share,
        docs_required: request.docs_required,
        docs_received_count: request.docs_received_count,
        docs_required_count: request.docs_required_count,
        docs_complete: request.docs_complete,
        trade_confirm_sent: request.trade_confirm_sent,
        trade_confirm_sent_at: request.trade_confirm_sent_at,
        trade_confirm_received: request.trade_confirm_received,
        trade_confirm_received_at: request.trade_confirm_received_at,
        special_terms: request.special_terms,
        representations_and_warranties: request.representations_and_warranties,
        conditions_precedent: request.conditions_precedent,
        post_closing_obligations: request.post_closing_obligations,
        internal_notes: request.internal_notes,
        trader_notes: request.trader_notes,
        compliance_notes: request.compliance_notes,
        market_segment: request.market_segment,
        credit_rating: request.credit_rating,
        sector: request.sector,
        yield_to_maturity: request.yield_to_maturity,
        spread_to_benchmark_bps: request.spread_to_benchmark_bps,
        seller_ssi_id,
        buyer_ssi_id,
        settlement_method: request.settlement_method,
        settlement_account_seller: request.settlement_account_seller,
        settlement_account_buyer: request.settlement_account_buyer,
        metadata: request.metadata,
        created_by,
    };

    let record = create_secondary_trade(&state.pool, db_request)
        .await
        .map_err(|e| internal_error("Create secondary trade", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "create")
        .record(duration.as_secs_f64());

    Ok((StatusCode::CREATED, Json(db_to_api(record))))
}

/// Create secondary trades in batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/secondary-trades/batch",
    request_body = BatchCreateSecondaryTradesRequest,
    responses(
        (status = 200, description = "Batch operation results", body = BatchOperationResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn create_secondary_trades_batch_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BatchCreateSecondaryTradesRequest>,
) -> Result<Json<BatchOperationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!(
        "🔍 API: Batch creating {} secondary trades",
        request.secondary_trades.len()
    );
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "batch_create")
        .increment(1);

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let created_by = current_user.as_ref().map(|u| u.id);

    let mut db_requests = Vec::new();
    for req in request.secondary_trades {
        let credit_agreement_id = if let Some(id_str) = req.credit_agreement_id {
            match parse_uuid(&id_str, "credit_agreement_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let seller_party_id = match parse_uuid(&req.seller_party_id, "seller_party_id") {
            Ok(id) => id,
            Err(_) => continue,
        };

        let buyer_party_id = match parse_uuid(&req.buyer_party_id, "buyer_party_id") {
            Ok(id) => id,
            Err(_) => continue,
        };

        let broker_party_id = if let Some(id_str) = req.broker_party_id {
            match parse_uuid(&id_str, "broker_party_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let seller_ssi_id = if let Some(id_str) = req.seller_ssi_id {
            match parse_uuid(&id_str, "seller_ssi_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        let buyer_ssi_id = if let Some(id_str) = req.buyer_ssi_id {
            match parse_uuid(&id_str, "buyer_ssi_id") {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        };

        db_requests.push(DbCreateRequest {
            trade_id: req.trade_id,
            credit_agreement_id,
            trade_confirm_hash: req.trade_confirm_hash,
            instrument_id: req.instrument_id,
            instrument_type: req.instrument_type,
            seller_party_id,
            seller_legal_name: req.seller_legal_name,
            seller_role: req.seller_role,
            buyer_party_id,
            buyer_legal_name: req.buyer_legal_name,
            buyer_role: req.buyer_role,
            broker_party_id,
            broker_name: req.broker_name,
            broker_fee_amount: req.broker_fee_amount,
            broker_fee_currency: req.broker_fee_currency,
            trade_datetime: req.trade_datetime,
            trade_date: req.trade_date,
            trade_type: req.trade_type,
            trade_status: req.trade_status,
            effective_date: req.effective_date,
            settlement_date: req.settlement_date,
            settlement_days: req.settlement_days,
            actual_settlement_date: req.actual_settlement_date,
            notional_amount: req.notional_amount,
            notional_currency: req.notional_currency,
            price_type: req.price_type,
            price: req.price,
            price_in_dollars: req.price_in_dollars,
            accrued_interest_amount: req.accrued_interest_amount,
            accrued_interest_currency: req.accrued_interest_currency,
            accrued_interest_calculation_method: req.accrued_interest_calculation_method,
            accrued_interest_from_date: req.accrued_interest_from_date,
            accrued_interest_to_date: req.accrued_interest_to_date,
            total_consideration: req.total_consideration,
            total_consideration_currency: req.total_consideration_currency,
            transfer_fee_amount: req.transfer_fee_amount,
            transfer_fee_currency: req.transfer_fee_currency,
            transfer_fee_paid_by: req.transfer_fee_paid_by,
            other_fees: req.other_fees,
            eligibility_checks: req.eligibility_checks,
            kyc_checks_passed: req.kyc_checks_passed,
            aml_checks_passed: req.aml_checks_passed,
            sanctions_screening_passed: req.sanctions_screening_passed,
            transfer_restrictions_apply: req.transfer_restrictions_apply,
            transfer_consent_required: req.transfer_consent_required,
            transfer_consent_obtained: req.transfer_consent_obtained,
            transfer_consent_date: req.transfer_consent_date,
            transfer_consent_obtained_from: req.transfer_consent_obtained_from,
            allocation_id: req.allocation_id,
            allocation_percentage: req.allocation_percentage,
            pro_rata_share: req.pro_rata_share,
            docs_required: req.docs_required,
            docs_received_count: req.docs_received_count,
            docs_required_count: req.docs_required_count,
            docs_complete: req.docs_complete,
            trade_confirm_sent: req.trade_confirm_sent,
            trade_confirm_sent_at: req.trade_confirm_sent_at,
            trade_confirm_received: req.trade_confirm_received,
            trade_confirm_received_at: req.trade_confirm_received_at,
            special_terms: req.special_terms,
            representations_and_warranties: req.representations_and_warranties,
            conditions_precedent: req.conditions_precedent,
            post_closing_obligations: req.post_closing_obligations,
            internal_notes: req.internal_notes,
            trader_notes: req.trader_notes,
            compliance_notes: req.compliance_notes,
            market_segment: req.market_segment,
            credit_rating: req.credit_rating,
            sector: req.sector,
            yield_to_maturity: req.yield_to_maturity,
            spread_to_benchmark_bps: req.spread_to_benchmark_bps,
            seller_ssi_id,
            buyer_ssi_id,
            settlement_method: req.settlement_method,
            settlement_account_seller: req.settlement_account_seller,
            settlement_account_buyer: req.settlement_account_buyer,
            metadata: req.metadata,
            created_by,
        });
    }

    let total = db_requests.len();
    let results = create_secondary_trades_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("Batch create secondary trades", e))?;

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
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "batch_create")
        .record(duration.as_secs_f64());

    Ok(Json(BatchOperationResponse {
        success_count,
        error_count,
        total,
        successful_ids,
        errors,
    }))
}

/// Update secondary trade
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/secondary-trades/{id}",
    params(
        ("id" = String, Path, description = "Secondary trade UUID")
    ),
    request_body = UpdateSecondaryTradeApiRequest,
    responses(
        (status = 200, description = "Secondary trade updated", body = SecondaryTradeResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Secondary trade not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn update_secondary_trade_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateSecondaryTradeApiRequest>,
) -> Result<Json<SecondaryTradeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Updating secondary trade {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "update").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    // Get current user for audit trail
    let current_user = extract_current_user(&headers);
    let updated_by = current_user.as_ref().map(|u| u.id);

    let credit_agreement_id = if let Some(id_str) = request.credit_agreement_id {
        Some(parse_uuid(&id_str, "credit_agreement_id")?)
    } else {
        None
    };

    let seller_party_id = if let Some(id_str) = request.seller_party_id {
        Some(parse_uuid(&id_str, "seller_party_id")?)
    } else {
        None
    };

    let buyer_party_id = if let Some(id_str) = request.buyer_party_id {
        Some(parse_uuid(&id_str, "buyer_party_id")?)
    } else {
        None
    };

    let broker_party_id = if let Some(id_str) = request.broker_party_id {
        Some(parse_uuid(&id_str, "broker_party_id")?)
    } else {
        None
    };

    let seller_ssi_id = if let Some(id_str) = request.seller_ssi_id {
        Some(parse_uuid(&id_str, "seller_ssi_id")?)
    } else {
        None
    };

    let buyer_ssi_id = if let Some(id_str) = request.buyer_ssi_id {
        Some(parse_uuid(&id_str, "buyer_ssi_id")?)
    } else {
        None
    };

    let db_request = DbUpdateRequest {
        trade_id: request.trade_id,
        credit_agreement_id,
        trade_confirm_hash: request.trade_confirm_hash,
        instrument_id: request.instrument_id,
        instrument_type: request.instrument_type,
        seller_party_id,
        seller_legal_name: request.seller_legal_name,
        seller_role: request.seller_role,
        buyer_party_id,
        buyer_legal_name: request.buyer_legal_name,
        buyer_role: request.buyer_role,
        broker_party_id,
        broker_name: request.broker_name,
        broker_fee_amount: request.broker_fee_amount,
        broker_fee_currency: request.broker_fee_currency,
        trade_datetime: request.trade_datetime,
        trade_date: request.trade_date,
        trade_type: request.trade_type,
        trade_status: request.trade_status,
        effective_date: request.effective_date,
        settlement_date: request.settlement_date,
        settlement_days: request.settlement_days,
        actual_settlement_date: request.actual_settlement_date,
        notional_amount: request.notional_amount,
        notional_currency: request.notional_currency,
        price_type: request.price_type,
        price: request.price,
        price_in_dollars: request.price_in_dollars,
        accrued_interest_amount: request.accrued_interest_amount,
        accrued_interest_currency: request.accrued_interest_currency,
        accrued_interest_calculation_method: request.accrued_interest_calculation_method,
        accrued_interest_from_date: request.accrued_interest_from_date,
        accrued_interest_to_date: request.accrued_interest_to_date,
        total_consideration: request.total_consideration,
        total_consideration_currency: request.total_consideration_currency,
        transfer_fee_amount: request.transfer_fee_amount,
        transfer_fee_currency: request.transfer_fee_currency,
        transfer_fee_paid_by: request.transfer_fee_paid_by,
        other_fees: request.other_fees,
        eligibility_checks: request.eligibility_checks,
        kyc_checks_passed: request.kyc_checks_passed,
        aml_checks_passed: request.aml_checks_passed,
        sanctions_screening_passed: request.sanctions_screening_passed,
        transfer_restrictions_apply: request.transfer_restrictions_apply,
        transfer_consent_required: request.transfer_consent_required,
        transfer_consent_obtained: request.transfer_consent_obtained,
        transfer_consent_date: request.transfer_consent_date,
        transfer_consent_obtained_from: request.transfer_consent_obtained_from,
        allocation_id: request.allocation_id,
        allocation_percentage: request.allocation_percentage,
        pro_rata_share: request.pro_rata_share,
        docs_required: request.docs_required,
        docs_received_count: request.docs_received_count,
        docs_required_count: request.docs_required_count,
        docs_complete: request.docs_complete,
        trade_confirm_sent: request.trade_confirm_sent,
        trade_confirm_sent_at: request.trade_confirm_sent_at,
        trade_confirm_received: request.trade_confirm_received,
        trade_confirm_received_at: request.trade_confirm_received_at,
        special_terms: request.special_terms,
        representations_and_warranties: request.representations_and_warranties,
        conditions_precedent: request.conditions_precedent,
        post_closing_obligations: request.post_closing_obligations,
        internal_notes: request.internal_notes,
        trader_notes: request.trader_notes,
        compliance_notes: request.compliance_notes,
        market_segment: request.market_segment,
        credit_rating: request.credit_rating,
        sector: request.sector,
        yield_to_maturity: request.yield_to_maturity,
        spread_to_benchmark_bps: request.spread_to_benchmark_bps,
        seller_ssi_id,
        buyer_ssi_id,
        settlement_method: request.settlement_method,
        settlement_account_seller: request.settlement_account_seller,
        settlement_account_buyer: request.settlement_account_buyer,
        metadata: request.metadata,
        updated_by,
    };

    let record = update_secondary_trade(&state.pool, uuid, db_request, updated_by)
        .await
        .map_err(|e| internal_error("Update secondary trade", e))?;

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "update")
        .record(duration.as_secs_f64());

    Ok(Json(db_to_api(record)))
}

/// Delete secondary trade
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/secondary-trades/{id}",
    params(
        ("id" = String, Path, description = "Secondary trade UUID")
    ),
    responses(
        (status = 200, description = "Secondary trade deleted", body = DeleteResponse),
        (status = 404, description = "Secondary trade not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Secondary Trades"
)]
#[instrument(skip(state))]
pub async fn delete_secondary_trade_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    info!("🔍 API: Deleting secondary trade {}", id);
    metrics::counter!("taxonomy_operations_total", "table" => "secondary_trades", "operation" => "delete").increment(1);

    let uuid = parse_uuid(&id, "id")?;

    let deleted = delete_secondary_trade(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("Delete secondary trade", e))?;

    if !deleted {
        return Err(not_found_error("Secondary trade", &id));
    }

    let duration = start_time.elapsed();
    metrics::histogram!("taxonomy_operation_duration", "table" => "secondary_trades", "operation" => "delete")
        .record(duration.as_secs_f64());

    Ok(Json(DeleteResponse {
        id: id.clone(),
        success: true,
        message: format!("Secondary trade {} deleted successfully", id),
    }))
}
