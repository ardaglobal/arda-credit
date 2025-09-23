//! Authentication handlers for the Arda Credit API
//!
//! This module contains all authentication-related API endpoints including:
//! - User login and logout
//! - Token management
//! - Invite code generation and validation
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;

use crate::paths;
use crate::types::{ApiState, UserType};

// Use Axum's built-in response types for Axum 0.8 compatibility
use arda_credit_db::{
    generate_invite_code, get_identity_by_email, get_invite_code_info, update_last_login,
    use_invite_code,
};

// ============================================================================
// AUTHENTICATION TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Request to logout (optional - for token blacklisting)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LogoutRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoke_all_tokens: Option<bool>,
}

/// Response for logout
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LogoutResponse {
    pub message: String,
}

/// Invite code validation response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InviteCodeResponse {
    pub code: String,
    pub is_valid: bool,
    pub message: String,
}

/// Response from validating an invite code (doesn't mark as used)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ValidateInviteCodeResponse {
    pub code: String,
    pub is_valid: bool,
    pub is_admin_code: bool, // New field to indicate if this is an admin invite
    pub user_type: Option<String>, // The user type this invite is for ('admin' or 'regular')
    pub expires_at: Option<DateTime<Utc>>,
    pub message: String,
}

/// Request to generate a new invite code
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GenerateInviteCodeRequest {
    pub max_uses: Option<i32>,
    pub expires_in_hours: Option<i32>,
    pub user_type: Option<String>, // 'admin' or 'regular' (defaults to 'regular')
}

/// Response from generating an invite code
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GenerateInviteCodeResponse {
    pub code: String,
    pub max_uses: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub user_type: String,   // 'admin' or 'regular'
    pub is_admin_code: bool, // Convenience field
    pub message: String,
}

// ============================================================================
// AUTHENTICATION ENDPOINTS
// ============================================================================

/// Logout endpoint (stateless JWT - mainly for logging)
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    request_body = LogoutRequest,
    responses(
        (status = 200, description = "Logout successful", body = LogoutResponse),
        (status = 401, description = "Invalid or expired token"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Authentication"
)]
#[instrument(skip(_state), level = "info")]
pub async fn logout_endpoint(
    State(_state): State<ApiState>,
    Json(_request): Json<LogoutRequest>,
) -> Result<Json<LogoutResponse>, (StatusCode, String)> {
    // For stateless JWT, we don't need to invalidate tokens on the server side
    // In a production system, you might:
    // 1. Add token to a blacklist with expiration
    // 2. Use short-lived tokens with refresh tokens
    // 3. Store active sessions in Redis

    info!("📤 API: User logged out");

    Ok(Json(LogoutResponse {
        message: "Logout successful. Please remove the token from client storage.".to_string(),
    }))
}

/// Validate invite code (doesn't mark as used)
#[utoipa::path(
    post,
    path = "/api/v1/invite-code/{code}/validate",
    responses(
        (status = 200, description = "Invite code validation result", body = ValidateInviteCodeResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("code" = String, Path, description = "Invite code to validate")
    ),
    tag = "Authentication"
)]
#[instrument(skip(state), level = "info")]
pub async fn validate_invite_code_endpoint(
    State(state): State<ApiState>,
    Path(code): Path<String>,
) -> Result<Json<ValidateInviteCodeResponse>, (StatusCode, String)> {
    info!("🎫 API: Validating invite code: code={}", code);

    // Validate input
    if code.trim().is_empty() {
        warn!("❌ API: Empty invite code provided");
        return Err((
            StatusCode::BAD_REQUEST,
            "Invite code cannot be empty".to_string(),
        ));
    }

    // Get detailed invite code information
    match get_invite_code_info(&state.pool, &code).await {
        Ok(invite_info) => {
            let is_admin_code = invite_info
                .user_type
                .as_ref()
                .map_or(false, |t| t == "admin");

            let message = if invite_info.is_valid {
                format!(
                    "Invite code is valid for {} user registration",
                    invite_info.user_type.as_deref().unwrap_or("regular")
                )
            } else {
                "Invite code is invalid or expired".to_string()
            };

            info!(
                "✅ API: Invite code validated: code={}, valid={}, user_type={:?}",
                code, invite_info.is_valid, invite_info.user_type
            );

            Ok(Json(ValidateInviteCodeResponse {
                code,
                is_valid: invite_info.is_valid,
                is_admin_code,
                user_type: invite_info.user_type,
                expires_at: invite_info.expires_at,
                message,
            }))
        }
        Err(e) => {
            error!("❌ API: Failed to validate invite code: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate invite code".to_string(),
            ))
        }
    }
}

/// Generate new invite code
#[utoipa::path(
    post,
    path = "/api/v1/invite-code",
    request_body = GenerateInviteCodeRequest,
    responses(
        (status = 200, description = "New invite code generated successfully", body = GenerateInviteCodeResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication"
)]
#[instrument(skip(state), level = "info")]
pub async fn generate_invite_code_endpoint(
    State(state): State<ApiState>,
    Json(request_body): Json<GenerateInviteCodeRequest>,
) -> Result<Json<GenerateInviteCodeResponse>, (StatusCode, String)> {
    info!("🎫 API: Generating new invite code");

    // For now, use a default user ID - TODO: Add proper authentication
    let default_user_id =
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").expect("Valid UUID");

    // Extract request parameters with defaults
    let max_uses = request_body.max_uses.unwrap_or(1);
    let expires_in_hours = request_body.expires_in_hours.unwrap_or(24 * 7); // Default 7 days
    let user_type = request_body
        .user_type
        .unwrap_or_else(|| "regular".to_string());

    // Validate user_type
    if user_type != "admin" && user_type != "regular" {
        warn!("❌ API: Invalid user_type provided: {}", user_type);
        return Err((
            StatusCode::BAD_REQUEST,
            "user_type must be either 'admin' or 'regular'".to_string(),
        ));
    }

    info!(
        "🎫 API: Generating {} invite code with max_uses={}, expires_in_hours={}",
        user_type, max_uses, expires_in_hours
    );

    // Generate invite code
    match generate_invite_code(
        &state.pool,
        default_user_id,
        Some(max_uses),
        Some(expires_in_hours),
        Some(user_type.clone()),
    )
    .await
    {
        Ok(code) => {
            let expires_at =
                Some(chrono::Utc::now() + chrono::Duration::hours(expires_in_hours as i64));
            let is_admin_code = user_type == "admin";

            info!(
                "✅ API: Generated {} invite code: {} for user: {}",
                user_type, code, default_user_id
            );

            Ok(Json(GenerateInviteCodeResponse {
                code,
                max_uses,
                expires_at,
                user_type,
                is_admin_code,
                message: format!(
                    "Invite code for {} users generated successfully",
                    if is_admin_code { "admin" } else { "regular" }
                ),
            }))
        }
        Err(e) => {
            error!("❌ API: Failed to generate invite code: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate invite code".to_string(),
            ))
        }
    }
}
