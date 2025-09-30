//! Health check handlers for the Arda Credit API
//!
//! This module contains health-related API endpoints:
//! - Health check endpoint for monitoring service status
//! - API information endpoint for discovery
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{extract::State, http::StatusCode, response::Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::ToSchema;

// OpenTelemetry imports
use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer},
    Context, KeyValue,
};

use crate::config::paths;
use crate::config::types::ApiState;

// ============================================================================
// HEALTH CHECK TYPES
// ============================================================================

/// Health check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub database_connected: bool,
    pub api_version: String,
    pub git_tag: String,
    pub git_sha: String,
    pub git_sha_short: String,
    pub git_dirty: bool,
}

/// API information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiInfoResponse {
    pub server_name: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub description: String,
}

// ============================================================================
// HEALTH CHECK ENDPOINTS
// ============================================================================

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Service is unhealthy")
    ),
    tag = "Health"
)]
#[instrument(skip(state), level = "info")]
pub async fn health_check(
    State(state): State<ApiState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    // Get the global tracer
    let tracer = global::tracer("arda-credit-api");

    // Create a span for the health check operation
    let span = tracer.start("health_check");
    let cx = Context::current_with_span(span);

    // Set attributes on the span
    cx.span().set_attributes(vec![
        KeyValue::new("endpoint", "/api/v1/health"),
        KeyValue::new("operation", "health_check"),
        KeyValue::new("component", "api"),
    ]);

    info!("🔍 API: Health check requested");

    // Add event for the start of health check
    cx.span().add_event(
        "health_check_started",
        vec![KeyValue::new("timestamp", Utc::now().to_rfc3339())],
    );

    // Test database connection with tracing
    let db_connected = {
        let db_span = tracer.start("database_health_check");
        let db_cx = cx.with_span(db_span);

        db_cx.span().set_attributes(vec![
            KeyValue::new("db.system", "postgresql"),
            KeyValue::new("db.operation", "health_check"),
            KeyValue::new("db.statement", "SELECT 1"),
        ]);

        match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
            Ok(_) => {
                db_cx
                    .span()
                    .set_attribute(KeyValue::new("db.success", true));
                db_cx.span().add_event(
                    "database_connection_success",
                    vec![KeyValue::new("query", "SELECT 1")],
                );
                true
            }
            Err(e) => {
                db_cx.span().set_attributes(vec![
                    KeyValue::new("db.success", false),
                    KeyValue::new("error.message", e.to_string()),
                    KeyValue::new("error.type", "database_connection_error"),
                ]);
                db_cx.span().add_event(
                    "database_connection_failed",
                    vec![
                        KeyValue::new("error", e.to_string()),
                        KeyValue::new("query", "SELECT 1"),
                    ],
                );
                error!("Database health check failed: {}", e);
                false
            }
        }
    };

    let response = HealthResponse {
        status: if db_connected {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Utc::now(),
        database_connected: db_connected,
        api_version: state.config.version.clone(),
        git_tag: std::env::var("GIT_TAG").unwrap_or_else(|_| "unknown".to_string()),
        git_sha: std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".to_string()),
        git_sha_short: std::env::var("GIT_SHA_SHORT").unwrap_or_else(|_| "unknown".to_string()),
        git_dirty: std::env::var("GIT_DIRTY")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false),
    };

    // Final span attributes
    cx.span().set_attributes(vec![
        KeyValue::new("health.status", response.status.clone()),
        KeyValue::new("database.connected", db_connected),
        KeyValue::new("api.version", response.api_version.clone()),
    ]);

    if db_connected {
        info!("✅ API: Health check passed");
        cx.span().add_event(
            "health_check_completed",
            vec![KeyValue::new("result", "success")],
        );
        Ok(Json(response))
    } else {
        error!("❌ API: Health check failed - database not connected");
        cx.span().set_attributes(vec![
            KeyValue::new("error", true),
            KeyValue::new("error.message", "Database not available"),
        ]);
        cx.span().add_event(
            "health_check_completed",
            vec![KeyValue::new("result", "failure")],
        );
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available".to_string(),
        ))
    }
}

/// API information endpoint
#[instrument(skip(state), level = "info")]
/// Get API information
#[utoipa::path(
    get,
    path = "/api/v1/info",
    responses(
        (status = 200, description = "API information", body = ApiInfoResponse),
    ),
    tag = "Info"
)]
pub async fn api_info(State(state): State<ApiState>) -> Json<ApiInfoResponse> {
    info!("📋 API: API info requested");

    let endpoints = paths::get_all_endpoints();

    let response = ApiInfoResponse {
        server_name: state.config.server_name.clone(),
        version: state.config.version.clone(),
        timestamp: Utc::now(),
        endpoints,
    };

    info!("✅ API: API info returned");
    Json(response)
}
