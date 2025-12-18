//! Withdrawal handlers for the Arda Credit API
//!
//! This module contains all withdrawal-related API endpoints including:
//! - Withdrawal request initiation with KYC and balance validation
//! - Withdrawal status tracking and history
//! - Withdrawal cancellation (before submission)
//! - Admin management of pending withdrawals
//! - User balance queries for withdrawal purposes
//!
//! All routes are documented with OpenAPI and use consistent error handling.
//! These endpoints integrate with the WithdrawalTransactionService for complete
//! withdrawal lifecycle management.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
// rust_decimal::Decimal import removed - no longer used
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;

use crate::config::paths;
use crate::config::types::{ApiState, ErrorResponse};
use crate::services::withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalServiceError,
    WithdrawalStatusResponse,
};

// ============================================================================
// WITHDRAWAL HANDLERS
// ============================================================================

/// Initiate a new withdrawal request
#[utoipa::path(
    post,
    path = "/api/v1/withdrawals",
    request_body = InitiateWithdrawalRequest,
    responses(
        (status = 200, description = "Withdrawal initiated successfully", body = InitiateWithdrawalResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(state), level = "info")]
pub async fn initiate_withdrawal_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<InitiateWithdrawalRequest>,
) -> Result<Json<InitiateWithdrawalResponse>, StatusCode> {
    match state.withdrawal_service.initiate_withdrawal(request).await {
        Ok(response) => {
            info!("Withdrawal initiated successfully: {}", response.request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to initiate withdrawal: {:?}", e);
            match e {
                WithdrawalServiceError::UserNotFound(_) => Err(StatusCode::NOT_FOUND),
                WithdrawalServiceError::InvalidAmount(_) => Err(StatusCode::BAD_REQUEST),
                WithdrawalServiceError::KycValidationFailed(_) => Err(StatusCode::BAD_REQUEST),
                WithdrawalServiceError::InsufficientBalance(_) => Err(StatusCode::BAD_REQUEST),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get withdrawal request status
#[utoipa::path(
    get,
    path = "/api/v1/withdrawals/{request_id}",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID")
    ),
    responses(
        (status = 200, description = "Withdrawal status retrieved", body = WithdrawalStatusResponse),
        (status = 404, description = "Withdrawal request not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_withdrawal_status_endpoint(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
) -> Result<Json<WithdrawalStatusResponse>, StatusCode> {
    match state
        .withdrawal_service
        .get_withdrawal_status(request_id)
        .await
    {
        Ok(response) => {
            info!("Retrieved withdrawal status for request {}", request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get withdrawal status: {:?}", e);
            match e {
                WithdrawalServiceError::Sql(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get user's withdrawal history
#[utoipa::path(
    get,
    path = "/api/v1/users/{address}/withdrawals",
    params(
        ("address" = String, Path, description = "User's wallet address")
    ),
    responses(
        (status = 200, description = "Withdrawal history retrieved", body = Vec<WithdrawalStatusResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_withdrawal_history_endpoint(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<Vec<WithdrawalStatusResponse>>, StatusCode> {
    match state
        .withdrawal_service
        .get_user_withdrawal_history(&address)
        .await
    {
        Ok(history) => {
            info!("Retrieved withdrawal history for address {}", address);
            Ok(Json(history))
        }
        Err(e) => {
            error!("Failed to get withdrawal history: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Cancel a withdrawal request
#[utoipa::path(
    post,
    path = "/api/v1/withdrawals/{request_id}/cancel",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID to cancel")
    ),
    responses(
        (status = 200, description = "Withdrawal cancelled successfully", body = WithdrawalStatusResponse),
        (status = 400, description = "Cannot cancel withdrawal"),
        (status = 404, description = "Withdrawal request not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(state), level = "info")]
pub async fn cancel_withdrawal_endpoint(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
) -> Result<Json<WithdrawalStatusResponse>, StatusCode> {
    match state.withdrawal_service.cancel_withdrawal(request_id).await {
        Ok(response) => {
            info!("Cancelled withdrawal request {}", request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to cancel withdrawal: {:?}", e);
            match e {
                WithdrawalServiceError::Sql(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
                WithdrawalServiceError::InvalidAmount(_) => Err(StatusCode::BAD_REQUEST),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get pending withdrawal requests (admin endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/admin/withdrawals/pending",
    params(
        ("limit" = Option<i32>, Query, description = "Maximum number of requests to return")
    ),
    responses(
        (status = 200, description = "Pending withdrawals retrieved", body = Vec<WithdrawalStatusResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_pending_withdrawals_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<WithdrawalStatusResponse>>, StatusCode> {
    let limit = params.get("limit").and_then(|s| s.parse().ok());

    match state
        .withdrawal_service
        .get_pending_withdrawals(limit)
        .await
    {
        Ok(withdrawals) => {
            info!("Retrieved {} pending withdrawals", withdrawals.len());
            Ok(Json(withdrawals))
        }
        Err(e) => {
            error!("Failed to get pending withdrawals: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/v1/admin/withdrawals/execution/manual/{request_id} - Manually execute a withdrawal
#[utoipa::path(
    post,
    path = "/api/v1/admin/withdrawals/execution/manual/{request_id}",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID")
    ),
    responses(
        (status = 200, description = "Withdrawal executed successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Withdrawal request not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Admin"
)]
#[instrument(skip(_state))]
pub async fn manual_execute_withdrawal(
    State(_state): State<ApiState>,
    Path(request_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Withdrawal executor functionality removed
    warn!("Manual withdrawal execution not available - withdrawal executor removed");
    Err((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse::new(
            "executor_not_available",
            "Withdrawal executor service is not running",
        )),
    ))
}

/// POST /api/v1/admin/withdrawals/{request_id}/approve - Approve a withdrawal request
///
/// ## Wallet Permission Authorization
///
/// When authentication middleware is implemented, this endpoint should verify that the
/// approving user has the necessary wallet permissions:
///
/// ```rust,ignore
/// // 1. Extract authenticated user_id from request context (via auth middleware)
/// // 2. Get the withdrawal request to find the associated wallet_address_id
/// // 3. Call: arda_credit_db::business::wallet::can_authorize_transaction(&pool, user_id, wallet_address_id)
/// // 4. If false, return 403 Forbidden: "Only admin users with approve_withdrawal permission can authorize transactions"
/// ```
///
/// The database enforces that only admin users with 'approve_withdrawal' permission
/// can authorize transactions (via trigger: enforce_admin_only_permissions).
#[utoipa::path(
    post,
    path = "/api/v1/admin/withdrawals/{request_id}/approve",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID")
    ),
    request_body = ApproveWithdrawalRequest,
    responses(
        (status = 200, description = "Withdrawal approved successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 403, description = "Access denied - requires admin role with approve_withdrawal permission", body = ErrorResponse),
        (status = 404, description = "Withdrawal request not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Admin"
)]
#[instrument(skip(state))]
pub async fn approve_withdrawal_request(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
    Json(request): Json<ApproveWithdrawalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // TODO: Extract authenticated user_id from request when auth middleware is implemented
    // TODO: Verify user has approve_withdrawal permission via: can_authorize_transaction(&pool, user_id, wallet_address_id)

    info!("Admin approving withdrawal request {}", request_id);

    match arda_credit_db::approve_withdrawal_request(&state.pool, request_id, &request.admin_notes)
        .await
    {
        Ok(withdrawal) => {
            info!(
                "Withdrawal {} approved successfully by admin. Amount: {:?} USDC, User: {}",
                request_id, withdrawal.amount_usdc, withdrawal.user_address
            );

            // Execute withdrawal immediately upon approval
            if let Some(executor) = &state.withdrawal_executor {
                match executor.execute_withdrawal(request_id).await {
                    Ok(()) => {
                        info!(
                            "✅ Withdrawal {} executed successfully on blockchain",
                            request_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to execute withdrawal {} on blockchain: {}",
                            request_id, e
                        );
                        // Log the error but continue with approval response
                        // The withdrawal is still approved in the database and can be retried
                    }
                }
            } else {
                warn!(
                    "⚠️ Withdrawal executor not available - withdrawal approved but not executed"
                );
                warn!("💡 Configure ETHEREUM_RPC_URL, ETHEREUM_CONTRACT_ADDRESS, and ETHEREUM_WALLET_PRIVATE_KEY to enable automatic execution");
            }

            Ok(Json(
                serde_json::to_value(&withdrawal).unwrap_or_else(|_| serde_json::json!({})),
            ))
        }
        Err(e) => {
            error!("Failed to approve withdrawal {}: {:?}", request_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "approval_error",
                    &format!("Failed to approve withdrawal: {}", e),
                )),
            ))
        }
    }
}

/// POST /api/v1/admin/withdrawals/{request_id}/reject - Reject a withdrawal request
///
/// ## Wallet Permission Authorization
///
/// When authentication middleware is implemented, this endpoint should verify that the
/// rejecting user has the necessary wallet permissions (same as approve).
#[utoipa::path(
    post,
    path = "/api/v1/admin/withdrawals/{request_id}/reject",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID")
    ),
    request_body = RejectWithdrawalRequest,
    responses(
        (status = 200, description = "Withdrawal rejected successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 403, description = "Access denied - requires admin role with approve_withdrawal permission", body = ErrorResponse),
        (status = 404, description = "Withdrawal request not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Admin"
)]
#[instrument(skip(state))]
pub async fn reject_withdrawal_request(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
    Json(request): Json<RejectWithdrawalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // TODO: Extract authenticated user_id from request when auth middleware is implemented
    // TODO: Verify user has approve_withdrawal permission via: can_authorize_transaction(&pool, user_id, wallet_address_id)

    info!("Admin rejecting withdrawal request {}", request_id);

    match arda_credit_db::reject_withdrawal_request(
        &state.pool,
        request_id,
        &request.rejection_reason,
        request.admin_notes.as_deref(),
    )
    .await
    {
        Ok(withdrawal) => {
            info!(
                "Withdrawal {} rejected by admin. Reason: {}",
                request_id, request.rejection_reason
            );
            Ok(Json(
                serde_json::to_value(&withdrawal).unwrap_or_else(|_| serde_json::json!({})),
            ))
        }
        Err(e) => {
            error!("Failed to reject withdrawal {}: {:?}", request_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "rejection_error",
                    &format!("Failed to reject withdrawal: {}", e),
                )),
            ))
        }
    }
}

/// Request body for approving withdrawals
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ApproveWithdrawalRequest {
    /// Optional admin notes for the approval
    pub admin_notes: Option<String>,
}

/// Request body for rejecting withdrawals
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RejectWithdrawalRequest {
    /// Reason for rejection
    pub rejection_reason: String,
    /// Optional admin notes
    pub admin_notes: Option<String>,
}

/// GET /api/v1/admin/dashboard/withdrawals - Admin dashboard data for withdrawals
#[utoipa::path(
    get,
    path = "/api/v1/admin/dashboard/withdrawals",
    responses(
        (status = 200, description = "Admin dashboard data retrieved successfully", body = AdminWithdrawalDashboard),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Admin"
)]
#[instrument(skip(state))]
pub async fn get_admin_withdrawal_dashboard(
    State(state): State<ApiState>,
) -> Result<Json<AdminWithdrawalDashboard>, (StatusCode, Json<ErrorResponse>)> {
    // Get dashboard statistics
    let pending_approval =
        match arda_credit_db::get_withdrawals_pending_admin_approval(&state.pool, 10, 0).await {
            Ok(requests) => requests,
            Err(e) => {
                error!("Failed to get pending approvals: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "dashboard_error",
                        "Failed to retrieve dashboard data",
                    )),
                ));
            }
        };

    let pending_execution =
        match arda_credit_db::get_validated_pending_withdrawals(&state.pool, Some(10)).await {
            Ok(requests) => requests,
            Err(e) => {
                error!("Failed to get pending executions: {:?}", e);
                vec![]
            }
        };

    let pending_approval_count = pending_approval.len() as i64;
    let pending_execution_count = pending_execution.len() as i64;
    let total_pending_amount = pending_approval
        .iter()
        .map(|w| {
            w.amount_usdc
                .as_ref()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0)
        })
        .sum::<f64>();

    let recent_requests: Vec<WithdrawalRequestSummary> = pending_approval
        .iter()
        .map(|w| WithdrawalRequestSummary {
            id: w.id,
            user_address: w.user_address.clone(),
            recipient_address: w.recipient_address.clone(),
            amount_usdc: w.amount_usdc.as_ref().map(|d| d.to_string()),
            state: w.state.clone(),
            created_at: w
                .created_at
                .map(|dt| dt.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
        })
        .collect();

    let stats = AdminWithdrawalDashboard {
        pending_approval_count,
        pending_execution_count,
        total_pending_amount: format!("{:.2}", total_pending_amount),
        recent_requests,
        execution_stats: None,
    };

    info!("Retrieved admin withdrawal dashboard stats");
    Ok(Json(stats))
}

/// Admin withdrawal dashboard data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AdminWithdrawalDashboard {
    pub pending_approval_count: i64,
    pub pending_execution_count: i64,
    pub total_pending_amount: String,
    pub recent_requests: Vec<WithdrawalRequestSummary>,
    pub execution_stats: Option<serde_json::Value>,
}

/// Summary of a withdrawal request for admin dashboard
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WithdrawalRequestSummary {
    pub id: i32,
    pub user_address: String,
    pub recipient_address: String,
    pub amount_usdc: Option<String>,
    pub state: String,
    pub created_at: String,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Get all withdrawal-related routes for the router
pub fn withdrawal_routes() -> Vec<axum::Router<ApiState>> {
    vec![
        axum::Router::new()
            .route(
                paths::WITHDRAWALS,
                axum::routing::post(initiate_withdrawal_endpoint),
            )
            .route(
                paths::WITHDRAWAL_BY_ID,
                axum::routing::get(get_withdrawal_status_endpoint),
            )
            .route(
                paths::WITHDRAWAL_CANCEL,
                axum::routing::post(cancel_withdrawal_endpoint),
            ),
        axum::Router::new().route(
            paths::USER_WITHDRAWALS,
            axum::routing::get(get_user_withdrawal_history_endpoint),
        ),
        axum::Router::new().route(
            paths::ADMIN_PENDING_WITHDRAWALS,
            axum::routing::get(get_pending_withdrawals_endpoint),
        ),
        axum::Router::new().route(
            "/api/v1/admin/withdrawals/execution/manual/:request_id",
            axum::routing::post(manual_execute_withdrawal),
        ),
        axum::Router::new()
            .route(
                "/api/v1/admin/withdrawals/:request_id/approve",
                axum::routing::post(approve_withdrawal_request),
            )
            .route(
                "/api/v1/admin/withdrawals/:request_id/reject",
                axum::routing::post(reject_withdrawal_request),
            )
            .route(
                "/api/v1/admin/dashboard/withdrawals",
                axum::routing::get(get_admin_withdrawal_dashboard),
            ),
    ]
}
