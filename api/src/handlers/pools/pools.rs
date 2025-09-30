use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Json as JsonExtractor,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{error, info, instrument};
use utoipa;
use uuid::Uuid;

use crate::api::pool_api::*;
use crate::config::types::ApiState;
use crate::BalanceVerificationService;
use arda_credit_db::{
    get_all_liquidity_pools,
    /* get_enhanced_investor_metrics, */ get_investor_dashboard_metrics,
    get_investor_pool_metrics, get_investor_pool_summaries as db_get_investor_pool_summaries,
    get_liquidity_pool_by_id, get_market_overview_metrics, get_pool_summary,
    get_pool_transactions as db_get_pool_transactions, get_pool_utilization_history,
};

// ============================================================================
// POOL QUERY ENDPOINTS
// ============================================================================

/// GET /api/pools - List all liquidity pools with filtering and pagination
#[utoipa::path(
    get,
    path = "/api/v1/pools",
    params(PoolListQuery),
    responses(
        (status = 200, description = "Pool list retrieved successfully", body = PoolListResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn list_pools(
    State(state): State<ApiState>,
    Query(query): Query<PoolListQuery>,
) -> Result<Json<PoolListResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Listing pools with filters - status: {:?}, investor_id: {:?}, limit: {}, offset: {:?}",
        query.status, query.investor_id, query.limit, query.offset
    );

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit;

    match get_all_liquidity_pools(
        &state.pool,
        query.status.as_deref(),
        query.investor_id,
        Some(limit),
        Some(offset),
        Some(&query.order_by),
    )
    .await
    {
        Ok(pools) => {
            let standard_pools: Vec<StandardPoolObject> =
                pools.into_iter().map(StandardPoolObject::from).collect();

            let total_count = standard_pools.len() as i64;
            let has_next_page = standard_pools.len() == limit as usize;
            let has_previous_page = offset > 0;

            Ok(Json(PoolListResponse {
                pools: standard_pools,
                total_count,
                page_info: PageInfo {
                    limit,
                    offset,
                    has_next_page,
                    has_previous_page,
                },
            }))
        }
        Err(e) => {
            error!("Database error listing pools: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pools",
                )),
            ))
        }
    }
}

/// GET /api/pools/:pool_id - Get detailed information about a specific pool
#[utoipa::path(
    get,
    path = "/api/v1/pools/{pool_id}",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    responses(
        (status = 200, description = "Pool details retrieved successfully", body = PoolDetailsResponse),
        (status = 404, description = "Pool not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn get_pool_details(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
) -> Result<Json<PoolDetailsResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    info!("Getting details for pool: {}", pool_uuid);

    // Get pool details
    let pool = match get_liquidity_pool_by_id(&state.pool, pool_uuid).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Database error getting pool details: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool details",
                )),
            ));
        }
    };

    // Get pool summary with metrics
    let summary = match get_pool_summary(&state.pool, pool_uuid).await {
        Ok(Some(summary)) => summary,
        Ok(None) => {
            // Return pool data with zero metrics if summary doesn't exist yet
            return Ok(Json(PoolDetailsResponse {
                pool_id: pool.id.to_string(),
                name: pool.name,
                investor_id: pool.investor_id.to_string(),
                total_deposits: pool.total_deposits.to_string(),
                available_liquidity: pool.available_liquidity.to_string(),
                utilized_liquidity: pool.utilized_liquidity.to_string(),
                max_ltv_ratio: pool.max_ltv_ratio.to_string(),
                max_interest_rate: pool.max_interest_rate.to_string(),
                repayment_period_days: pool.repayment_period_days,
                status: pool.status,
                created_at: pool.created_at,
                updated_at: pool.updated_at,
            }));
        }
        Err(e) => {
            error!("Database error getting pool summary: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool summary",
                )),
            ));
        }
    };

    Ok(Json(PoolDetailsResponse {
        pool_id: pool.id.to_string(),
        name: pool.name,
        investor_id: pool.investor_id.to_string(),
        total_deposits: summary.total_deposits.to_string(),
        available_liquidity: summary.available_liquidity.to_string(),
        utilized_liquidity: summary.utilized_liquidity.to_string(),
        max_ltv_ratio: pool.max_ltv_ratio.to_string(),
        max_interest_rate: pool.max_interest_rate.to_string(),
        repayment_period_days: pool.repayment_period_days,
        status: pool.status,
        created_at: pool.created_at,
        updated_at: pool.updated_at,
    }))
}

/// GET /api/pools/:pool_id/utilization - Get pool utilization history
#[instrument(skip(state))]
pub async fn get_pool_utilization(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    Query(query): Query<UtilizationHistoryQuery>,
) -> Result<Json<PoolUtilizationResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    info!(
        "Getting utilization history for pool: {}, days: {}, interval: {}",
        pool_uuid, query.days, query.interval
    );

    match get_pool_utilization_history(&state.pool, pool_uuid, query.days).await {
        Ok(utilization_data) => {
            let historical_data: Vec<UtilizationDataPoint> = utilization_data
                .into_iter()
                .map(UtilizationDataPoint::from)
                .collect();

            let current_utilization_rate = historical_data
                .last()
                .map(|data| data.utilization_rate.clone())
                .unwrap_or_else(|| Decimal::ZERO.to_string());

            Ok(Json(PoolUtilizationResponse {
                pool_id: pool_id.clone(),
                current_utilization_rate,
                historical_data,
            }))
        }
        Err(e) => {
            error!("Database error getting pool utilization: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool utilization data",
                )),
            ))
        }
    }
}

/// GET /api/pools/:pool_id/transactions - Get pool transaction history
#[instrument(skip(state))]
pub async fn get_pool_transactions(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    Query(query): Query<TransactionHistoryQuery>,
) -> Result<Json<PoolTransactionsResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit;

    info!(
        "Getting transactions for pool: {}, limit: {}, offset: {}, operation_type: {:?}",
        pool_uuid, limit, offset, query.operation_type
    );

    match db_get_pool_transactions(
        &state.pool,
        pool_uuid,
        query.operation_type.as_deref(),
        Some(limit),
        Some(offset),
    )
    .await
    {
        Ok(transactions) => {
            let transaction_infos: Vec<PoolTransactionInfo> = transactions
                .into_iter()
                .map(PoolTransactionInfo::from)
                .collect();

            let total_count = transaction_infos.len() as i64;
            let has_next_page = transaction_infos.len() == limit as usize;
            let has_previous_page = offset > 0;

            Ok(Json(PoolTransactionsResponse {
                pool_id: pool_id.clone(),
                transactions: transaction_infos,
                total_count,
                page_info: PageInfo {
                    limit,
                    offset,
                    has_next_page,
                    has_previous_page,
                },
            }))
        }
        Err(e) => {
            error!("Database error getting pool transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool transactions",
                )),
            ))
        }
    }
}

/// POST /api/pools - Create a new liquidity pool
#[utoipa::path(
    post,
    path = "/api/v1/pools",
    request_body = CreatePoolRequest,
    responses(
        (status = 201, description = "Pool created successfully", body = CreatePoolResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn create_pool(
    State(state): State<ApiState>,
    Json(request): Json<CreatePoolRequest>,
) -> Result<Json<CreatePoolResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🏦 Creating liquidity pool: name={}", request.name);

    // Validate parameters
    if request.name.trim().is_empty() || request.name.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_name",
                "Pool name must be between 1 and 255 characters",
            )),
        ));
    }

    // Parse and validate decimal parameters
    let max_ltv_ratio = match Decimal::from_str(&request.max_ltv_ratio) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_ltv_ratio",
                    "Invalid LTV ratio format, must be a decimal string",
                )),
            ));
        }
    };

    let max_interest_rate = match Decimal::from_str(&request.max_interest_rate) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_interest_rate",
                    "Invalid interest rate format, must be a decimal string",
                )),
            ));
        }
    };

    // Validate ranges
    if max_ltv_ratio < Decimal::from_str("0.1").unwrap()
        || max_ltv_ratio > Decimal::from_str("0.95").unwrap()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "ltv_out_of_range",
                "LTV ratio must be between 0.1 and 0.95",
            )),
        ));
    }

    if max_interest_rate < Decimal::from_str("0.01").unwrap()
        || max_interest_rate > Decimal::from_str("0.30").unwrap()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "interest_rate_out_of_range",
                "Interest rate must be between 0.01 and 0.30",
            )),
        ));
    }

    if request.repayment_period_days < 30 || request.repayment_period_days > 3650 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "repayment_period_out_of_range",
                "Repayment period must be between 30 and 3650 days",
            )),
        ));
    }

    // TODO: Extract investor_id from authentication middleware
    // For now, using a placeholder - this should be extracted from authenticated user context
    let investor_id = match request
        .investor_id
        .as_ref()
        .and_then(|id| Uuid::parse_str(id).ok())
    {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "missing_investor_id",
                    "Investor ID is required",
                )),
            ));
        }
    };

    // Create the pool using database function
    let pool_request = arda_credit_db::CreateLiquidityPoolRequest {
        investor_id,
        name: request.name.clone(),
        max_ltv_ratio,
        max_interest_rate,
        repayment_period_days: request.repayment_period_days,
    };

    match arda_credit_db::create_liquidity_pool(&state.pool, &pool_request).await {
        Ok(pool) => {
            info!("✅ Pool created successfully: id={}", pool.id);
            Ok(Json(CreatePoolResponse {
                pool_id: pool.id.to_string(),
                name: pool.name,
                investor_id: pool.investor_id.to_string(),
                max_ltv_ratio: pool.max_ltv_ratio.to_string(),
                max_interest_rate: pool.max_interest_rate.to_string(),
                repayment_period_days: pool.repayment_period_days,
                status: pool.status,
                created_at: pool.created_at,
            }))
        }
        Err(e) => {
            error!("❌ Database error creating pool: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to create liquidity pool",
                )),
            ))
        }
    }
}

/// PUT /api/pools/{pool_id} - Update pool status
#[utoipa::path(
    put,
    path = "/api/v1/pools/{pool_id}",
    params(
        ("pool_id" = String, Path, description = "Pool ID")
    ),
    request_body = UpdatePoolStatusRequest,
    responses(
        (status = 200, description = "Pool updated successfully", body = PoolDetailsResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 404, description = "Pool not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn update_pool_status(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    Json(request): Json<UpdatePoolStatusRequest>,
) -> Result<Json<PoolDetailsResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    info!(
        "🔄 Updating pool status: id={}, status={}",
        pool_id, request.status
    );

    // Validate status
    if !["ACTIVE", "PAUSED", "CLOSED"].contains(&request.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_status",
                "Status must be ACTIVE, PAUSED, or CLOSED",
            )),
        ));
    }

    // Update pool status using database function
    match arda_credit_db::update_liquidity_pool_status(&state.pool, pool_uuid, &request.status)
        .await
    {
        Ok(pool) => {
            info!("✅ Pool status updated successfully: id={}", pool.id);
            Ok(Json(PoolDetailsResponse {
                pool_id: pool.id.to_string(),
                name: pool.name,
                investor_id: pool.investor_id.to_string(),
                total_deposits: pool.total_deposits.to_string(),
                available_liquidity: pool.available_liquidity.to_string(),
                utilized_liquidity: pool.utilized_liquidity.to_string(),
                max_ltv_ratio: pool.max_ltv_ratio.to_string(),
                max_interest_rate: pool.max_interest_rate.to_string(),
                repayment_period_days: pool.repayment_period_days,
                status: pool.status,
                created_at: pool.created_at,
                updated_at: pool.updated_at,
            }))
        }
        Err(e) => {
            error!("❌ Database error updating pool status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to update pool status",
                )),
            ))
        }
    }
}

/// POST /api/pools/{pool_id}/deposit - Deposit funds from user account to pool
#[utoipa::path(
    post,
    path = "/api/v1/pools/{pool_id}/deposit",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    request_body = DepositToPoolRequest,
    responses(
        (status = 201, description = "Deposit to pool successful", body = DepositToPoolResponse),
        (status = 400, description = "Invalid request parameters", body = ErrorResponse),
        (status = 403, description = "Insufficient balance", body = ErrorResponse),
        (status = 404, description = "Pool not found or user not authorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn deposit_to_pool(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    JsonExtractor(request): JsonExtractor<DepositToPoolRequest>,
) -> Result<Json<DepositToPoolResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    info!(
        "Depositing {} USDC from user {} to pool {}",
        request.amount_usdc, user_uuid, pool_uuid
    );

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

    // Check if user has sufficient balance
    let balance_service = BalanceVerificationService::new(state.pool.clone());
    let has_sufficient_balance = match balance_service
        .has_sufficient_balance(user_uuid, amount)
        .await
    {
        Ok(sufficient) => sufficient,
        Err(e) => {
            error!("Error checking user balance: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "balance_check_error",
                    "Failed to verify user balance",
                )),
            ));
        }
    };

    if !has_sufficient_balance {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "insufficient_balance",
                "User does not have sufficient balance for this deposit",
            )),
        ));
    }

    // Verify pool exists and user owns it
    let pool = match get_liquidity_pool_by_id(&state.pool, pool_uuid).await {
        Ok(pool) => pool,
        Err(sqlx::Error::RowNotFound) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("pool_not_found", "Pool not found")),
            ));
        }
        Err(e) => {
            error!("Database error getting pool: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool information",
                )),
            ));
        }
    };

    // Verify user owns the pool
    if pool.investor_id != user_uuid {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "unauthorized",
                "User does not own this pool",
            )),
        ));
    }

    // Create the pool funding transaction
    let db_request = arda_credit_db::CreatePoolFinancialTransactionRequest {
        pool_id: pool_uuid,
        user_id: user_uuid,
        transaction_type: "FUND".to_string(),
        amount,
        currency: "USDC".to_string(),
        state_transaction_id: None,
        transaction_hash: request.transaction_hash.clone(),
        description: Some(format!("Deposit {} USDC to pool", request.amount_usdc)),
    };

    match arda_credit_db::create_pool_financial_transaction(&state.pool, &db_request).await {
        Ok(transaction) => {
            // Update pool balances
            if let Err(e) = arda_credit_db::update_pool_balance_after_transaction(
                &state.pool,
                pool_uuid,
                "FUND",
                amount,
            )
            .await
            {
                error!("Failed to update pool balance after deposit: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "balance_update_error",
                        "Deposit recorded but pool balance update failed",
                    )),
                ));
            }

            // Create corresponding account withdrawal transaction to debit user's account
            let account_withdrawal_request =
                arda_credit_db::CreateAccountFinancialTransactionRequest {
                    user_id: user_uuid,
                    transaction_type: "WITHDRAWAL".to_string(),
                    amount,
                    currency: "USDC".to_string(),
                    transaction_hash: request
                        .transaction_hash
                        .clone()
                        .unwrap_or_else(|| "pool_deposit_internal".to_string()),
                    blockchain_status: "CONFIRMED".to_string(),
                    block_number: request.block_number,
                };

            if let Err(e) = arda_credit_db::create_account_financial_transaction(
                &state.pool,
                &account_withdrawal_request,
            )
            .await
            {
                error!("Failed to create account withdrawal transaction: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "account_transaction_error",
                        "Pool deposit recorded but account withdrawal failed",
                    )),
                ));
            }

            info!(
                "Successfully deposited {} USDC from user {} to pool {}",
                request.amount_usdc, user_uuid, pool_uuid
            );

            Ok(Json(DepositToPoolResponse {
                success: true,
                pool_id: pool_id.clone(),
                user_id: request.user_id.clone(),
                amount_deposited: request.amount_usdc.clone(),
                new_pool_balance: (pool.total_deposits + amount).to_string(),
                transaction_id: transaction.id.to_string(),
                message: "Deposit to pool completed successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Database error recording pool deposit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to record pool deposit",
                )),
            ))
        }
    }
}

// ============================================================================
// INVESTOR ENDPOINTS
// ============================================================================

/// GET /api/investor/dashboard - Get investor dashboard metrics
#[instrument(skip(state))]
pub async fn get_investor_dashboard(
    State(state): State<ApiState>,
    // TODO: Extract investor_id from authentication middleware
    Query(investor_query): Query<InvestorQuery>,
) -> Result<Json<InvestorDashboardResponse>, (StatusCode, Json<ErrorResponse>)> {
    let investor_id = match investor_query.investor_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "missing_investor_id",
                    "Investor ID is required",
                )),
            ));
        }
    };

    info!("Getting dashboard metrics for investor: {}", investor_id);

    match get_investor_dashboard_metrics(&state.pool, investor_id).await {
        Ok(metrics) => {
            let investor_metrics = InvestorMetrics::from(metrics);

            // TODO: Implement recent activity query
            let recent_activity = Vec::new();

            Ok(Json(InvestorDashboardResponse {
                metrics: investor_metrics,
                recent_activity,
            }))
        }
        Err(e) => {
            error!("Database error getting investor dashboard: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve investor dashboard",
                )),
            ))
        }
    }
}

/// GET /api/investor/pools/summary - Get investor pool summaries
#[instrument(skip(state))]
pub async fn get_investor_pool_summaries(
    State(state): State<ApiState>,
    Query(investor_query): Query<InvestorQuery>,
) -> Result<Json<InvestorPoolSummariesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let investor_id = match investor_query.investor_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "missing_investor_id",
                    "Investor ID is required",
                )),
            ));
        }
    };

    info!("Getting pool summaries for investor: {}", investor_id);

    match db_get_investor_pool_summaries(&state.pool, investor_id).await {
        Ok(summaries) => {
            let investor_summaries: Vec<InvestorPoolSummary> = summaries
                .into_iter()
                .map(|pool_summary| {
                    // Convert PoolSummary to InvestorPoolSummary
                    // PoolSummary contains both pool info and basic metrics in one struct
                    let roi = calculate_roi(
                        pool_summary.total_interest_earned,
                        pool_summary.total_deposits,
                    );

                    InvestorPoolSummary {
                        pool: StandardPoolObject {
                            pool_id: pool_summary.id.to_string(),
                            name: pool_summary.name.clone(),
                            investor_id: pool_summary.investor_id.to_string(),
                            total_deposits: pool_summary.total_deposits.to_string(),
                            available_liquidity: pool_summary.available_liquidity.to_string(),
                            utilized_liquidity: pool_summary.utilized_liquidity.to_string(),
                            utilization_rate: pool_summary.utilization_rate.map(|u| u.to_string()),
                            max_ltv_ratio: pool_summary.max_ltv_ratio.to_string(),
                            max_interest_rate: pool_summary.max_interest_rate.to_string(),
                            repayment_period_days: pool_summary.repayment_period_days,
                            status: pool_summary.status.clone(),
                            created_at: pool_summary.created_at,
                            updated_at: pool_summary.updated_at,
                        },
                        performance: PoolPerformanceMetrics {
                            total_loans: pool_summary.total_loans,
                            active_loans: pool_summary.active_loans,
                            total_loan_volume: pool_summary.total_loan_volume.to_string(),
                            total_interest_earned: pool_summary.total_interest_earned.to_string(),
                            default_rate: pool_summary.default_rate.to_string(),
                            roi: roi.to_string(),
                        },
                    }
                })
                .collect();

            let total_count = investor_summaries.len() as i64;

            Ok(Json(InvestorPoolSummariesResponse {
                summaries: investor_summaries,
                total_count,
            }))
        }
        Err(e) => {
            error!("Database error getting investor pool summaries: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve investor pool summaries",
                )),
            ))
        }
    }
}

/* /// GET /api/investor/metrics - Get enhanced investor metrics with APR and yield calculations
#[utoipa::path(
    get,
    path = "/api/v1/investor/metrics",
    params(
        ("investor_id" = Uuid, Query, description = "Investor ID")
    ),
    responses(
        (status = 200, description = "Enhanced investor metrics retrieved successfully", body = EnhancedInvestorMetricsResponse),
        (status = 400, description = "Invalid investor ID", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn get_enhanced_investor_metrics_handler(
    State(state): State<ApiState>,
    Query(investor_query): Query<InvestorQuery>,
) -> Result<Json<EnhancedInvestorMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let investor_id = match investor_query.investor_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "missing_investor_id",
                    "Investor ID is required",
                )),
            ));
        }
    };

    info!("Getting enhanced metrics for investor: {}", investor_id);

    match get_enhanced_investor_metrics(&state.pool, investor_id).await {
        Ok(db_metrics) => {
            let api_metrics = EnhancedInvestorMetrics::from(db_metrics);

            // Calculate summary statistics
            let summary = InvestorMetricsSummary {
                total_return_percentage: api_metrics.lifetime_yield.clone(),
                annualized_return: api_metrics.lifetime_apr.clone(),
                risk_assessment: calculate_risk_assessment(&api_metrics),
                portfolio_health: calculate_portfolio_health(&api_metrics),
            };

            Ok(Json(EnhancedInvestorMetricsResponse {
                metrics: api_metrics,
                summary,
            }))
        }
        Err(e) => {
            error!("Database error getting enhanced investor metrics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve enhanced investor metrics",
                )),
            ))
        }
    }
} */

/// GET /api/investor/pool-metrics - Get investor pool metrics for dashboard
#[utoipa::path(
    get,
    path = "/api/v1/investor/pool-metrics",
    params(
        ("investor_id" = Uuid, Query, description = "Investor ID")
    ),
    responses(
        (status = 200, description = "Investor pool metrics retrieved successfully", body = InvestorPoolMetricsResponse),
        (status = 400, description = "Invalid investor ID", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn get_investor_pool_metrics_handler(
    State(state): State<ApiState>,
    Query(investor_query): Query<InvestorQuery>,
) -> Result<Json<InvestorPoolMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let investor_id = match investor_query.investor_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "missing_investor_id",
                    "Investor ID is required",
                )),
            ));
        }
    };

    info!("Getting pool metrics for investor: {}", investor_id);

    match get_investor_pool_metrics(&state.pool, investor_id).await {
        Ok(db_metrics) => {
            let api_metrics = InvestorPoolMetricsData::from(db_metrics);

            Ok(Json(InvestorPoolMetricsResponse {
                success: true,
                data: api_metrics,
                metadata: InvestorPoolMetricsMetadata {
                    last_updated: chrono::Utc::now(),
                    calculation_basis:
                        "Real-time calculation based on current pool states and transaction history"
                            .to_string(),
                    data_freshness: "Live".to_string(),
                },
            }))
        }
        Err(e) => {
            error!(
                "Failed to retrieve pool metrics for investor {}: {}",
                investor_id, e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    &format!("Failed to retrieve pool metrics: {}", e),
                )),
            ))
        }
    }
}

// ============================================================================
// ANALYTICS ENDPOINTS
// ============================================================================

/// GET /api/analytics/market-overview - Get platform-wide market overview
#[instrument(skip(state))]
pub async fn get_market_overview(
    State(state): State<ApiState>,
) -> Result<Json<MarketOverviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Getting market overview metrics");

    match get_market_overview_metrics(&state.pool).await {
        Ok(metrics) => {
            let platform_metrics = PlatformMetrics::from(metrics);

            // Calculate market trends (simplified implementation)
            let utilization_rate = Decimal::from_str(&platform_metrics.platform_utilization_rate)
                .unwrap_or(Decimal::ZERO);
            let total_liquidity =
                Decimal::from_str(&platform_metrics.total_liquidity).unwrap_or(Decimal::ZERO);
            let market_trends = MarketTrends {
                liquidity_growth_rate: "0".to_string(), // TODO: Calculate from historical data
                loan_volume_growth_rate: "0".to_string(), // TODO: Calculate from historical data
                utilization_trend: if utilization_rate > Decimal::from(75) / Decimal::from(100) {
                    "High".to_string()
                } else if utilization_rate > Decimal::from(50) / Decimal::from(100) {
                    "Medium".to_string()
                } else {
                    "Low".to_string()
                },
                average_pool_size: if platform_metrics.total_pools > 0 {
                    (total_liquidity / Decimal::from(platform_metrics.total_pools)).to_string()
                } else {
                    "0".to_string()
                },
            };

            Ok(Json(MarketOverviewResponse {
                platform_metrics,
                market_trends,
            }))
        }
        Err(e) => {
            error!("Database error getting market overview: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve market overview",
                )),
            ))
        }
    }
}

/// GET /api/analytics/pool-performance - Get pool performance analytics
#[instrument(skip(state))]
pub async fn get_pool_performance(
    State(state): State<ApiState>,
) -> Result<Json<PoolPerformanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Getting pool performance analytics");

    // For now, return a simplified implementation
    // TODO: Implement comprehensive pool performance analytics
    match get_all_liquidity_pools(
        &state.pool,
        None,
        None,
        None,
        None,
        Some("total_deposits DESC"),
    )
    .await
    {
        Ok(pools) => {
            let pool_performance: Vec<PoolPerformanceData> = pools
                .into_iter()
                .map(|pool| PoolPerformanceData {
                    pool_id: pool.id.to_string(),
                    pool_name: pool.name.clone(),
                    total_return: "0".to_string(), // TODO: Calculate actual returns
                    annualized_return: "0".to_string(), // TODO: Calculate annualized returns
                    risk_score: "50".to_string(),  // TODO: Calculate risk score
                    utilization_rate: if pool.total_deposits > Decimal::ZERO {
                        (pool.utilized_liquidity / pool.total_deposits).to_string()
                    } else {
                        "0".to_string()
                    },
                    loan_count: 0,                 // TODO: Get actual loan count
                    default_rate: "0".to_string(), // TODO: Calculate default rate
                })
                .collect();

            let benchmark_data = BenchmarkData {
                average_return: "0".to_string(), // TODO: Calculate from performance data
                median_utilization: "0.5".to_string(), // TODO: Calculate median
                average_risk_score: "50".to_string(), // TODO: Calculate average risk
                platform_default_rate: "0".to_string(), // TODO: Calculate platform default rate
            };

            Ok(Json(PoolPerformanceResponse {
                pool_performance,
                benchmark_data,
            }))
        }
        Err(e) => {
            error!("Database error getting pool performance: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool performance data",
                )),
            ))
        }
    }
}

// ============================================================================
// POOL METRICS OVERVIEW ENDPOINTS
// ============================================================================

/// GET /api/pools/metrics/overview - Get platform-wide pool metrics overview
#[utoipa::path(
    get,
    path = "/api/v1/pools/metrics/overview",
    responses(
        (status = 200, description = "Pool metrics overview retrieved successfully", body = PoolMetricsOverviewResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn get_pool_metrics_overview(
    State(state): State<ApiState>,
) -> Result<Json<PoolMetricsOverviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Getting platform-wide pool metrics overview");

    match get_market_overview_metrics(&state.pool).await {
        Ok(market_metrics) => {
            // For now, we'll use the existing market metrics. In the future, we might want to
            // enhance this with additional calculations for lifetime yield and APR
            let response = PoolMetricsOverviewResponse {
                total_pools: market_metrics.total_pools,
                active_pools: market_metrics.active_pools,
                total_capital_deployed: market_metrics.total_utilized_liquidity.to_string(),
                active_total_capital: market_metrics.total_liquidity.to_string(),
                platform_utilization_rate: (market_metrics.platform_utilization_rate
                    * Decimal::from(100))
                .to_string(),
                total_loans_originated: market_metrics.total_loans,
                total_loan_volume: market_metrics.total_loan_volume.to_string(),
                average_interest_rate: market_metrics.average_interest_rate.to_string(),
                platform_default_rate: market_metrics.platform_default_rate.to_string(),
                // TODO: Calculate these from enhanced market metrics when available
                average_lifetime_yield: None,
                average_current_apr: None,
                last_updated: chrono::Utc::now(),
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error getting market overview metrics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve pool metrics overview",
                )),
            ))
        }
    }
}

/// GET /api/pools/metrics/investor/{investor_id} - Get investor-specific pool metrics overview
#[utoipa::path(
    get,
    path = "/api/v1/pools/metrics/investor/{investor_id}",
    params(
        ("investor_id" = String, Path, description = "Investor ID (UUID)")
    ),
    responses(
        (status = 200, description = "Investor pool metrics retrieved successfully", body = InvestorPoolMetricsOverviewResponse),
        (status = 400, description = "Invalid investor ID", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Analytics"
)]
#[instrument(skip(state))]
pub async fn get_investor_pool_metrics_overview(
    State(state): State<ApiState>,
    Path(investor_id): Path<String>,
) -> Result<Json<InvestorPoolMetricsOverviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    let investor_uuid = match Uuid::parse_str(&investor_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_investor_id",
                    "Invalid investor ID format",
                )),
            ));
        }
    };

    info!(
        "Getting pool metrics overview for investor: {}",
        investor_uuid
    );

    match get_investor_pool_metrics(&state.pool, investor_uuid).await {
        Ok(investor_metrics) => {
            let response = InvestorPoolMetricsOverviewResponse {
                investor_id: investor_metrics.investor_id.to_string(),
                total_pools: investor_metrics.total_pools,
                active_pools: investor_metrics.active_pools,
                total_capital_deployed: investor_metrics.total_capital_deployed.to_string(),
                active_total_capital: investor_metrics.active_total_capital.to_string(),
                average_utilization_rate: investor_metrics.average_utilization_rate.to_string(),
                total_interest_earned: investor_metrics.total_interest_earned.to_string(),
                total_loans_originated: investor_metrics.total_loans_originated,
                default_rate: investor_metrics.default_rate.to_string(),
                lifetime_percentage_yield: investor_metrics
                    .lifetime_percentage_yield
                    .map(|v| v.to_string()),
                current_annual_yield_apr: investor_metrics
                    .current_annual_yield_apr
                    .map(|v| v.to_string()),
                last_updated: chrono::Utc::now(),
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("Database error getting investor pool metrics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve investor pool metrics",
                )),
            ))
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn calculate_roi(interest_earned: Decimal, total_deposits: Decimal) -> Decimal {
    if total_deposits > Decimal::ZERO {
        interest_earned / total_deposits
    } else {
        Decimal::ZERO
    }
}

fn calculate_risk_assessment(metrics: &EnhancedInvestorMetrics) -> String {
    let default_rate = Decimal::from_str(&metrics.default_rate).unwrap_or(Decimal::ZERO);

    if default_rate <= Decimal::from_str("2.0").unwrap() {
        "LOW".to_string()
    } else if default_rate <= Decimal::from_str("5.0").unwrap() {
        "MEDIUM".to_string()
    } else {
        "HIGH".to_string()
    }
}

fn calculate_portfolio_health(metrics: &EnhancedInvestorMetrics) -> String {
    let utilization_rate =
        Decimal::from_str(&metrics.average_utilization_rate).unwrap_or(Decimal::ZERO);
    let default_rate = Decimal::from_str(&metrics.default_rate).unwrap_or(Decimal::ZERO);

    // Portfolio health based on utilization and default rate
    if utilization_rate >= Decimal::from_str("0.7").unwrap()
        && default_rate <= Decimal::from_str("2.0").unwrap()
    {
        "EXCELLENT".to_string()
    } else if utilization_rate >= Decimal::from_str("0.5").unwrap()
        && default_rate <= Decimal::from_str("4.0").unwrap()
    {
        "GOOD".to_string()
    } else if utilization_rate >= Decimal::from_str("0.3").unwrap()
        || default_rate <= Decimal::from_str("6.0").unwrap()
    {
        "FAIR".to_string()
    } else {
        "POOR".to_string()
    }
}

// ============================================================================
// QUERY PARAMETERS
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub struct InvestorQuery {
    pub investor_id: Option<Uuid>,
}
