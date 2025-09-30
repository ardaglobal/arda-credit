//! Pool Borough Limit API Handlers
//!
//! This module implements API handlers for managing borough percentage limits on liquidity pools.
//! Borough limits restrict the maximum percentage of a pool's total deposits that can be lent
//! to borrowers in specific geographic regions.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Json as JsonExtractor,
};
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use arda_credit_db::{
    delete_pool_borough_limit, get_pool_borough_limits, get_pool_borough_summary,
    set_pool_borough_limit, validate_borough_borrowing_limit, BoroughValidationResult,
    PoolBoroughLimit, PoolBoroughSummary, SetPoolBoroughLimitRequest,
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to set or update a borough percentage limit
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetBoroughLimitRequest {
    /// Maximum percentage of pool's total deposits that can be allocated to this borough (0.0-1.0)
    pub max_percentage: f64,
    /// Geographic region identifier (e.g., "Manhattan", "Brooklyn", "Queens")
    pub borough: String,
}

/// Response for borough limit operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoroughLimitResponse {
    pub id: String,
    pub pool_id: String,
    pub borough: String,
    /// Maximum percentage formatted as display percentage (e.g., "10.5%")
    pub max_percentage: String,
    /// Currently allocated amount in dollars
    pub current_allocated: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Borough summary with utilization details
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoroughSummaryResponse {
    pub borough: String,
    /// Currently allocated amount in dollars
    pub current_allocated: String,
    /// Maximum percentage formatted as display percentage (e.g., "10.5%")
    pub max_percentage: String,
    /// Maximum allowed amount in dollars based on current pool deposits
    pub max_allowed: String,
    /// Remaining capacity for new loans in dollars
    pub remaining_capacity: String,
    /// Current utilization as percentage (e.g., "85.2%")
    pub utilization_percentage: String,
}

/// Query parameters for borough validation
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct ValidateBoroughQuery {
    /// Borough to validate against
    pub borough: String,
    /// Requested loan amount in cents
    pub amount: i64,
}

/// Borough validation result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoroughValidationResponse {
    pub is_valid: bool,
    pub borough: String,
    /// Currently allocated amount in dollars
    pub current_allocated: String,
    /// Maximum allowed amount in dollars
    pub max_allowed: String,
    /// Total pool deposits in dollars
    pub total_pool_deposits: String,
    /// Percentage after the proposed loan would be approved
    pub percentage_after_loan: String,
    /// Maximum percentage limit
    pub max_percentage: String,
    pub message: Option<String>,
}

/// Error response for borough operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoroughErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<String>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

impl From<PoolBoroughLimit> for BoroughLimitResponse {
    fn from(limit: PoolBoroughLimit) -> Self {
        Self {
            id: limit.id.to_string(),
            pool_id: limit.pool_id.to_string(),
            borough: limit.borough,
            max_percentage: format!(
                "{:.1}%",
                (limit.max_percentage * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            current_allocated: format!(
                "${:.2}",
                (limit.current_allocated / Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            created_at: limit.created_at.to_rfc3339(),
            updated_at: limit.updated_at.to_rfc3339(),
        }
    }
}

impl From<PoolBoroughSummary> for BoroughSummaryResponse {
    fn from(summary: PoolBoroughSummary) -> Self {
        Self {
            borough: summary.borough,
            current_allocated: format!(
                "${:.2}",
                (summary.current_allocated / Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            max_percentage: format!(
                "{:.1}%",
                (summary.max_percentage * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            max_allowed: format!(
                "${:.2}",
                (summary.max_allowed / Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            remaining_capacity: format!(
                "${:.2}",
                (summary.remaining_capacity / Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
            utilization_percentage: format!(
                "{:.1}%",
                (summary.utilization_percentage * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            ),
        }
    }
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Set or update a borough percentage limit for a pool
#[utoipa::path(
    put,
    path = "/api/v1/pools/{pool_id}/borough-limits",
    summary = "Set borough percentage limit",
    description = "Set or update the maximum percentage of pool deposits that can be allocated to a specific borough",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    request_body = SetBoroughLimitRequest,
    responses(
        (status = 200, description = "Borough limit updated successfully", body = BoroughLimitResponse),
        (status = 400, description = "Invalid request parameters", body = BoroughErrorResponse),
        (status = 404, description = "Pool not found", body = BoroughErrorResponse),
        (status = 500, description = "Internal server error", body = BoroughErrorResponse)
    ),
    tag = "Pool Borough Limits"
)]
#[instrument(skip(state))]
pub async fn set_borough_limit(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    JsonExtractor(body): JsonExtractor<SetBoroughLimitRequest>,
) -> Result<Json<BoroughLimitResponse>, (StatusCode, Json<BoroughErrorResponse>)> {
    info!(
        "Setting borough limit for pool {} - borough: {}, percentage: {}%",
        pool_id,
        body.borough,
        body.max_percentage * 100.0
    );

    // Validate pool ID
    let pool_uuid = match Uuid::from_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(BoroughErrorResponse {
                    error: "Invalid pool ID format".to_string(),
                    code: "INVALID_POOL_ID".to_string(),
                    details: None,
                }),
            ))
        }
    };

    // Validate percentage (0.0 to 1.0)
    if body.max_percentage < 0.0 || body.max_percentage > 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BoroughErrorResponse {
                error: "Percentage must be between 0.0 and 1.0 (0% to 100%)".to_string(),
                code: "INVALID_PERCENTAGE".to_string(),
                details: Some(format!("Provided: {:.2}%", body.max_percentage * 100.0)),
            }),
        ));
    }

    // Validate borough name
    if body.borough.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BoroughErrorResponse {
                error: "Borough name cannot be empty".to_string(),
                code: "INVALID_BOROUGH".to_string(),
                details: None,
            }),
        ));
    }

    let request = SetPoolBoroughLimitRequest {
        pool_id: pool_uuid,
        borough: body.borough.trim().to_string(),
        max_percentage: Decimal::from_f64(body.max_percentage).unwrap_or(Decimal::ZERO),
    };

    match set_pool_borough_limit(&state.pool, request).await {
        Ok(limit) => {
            info!("Successfully set borough limit for pool {}", pool_id);
            Ok(Json(BoroughLimitResponse::from(limit)))
        }
        Err(e) => {
            error!("Failed to set borough limit: {}", e);

            // Check if it's a foreign key constraint error (pool doesn't exist)
            let error_msg = e.to_string();
            if error_msg.contains("foreign key constraint")
                || error_msg.contains("violates foreign key")
            {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(BoroughErrorResponse {
                        error: "Pool not found".to_string(),
                        code: "POOL_NOT_FOUND".to_string(),
                        details: Some(format!("Pool ID: {}", pool_id)),
                    }),
                ))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(BoroughErrorResponse {
                        error: "Failed to set borough limit".to_string(),
                        code: "DATABASE_ERROR".to_string(),
                        details: None,
                    }),
                ))
            }
        }
    }
}

/// Get all borough limits for a pool
#[utoipa::path(
    get,
    path = "/api/v1/pools/{pool_id}/borough-limits",
    summary = "Get pool borough limits",
    description = "Retrieve all borough percentage limits configured for a specific pool",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    responses(
        (status = 200, description = "Borough limits retrieved successfully", body = Vec<BoroughLimitResponse>),
        (status = 400, description = "Invalid pool ID", body = BoroughErrorResponse),
        (status = 500, description = "Internal server error", body = BoroughErrorResponse)
    ),
    tag = "Pool Borough Limits"
)]
#[instrument(skip(state))]
pub async fn get_borough_limits(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
) -> Result<Json<Vec<BoroughLimitResponse>>, (StatusCode, Json<BoroughErrorResponse>)> {
    info!("Retrieving borough limits for pool {}", pool_id);

    // Validate pool ID
    let pool_uuid = match Uuid::from_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(BoroughErrorResponse {
                    error: "Invalid pool ID format".to_string(),
                    code: "INVALID_POOL_ID".to_string(),
                    details: None,
                }),
            ))
        }
    };

    match get_pool_borough_limits(&state.pool, pool_uuid).await {
        Ok(limits) => {
            info!(
                "Retrieved {} borough limits for pool {}",
                limits.len(),
                pool_id
            );
            let response: Vec<BoroughLimitResponse> =
                limits.into_iter().map(BoroughLimitResponse::from).collect();
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get borough limits: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BoroughErrorResponse {
                    error: "Failed to retrieve borough limits".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ))
        }
    }
}

/// Get borough allocation summary for a pool
#[utoipa::path(
    get,
    path = "/api/v1/pools/{pool_id}/borough-summary",
    summary = "Get borough allocation summary",
    description = "Retrieve allocation summary showing utilization for all boroughs with limits",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    responses(
        (status = 200, description = "Borough summary retrieved successfully", body = Vec<BoroughSummaryResponse>),
        (status = 400, description = "Invalid pool ID", body = BoroughErrorResponse),
        (status = 500, description = "Internal server error", body = BoroughErrorResponse)
    ),
    tag = "Pool Borough Limits"
)]
#[instrument(skip(state))]
pub async fn get_borough_summary(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
) -> Result<Json<Vec<BoroughSummaryResponse>>, (StatusCode, Json<BoroughErrorResponse>)> {
    info!("Retrieving borough summary for pool {}", pool_id);

    // Validate pool ID
    let pool_uuid = match Uuid::from_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(BoroughErrorResponse {
                    error: "Invalid pool ID format".to_string(),
                    code: "INVALID_POOL_ID".to_string(),
                    details: None,
                }),
            ))
        }
    };

    match get_pool_borough_summary(&state.pool, pool_uuid).await {
        Ok(summaries) => {
            info!(
                "Retrieved {} borough summaries for pool {}",
                summaries.len(),
                pool_id
            );
            let response: Vec<BoroughSummaryResponse> = summaries
                .into_iter()
                .map(BoroughSummaryResponse::from)
                .collect();
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get borough summary: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BoroughErrorResponse {
                    error: "Failed to retrieve borough summary".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ))
        }
    }
}

/// Validate a potential loan against borough limits
#[utoipa::path(
    get,
    path = "/api/v1/pools/{pool_id}/borough-validation",
    summary = "Validate borough borrowing limit",
    description = "Check if a proposed loan amount would exceed borough percentage limits",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)")
    ),
    params(ValidateBoroughQuery),
    responses(
        (status = 200, description = "Validation result", body = BoroughValidationResponse),
        (status = 400, description = "Invalid request parameters", body = BoroughErrorResponse),
        (status = 500, description = "Internal server error", body = BoroughErrorResponse)
    ),
    tag = "Pool Borough Limits"
)]
#[instrument(skip(state))]
pub async fn validate_borough_limit(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
    Query(query): Query<ValidateBoroughQuery>,
) -> Result<Json<BoroughValidationResponse>, (StatusCode, Json<BoroughErrorResponse>)> {
    info!(
        "Validating borough limit for pool {} - borough: {}, amount: ${}",
        pool_id,
        query.borough,
        query.amount as f64 / 100.0
    );

    // Validate pool ID
    let pool_uuid = match Uuid::from_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(BoroughErrorResponse {
                    error: "Invalid pool ID format".to_string(),
                    code: "INVALID_POOL_ID".to_string(),
                    details: None,
                }),
            ))
        }
    };

    // Validate amount
    if query.amount <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BoroughErrorResponse {
                error: "Amount must be positive".to_string(),
                code: "INVALID_AMOUNT".to_string(),
                details: None,
            }),
        ));
    }

    let requested_amount = Decimal::from(query.amount);

    match validate_borough_borrowing_limit(&state.pool, pool_uuid, &query.borough, requested_amount)
        .await
    {
        Ok(validation) => {
            let message = if validation.is_valid {
                Some("Loan amount is within borough limits".to_string())
            } else {
                Some(format!(
                    "Loan would exceed {:.1}% limit for {}",
                    (validation.max_percentage * Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0),
                    query.borough
                ))
            };

            let response = BoroughValidationResponse {
                is_valid: validation.is_valid,
                borough: query.borough,
                current_allocated: format!(
                    "${:.2}",
                    (validation.current_allocated / Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0)
                ),
                max_allowed: format!(
                    "${:.2}",
                    (validation.max_allowed / Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0)
                ),
                total_pool_deposits: format!(
                    "${:.2}",
                    (validation.total_pool_deposits / Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0)
                ),
                percentage_after_loan: format!(
                    "{:.2}%",
                    (validation.percentage_after_loan * Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0)
                ),
                max_percentage: format!(
                    "{:.1}%",
                    (validation.max_percentage * Decimal::from(100))
                        .to_f64()
                        .unwrap_or(0.0)
                ),
                message,
            };

            info!("Borough validation result: valid={}", validation.is_valid);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to validate borough limit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BoroughErrorResponse {
                    error: "Failed to validate borough limit".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ))
        }
    }
}

/// Delete a borough limit for a pool
#[utoipa::path(
    delete,
    path = "/api/v1/pools/{pool_id}/borough-limits/{borough}",
    summary = "Delete borough limit",
    description = "Remove a borough percentage limit from a pool (reverts to no limit for that borough)",
    params(
        ("pool_id" = String, Path, description = "Pool ID (UUID)"),
        ("borough" = String, Path, description = "Borough name to remove limit for")
    ),
    responses(
        (status = 200, description = "Borough limit deleted successfully"),
        (status = 400, description = "Invalid parameters", body = BoroughErrorResponse),
        (status = 404, description = "Borough limit not found", body = BoroughErrorResponse),
        (status = 500, description = "Internal server error", body = BoroughErrorResponse)
    ),
    tag = "Pool Borough Limits"
)]
#[instrument(skip(state))]
pub async fn delete_borough_limit(
    State(state): State<ApiState>,
    Path((pool_id, borough)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<BoroughErrorResponse>)> {
    info!(
        "Deleting borough limit for pool {} - borough: {}",
        pool_id, borough
    );

    // Validate pool ID
    let pool_uuid = match Uuid::from_str(&pool_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(BoroughErrorResponse {
                    error: "Invalid pool ID format".to_string(),
                    code: "INVALID_POOL_ID".to_string(),
                    details: None,
                }),
            ))
        }
    };

    match delete_pool_borough_limit(&state.pool, pool_uuid, &borough).await {
        Ok(true) => {
            info!(
                "Successfully deleted borough limit for pool {}, borough {}",
                pool_id, borough
            );
            Ok(StatusCode::OK)
        }
        Ok(false) => {
            info!(
                "Borough limit not found for pool {}, borough {}",
                pool_id, borough
            );
            Err((
                StatusCode::NOT_FOUND,
                Json(BoroughErrorResponse {
                    error: "Borough limit not found".to_string(),
                    code: "LIMIT_NOT_FOUND".to_string(),
                    details: Some(format!("Pool: {}, Borough: {}", pool_id, borough)),
                }),
            ))
        }
        Err(e) => {
            error!("Failed to delete borough limit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BoroughErrorResponse {
                    error: "Failed to delete borough limit".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ))
        }
    }
}
