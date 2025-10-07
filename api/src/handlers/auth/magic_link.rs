//! Magic Link Authentication API Handlers
//!
//! This module contains all magic link authentication endpoints:
//! - Magic link request via email
//! - Magic link verification and session creation
//! - Current user info (session-based)
//! - Session-based logout

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use serde::Serialize;
use tracing::{error, info, instrument, warn};

use crate::config::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
};
use crate::config::types::ApiState;
use crate::middleware::cookie_utils::{create_expired_session_cookie, create_session_cookie};
use crate::services::magic_link_service::MagicLinkError;
use crate::services::session_service::SessionError;

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
    tag = "Auth"
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
///
/// This endpoint verifies a magic link token and creates an authenticated session.
/// It also handles automatic team invitation acceptance if the magic link includes
/// an invitation token in the redirect_uri.
///
/// ## Flow:
/// 1. User clicks magic link in their email
/// 2. Frontend calls this endpoint with token and state from URL
/// 3. System verifies the magic link and creates a session
/// 4. If redirect_uri contains an invitation token (e.g., `/invitation/accept?token=ABC123`):
///    - System automatically accepts the team invitation
///    - User is added to the company with the specified role and profile type
/// 5. User is authenticated and can access the application
///
/// ## Team Invitation Integration:
/// When this endpoint is called with a magic link from a team invitation email,
/// the invitation is automatically accepted upon successful authentication.
/// The frontend should check the redirect_uri to navigate the user appropriately.
#[utoipa::path(
    post,
    path = "/api/v1/auth/verify",
    request_body = VerifyMagicLinkRequest,
    responses(
        (status = 200, description = "Magic link verified successfully. If invitation token present, invitation is also accepted.", body = VerifyMagicLinkResponse),
        (status = 400, description = "Invalid token or state parameter"),
        (status = 404, description = "Magic link not found or expired"),
        (status = 410, description = "Magic link already used"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Auth"
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
        Ok((session_token, _session)) => {
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

            // Check if this is a team invitation acceptance (invitation token in redirect_uri)
            if let Some(ref redirect) = request.redirect_uri {
                if let Some(invitation_token) = extract_invitation_token(redirect) {
                    info!("🎫 Team invitation token found in redirect, accepting invitation");

                    // Accept the invitation if TeamInvitationService is available
                    if let Some(team_invitation_service) = &state.team_invitation_service {
                        match team_invitation_service
                            .accept_invitation(&invitation_token, user.id)
                            .await
                        {
                            Ok(invitation) => {
                                info!(
                                    "✅ Team invitation accepted: user={}, company={}, role={}, profile={}",
                                    user.email, invitation.company_id, invitation.role, invitation.profile_type
                                );
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to accept team invitation: {}", e);
                                // Don't fail the authentication, just log the error
                                // The user can still sign in, but the invitation won't be accepted
                            }
                        }
                    }
                }
            }

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
                    session_token: Some(session_token),
                    expires_in_seconds: 30 * 24 * 60 * 60, // 30 days in seconds
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
    tag = "Auth",
    security(("session_cookie" = []))
)]
#[instrument(skip(state), level = "info")]
pub async fn get_current_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<UserInfo>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    // Extract session token from cookie header
    let session_token = extract_session_token_from_headers(&headers).ok_or_else(|| {
        warn!("❌ No session cookie found");
        (
            StatusCode::UNAUTHORIZED,
            Json(MagicLinkErrorResponse {
                error: "NOT_AUTHENTICATED".to_string(),
                message: "Authentication required".to_string(),
                details: None,
                timestamp: Utc::now(),
            }),
        )
    })?;

    // Validate session and get user identity
    match state.session_service.validate_session(&session_token).await {
        Ok(identity) => {
            info!("✅ Current user retrieved: {}", identity.email);

            Ok(Json(UserInfo {
                user_id: identity.id.to_string(),
                email: identity.email,
                user_type: identity.user_type,
                kyc_status: "unknown".to_string(), // Would need to fetch from DB for full info
                email_verified: true,              // Assume verified since they used magic link
            }))
        }
        Err(e) => {
            warn!("❌ Session validation failed: {}", e);
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
    tag = "Auth",
    security(("session_cookie" = []))
)]
#[instrument(skip(_state), level = "info")]
/// Logout endpoint for magic link authentication
pub async fn magic_link_logout_endpoint(
    State(_state): State<ApiState>,
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
    _headers: HeaderMap,
) -> Result<JsonWithCookie<serde_json::Value>, (StatusCode, Json<MagicLinkErrorResponse>)> {
    let _magic_link_service = state.magic_link_service.as_ref();
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

/// Extract session token from cookie header
fn extract_session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;

    // Parse cookies to find session_token
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(eq_pos) = cookie.find('=') {
            let (name, value) = cookie.split_at(eq_pos);
            if name.trim() == "session_token" {
                return Some(value[1..].to_string()); // Skip the '=' character
            }
        }
    }

    None
}

/// Extract invitation token from redirect URI
/// Looks for patterns like "/invitation/accept?token=ABC123" or "?token=ABC123"
fn extract_invitation_token(redirect_uri: &str) -> Option<String> {
    // Parse the redirect URI to extract the token query parameter
    if let Some(query_start) = redirect_uri.find('?') {
        let query = &redirect_uri[query_start + 1..];
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "token" {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}
