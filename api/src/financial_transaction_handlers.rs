use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Json as JsonExtractor,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{rest::ApiState, ErrorResponse};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Query parameters for financial transaction endpoints
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct FinancialTransactionQuery {
    /// Maximum number of transactions to return (default: 50, max: 100)
    #[serde(default = "default_limit")]
    pub limit: Option<i64>,
    /// Number of transactions to skip (default: 0)
    #[serde(default)]
    pub offset: Option<i64>,
}

fn default_limit() -> Option<i64> {
    Some(50)
}

/// Create account financial transaction request
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateAccountFinancialTransactionRequest {
    /// User ID for the transaction
    pub user_id: String,
    /// Transaction type (DEPOSIT, WITHDRAWAL)
    pub transaction_type: String,
    /// Amount in USDC (human readable, e.g., "100.50")
    pub amount_usdc: String,
    /// Smart contract transaction hash
    pub transaction_hash: String,
    /// Blockchain status (PENDING, CONFIRMED, FAILED)
    pub blockchain_status: String,
    /// Block number (optional)
    pub block_number: Option<i64>,
}

/// Create pool financial transaction request
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreatePoolFinancialTransactionRequest {
    /// Pool ID
    pub pool_id: String,
    /// User ID
    pub user_id: String,
    /// Transaction type (FUND, WITHDRAW, INTEREST, RETURN)
    pub transaction_type: String,
    /// Amount in USDC (human readable, e.g., "100.50")
    pub amount_usdc: String,
    /// State transaction ID (optional, if linked to state transaction)
    pub state_transaction_id: Option<String>,
    /// Blockchain transaction hash (optional)
    pub transaction_hash: Option<String>,
    /// Description of the transaction
    pub description: Option<String>,
}

/// Create credit line financial transaction request
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCreditLineFinancialTransactionRequest {
    /// Credit line ID
    pub credit_line_id: String,
    /// Merchant ID
    pub merchant_id: String,
    /// Transaction type (DRAW, PAYMENT, INTEREST, FEE, PENALTY)
    pub transaction_type: String,
    /// Amount in USDC (human readable, e.g., "100.50")
    pub amount_usdc: String,
    /// State transaction ID (optional, if linked to state transaction)
    pub state_transaction_id: Option<String>,
    /// Blockchain transaction hash (optional)
    pub transaction_hash: Option<String>,
    /// Description of the transaction
    pub description: Option<String>,
}

/// Account financial transaction response
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountFinancialTransactionResponse {
    pub id: String,
    pub user_id: String,
    pub transaction_type: String,
    pub amount_usdc: String,
    pub currency: String,
    pub transaction_hash: String,
    pub blockchain_status: String,
    pub block_number: Option<i64>,
    pub processed_at: chrono::DateTime<chrono::Utc>,
}

/// Pool financial transaction response
#[derive(Debug, Serialize, ToSchema)]
pub struct PoolFinancialTransactionResponse {
    pub id: String,
    pub pool_id: String,
    pub user_id: String,
    pub transaction_type: String,
    pub amount_usdc: String,
    pub currency: String,
    pub state_transaction_id: Option<String>,
    pub transaction_hash: Option<String>,
    pub description: Option<String>,
    pub processed_at: chrono::DateTime<chrono::Utc>,
}

/// Credit line financial transaction response
#[derive(Debug, Serialize, ToSchema)]
pub struct CreditLineFinancialTransactionResponse {
    pub id: String,
    pub credit_line_id: String,
    pub merchant_id: String,
    pub transaction_type: String,
    pub amount_usdc: String,
    pub currency: String,
    pub state_transaction_id: Option<String>,
    pub transaction_hash: Option<String>,
    pub description: Option<String>,
    pub processed_at: chrono::DateTime<chrono::Utc>,
}

/// Unified financial transaction response
#[derive(Debug, Serialize, ToSchema)]
pub struct UnifiedFinancialTransactionResponse {
    pub transaction_category: String,
    pub id: String,
    pub user_id: String,
    pub transaction_type: String,
    pub amount_usdc: String,
    pub currency: String,
    pub transaction_hash: Option<String>,
    pub processed_at: chrono::DateTime<chrono::Utc>,
    pub pool_id: Option<String>,
    pub credit_line_id: Option<String>,
    pub description: String,
}

/// Financial summary response
#[derive(Debug, Serialize, ToSchema)]
pub struct FinancialSummaryResponse {
    pub total_deposits_usdc: String,
    pub total_withdrawals_usdc: String,
    pub net_account_flow_usdc: String,
    pub total_pool_funding_usdc: String,
    pub total_pool_withdrawals_usdc: String,
    pub total_credit_line_payments_usdc: String,
    pub total_credit_line_draws_usdc: String,
}

/// Paginated response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedFinancialTransactionsResponse<T> {
    pub transactions: Vec<T>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// POST /api/v1/user/{user_id}/financial/account/deposit - Record account deposit
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/financial/account/deposit",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    request_body = CreateAccountFinancialTransactionRequest,
    responses(
        (status = 201, description = "Deposit recorded successfully", body = AccountFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_account_deposit(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreateAccountFinancialTransactionRequest>,
) -> Result<Json<AccountFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "DEPOSIT" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be DEPOSIT for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Create database request
    let db_request = arda_credit_db::CreateAccountFinancialTransactionRequest {
        user_id: user_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        transaction_hash: request.transaction_hash.clone(),
        blockchain_status: request.blockchain_status.clone(),
        block_number: request.block_number,
    };

    match arda_credit_db::create_account_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                user_id = %user_id,
                amount = %request.amount_usdc,
                "Account deposit recorded successfully"
            );

            let response = AccountFinancialTransactionResponse {
                id: transaction.id.to_string(),
                user_id: transaction.user_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                transaction_hash: transaction.transaction_hash,
                blockchain_status: transaction.blockchain_status,
                block_number: transaction.block_number,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording account deposit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record account deposit",
                )),
            ))
        }
    }
}

/// POST /api/v1/user/{user_id}/financial/account/withdrawal - Record account withdrawal
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/financial/account/withdrawal",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    request_body = CreateAccountFinancialTransactionRequest,
    responses(
        (status = 201, description = "Withdrawal recorded successfully", body = AccountFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_account_withdrawal(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreateAccountFinancialTransactionRequest>,
) -> Result<Json<AccountFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "WITHDRAWAL" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be WITHDRAWAL for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Create database request
    let db_request = arda_credit_db::CreateAccountFinancialTransactionRequest {
        user_id: user_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        transaction_hash: request.transaction_hash.clone(),
        blockchain_status: request.blockchain_status.clone(),
        block_number: request.block_number,
    };

    match arda_credit_db::create_account_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                user_id = %user_id,
                amount = %request.amount_usdc,
                "Account withdrawal recorded successfully"
            );

            let response = AccountFinancialTransactionResponse {
                id: transaction.id.to_string(),
                user_id: transaction.user_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                transaction_hash: transaction.transaction_hash,
                blockchain_status: transaction.blockchain_status,
                block_number: transaction.block_number,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording account withdrawal: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record account withdrawal",
                )),
            ))
        }
    }
}

/// GET /api/v1/user/{user_id}/financial/account - Get account financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/financial/account",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Account transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<AccountFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_account_financial_transactions(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<AccountFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_user_account_financial_transactions(
        &state.pool,
        user_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<AccountFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| AccountFinancialTransactionResponse {
                    id: t.id.to_string(),
                    user_id: t.user_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    transaction_hash: t.transaction_hash,
                    blockchain_status: t.blockchain_status,
                    block_number: t.block_number,
                    processed_at: t.processed_at,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!(
                "Database error getting account financial transactions: {}",
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve account financial transactions",
                )),
            ))
        }
    }
}

/// POST /api/v1/pool/{pool_id}/financial/fund - Record pool funding
#[utoipa::path(
    post,
    path = "/api/v1/pool/{pool_id}/financial/fund",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    request_body = CreatePoolFinancialTransactionRequest,
    responses(
        (status = 201, description = "Pool funding recorded successfully", body = PoolFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "Pool not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_pool_funding(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreatePoolFinancialTransactionRequest>,
) -> Result<Json<PoolFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pool_uuid = match Uuid::parse_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_pool_id",
                    "Invalid pool ID format",
                )),
            ));
        }
    };

    let user_uuid = match Uuid::parse_str(&request.user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "FUND" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be FUND for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Parse optional state transaction ID
    let state_transaction_id = if let Some(ref state_tx_id) = request.state_transaction_id {
        Some(Uuid::parse_str(state_tx_id).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_state_transaction_id",
                    "Invalid state transaction ID format",
                )),
            )
        })?)
    } else {
        None
    };

    // Create database request
    let db_request = arda_credit_db::CreatePoolFinancialTransactionRequest {
        pool_id: pool_uuid,
        user_id: user_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        state_transaction_id,
        transaction_hash: request.transaction_hash.clone(),
        description: request.description.clone(),
    };

    match arda_credit_db::create_pool_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                pool_id = %pool_id,
                user_id = %request.user_id,
                amount = %request.amount_usdc,
                "Pool funding recorded successfully"
            );

            let response = PoolFinancialTransactionResponse {
                id: transaction.id.to_string(),
                pool_id: transaction.pool_id.to_string(),
                user_id: transaction.user_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                state_transaction_id: transaction.state_transaction_id.map(|id| id.to_string()),
                transaction_hash: transaction.transaction_hash,
                description: transaction.description,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording pool funding: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record pool funding",
                )),
            ))
        }
    }
}

/// POST /api/v1/pool/{pool_id}/financial/withdraw - Record pool withdrawal
#[utoipa::path(
    post,
    path = "/api/v1/pool/{pool_id}/financial/withdraw",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    request_body = CreatePoolFinancialTransactionRequest,
    responses(
        (status = 201, description = "Pool withdrawal recorded successfully", body = PoolFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "Pool not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_pool_withdrawal(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreatePoolFinancialTransactionRequest>,
) -> Result<Json<PoolFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pool_uuid = match Uuid::parse_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_pool_id",
                    "Invalid pool ID format",
                )),
            ));
        }
    };

    let user_uuid = match Uuid::parse_str(&request.user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "WITHDRAW" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be WITHDRAW for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Parse optional state transaction ID
    let state_transaction_id = if let Some(ref state_tx_id) = request.state_transaction_id {
        Some(Uuid::parse_str(state_tx_id).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_state_transaction_id",
                    "Invalid state transaction ID format",
                )),
            )
        })?)
    } else {
        None
    };

    // Create database request
    let db_request = arda_credit_db::CreatePoolFinancialTransactionRequest {
        pool_id: pool_uuid,
        user_id: user_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        state_transaction_id,
        transaction_hash: request.transaction_hash.clone(),
        description: request.description.clone(),
    };

    match arda_credit_db::create_pool_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                pool_id = %pool_id,
                user_id = %request.user_id,
                amount = %request.amount_usdc,
                "Pool withdrawal recorded successfully"
            );

            let response = PoolFinancialTransactionResponse {
                id: transaction.id.to_string(),
                pool_id: transaction.pool_id.to_string(),
                user_id: transaction.user_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                state_transaction_id: transaction.state_transaction_id.map(|id| id.to_string()),
                transaction_hash: transaction.transaction_hash,
                description: transaction.description,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording pool withdrawal: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record pool withdrawal",
                )),
            ))
        }
    }
}

/// GET /api/v1/pool/{pool_id}/financial/transactions - Get pool financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/pool/{pool_id}/financial/transactions",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Pool transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<PoolFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_pool_financial_transactions(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<PoolFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let pool_uuid = match Uuid::parse_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_pool_id",
                    "Invalid pool ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_pool_financial_transactions(
        &state.pool,
        pool_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<PoolFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| PoolFinancialTransactionResponse {
                    id: t.id.to_string(),
                    pool_id: t.pool_id.to_string(),
                    user_id: t.user_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    state_transaction_id: t.state_transaction_id.map(|id| id.to_string()),
                    transaction_hash: t.transaction_hash,
                    description: t.description,
                    processed_at: t.processed_at,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error getting pool financial transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool financial transactions",
                )),
            ))
        }
    }
}

/// POST /api/v1/credit-line/{credit_line_id}/financial/draw - Record credit line draw
#[utoipa::path(
    post,
    path = "/api/v1/credit-line/{credit_line_id}/financial/draw",
    params(
        ("credit_line_id" = String, Path, description = "Credit Line ID (UUID)")
    ),
    request_body = CreateCreditLineFinancialTransactionRequest,
    responses(
        (status = 201, description = "Credit line draw recorded successfully", body = CreditLineFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "Credit line not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_credit_line_draw(
    State(state): State<ApiState>,
    Path(credit_line_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreateCreditLineFinancialTransactionRequest>,
) -> Result<Json<CreditLineFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let credit_line_uuid = match Uuid::parse_str(&credit_line_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_credit_line_id",
                    "Invalid credit line ID format",
                )),
            ));
        }
    };

    let merchant_uuid = match Uuid::parse_str(&request.merchant_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_merchant_id",
                    "Invalid merchant ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "DRAW" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be DRAW for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Parse optional state transaction ID
    let state_transaction_id = if let Some(ref state_tx_id) = request.state_transaction_id {
        Some(Uuid::parse_str(state_tx_id).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_state_transaction_id",
                    "Invalid state transaction ID format",
                )),
            )
        })?)
    } else {
        None
    };

    // Create database request
    let db_request = arda_credit_db::CreateCreditLineFinancialTransactionRequest {
        credit_line_id: credit_line_uuid,
        merchant_id: merchant_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        state_transaction_id,
        transaction_hash: request.transaction_hash.clone(),
        description: request.description.clone(),
    };

    match arda_credit_db::create_credit_line_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                credit_line_id = %credit_line_id,
                merchant_id = %request.merchant_id,
                amount = %request.amount_usdc,
                "Credit line draw recorded successfully"
            );

            let response = CreditLineFinancialTransactionResponse {
                id: transaction.id.to_string(),
                credit_line_id: transaction.credit_line_id.to_string(),
                merchant_id: transaction.merchant_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                state_transaction_id: transaction.state_transaction_id.map(|id| id.to_string()),
                transaction_hash: transaction.transaction_hash,
                description: transaction.description,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording credit line draw: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record credit line draw",
                )),
            ))
        }
    }
}

/// POST /api/v1/credit-line/{credit_line_id}/financial/payment - Record credit line payment
#[utoipa::path(
    post,
    path = "/api/v1/credit-line/{credit_line_id}/financial/payment",
    params(
        ("credit_line_id" = String, Path, description = "Credit Line ID (UUID)")
    ),
    request_body = CreateCreditLineFinancialTransactionRequest,
    responses(
        (status = 201, description = "Credit line payment recorded successfully", body = CreditLineFinancialTransactionResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "Credit line not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn record_credit_line_payment(
    State(state): State<ApiState>,
    Path(credit_line_id): Path<String>,
    JsonExtractor(request): JsonExtractor<CreateCreditLineFinancialTransactionRequest>,
) -> Result<Json<CreditLineFinancialTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let credit_line_uuid = match Uuid::parse_str(&credit_line_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_credit_line_id",
                    "Invalid credit line ID format",
                )),
            ));
        }
    };

    let merchant_uuid = match Uuid::parse_str(&request.merchant_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_merchant_id",
                    "Invalid merchant ID format",
                )),
            ));
        }
    };

    // Validate transaction type
    if request.transaction_type != "PAYMENT" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_transaction_type",
                "Transaction type must be PAYMENT for this endpoint",
            )),
        ));
    }

    // Parse amount
    let amount = match Decimal::from_str(&request.amount_usdc) {
        Ok(amount) if amount > Decimal::ZERO => amount,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_amount",
                    "Amount must be a positive decimal number",
                )),
            ));
        }
    };

    // Parse optional state transaction ID
    let state_transaction_id = if let Some(ref state_tx_id) = request.state_transaction_id {
        Some(Uuid::parse_str(state_tx_id).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_state_transaction_id",
                    "Invalid state transaction ID format",
                )),
            )
        })?)
    } else {
        None
    };

    // Create database request
    let db_request = arda_credit_db::CreateCreditLineFinancialTransactionRequest {
        credit_line_id: credit_line_uuid,
        merchant_id: merchant_uuid,
        transaction_type: request.transaction_type.clone(),
        amount,
        currency: "USDC".to_string(),
        state_transaction_id,
        transaction_hash: request.transaction_hash.clone(),
        description: request.description.clone(),
    };

    match arda_credit_db::create_credit_line_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            info!(
                transaction_id = %transaction.id,
                credit_line_id = %credit_line_id,
                merchant_id = %request.merchant_id,
                amount = %request.amount_usdc,
                "Credit line payment recorded successfully"
            );

            let response = CreditLineFinancialTransactionResponse {
                id: transaction.id.to_string(),
                credit_line_id: transaction.credit_line_id.to_string(),
                merchant_id: transaction.merchant_id.to_string(),
                transaction_type: transaction.transaction_type,
                amount_usdc: transaction.amount.to_string(),
                currency: transaction.currency,
                state_transaction_id: transaction.state_transaction_id.map(|id| id.to_string()),
                transaction_hash: transaction.transaction_hash,
                description: transaction.description,
                processed_at: transaction.processed_at,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error recording credit line payment: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record credit line payment",
                )),
            ))
        }
    }
}

/// GET /api/v1/credit-line/{credit_line_id}/financial/transactions - Get credit line financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/credit-line/{credit_line_id}/financial/transactions",
    params(
        ("credit_line_id" = String, Path, description = "Credit Line ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Credit line transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<CreditLineFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_credit_line_financial_transactions(
    State(state): State<ApiState>,
    Path(credit_line_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<CreditLineFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let credit_line_uuid = match Uuid::parse_str(&credit_line_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_credit_line_id",
                    "Invalid credit line ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_credit_line_financial_transactions(
        &state.pool,
        credit_line_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<CreditLineFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| CreditLineFinancialTransactionResponse {
                    id: t.id.to_string(),
                    credit_line_id: t.credit_line_id.to_string(),
                    merchant_id: t.merchant_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    state_transaction_id: t.state_transaction_id.map(|id| id.to_string()),
                    transaction_hash: t.transaction_hash,
                    description: t.description,
                    processed_at: t.processed_at,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!(
                "Database error getting credit line financial transactions: {}",
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve credit line financial transactions",
                )),
            ))
        }
    }
}

/// GET /api/v1/user/{user_id}/financial/transactions - Get unified financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/financial/transactions",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Financial transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<UnifiedFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_user_financial_transactions(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<UnifiedFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_user_financial_transactions(
        &state.pool,
        user_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<UnifiedFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| UnifiedFinancialTransactionResponse {
                    transaction_category: t.transaction_category,
                    id: t.id.to_string(),
                    user_id: t.user_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    transaction_hash: t.transaction_hash,
                    processed_at: t.processed_at,
                    pool_id: t.pool_id.map(|id| id.to_string()),
                    credit_line_id: t.credit_line_id.map(|id| id.to_string()),
                    description: t.description,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!(
                "Database error getting unified financial transactions: {}",
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve unified financial transactions",
                )),
            ))
        }
    }
}

/// GET /api/v1/user/{user_id}/financial/summary - Get financial summary
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/financial/summary",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    responses(
        (status = 200, description = "Financial summary retrieved successfully", body = FinancialSummaryResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_user_financial_summary(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<FinancialSummaryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    match arda_credit_db::get_user_financial_summary(&state.pool, user_uuid).await {
        Ok(summary) => {
            let response = FinancialSummaryResponse {
                total_deposits_usdc: summary.total_deposits.to_string(),
                total_withdrawals_usdc: summary.total_withdrawals.to_string(),
                net_account_flow_usdc: summary.net_account_flow.to_string(),
                total_pool_funding_usdc: summary.total_pool_funding.to_string(),
                total_pool_withdrawals_usdc: summary.total_pool_withdrawals.to_string(),
                total_credit_line_payments_usdc: summary.total_credit_line_payments.to_string(),
                total_credit_line_draws_usdc: summary.total_credit_line_draws.to_string(),
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error getting financial summary: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve financial summary",
                )),
            ))
        }
    }
}

/// GET /api/v1/user/{user_id}/financial/pools - Get user's pool financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/financial/pools",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Pool transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<PoolFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_user_pool_financial_transactions(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<PoolFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_user_pool_financial_transactions(
        &state.pool,
        user_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<PoolFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| PoolFinancialTransactionResponse {
                    id: t.id.to_string(),
                    pool_id: t.pool_id.to_string(),
                    user_id: t.user_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    state_transaction_id: t.state_transaction_id.map(|id| id.to_string()),
                    transaction_hash: t.transaction_hash,
                    description: t.description,
                    processed_at: t.processed_at,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!(
                "Database error getting user pool financial transactions: {}",
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve user pool financial transactions",
                )),
            ))
        }
    }
}

/// GET /api/v1/merchant/{merchant_id}/financial/credit-lines - Get merchant's credit line financial transactions
#[utoipa::path(
    get,
    path = "/api/v1/merchant/{merchant_id}/financial/credit-lines",
    params(
        ("merchant_id" = String, Path, description = "Merchant ID (UUID)"),
        FinancialTransactionQuery
    ),
    responses(
        (status = 200, description = "Credit line transactions retrieved successfully", body = PaginatedFinancialTransactionsResponse<CreditLineFinancialTransactionResponse>),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Financial Transactions"
)]
#[instrument(skip(state))]
pub async fn get_merchant_credit_line_financial_transactions(
    State(state): State<ApiState>,
    Path(merchant_id): Path<String>,
    Query(query): Query<FinancialTransactionQuery>,
) -> Result<
    Json<PaginatedFinancialTransactionsResponse<CreditLineFinancialTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let merchant_uuid = match Uuid::parse_str(&merchant_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_merchant_id",
                    "Invalid merchant ID format",
                )),
            ));
        }
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match arda_credit_db::get_merchant_credit_line_financial_transactions(
        &state.pool,
        merchant_uuid,
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let total_count = transactions.len() as i64;
            let has_next_page = total_count == limit;
            let has_previous_page = offset > 0;

            let response_transactions: Vec<CreditLineFinancialTransactionResponse> = transactions
                .into_iter()
                .map(|t| CreditLineFinancialTransactionResponse {
                    id: t.id.to_string(),
                    credit_line_id: t.credit_line_id.to_string(),
                    merchant_id: t.merchant_id.to_string(),
                    transaction_type: t.transaction_type,
                    amount_usdc: t.amount.to_string(),
                    currency: t.currency,
                    state_transaction_id: t.state_transaction_id.map(|id| id.to_string()),
                    transaction_hash: t.transaction_hash,
                    description: t.description,
                    processed_at: t.processed_at,
                })
                .collect();

            let response = PaginatedFinancialTransactionsResponse {
                transactions: response_transactions,
                total_count,
                limit,
                offset,
                has_next_page,
                has_previous_page,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!(
                "Database error getting merchant credit line financial transactions: {}",
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve merchant credit line financial transactions",
                )),
            ))
        }
    }
}
