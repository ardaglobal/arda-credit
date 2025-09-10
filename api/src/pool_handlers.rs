use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{error, info, instrument};
use utoipa;
use uuid::Uuid;

use crate::pool_api::*;
use crate::rest::{
    ApiState, CreatePoolRequest, CreatePoolResponse, PoolDetailsResponse, UpdatePoolStatusRequest,
};
use arda_credit_db::{
    get_all_liquidity_pools, get_investor_dashboard_metrics,
    get_investor_pool_summaries as db_get_investor_pool_summaries, get_liquidity_pool_by_id,
    get_market_overview_metrics, get_pool_summary,
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
    tag = "Liquidity Pools"
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
    tag = "Liquidity Pools"
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

    // Get pool summary
    let _summary = match get_pool_summary(&state.pool, pool_uuid).await {
        Ok(Some(_summary)) => PoolSummaryInfo::from(_summary),
        Ok(None) => {
            // Return default summary if none exists
            PoolSummaryInfo {
                total_loans: 0,
                active_loans: 0,
                total_loan_volume: "0".to_string(),
                average_loan_amount: None,
                default_rate: "0".to_string(),
                total_interest_earned: "0".to_string(),
                total_principal_repaid: "0".to_string(),
            }
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
        total_deposits: "0".to_string(), // TODO: Calculate from database
        available_liquidity: "0".to_string(), // TODO: Calculate from database
        utilized_liquidity: "0".to_string(), // TODO: Calculate from database
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
    tag = "Liquidity Pools"
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
    tag = "Liquidity Pools"
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
                total_deposits: "0".to_string(), // TODO: Calculate from database
                available_liquidity: "0".to_string(), // TODO: Calculate from database
                utilized_liquidity: "0".to_string(), // TODO: Calculate from database
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
// HELPER FUNCTIONS
// ============================================================================

fn calculate_roi(interest_earned: Decimal, total_deposits: Decimal) -> Decimal {
    if total_deposits > Decimal::ZERO {
        interest_earned / total_deposits
    } else {
        Decimal::ZERO
    }
}

// ============================================================================
// QUERY PARAMETERS
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub struct InvestorQuery {
    pub investor_id: Option<Uuid>,
}
