//! Secondary Trades Database Operations
//!
//! Provides CRUD operations for secondary trade documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Secondary Trade record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SecondaryTrade {
    pub id: Uuid,
    pub trade_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_confirm_hash: String,
    pub instrument_id: String,
    pub instrument_type: Option<String>,
    pub seller_party_id: Uuid,
    pub seller_legal_name: String,
    pub seller_role: Option<String>,
    pub buyer_party_id: Uuid,
    pub buyer_legal_name: String,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<Uuid>,
    pub broker_name: Option<String>,
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
    pub notional_amount: Decimal,
    pub notional_currency: Option<String>,
    pub price_type: String,
    pub price: Decimal,
    pub price_in_dollars: Option<Decimal>,
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    pub total_consideration: Decimal,
    pub total_consideration_currency: Option<String>,
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
    pub yield_to_maturity: Option<Decimal>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<Uuid>,
    pub buyer_ssi_id: Option<Uuid>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub confirmed_by: Option<Uuid>,
    pub settled_by: Option<Uuid>,
}

/// Request to create a secondary trade
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSecondaryTradeRequest {
    pub trade_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_confirm_hash: String,
    pub instrument_id: String,
    pub instrument_type: Option<String>,
    pub seller_party_id: Uuid,
    pub seller_legal_name: String,
    pub seller_role: Option<String>,
    pub buyer_party_id: Uuid,
    pub buyer_legal_name: String,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<Uuid>,
    pub broker_name: Option<String>,
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
    pub notional_amount: Decimal,
    pub notional_currency: Option<String>,
    pub price_type: String,
    pub price: Decimal,
    pub price_in_dollars: Option<Decimal>,
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    pub total_consideration: Decimal,
    pub total_consideration_currency: Option<String>,
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
    pub yield_to_maturity: Option<Decimal>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<Uuid>,
    pub buyer_ssi_id: Option<Uuid>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a secondary trade
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSecondaryTradeRequest {
    pub trade_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_confirm_hash: Option<String>,
    pub instrument_id: Option<String>,
    pub instrument_type: Option<String>,
    pub seller_party_id: Option<Uuid>,
    pub seller_legal_name: Option<String>,
    pub seller_role: Option<String>,
    pub buyer_party_id: Option<Uuid>,
    pub buyer_legal_name: Option<String>,
    pub buyer_role: Option<String>,
    pub broker_party_id: Option<Uuid>,
    pub broker_name: Option<String>,
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
    pub notional_amount: Option<Decimal>,
    pub notional_currency: Option<String>,
    pub price_type: Option<String>,
    pub price: Option<Decimal>,
    pub price_in_dollars: Option<Decimal>,
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub accrued_interest_calculation_method: Option<String>,
    pub accrued_interest_from_date: Option<NaiveDate>,
    pub accrued_interest_to_date: Option<NaiveDate>,
    pub total_consideration: Option<Decimal>,
    pub total_consideration_currency: Option<String>,
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
    pub yield_to_maturity: Option<Decimal>,
    pub spread_to_benchmark_bps: Option<i32>,
    pub seller_ssi_id: Option<Uuid>,
    pub buyer_ssi_id: Option<Uuid>,
    pub settlement_method: Option<String>,
    pub settlement_account_seller: Option<String>,
    pub settlement_account_buyer: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new secondary trade
#[instrument(skip(pool))]
pub async fn create_secondary_trade(
    pool: &PgPool,
    req: CreateSecondaryTradeRequest,
) -> Result<SecondaryTrade, DbError> {
    let id = Uuid::new_v4();

    let record = sqlx::query_as!(
        SecondaryTrade,
        r#"
        INSERT INTO secondary_trades (
            id, trade_id, credit_agreement_id, trade_confirm_hash, instrument_id, instrument_type,
            seller_party_id, seller_legal_name, seller_role, buyer_party_id, buyer_legal_name, buyer_role,
            broker_party_id, broker_name, broker_fee_amount, broker_fee_currency,
            trade_datetime, trade_date, trade_type, trade_status, effective_date, settlement_date, settlement_days, actual_settlement_date,
            notional_amount, notional_currency, price_type, price, price_in_dollars,
            accrued_interest_amount, accrued_interest_currency, accrued_interest_calculation_method,
            accrued_interest_from_date, accrued_interest_to_date,
            total_consideration, total_consideration_currency,
            transfer_fee_amount, transfer_fee_currency, transfer_fee_paid_by,
            other_fees, eligibility_checks,
            kyc_checks_passed, aml_checks_passed, sanctions_screening_passed,
            transfer_restrictions_apply, transfer_consent_required, transfer_consent_obtained,
            transfer_consent_date, transfer_consent_obtained_from,
            allocation_id, allocation_percentage, pro_rata_share,
            docs_required, docs_received_count, docs_required_count, docs_complete,
            trade_confirm_sent, trade_confirm_sent_at, trade_confirm_received, trade_confirm_received_at,
            special_terms, representations_and_warranties, conditions_precedent, post_closing_obligations,
            internal_notes, trader_notes, compliance_notes,
            market_segment, credit_rating, sector, yield_to_maturity, spread_to_benchmark_bps,
            seller_ssi_id, buyer_ssi_id, settlement_method, settlement_account_seller, settlement_account_buyer,
            metadata, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38,
            $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56,
            $57, $58, $59, $60, $61, $62, $63, $64, $65, $66, $67, $68, $69, $70, $71, $72, $73, $74,
            $75, $76, $77, $78, $79
        )
        RETURNING *
        "#,
        id,
        req.trade_id,
        req.credit_agreement_id,
        req.trade_confirm_hash,
        req.instrument_id,
        req.instrument_type,
        req.seller_party_id,
        req.seller_legal_name,
        req.seller_role,
        req.buyer_party_id,
        req.buyer_legal_name,
        req.buyer_role,
        req.broker_party_id,
        req.broker_name,
        req.broker_fee_amount,
        req.broker_fee_currency,
        req.trade_datetime,
        req.trade_date,
        req.trade_type,
        req.trade_status,
        req.effective_date,
        req.settlement_date,
        req.settlement_days,
        req.actual_settlement_date,
        req.notional_amount,
        req.notional_currency,
        req.price_type,
        req.price,
        req.price_in_dollars,
        req.accrued_interest_amount,
        req.accrued_interest_currency,
        req.accrued_interest_calculation_method,
        req.accrued_interest_from_date,
        req.accrued_interest_to_date,
        req.total_consideration,
        req.total_consideration_currency,
        req.transfer_fee_amount,
        req.transfer_fee_currency,
        req.transfer_fee_paid_by,
        req.other_fees,
        req.eligibility_checks,
        req.kyc_checks_passed,
        req.aml_checks_passed,
        req.sanctions_screening_passed,
        req.transfer_restrictions_apply,
        req.transfer_consent_required,
        req.transfer_consent_obtained,
        req.transfer_consent_date,
        req.transfer_consent_obtained_from,
        req.allocation_id,
        req.allocation_percentage,
        req.pro_rata_share,
        req.docs_required,
        req.docs_received_count,
        req.docs_required_count,
        req.docs_complete,
        req.trade_confirm_sent,
        req.trade_confirm_sent_at,
        req.trade_confirm_received,
        req.trade_confirm_received_at,
        req.special_terms,
        req.representations_and_warranties,
        req.conditions_precedent,
        req.post_closing_obligations,
        req.internal_notes,
        req.trader_notes,
        req.compliance_notes,
        req.market_segment,
        req.credit_rating,
        req.sector,
        req.yield_to_maturity,
        req.spread_to_benchmark_bps,
        req.seller_ssi_id,
        req.buyer_ssi_id,
        req.settlement_method,
        req.settlement_account_seller,
        req.settlement_account_buyer,
        req.metadata,
        req.created_by
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get secondary trade by ID
#[instrument(skip(pool))]
pub async fn get_secondary_trade_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<SecondaryTrade>, DbError> {
    let record = sqlx::query_as!(
        SecondaryTrade,
        r#"
        SELECT * FROM secondary_trades WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get secondary trade by trade ID
#[instrument(skip(pool))]
pub async fn get_secondary_trade_by_trade_id(
    pool: &PgPool,
    trade_id: &str,
) -> Result<Option<SecondaryTrade>, DbError> {
    let record = sqlx::query_as!(
        SecondaryTrade,
        r#"
        SELECT * FROM secondary_trades WHERE trade_id = $1
        "#,
        trade_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List secondary trades with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_secondary_trades(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    trade_status: Option<String>,
    trade_type: Option<String>,
    seller_party_id: Option<Uuid>,
    buyer_party_id: Option<Uuid>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<SecondaryTrade>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_index = 3;

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", param_index));
        param_index += 1;
    }
    if trade_status.is_some() {
        where_clauses.push(format!("trade_status = ${}", param_index));
        param_index += 1;
    }
    if trade_type.is_some() {
        where_clauses.push(format!("trade_type = ${}", param_index));
        param_index += 1;
    }
    if seller_party_id.is_some() {
        where_clauses.push(format!("seller_party_id = ${}", param_index));
        param_index += 1;
    }
    if buyer_party_id.is_some() {
        where_clauses.push(format!("buyer_party_id = ${}", param_index));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("trade_id") => "trade_id",
        Some("trade_date") => "trade_date",
        Some("settlement_date") => "settlement_date",
        Some("trade_datetime") => "trade_datetime",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM secondary_trades {}",
        where_clause
    );

    let mut count_q = sqlx::query_scalar(&count_query);
    if credit_agreement_id.is_some() {
        count_q = count_q.bind(&credit_agreement_id);
    }
    if trade_status.is_some() {
        count_q = count_q.bind(&trade_status);
    }
    if trade_type.is_some() {
        count_q = count_q.bind(&trade_type);
    }
    if seller_party_id.is_some() {
        count_q = count_q.bind(&seller_party_id);
    }
    if buyer_party_id.is_some() {
        count_q = count_q.bind(&buyer_party_id);
    }

    let total: i64 = count_q.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM secondary_trades {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut records_q = sqlx::query_as::<_, SecondaryTrade>(&query)
        .bind(limit)
        .bind(offset);

    if credit_agreement_id.is_some() {
        records_q = records_q.bind(&credit_agreement_id);
    }
    if trade_status.is_some() {
        records_q = records_q.bind(&trade_status);
    }
    if trade_type.is_some() {
        records_q = records_q.bind(&trade_type);
    }
    if seller_party_id.is_some() {
        records_q = records_q.bind(&seller_party_id);
    }
    if buyer_party_id.is_some() {
        records_q = records_q.bind(&buyer_party_id);
    }

    let records = records_q.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update secondary trade
#[instrument(skip(pool))]
pub async fn update_secondary_trade(
    pool: &PgPool,
    id: Uuid,
    req: UpdateSecondaryTradeRequest,
    updated_by: Option<Uuid>,
) -> Result<SecondaryTrade, DbError> {
    // Get existing record
    let existing = get_secondary_trade_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Secondary trade {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        SecondaryTrade,
        r#"
        UPDATE secondary_trades SET
            trade_id = $2,
            credit_agreement_id = $3,
            trade_confirm_hash = $4,
            instrument_id = $5,
            instrument_type = $6,
            seller_party_id = $7,
            seller_legal_name = $8,
            seller_role = $9,
            buyer_party_id = $10,
            buyer_legal_name = $11,
            buyer_role = $12,
            broker_party_id = $13,
            broker_name = $14,
            broker_fee_amount = $15,
            broker_fee_currency = $16,
            trade_datetime = $17,
            trade_date = $18,
            trade_type = $19,
            trade_status = $20,
            effective_date = $21,
            settlement_date = $22,
            settlement_days = $23,
            actual_settlement_date = $24,
            notional_amount = $25,
            notional_currency = $26,
            price_type = $27,
            price = $28,
            price_in_dollars = $29,
            accrued_interest_amount = $30,
            accrued_interest_currency = $31,
            accrued_interest_calculation_method = $32,
            accrued_interest_from_date = $33,
            accrued_interest_to_date = $34,
            total_consideration = $35,
            total_consideration_currency = $36,
            transfer_fee_amount = $37,
            transfer_fee_currency = $38,
            transfer_fee_paid_by = $39,
            other_fees = $40,
            eligibility_checks = $41,
            kyc_checks_passed = $42,
            aml_checks_passed = $43,
            sanctions_screening_passed = $44,
            transfer_restrictions_apply = $45,
            transfer_consent_required = $46,
            transfer_consent_obtained = $47,
            transfer_consent_date = $48,
            transfer_consent_obtained_from = $49,
            allocation_id = $50,
            allocation_percentage = $51,
            pro_rata_share = $52,
            docs_required = $53,
            docs_received_count = $54,
            docs_required_count = $55,
            docs_complete = $56,
            trade_confirm_sent = $57,
            trade_confirm_sent_at = $58,
            trade_confirm_received = $59,
            trade_confirm_received_at = $60,
            special_terms = $61,
            representations_and_warranties = $62,
            conditions_precedent = $63,
            post_closing_obligations = $64,
            internal_notes = $65,
            trader_notes = $66,
            compliance_notes = $67,
            market_segment = $68,
            credit_rating = $69,
            sector = $70,
            yield_to_maturity = $71,
            spread_to_benchmark_bps = $72,
            seller_ssi_id = $73,
            buyer_ssi_id = $74,
            settlement_method = $75,
            settlement_account_seller = $76,
            settlement_account_buyer = $77,
            metadata = $78,
            updated_by = $79
        WHERE id = $1
        RETURNING *
        "#,
        id,
        req.trade_id.unwrap_or(existing.trade_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.trade_confirm_hash
            .unwrap_or(existing.trade_confirm_hash),
        req.instrument_id.unwrap_or(existing.instrument_id),
        req.instrument_type.or(existing.instrument_type),
        req.seller_party_id.unwrap_or(existing.seller_party_id),
        req.seller_legal_name.unwrap_or(existing.seller_legal_name),
        req.seller_role.or(existing.seller_role),
        req.buyer_party_id.unwrap_or(existing.buyer_party_id),
        req.buyer_legal_name.unwrap_or(existing.buyer_legal_name),
        req.buyer_role.or(existing.buyer_role),
        req.broker_party_id.or(existing.broker_party_id),
        req.broker_name.or(existing.broker_name),
        req.broker_fee_amount.or(existing.broker_fee_amount),
        req.broker_fee_currency.or(existing.broker_fee_currency),
        req.trade_datetime.unwrap_or(existing.trade_datetime),
        req.trade_date.unwrap_or(existing.trade_date),
        req.trade_type.unwrap_or(existing.trade_type),
        req.trade_status.or(existing.trade_status),
        req.effective_date.unwrap_or(existing.effective_date),
        req.settlement_date.unwrap_or(existing.settlement_date),
        req.settlement_days.or(existing.settlement_days),
        req.actual_settlement_date
            .or(existing.actual_settlement_date),
        req.notional_amount.unwrap_or(existing.notional_amount),
        req.notional_currency.or(existing.notional_currency),
        req.price_type.unwrap_or(existing.price_type),
        req.price.unwrap_or(existing.price),
        req.price_in_dollars.or(existing.price_in_dollars),
        req.accrued_interest_amount
            .or(existing.accrued_interest_amount),
        req.accrued_interest_currency
            .or(existing.accrued_interest_currency),
        req.accrued_interest_calculation_method
            .or(existing.accrued_interest_calculation_method),
        req.accrued_interest_from_date
            .or(existing.accrued_interest_from_date),
        req.accrued_interest_to_date
            .or(existing.accrued_interest_to_date),
        req.total_consideration
            .unwrap_or(existing.total_consideration),
        req.total_consideration_currency
            .or(existing.total_consideration_currency),
        req.transfer_fee_amount.or(existing.transfer_fee_amount),
        req.transfer_fee_currency.or(existing.transfer_fee_currency),
        req.transfer_fee_paid_by.or(existing.transfer_fee_paid_by),
        req.other_fees.or(existing.other_fees),
        req.eligibility_checks.or(existing.eligibility_checks),
        req.kyc_checks_passed.or(existing.kyc_checks_passed),
        req.aml_checks_passed.or(existing.aml_checks_passed),
        req.sanctions_screening_passed
            .or(existing.sanctions_screening_passed),
        req.transfer_restrictions_apply
            .or(existing.transfer_restrictions_apply),
        req.transfer_consent_required
            .or(existing.transfer_consent_required),
        req.transfer_consent_obtained
            .or(existing.transfer_consent_obtained),
        req.transfer_consent_date.or(existing.transfer_consent_date),
        req.transfer_consent_obtained_from
            .or(existing.transfer_consent_obtained_from),
        req.allocation_id.or(existing.allocation_id),
        req.allocation_percentage.or(existing.allocation_percentage),
        req.pro_rata_share.or(existing.pro_rata_share),
        req.docs_required.or(existing.docs_required),
        req.docs_received_count.or(existing.docs_received_count),
        req.docs_required_count.or(existing.docs_required_count),
        req.docs_complete.or(existing.docs_complete),
        req.trade_confirm_sent.or(existing.trade_confirm_sent),
        req.trade_confirm_sent_at.or(existing.trade_confirm_sent_at),
        req.trade_confirm_received
            .or(existing.trade_confirm_received),
        req.trade_confirm_received_at
            .or(existing.trade_confirm_received_at),
        req.special_terms.or(existing.special_terms),
        req.representations_and_warranties
            .or(existing.representations_and_warranties),
        req.conditions_precedent.or(existing.conditions_precedent),
        req.post_closing_obligations
            .or(existing.post_closing_obligations),
        req.internal_notes.or(existing.internal_notes),
        req.trader_notes.or(existing.trader_notes),
        req.compliance_notes.or(existing.compliance_notes),
        req.market_segment.or(existing.market_segment),
        req.credit_rating.or(existing.credit_rating),
        req.sector.or(existing.sector),
        req.yield_to_maturity.or(existing.yield_to_maturity),
        req.spread_to_benchmark_bps
            .or(existing.spread_to_benchmark_bps),
        req.seller_ssi_id.or(existing.seller_ssi_id),
        req.buyer_ssi_id.or(existing.buyer_ssi_id),
        req.settlement_method.or(existing.settlement_method),
        req.settlement_account_seller
            .or(existing.settlement_account_seller),
        req.settlement_account_buyer
            .or(existing.settlement_account_buyer),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete secondary trade
#[instrument(skip(pool))]
pub async fn delete_secondary_trade(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM secondary_trades WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple secondary trades in a batch
#[instrument(skip(pool, requests))]
pub async fn create_secondary_trades_batch(
    pool: &PgPool,
    requests: Vec<CreateSecondaryTradeRequest>,
) -> Result<Vec<Result<SecondaryTrade, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_secondary_trade(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
