//! Magic Link Authentication API Handlers
//!
//! This module contains all magic link authentication endpoints:
//! - Magic link request via email
//! - Magic link verification and session creation
//! - Current user info (session-based)
//! - Session-based logout

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;

use crate::cookie_utils::{create_expired_session_cookie, create_session_cookie};
use crate::magic_link_service::{MagicLinkError, MagicLinkService};
use crate::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
};
use crate::session_middleware::extract_current_user;
use crate::session_service::{SessionError, SessionService};
use crate::types::ApiState;

/// Custom response that includes JSON body and cookie headers
pub struct JsonWithCookie<T: Serialize> {
    body: T,
    cookies: Vec<axum_extra::extract::cookie::Cookie<'static>>,
}

impl<T: Serialize> IntoResponse for JsonWithCookie<T> {
    fn into_response(self) -> Response {
        let mut response = Json(self.body).into_response();
        for cookie in self.cookies {
            response
                .headers_mut()
                .append(header::SET_COOKIE, cookie.to_string().parse().unwrap());
        }
        response
    }
}

// ============================================================================
// MAGIC LINK REQUEST ENDPOINT
// ============================================================================

/// Request a magic link to be sent via email
#[utoipa::path(
    post,
    path = "/api/v1/auth/magic-link",
    request_body = MagicLinkRequest,
    responses(
        (status = 200, description = "Magic link sent successfully", body = MagicLinkResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 429, description = "Rate limit exceeded"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication"
)]
#[instrument(skip(state), level = "info")]
pub async fn request_magic_link(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<MagicLinkRequest>,
) -> Result<Json<MagicLinkResponse>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    let magic_link_service = state.magic_link_service.as_ref().ok_or_else(|| {
        error!("❌ Magic link service not configured");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MagicLinkErrorResponse {
                error: "SERVICE_UNAVAILABLE".to_string(),
                message: "Magic link service not available".to_string(),
                details: None,
                timestamp: Utc::now(),
            }),
        )
    })?;

    info!("🔗 Magic link requested for email: {}", request.email);

    // Extract client information for rate limiting and logging
    let client_ip = extract_client_ip(&headers);
    let user_agent = extract_user_agent(&headers);

    // Generate and send magic link
    match magic_link_service
        .generate_magic_link(
            &request.email,
            request.redirect_uri.clone(),
            client_ip,
            user_agent,
        )
        .await
    {
        Ok(message_id) => {
            info!(
                "✅ Magic link sent successfully: email={}, message_id={}",
                request.email, message_id
            );

            Ok(Json(MagicLinkResponse {
                success: true,
                message: "Magic link sent to your email address".to_string(),
                message_id: Some(message_id),
                expires_in_minutes: 10, // From config
                timestamp: Utc::now(),
            }))
        }
        Err(MagicLinkError::UserNotFound(email)) => {
            warn!("❌ Magic link request for non-existent user: {}", email);
            Err((
                StatusCode::NOT_FOUND,
                Json(MagicLinkErrorResponse {
                    error: "USER_NOT_FOUND".to_string(),
                    message: "No account found with this email address".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::RateLimit {
            client_id: _,
            limit,
            window_seconds,
            retry_after_seconds,
        }) => {
            warn!("❌ Rate limit exceeded for email: {}", request.email);
            Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(MagicLinkErrorResponse {
                    error: "RATE_LIMIT_EXCEEDED".to_string(),
                    message: "Too many magic link requests. Please try again later.".to_string(),
                    details: Some(serde_json::json!({
                        "limit": limit,
                        "window_seconds": window_seconds,
                        "retry_after_seconds": retry_after_seconds
                    })),
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::Email(email_error)) => {
            error!("❌ Email service error: {}", email_error);
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(MagicLinkErrorResponse {
                    error: "EMAIL_SERVICE_ERROR".to_string(),
                    message: "Unable to send email at this time. Please try again later."
                        .to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(e) => {
            error!("❌ Unexpected error generating magic link: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MagicLinkErrorResponse {
                    error: "INTERNAL_ERROR".to_string(),
                    message: "An unexpected error occurred. Please try again.".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
    }
}

// ============================================================================
// MAGIC LINK VERIFICATION ENDPOINT
// ============================================================================

/// Verify a magic link token and create a session
#[utoipa::path(
    post,
    path = "/api/v1/auth/verify",
    request_body = VerifyMagicLinkRequest,
    responses(
        (status = 200, description = "Magic link verified successfully", body = VerifyMagicLinkResponse),
        (status = 400, description = "Invalid token or state parameter"),
        (status = 404, description = "Magic link not found or expired"),
        (status = 410, description = "Magic link already used"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication"
)]
#[instrument(skip(state), level = "info")]
pub async fn verify_magic_link(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<VerifyMagicLinkRequest>,
) -> Result<JsonWithCookie<VerifyMagicLinkResponse>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    let magic_link_service = state.magic_link_service.as_ref().ok_or_else(|| {
        error!("❌ Magic link service not configured");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MagicLinkErrorResponse {
                error: "SERVICE_UNAVAILABLE".to_string(),
                message: "Magic link service not available".to_string(),
                details: None,
                timestamp: Utc::now(),
            }),
        )
    })?;

    info!("🔓 Magic link verification attempt");

    // Extract client information
    let client_ip = extract_client_ip(&headers);
    let user_agent = extract_user_agent(&headers);

    // Verify magic link and create session
    match magic_link_service
        .verify_magic_link(&request.token, &request.state, client_ip, user_agent)
        .await
    {
        Ok((session_token, session)) => {
            // Get user information
            let user = match state.session_service.validate_session(&session_token).await {
                Ok(identity) => identity,
                Err(e) => {
                    error!("❌ Failed to validate newly created session: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(MagicLinkErrorResponse {
                            error: "SESSION_ERROR".to_string(),
                            message: "Authentication succeeded but session creation failed"
                                .to_string(),
                            details: None,
                            timestamp: Utc::now(),
                        }),
                    ));
                }
            };

            // Create session cookie
            let session_cookie = create_session_cookie(session_token.clone(), 30, None); // 30 days

            info!("✅ Magic link verified successfully: user={}", user.email);

            Ok(JsonWithCookie {
                body: VerifyMagicLinkResponse {
                    success: true,
                    message: "Authentication successful".to_string(),
                    redirect_uri: request.redirect_uri,
                    user: UserInfo {
                        user_id: user.id.to_string(),
                        email: user.email,
                        user_type: user.user_type,
                        kyc_status: "unknown".to_string(), // Would need to fetch from DB for full info
                        email_verified: true, // Assume verified since they used magic link
                    },
                    timestamp: Utc::now(),
                },
                cookies: vec![session_cookie],
            })
        }
        Err(MagicLinkError::NotFound) => {
            warn!("❌ Magic link not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(MagicLinkErrorResponse {
                    error: "MAGIC_LINK_NOT_FOUND".to_string(),
                    message: "Magic link not found or has expired".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::AlreadyConsumed) => {
            warn!("❌ Magic link already used");
            Err((
                StatusCode::GONE,
                Json(MagicLinkErrorResponse {
                    error: "MAGIC_LINK_ALREADY_USED".to_string(),
                    message: "This magic link has already been used".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::Expired) => {
            warn!("❌ Magic link expired");
            Err((
                StatusCode::GONE,
                Json(MagicLinkErrorResponse {
                    error: "MAGIC_LINK_EXPIRED".to_string(),
                    message: "This magic link has expired. Please request a new one.".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::InvalidState) => {
            warn!("❌ Invalid state parameter (CSRF)");
            Err((
                StatusCode::BAD_REQUEST,
                Json(MagicLinkErrorResponse {
                    error: "INVALID_STATE".to_string(),
                    message: "Invalid request. Please start over.".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(MagicLinkError::InvalidToken) => {
            warn!("❌ Invalid token format");
            Err((
                StatusCode::BAD_REQUEST,
                Json(MagicLinkErrorResponse {
                    error: "INVALID_TOKEN".to_string(),
                    message: "Invalid magic link format".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
        Err(e) => {
            error!("❌ Unexpected error verifying magic link: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MagicLinkErrorResponse {
                    error: "VERIFICATION_ERROR".to_string(),
                    message: "Unable to verify magic link. Please try again.".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
    }
}

// ============================================================================
// CURRENT USER ENDPOINT
// ============================================================================

/// Get current authenticated user information
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current user information", body = UserInfo),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication",
    security(("session_cookie" = []))
)]
#[instrument(skip(state), level = "info")]
pub async fn get_current_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<UserInfo>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    // Extract current user from session middleware headers
    match extract_current_user(&headers) {
        Some(current_user) => {
            info!("✅ Current user retrieved: {}", current_user.email);

            Ok(Json(UserInfo {
                user_id: current_user.id.to_string(),
                email: current_user.email,
                user_type: current_user.user_type,
                kyc_status: "unknown".to_string(), // Would need to fetch from DB for full info
                email_verified: true,              // Assume verified since they used magic link
            }))
        }
        None => {
            warn!("❌ No authenticated user found");
            Err((
                StatusCode::UNAUTHORIZED,
                Json(MagicLinkErrorResponse {
                    error: "NOT_AUTHENTICATED".to_string(),
                    message: "Authentication required".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                }),
            ))
        }
    }
}

// ============================================================================
// LOGOUT ENDPOINT
// ============================================================================

/// Logout and invalidate current session
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful"),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication",
    security(("session_cookie" = []))
)]
#[instrument(skip(state), level = "info")]
/// Logout endpoint for magic link authentication
pub async fn logout_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    // This is a simple logout that just returns success
    // In session-based auth, the client should remove the session cookie
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Logout successful. Please remove the session cookie from client storage."
    })))
}

pub async fn logout(
    State(state): State<ApiState>,
    cookies: CookieJar,
    headers: HeaderMap,
) -> Result<JsonWithCookie<serde_json::Value>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    let magic_link_service = state.magic_link_service.as_ref();
    // Extract session token from cookie
    let session_token = cookies
        .get("session_token")
        .map(|cookie| cookie.value().to_string());

    if let Some(token) = session_token {
        // Invalidate session in database
        match state.session_service.invalidate_session(&token).await {
            Ok(_) => {
                info!("✅ Session invalidated successfully");
            }
            Err(SessionError::NotFound) => {
                warn!("⚠️ Session not found during logout (already expired?)");
            }
            Err(e) => {
                error!("❌ Error invalidating session: {}", e);
                // Continue with logout anyway
            }
        }
    }

    // Create expired cookie to clear session
    let expired_cookie = create_expired_session_cookie();

    Ok(JsonWithCookie {
        body: serde_json::json!({
            "success": true,
            "message": "Logged out successfully",
            "timestamp": Utc::now()
        }),
        cookies: vec![expired_cookie],
    })
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Extract client IP from request headers
fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    // Try common proxy headers first
    let ip_str = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-client-ip"))
        .and_then(|header| header.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim());

    ip_str.map(|s| s.to_string())
}

/// Extract user agent from request headers
fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|header| header.to_str().ok())
        .map(|s| s.to_string())
}
