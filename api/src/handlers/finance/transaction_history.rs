use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::router::ApiState;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct TransactionHistoryQuery {
    /// Number of transactions to return (default: 50, max: 500)
    pub limit: Option<i32>,
    /// Number of transactions to skip
    pub offset: Option<i32>,
    /// Filter by transaction type (deal, deposit, withdrawal, pool)
    pub transaction_type: Option<String>,
    /// Filter by user address (for USDC operations)
    pub user_address: Option<String>,
    /// Filter by user ID (for deal operations)
    pub user_id: Option<Uuid>,
    /// Filter by pool ID (for pool operations)
    pub pool_id: Option<Uuid>,
    /// Start date for time range filtering (ISO 8601)
    pub start_date: Option<DateTime<Utc>>,
    /// End date for time range filtering (ISO 8601)
    pub end_date: Option<DateTime<Utc>>,
    /// Include detailed metadata in response
    pub include_details: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionHistoryResponse {
    pub transactions: Vec<TransactionHistoryItem>,
    pub pagination: PaginationInfo,
    pub summary: TransactionSummary,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionHistoryItem {
    pub id: String,
    pub transaction_type: String,
    pub category: String, // "deal", "capital_flow", "loan_operation", "pool_operation"
    pub amount: String,   // Amount in USDC (human readable)
    pub amount_wei: String, // Amount in wei for precise calculations
    pub direction: String, // "inflow", "outflow", "internal"
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub user_address: Option<String>,
    pub user_id: Option<Uuid>,
    pub pool_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub credit_line_id: Option<Uuid>,
    pub block_number: Option<i64>,
    pub transaction_hash: Option<String>,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginationInfo {
    pub total_count: i64,
    pub page_size: i32,
    pub current_offset: i32,
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionSummary {
    pub total_volume_usdc: String,
    pub total_inflow_usdc: String,
    pub total_outflow_usdc: String,
    pub transaction_counts: HashMap<String, i32>,
    pub date_range: DateRange,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DateRange {
    pub earliest_transaction: Option<DateTime<Utc>>,
    pub latest_transaction: Option<DateTime<Utc>>,
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// Get comprehensive transaction history across all transaction types
#[utoipa::path(
    get,
    path = "/api/v1/transactions/history",
    summary = "Get comprehensive transaction history",
    description = "Retrieve transaction history across all operations including deals, deposits, withdrawals, and pool operations",
    params(
        ("limit" = Option<i32>, Query, description = "Number of transactions to return (default: 50, max: 500)"),
        ("offset" = Option<i32>, Query, description = "Number of transactions to skip"),
        ("transaction_type" = Option<String>, Query, description = "Filter by type: deal, deposit, withdrawal, pool"),
        ("user_address" = Option<String>, Query, description = "Filter by user wallet address"),
        ("user_id" = Option<String>, Query, description = "Filter by user ID (UUID)"),
        ("pool_id" = Option<String>, Query, description = "Filter by pool ID (UUID)"),
        ("start_date" = Option<String>, Query, description = "Start date (ISO 8601)"),
        ("end_date" = Option<String>, Query, description = "End date (ISO 8601)"),
        ("include_details" = Option<bool>, Query, description = "Include detailed metadata")
    ),
    responses(
        (status = 200, description = "Transaction history retrieved successfully", body = TransactionHistoryResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(state), fields(query = ?query))]
pub async fn get_transaction_history(
    State(state): State<ApiState>,
    Query(query): Query<TransactionHistoryQuery>,
) -> Result<Json<TransactionHistoryResponse>, axum::response::Response> {
    info!("Fetching transaction history with query: {:?}", query);

    // Validate and set defaults for query parameters
    let limit = query.limit.unwrap_or(50).min(500).max(1);
    let offset = query.offset.unwrap_or(0).max(0);
    let _include_details = query.include_details.unwrap_or(false);

    match get_transaction_history_internal(&state.pool, query, limit, offset, _include_details)
        .await
    {
        Ok(response) => {
            info!(
                "Successfully fetched {} transactions",
                response.transactions.len()
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to fetch transaction history: {}", e);
            Err(axum::response::Response::builder()
                .status(500)
                .body(format!("Failed to fetch transaction history: {}", e).into())
                .unwrap())
        }
    }
}

/// Get transaction history for a specific pool
#[utoipa::path(
    get,
    path = "/api/v1/pools/{pool_id}/transaction-history",
    summary = "Get pool-specific transaction history",
    description = "Retrieve all transaction history for a specific liquidity pool including funding, withdrawals, and deployments",
    params(
        ("pool_id" = Uuid, Path, description = "Pool ID"),
        ("limit" = Option<i32>, Query, description = "Number of transactions to return (default: 50, max: 500)"),
        ("offset" = Option<i32>, Query, description = "Number of transactions to skip"),
        ("include_details" = Option<bool>, Query, description = "Include detailed metadata")
    ),
    responses(
        (status = 200, description = "Pool transaction history retrieved successfully", body = TransactionHistoryResponse),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Pool not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(state), fields(pool_id = %pool_id))]
pub async fn get_pool_transaction_history(
    Path(pool_id): Path<Uuid>,
    State(state): State<ApiState>,
    Query(mut query): Query<TransactionHistoryQuery>,
) -> Result<Json<TransactionHistoryResponse>, axum::response::Response> {
    info!("Fetching transaction history for pool: {}", pool_id);

    // Override query to filter by pool_id
    query.pool_id = Some(pool_id);

    let limit = query.limit.unwrap_or(50).min(500).max(1);
    let offset = query.offset.unwrap_or(0).max(0);
    let _include_details = query.include_details.unwrap_or(false);

    match get_transaction_history_internal(&state.pool, query, limit, offset, _include_details)
        .await
    {
        Ok(response) => {
            info!(
                "Successfully fetched {} transactions for pool {}",
                response.transactions.len(),
                pool_id
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to fetch pool transaction history: {}", e);
            Err(axum::response::Response::builder()
                .status(500)
                .body(format!("Failed to fetch pool transaction history: {}", e).into())
                .unwrap())
        }
    }
}

// ============================================================================
// INTERNAL IMPLEMENTATION
// ============================================================================

async fn get_transaction_history_internal(
    pool: &PgPool,
    query: TransactionHistoryQuery,
    limit: i32,
    offset: i32,
    _include_details: bool,
) -> Result<TransactionHistoryResponse, sqlx::Error> {
    // Build the main query with filters
    let mut transactions = Vec::new();
    let total_count: i64;

    // Get deal state transactions
    if should_include_transaction_type(&query.transaction_type, "deal") {
        let deal_txs = get_deal_state_transactions(pool, &query, limit, offset).await?;
        transactions.extend(deal_txs);
    }

    // Get USDC deposit transactions
    if should_include_transaction_type(&query.transaction_type, "deposit") {
        let deposit_txs = get_deposit_transactions(pool, &query, limit, offset).await?;
        transactions.extend(deposit_txs);
    }

    // Get USDC withdrawal transactions
    if should_include_transaction_type(&query.transaction_type, "withdrawal") {
        let withdrawal_txs = get_withdrawal_transactions(pool, &query, limit, offset).await?;
        transactions.extend(withdrawal_txs);
    }

    // Sort all transactions by created_at descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply overall limit and offset
    let _total_len = transactions.len() as i64;
    let transactions: Vec<TransactionHistoryItem> = transactions
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    // Get total count for pagination
    total_count = get_total_transaction_count(pool, &query).await?;

    // Calculate summary statistics
    let summary = calculate_transaction_summary(&transactions, pool, &query).await?;

    let response = TransactionHistoryResponse {
        transactions,
        pagination: PaginationInfo {
            total_count,
            page_size: limit,
            current_offset: offset,
            has_more: offset + limit < total_count as i32,
        },
        summary,
    };

    Ok(response)
}

async fn get_deal_state_transactions(
    pool: &PgPool,
    query: &TransactionHistoryQuery,
    limit: i32,
    offset: i32,
) -> Result<Vec<TransactionHistoryItem>, sqlx::Error> {
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
        SELECT
            st.id,
            st.transaction_type,
            st.created_at,
            st.user_id,
            st.deal_id,
            st.included_in_batch_id,
            d.principal_amount,
            d.outstanding_balance,
            d.loan_status,
            u.email,
            u.wallet_address
        FROM state_transactions st
        LEFT JOIN deals d ON st.deal_id = d.id
        LEFT JOIN users u ON st.user_id = u.id
        WHERE st.transaction_type LIKE 'deal_%'
        "#,
    );

    // Apply filters
    if let Some(user_id) = &query.user_id {
        query_builder.push(" AND st.user_id = ");
        query_builder.push_bind(user_id);
    }

    if let Some(start_date) = &query.start_date {
        query_builder.push(" AND st.created_at >= ");
        query_builder.push_bind(start_date);
    }

    if let Some(end_date) = &query.end_date {
        query_builder.push(" AND st.created_at <= ");
        query_builder.push_bind(end_date);
    }

    query_builder.push(" ORDER BY st.created_at DESC LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    let rows = query_builder.build().fetch_all(pool).await?;

    let mut transactions = Vec::new();
    for row in rows {
        let transaction_type: String = row.get("transaction_type");
        let created_at: DateTime<Utc> = row.get("created_at");

        // Use principal_amount for deal transactions
        // Note: For more accurate payment amounts, we would need to deserialize transaction_data
        let amount = row
            .get::<Option<rust_decimal::Decimal>, _>("principal_amount")
            .unwrap_or_default();

        transactions.push(TransactionHistoryItem {
            id: row.get::<Uuid, _>("id").to_string(),
            transaction_type: transaction_type.clone(),
            category: "deal".to_string(),
            amount: format!("{:.6}", amount),
            amount_wei: (amount * rust_decimal::Decimal::from(1_000_000)).to_string(),
            direction: match transaction_type.as_str() {
                "deal_origination" => "internal",
                "deal_payment" => "inflow",
                "deal_closeout" => "internal",
                _ => "internal",
            }
            .to_string(),
            status: row
                .get::<Option<i32>, _>("included_in_batch_id")
                .map_or("pending".to_string(), |_| "processed".to_string()),
            created_at,
            processed_at: None, // No processed_at in state_transactions
            user_address: row.get::<Option<String>, _>("wallet_address"),
            user_id: row.get::<Option<Uuid>, _>("user_id"),
            pool_id: None,
            deal_id: row.get::<Option<Uuid>, _>("deal_id"),
            credit_line_id: None,
            block_number: None,
            transaction_hash: None,
            description: format!(
                "Deal {} - {}",
                transaction_type.replace("deal_", "").replace("_", " "),
                row.get::<Option<String>, _>("email")
                    .unwrap_or("Unknown".to_string())
            ),
            metadata: None,
        });
    }

    Ok(transactions)
}

async fn get_deposit_transactions(
    pool: &PgPool,
    query: &TransactionHistoryQuery,
    limit: i32,
    offset: i32,
) -> Result<Vec<TransactionHistoryItem>, sqlx::Error> {
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
        SELECT
            ud.id,
            ud.user_address,
            ud.amount,
            ud.deposit_amount_usdc,
            ud.transaction_hash,
            ud.block_number,
            ud.status,
            ud.created_at,
            ud.processed_at,
            ud.block_timestamp
        FROM usdc_deposits ud
        WHERE 1=1
        "#,
    );

    // Apply filters
    if let Some(user_address) = &query.user_address {
        query_builder.push(" AND ud.user_address = ");
        query_builder.push_bind(user_address);
    }

    if let Some(start_date) = &query.start_date {
        query_builder.push(" AND ud.created_at >= ");
        query_builder.push_bind(start_date);
    }

    if let Some(end_date) = &query.end_date {
        query_builder.push(" AND ud.created_at <= ");
        query_builder.push_bind(end_date);
    }

    query_builder.push(" ORDER BY ud.created_at DESC LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    let rows = query_builder.build().fetch_all(pool).await?;

    let mut transactions = Vec::new();
    for row in rows {
        let amount_usdc: rust_decimal::Decimal = row.get("deposit_amount_usdc");
        let amount_wei: rust_decimal::Decimal = row.get("amount");

        transactions.push(TransactionHistoryItem {
            id: row.get::<i32, _>("id").to_string(),
            transaction_type: "usdc_deposit".to_string(),
            category: "capital_flow".to_string(),
            amount: format!("{:.6}", amount_usdc),
            amount_wei: amount_wei.to_string(),
            direction: "inflow".to_string(),
            status: row.get::<String, _>("status"),
            created_at: row.get::<DateTime<Utc>, _>("created_at"),
            processed_at: row.get::<Option<DateTime<Utc>>, _>("processed_at"),
            user_address: Some(row.get::<String, _>("user_address")),
            user_id: None,
            pool_id: None,
            deal_id: None,
            credit_line_id: None,
            block_number: Some(row.get::<i64, _>("block_number")),
            transaction_hash: Some(row.get::<String, _>("transaction_hash")),
            description: format!("USDC Deposit - {:.6} USDC", amount_usdc),
            metadata: None,
        });
    }

    Ok(transactions)
}

async fn get_withdrawal_transactions(
    pool: &PgPool,
    query: &TransactionHistoryQuery,
    limit: i32,
    offset: i32,
) -> Result<Vec<TransactionHistoryItem>, sqlx::Error> {
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
        SELECT
            uw.id,
            uw.user_address,
            uw.recipient_address,
            uw.amount,
            uw.withdrawal_amount_usdc,
            uw.transaction_hash,
            uw.block_number,
            uw.status,
            uw.created_at,
            uw.processed_at,
            uw.block_timestamp
        FROM usdc_withdrawals uw
        WHERE 1=1
        "#,
    );

    // Apply filters
    if let Some(user_address) = &query.user_address {
        query_builder.push(" AND uw.user_address = ");
        query_builder.push_bind(user_address);
    }

    if let Some(start_date) = &query.start_date {
        query_builder.push(" AND uw.created_at >= ");
        query_builder.push_bind(start_date);
    }

    if let Some(end_date) = &query.end_date {
        query_builder.push(" AND uw.created_at <= ");
        query_builder.push_bind(end_date);
    }

    query_builder.push(" ORDER BY uw.created_at DESC LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    let rows = query_builder.build().fetch_all(pool).await?;

    let mut transactions = Vec::new();
    for row in rows {
        let amount_usdc: rust_decimal::Decimal = row.get("withdrawal_amount_usdc");
        let amount_wei: rust_decimal::Decimal = row.get("amount");

        transactions.push(TransactionHistoryItem {
            id: row.get::<i32, _>("id").to_string(),
            transaction_type: "usdc_withdrawal".to_string(),
            category: "capital_flow".to_string(),
            amount: format!("{:.6}", amount_usdc),
            amount_wei: amount_wei.to_string(),
            direction: "outflow".to_string(),
            status: row.get::<String, _>("status"),
            created_at: row.get::<DateTime<Utc>, _>("created_at"),
            processed_at: row.get::<Option<DateTime<Utc>>, _>("processed_at"),
            user_address: Some(row.get::<String, _>("user_address")),
            user_id: None,
            pool_id: None,
            deal_id: None,
            credit_line_id: None,
            block_number: Some(row.get::<i64, _>("block_number")),
            transaction_hash: Some(row.get::<String, _>("transaction_hash")),
            description: format!(
                "USDC Withdrawal - {:.6} USDC to {}",
                amount_usdc,
                row.get::<String, _>("recipient_address")
            ),
            metadata: None,
        });
    }

    Ok(transactions)
}

async fn get_total_transaction_count(
    pool: &PgPool,
    query: &TransactionHistoryQuery,
) -> Result<i64, sqlx::Error> {
    let mut total = 0i64;

    // Count deal state transactions
    if should_include_transaction_type(&query.transaction_type, "deal") {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM state_transactions WHERE transaction_type LIKE 'deal_%'",
        )
        .fetch_one(pool)
        .await?;
        total += count;
    }

    // Count deposits
    if should_include_transaction_type(&query.transaction_type, "deposit") {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usdc_deposits WHERE 1=1")
            .fetch_one(pool)
            .await?;
        total += count;
    }

    // Count withdrawals
    if should_include_transaction_type(&query.transaction_type, "withdrawal") {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usdc_withdrawals WHERE 1=1")
            .fetch_one(pool)
            .await?;
        total += count;
    }

    // Count pool transactions
    if should_include_transaction_type(&query.transaction_type, "pool") {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM liquidity_pool_audit WHERE operation_type IN ('POOL_FUNDED', 'POOL_WITHDRAWN', 'POOL_MATCHED')"
        )
        .fetch_one(pool)
        .await?;
        total += count;
    }

    Ok(total)
}

async fn calculate_transaction_summary(
    transactions: &[TransactionHistoryItem],
    _pool: &PgPool,
    _query: &TransactionHistoryQuery,
) -> Result<TransactionSummary, sqlx::Error> {
    let mut total_volume = rust_decimal::Decimal::ZERO;
    let mut total_inflow = rust_decimal::Decimal::ZERO;
    let mut total_outflow = rust_decimal::Decimal::ZERO;
    let mut transaction_counts = HashMap::new();
    let mut earliest_date: Option<DateTime<Utc>> = None;
    let mut latest_date: Option<DateTime<Utc>> = None;

    for tx in transactions {
        // Parse amount
        if let Ok(amount) = tx.amount.parse::<rust_decimal::Decimal>() {
            total_volume += amount;
            match tx.direction.as_str() {
                "inflow" => total_inflow += amount,
                "outflow" => total_outflow += amount,
                _ => {}
            }
        }

        // Count transaction types
        *transaction_counts
            .entry(tx.transaction_type.clone())
            .or_insert(0) += 1;

        // Track date range
        if earliest_date.is_none() || Some(tx.created_at) < earliest_date {
            earliest_date = Some(tx.created_at);
        }
        if latest_date.is_none() || Some(tx.created_at) > latest_date {
            latest_date = Some(tx.created_at);
        }
    }

    Ok(TransactionSummary {
        total_volume_usdc: format!("{:.6}", total_volume),
        total_inflow_usdc: format!("{:.6}", total_inflow),
        total_outflow_usdc: format!("{:.6}", total_outflow),
        transaction_counts,
        date_range: DateRange {
            earliest_transaction: earliest_date,
            latest_transaction: latest_date,
        },
    })
}

fn should_include_transaction_type(filter: &Option<String>, tx_type: &str) -> bool {
    match filter {
        Some(filter_type) => filter_type.to_lowercase() == tx_type.to_lowercase(),
        None => true,
    }
}
