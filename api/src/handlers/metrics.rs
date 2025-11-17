//! Metrics API handlers for the Arda Credit platform
//!
//! This module contains metrics-related API endpoints:
//! - Market value time series data for cumulative growth charts
//! - Pie chart distribution data for industry, region, and asset class breakdowns
//! - On-demand metrics refresh capability
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    config::types::ApiState,
    services::{
        MarketValueDataPoint, MarketValueQuery, MetricsService, PieChartDataPoint, PieChartQuery,
    },
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Query parameters for market value metrics
#[derive(Debug, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct MarketValueParams {
    /// Type of growth calculation: 'market_growth' (by originated_at) or 'arda_growth' (by created_at)
    #[param(example = "market_growth")]
    pub growth_type: String,

    /// Filter scope: 'all', 'originator', or 'investor'
    #[param(example = "all")]
    pub filter_by: String,

    /// Company ID when filter_by is 'originator' or 'investor'
    pub entity_id: Option<Uuid>,

    /// Time range: 'all_time' or 'last_year'
    #[param(example = "all_time")]
    pub time_range: String,

    /// Force cache refresh and recalculate data
    #[param(example = "false")]
    pub force_refresh: Option<bool>,
}

/// Query parameters for pie chart metrics
#[derive(Debug, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct PieChartParams {
    /// Type of pie chart: 'industry', 'region', or 'asset_class'
    #[param(example = "industry")]
    pub chart_type: String,

    /// Filter scope: 'all', 'originator', or 'investor'
    #[param(example = "all")]
    pub filter_by: String,

    /// Company ID when filter_by is 'originator' or 'investor'
    pub entity_id: Option<Uuid>,

    /// Time range: 'all_time' or 'last_year'
    #[param(example = "all_time")]
    pub time_range: String,

    /// Force cache refresh and recalculate data
    #[param(example = "false")]
    pub force_refresh: Option<bool>,
}

/// Market value response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketValueResponse {
    pub success: bool,
    pub data: Vec<MarketValueDataPoint>,
    pub metadata: MarketValueMetadata,
}

/// Market value metadata
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketValueMetadata {
    pub growth_type: String,
    pub filter_by: String,
    pub entity_id: Option<Uuid>,
    pub time_range: String,
    pub total_data_points: usize,
}

/// Pie chart response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PieChartResponse {
    pub success: bool,
    pub data: Vec<PieChartDataPoint>,
    pub metadata: PieChartMetadata,
}

/// Pie chart metadata
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PieChartMetadata {
    pub chart_type: String,
    pub filter_by: String,
    pub entity_id: Option<Uuid>,
    pub time_range: String,
    pub total_categories: usize,
}

/// Metrics refresh response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricsRefreshResponse {
    pub success: bool,
    pub market_value_points: usize,
    pub pie_chart_points: usize,
    pub message: String,
    pub error: Option<String>,
}

/// Error response for metrics endpoints
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricsErrorResponse {
    pub success: bool,
    pub error: String,
    pub details: Option<String>,
}

/// Cache management request parameters
#[derive(Debug, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct CacheManagementParams {
    /// Operation to perform: 'stats', 'cleanup', 'invalidate_all', or 'invalidate_entity'
    #[param(example = "stats")]
    pub operation: String,

    /// Entity ID for 'invalidate_entity' operation
    pub entity_id: Option<Uuid>,
}

/// Cache statistics response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CacheStatsResponse {
    pub success: bool,
    pub market_value_hits: u64,
    pub market_value_misses: u64,
    pub pie_chart_hits: u64,
    pub pie_chart_misses: u64,
    pub total_entries: usize,
    pub cleanup_runs: u64,
    pub entries_evicted: u64,
    pub market_value_hit_ratio: f64,
    pub pie_chart_hit_ratio: f64,
    pub overall_hit_ratio: f64,
    pub cache_enabled: bool,
}

/// Cache management response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CacheManagementResponse {
    pub success: bool,
    pub operation: String,
    pub message: String,
    pub entries_affected: Option<usize>,
}

// ============================================================================
// HANDLER FUNCTIONS
// ============================================================================

/// Get market value time series data
#[utoipa::path(
    get,
    path = "/api/v1/metrics/market-value",
    params(MarketValueParams),
    responses(
        (status = 200, description = "Market value data retrieved successfully", body = MarketValueResponse),
        (status = 400, description = "Invalid query parameters", body = MetricsErrorResponse),
        (status = 401, description = "Unauthorized access"),
        (status = 500, description = "Internal server error", body = MetricsErrorResponse)
    ),
    tag = "Metrics",
    summary = "Get market value time series data",
    description = "Retrieve cumulative market value data for charts. Supports both market growth (by originated_at) and Arda growth (by created_at) calculations."
)]
#[instrument(skip(state), level = "info")]
pub async fn get_market_value(
    State(state): State<ApiState>,
    Query(params): Query<MarketValueParams>,
) -> Result<Json<MarketValueResponse>, (StatusCode, Json<MetricsErrorResponse>)> {
    info!(
        "Retrieving market value data: growth_type={}, filter_by={}, time_range={}",
        params.growth_type, params.filter_by, params.time_range
    );

    // Create query object
    let query = MarketValueQuery {
        growth_type: params.growth_type.clone(),
        filter_by: params.filter_by.clone(),
        entity_id: params.entity_id,
        time_range: params.time_range.clone(),
        force_refresh: params.force_refresh.unwrap_or(false),
    };

    // Create metrics service with caching support
    let metrics_service =
        MetricsService::with_cache(state.pool.clone(), state.metrics_cache.clone());

    // Get data
    match metrics_service.get_market_value_data(query).await {
        Ok(data) => {
            info!(
                "Successfully retrieved {} market value data points",
                data.len()
            );
            Ok(Json(MarketValueResponse {
                success: true,
                metadata: MarketValueMetadata {
                    growth_type: params.growth_type,
                    filter_by: params.filter_by,
                    entity_id: params.entity_id,
                    time_range: params.time_range,
                    total_data_points: data.len(),
                },
                data,
            }))
        }
        Err(err) => {
            error!("Failed to retrieve market value data: {}", err);
            Err((
                StatusCode::BAD_REQUEST,
                Json(MetricsErrorResponse {
                    success: false,
                    error: "Failed to retrieve market value data".to_string(),
                    details: Some(err.to_string()),
                }),
            ))
        }
    }
}

/// Get pie chart distribution data
#[utoipa::path(
    get,
    path = "/api/v1/metrics/pie-charts",
    params(PieChartParams),
    responses(
        (status = 200, description = "Pie chart data retrieved successfully", body = PieChartResponse),
        (status = 400, description = "Invalid query parameters", body = MetricsErrorResponse),
        (status = 401, description = "Unauthorized access"),
        (status = 500, description = "Internal server error", body = MetricsErrorResponse)
    ),
    tag = "Metrics",
    summary = "Get pie chart distribution data",
    description = "Retrieve distribution data for pie charts by industry, region, or asset class."
)]
#[instrument(skip(state), level = "info")]
pub async fn get_pie_chart_data(
    State(state): State<ApiState>,
    Query(params): Query<PieChartParams>,
) -> Result<Json<PieChartResponse>, (StatusCode, Json<MetricsErrorResponse>)> {
    info!(
        "Retrieving pie chart data: chart_type={}, filter_by={}, time_range={}",
        params.chart_type, params.filter_by, params.time_range
    );

    // Create query object
    let query = PieChartQuery {
        chart_type: params.chart_type.clone(),
        filter_by: params.filter_by.clone(),
        entity_id: params.entity_id,
        time_range: params.time_range.clone(),
        force_refresh: params.force_refresh.unwrap_or(false),
    };

    // Create metrics service with caching support
    let metrics_service =
        MetricsService::with_cache(state.pool.clone(), state.metrics_cache.clone());

    // Get data
    match metrics_service.get_pie_chart_data(query).await {
        Ok(data) => {
            info!("Successfully retrieved {} pie chart categories", data.len());
            Ok(Json(PieChartResponse {
                success: true,
                metadata: PieChartMetadata {
                    chart_type: params.chart_type,
                    filter_by: params.filter_by,
                    entity_id: params.entity_id,
                    time_range: params.time_range,
                    total_categories: data.len(),
                },
                data,
            }))
        }
        Err(err) => {
            error!("Failed to retrieve pie chart data: {}", err);
            Err((
                StatusCode::BAD_REQUEST,
                Json(MetricsErrorResponse {
                    success: false,
                    error: "Failed to retrieve pie chart data".to_string(),
                    details: Some(err.to_string()),
                }),
            ))
        }
    }
}

/// Refresh all metrics data
#[utoipa::path(
    post,
    path = "/api/v1/metrics/refresh",
    responses(
        (status = 200, description = "Metrics refreshed successfully", body = MetricsRefreshResponse),
        (status = 401, description = "Unauthorized access"),
        (status = 500, description = "Internal server error", body = MetricsErrorResponse)
    ),
    tag = "Metrics",
    summary = "Refresh all metrics data",
    description = "Trigger on-demand recalculation of all cached metrics data. This operation may take some time to complete."
)]
#[instrument(skip(state), level = "info")]
pub async fn refresh_metrics(
    State(state): State<ApiState>,
) -> Result<Json<MetricsRefreshResponse>, (StatusCode, Json<MetricsErrorResponse>)> {
    info!("Starting metrics refresh operation");

    // Create metrics service
    let metrics_service = MetricsService::new(state.pool.clone());

    // Refresh all metrics
    match metrics_service.refresh_all_metrics().await {
        Ok(result) => {
            if result.success {
                info!(
                    "Metrics refresh completed successfully: {} market value points, {} pie chart points",
                    result.market_value_points, result.pie_chart_points
                );
                Ok(Json(MetricsRefreshResponse {
                    success: true,
                    market_value_points: result.market_value_points,
                    pie_chart_points: result.pie_chart_points,
                    message: "Metrics refreshed successfully".to_string(),
                    error: None,
                }))
            } else {
                error!("Metrics refresh failed: {:?}", result.error);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(MetricsErrorResponse {
                        success: false,
                        error: "Metrics refresh failed".to_string(),
                        details: result.error,
                    }),
                ))
            }
        }
        Err(err) => {
            error!("Metrics refresh operation failed: {}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MetricsErrorResponse {
                    success: false,
                    error: "Metrics refresh operation failed".to_string(),
                    details: Some(err.to_string()),
                }),
            ))
        }
    }
}

/// Get cache statistics
#[utoipa::path(
    get,
    path = "/api/v1/metrics/cache/stats",
    responses(
        (status = 200, description = "Cache statistics retrieved successfully", body = CacheStatsResponse),
        (status = 401, description = "Unauthorized access"),
        (status = 500, description = "Internal server error", body = MetricsErrorResponse)
    ),
    tag = "Metrics",
    summary = "Get cache statistics",
    description = "Retrieve detailed statistics about the metrics cache performance and usage."
)]
#[instrument(skip(state), level = "info")]
pub async fn get_cache_stats(
    State(state): State<ApiState>,
) -> Result<Json<CacheStatsResponse>, (StatusCode, Json<MetricsErrorResponse>)> {
    info!("Retrieving cache statistics");

    let stats = state.metrics_cache.get_stats().await;

    Ok(Json(CacheStatsResponse {
        success: true,
        market_value_hits: stats.market_value_hits,
        market_value_misses: stats.market_value_misses,
        pie_chart_hits: stats.pie_chart_hits,
        pie_chart_misses: stats.pie_chart_misses,
        total_entries: stats.total_entries,
        cleanup_runs: stats.cleanup_runs,
        entries_evicted: stats.entries_evicted,
        market_value_hit_ratio: stats.market_value_hit_ratio(),
        pie_chart_hit_ratio: stats.pie_chart_hit_ratio(),
        overall_hit_ratio: stats.overall_hit_ratio(),
        cache_enabled: true,
    }))
}

/// Manage metrics cache
#[utoipa::path(
    post,
    path = "/api/v1/metrics/cache/manage",
    params(CacheManagementParams),
    responses(
        (status = 200, description = "Cache operation completed successfully", body = CacheManagementResponse),
        (status = 400, description = "Invalid operation or parameters", body = MetricsErrorResponse),
        (status = 401, description = "Unauthorized access"),
        (status = 500, description = "Internal server error", body = MetricsErrorResponse)
    ),
    tag = "Metrics",
    summary = "Manage metrics cache",
    description = "Perform cache management operations: cleanup expired entries, invalidate all entries, or invalidate specific entity entries."
)]
#[instrument(skip(state), level = "info")]
pub async fn manage_cache(
    State(state): State<ApiState>,
    Query(params): Query<CacheManagementParams>,
) -> Result<Json<CacheManagementResponse>, (StatusCode, Json<MetricsErrorResponse>)> {
    info!(
        "Performing cache management operation: {}",
        params.operation
    );

    match params.operation.as_str() {
        "cleanup" => {
            state.metrics_cache.cleanup_expired_entries().await;
            let stats = state.metrics_cache.get_stats().await;

            Ok(Json(CacheManagementResponse {
                success: true,
                operation: "cleanup".to_string(),
                message: "Cache cleanup completed successfully".to_string(),
                entries_affected: Some(stats.total_entries),
            }))
        }
        "invalidate_all" => {
            let stats_before = state.metrics_cache.get_stats().await;
            state.metrics_cache.invalidate_all().await;

            Ok(Json(CacheManagementResponse {
                success: true,
                operation: "invalidate_all".to_string(),
                message: "All cache entries invalidated successfully".to_string(),
                entries_affected: Some(stats_before.total_entries),
            }))
        }
        "invalidate_entity" => {
            let entity_id = params.entity_id.ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(MetricsErrorResponse {
                        success: false,
                        error: "Invalid operation".to_string(),
                        details: Some(
                            "entity_id is required for invalidate_entity operation".to_string(),
                        ),
                    }),
                )
            })?;

            state.metrics_cache.invalidate_entity(entity_id).await;

            Ok(Json(CacheManagementResponse {
                success: true,
                operation: "invalidate_entity".to_string(),
                message: format!(
                    "Cache entries for entity {} invalidated successfully",
                    entity_id
                ),
                entries_affected: None, // Could track this in the future
            }))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(MetricsErrorResponse {
                success: false,
                error: "Invalid operation".to_string(),
                details: Some(
                    "Supported operations: cleanup, invalidate_all, invalidate_entity".to_string(),
                ),
            }),
        )),
    }
}
