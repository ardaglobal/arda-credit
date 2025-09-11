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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use utoipa::ToSchema;

use crate::balance_service::AvailableBalance;
use crate::paths;
use crate::types::ApiState;
use crate::withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalServiceError,
    WithdrawalStatusResponse, WithdrawalTransactionService,
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
    tag = "Withdrawals"
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
    tag = "Withdrawals"
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

/// Get user's available balance for withdrawals
#[utoipa::path(
    get,
    path = "/api/v1/users/{address}/available-balance",
    params(
        ("address" = String, Path, description = "User's wallet address")
    ),
    responses(
        (status = 200, description = "Available balance retrieved", body = AvailableBalance),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_available_balance_endpoint(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<AvailableBalance>, StatusCode> {
    match state.balance_service.get_available_balance(&address).await {
        Ok(Some(balance)) => {
            info!("Retrieved available balance for address {}", address);
            Ok(Json(balance))
        }
        Ok(None) => {
            info!("No balance found for address {}", address);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to get available balance: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
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
    tag = "Withdrawals"
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
    tag = "Withdrawals"
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

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Get all withdrawal-related routes for the router
pub fn withdrawal_routes() -> Vec<axum::Router<ApiState>> {
    vec![
        axum::Router::new()
            .route(paths::WITHDRAWALS, axum::routing::post(initiate_withdrawal_endpoint))
            .route(paths::WITHDRAWAL_BY_ID, axum::routing::get(get_withdrawal_status_endpoint))
            .route(paths::WITHDRAWAL_CANCEL, axum::routing::post(cancel_withdrawal_endpoint)),

        axum::Router::new()
            .route(paths::USER_AVAILABLE_BALANCE, axum::routing::get(get_user_available_balance_endpoint))
            .route(paths::USER_WITHDRAWALS, axum::routing::get(get_user_withdrawal_history_endpoint)),

        axum::Router::new()
            .route(paths::ADMIN_PENDING_WITHDRAWALS, axum::routing::get(get_pending_withdrawals_endpoint)),
    ]
}
