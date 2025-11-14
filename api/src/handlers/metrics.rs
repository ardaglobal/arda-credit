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
    };

    // Create metrics service
    let metrics_service = MetricsService::new(state.pool.clone());

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
    };

    // Create metrics service
    let metrics_service = MetricsService::new(state.pool.clone());

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
