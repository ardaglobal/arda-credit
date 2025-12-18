use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use tracing::{debug, info, instrument, warn};

use crate::services::session_service::SessionError;

/// Configuration for session-based authentication
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub enabled: bool,
    pub cookie_name: String,
    pub allow_anonymous_endpoints: Vec<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cookie_name: "session_token".to_string(),
            allow_anonymous_endpoints: vec![
                "/health".to_string(),
                "/".to_string(),
                "/playground".to_string(),
                "/metrics".to_string(),
                "/api/v1/auth/magic-link".to_string(),
                "/api/v1/auth/verify".to_string(),
                "/api/v1/user".to_string(), // User registration
                "/api-docs".to_string(),    // OpenAPI docs
                "/api-docs/openapi.json".to_string(),
                "/api/v1/collateral/batch".to_string(), // Collateral batch creation (for document ingestion)
            ],
        }
    }
}

/// Session-based authentication middleware (replaces JWT middleware)
#[instrument(skip(api_state, request, next), level = "debug")]
pub async fn session_auth_middleware(
    api_state: crate::api::router::ApiState,
    mut request: Request,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    let config = SessionConfig::default();

    if !config.enabled {
        debug!("Session authentication disabled");
        return Ok(next.run(request).await);
    }

    // NOTE: use owned String to avoid borrowing `request` across later mutations
    let path = request.uri().path().to_string();

    // Check if endpoint allows anonymous access
    // NOTE: `starts_with("/")` matches *every* path. Treat "/" as an exact match only.
    if config.allow_anonymous_endpoints.iter().any(|ep| {
        if ep == "/" {
            path == "/"
        } else {
            path.as_str().starts_with(ep)
        }
    }) {
        debug!("Anonymous access allowed for path: {}", path);

        return Ok(next.run(request).await);
    }

    // Extract session token from cookie header (manually parse)
    let session_token =
        match extract_session_token_from_cookie_header(request.headers(), &config.cookie_name) {
            Some(token) => token,
            None => {
                warn!("No session cookie found for path: {}", path);

                return Ok(create_unauthorized_response("Session required"));
            }
        };

    // Validate session and get user identity
    match api_state
        .session_service
        .validate_session(&session_token)
        .await
    {
        Ok(identity) => {
            info!(
                "✅ Session valid for user: {} ({}) - path: {}",
                identity.email, identity.user_type, path
            );

            // Add user information to request headers for downstream handlers
            let headers = request.headers_mut();
            headers.insert("x-user-id", identity.id.to_string().parse().unwrap());
            headers.insert("x-user-email", identity.email.parse().unwrap());
            headers.insert("x-user-type", identity.user_type.parse().unwrap());

            info!(
                "📌 Added headers: x-user-id={}, x-user-email={}",
                identity.id, identity.email
            );

            Ok(next.run(request).await)
        }
        Err(SessionError::NotFound) => {
            warn!("Invalid or expired session for path: {}", path);

            Ok(create_unauthorized_response("Session expired or invalid"))
        }
        Err(SessionError::Database(e)) => {
            warn!("Database error during session validation: {}", e);
            Ok(create_server_error_response(
                "Authentication service unavailable",
            ))
        }
        Err(e) => {
            warn!("Session validation error: {:?}", e);
            Ok(create_unauthorized_response("Session validation failed"))
        }
    }
}

/// Create unauthorized response
fn create_unauthorized_response(message: &str) -> Response {
    let error_body = json!({
        "error": "AUTHENTICATION_REQUIRED",
        "message": message,
        "timestamp": Utc::now()
    });

    (StatusCode::UNAUTHORIZED, axum::Json(error_body)).into_response()
}

/// Create server error response
fn create_server_error_response(message: &str) -> Response {
    let error_body = json!({
        "error": "AUTHENTICATION_SERVICE_ERROR",
        "message": message,
        "timestamp": Utc::now()
    });

    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(error_body)).into_response()
}

/// Extract session token from cookie header
fn extract_session_token_from_cookie_header(
    headers: &HeaderMap,
    cookie_name: &str,
) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;

    // Parse cookies to find the session token
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(eq_pos) = cookie.find('=') {
            let (name, value) = cookie.split_at(eq_pos);
            if name.trim() == cookie_name {
                return Some(value[1..].to_string()); // Skip the '=' character
            }
        }
    }

    None
}

/// Helper to extract current user from request headers (for handlers)
pub fn extract_current_user(headers: &HeaderMap) -> Option<CurrentUser> {
    let user_id = headers.get("x-user-id")?.to_str().ok()?;
    let email = headers.get("x-user-email")?.to_str().ok()?;
    let user_type = headers.get("x-user-type")?.to_str().ok()?;

    Some(CurrentUser {
        id: user_id.parse().ok()?,
        email: email.to_string(),
        user_type: user_type.to_string(),
    })
}

/// Current user information extracted from session
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: uuid::Uuid,
    pub email: String,
    pub user_type: String,
}
