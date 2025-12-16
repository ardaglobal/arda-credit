//! Test handlers for development and testing
//!
//! This module contains test endpoints that are only enabled in debug builds.
//! These endpoints are useful for manual testing and validation during development.

use axum::{extract::State, Json};
use serde_json::json;
use tracing::info;
use uuid::Uuid;

use crate::api::router::ApiState;
use arda_credit_lib::TransactionType;

/// Test endpoint for validating transaction recording service
///
/// This endpoint creates a test transaction to verify the TransactionRecordingService
/// is working correctly. Only available in debug builds.
///
/// # Returns
/// JSON response with transaction details or error
#[cfg(debug_assertions)]
pub async fn test_transaction_recording_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    info!("Test transaction recording endpoint called");

    let user_id = Uuid::new_v4();
    let metadata = Some(json!({
        "test": true,
        "endpoint": "test_transaction_recording",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }));

    match state
        .transaction_recording
        .record_transaction(
            TransactionType::UserRegistration,
            user_id,
            None,
            rust_decimal::Decimal::ZERO,
            metadata,
        )
        .await
    {
        Ok(transaction_id) => {
            info!("Test transaction recorded successfully: {}", transaction_id);

            Ok(Json(json!({
                "success": true,
                "transaction_id": transaction_id,
                "user_id": user_id,
                "transaction_type": "user_registration",
                "message": "Transaction recorded successfully",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })))
        }
        Err(e) => {
            tracing::error!("Test transaction recording failed: {}", e);

            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Transaction recording failed: {}", e),
            ))
        }
    }
}

/// Placeholder for non-debug builds
#[cfg(not(debug_assertions))]
pub async fn test_transaction_recording_endpoint(
    _state: State<ApiState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    Err((
        axum::http::StatusCode::NOT_FOUND,
        "Test endpoints not available in production".to_string(),
    ))
}
