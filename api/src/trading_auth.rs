//! Shared authentication utilities for marketplace and transfer operations
//!
//! This module provides authentication extraction and validation for both
//! marketplace and transfer operations.

use axum::http::{HeaderMap, StatusCode};
use tracing::{debug, warn};
use uuid::Uuid;

/// Extract authenticated user ID from request headers
///
/// This function should be used by both marketplace and transfer handlers
/// to get the current user's identity for authorization checks.
pub fn extract_user_id_from_headers(headers: &HeaderMap) -> Result<Uuid, (StatusCode, String)> {
    // Method 1: Try x-user-id header (set by auth middleware)
    if let Some(user_id_header) = headers.get("x-user-id") {
        if let Ok(user_id_str) = user_id_header.to_str() {
            if let Ok(user_id) = Uuid::parse_str(user_id_str) {
                debug!("Extracted user ID from x-user-id header: {}", user_id);
                return Ok(user_id);
            }
        }
    }

    // Method 2: For MVP testing, accept user_id from custom header
    if let Some(test_user_header) = headers.get("x-test-user-id") {
        if let Ok(user_id_str) = test_user_header.to_str() {
            if let Ok(user_id) = Uuid::parse_str(user_id_str) {
                debug!("Extracted user ID from test header: {}", user_id);
                return Ok(user_id);
            }
        }
    }

    warn!("No valid user ID found in request headers");

    Err((
        StatusCode::UNAUTHORIZED,
        "User authentication required - provide x-user-id or x-test-user-id header".to_string(),
    ))
}

/// Validate user is KYC approved (MVP auto-approval)
pub async fn validate_user_kyc_approved(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let kyc_check = sqlx::query!(
        r#"
        SELECT kyc_status
        FROM identities
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(_kyc) = kyc_check {
        // For MVP, auto-approve all KYC checks
        Ok(true)

        // TODO: When KYC is properly implemented, use:
        // Ok(kyc.kyc_status == "approved")
    } else {
        Ok(false) // User not found
    }
}

/// Validate user owns the specified loan
pub async fn validate_loan_ownership(
    pool: &sqlx::PgPool,
    loan_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let ownership_check = sqlx::query!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM loans
            WHERE id = $1 AND lender_id = $2
        ) as owns_loan
        "#,
        loan_id,
        user_id
    )
    .fetch_one(pool)
    .await?;

    Ok(ownership_check.owns_loan.unwrap_or(false))
}
