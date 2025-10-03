//! Admin handlers for the Arda Credit API
//!
//! This module contains all admin-specific API endpoints including:
//! - KYC management and approval workflows
//! - User administration and bulk operations
//! - System statistics and monitoring
//! - Administrative configuration management
//!
//! All routes are documented with OpenAPI and use consistent error handling.
//! These endpoints require admin-level authentication.
//!
//! NOTE: These are placeholder implementations for compilation testing.
//! TODO: Implement proper database integration.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

// Re-export types for OpenAPI documentation
pub use crate::api::openapi::*;

// Explicitly import types needed (these will shadow the glob re-exports)
use crate::config::types::ApiState;
// Re-export explicitly to avoid glob conflict
pub use crate::config::types::UserType;
pub use crate::handlers::users::users::{UserDetailsResponse, identity_to_user_details};
use arda_credit_db::{get_kyc_statistics, list_pending_kyc_applications, update_kyc_status};

// ============================================================================
// ADMIN-SPECIFIC TYPES
// ============================================================================

/// Admin view of user identity (wrapper around database Identity for OpenAPI)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AdminUserIdentity {
    pub id: String,
    pub email: String,
    pub user_type: String,
    pub kyc_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

impl From<serde_json::Value> for AdminUserIdentity {
    fn from(_value: serde_json::Value) -> Self {
        Self {
            id: "placeholder-id".to_string(),
            email: "admin@example.com".to_string(),
            user_type: "admin".to_string(),
            kyc_status: "approved".to_string(),
            created_at: chrono::Utc::now(),
            first_name: Some("Admin".to_string()),
            last_name: Some("User".to_string()),
        }
    }
}

/// Admin user list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AdminUserListResponse {
    pub users: Vec<AdminUserIdentity>,
    pub total_count: i64,
    pub page: u32,
    pub limit: u32,
}

/// Bulk operation response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkOperationResponse {
    pub total_requested: usize,
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

// ============================================================================
// KYC MANAGEMENT TYPES
// ============================================================================

/// Query parameters for listing pending KYC applications
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListPendingKycQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Response for listing pending KYC applications
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListPendingKycResponse {
    pub applications: Vec<UserDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Response for KYC statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct KycStatisticsResponse {
    pub total_users: u64,
    pub approved_count: u64,
    pub pending_count: u64,
    pub rejected_count: u64,
    pub not_submitted_count: u64,
    pub approval_rate: f64, // percentage of users with approved KYC
}

/// Response for platform metrics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PlatformMetricsResponse {
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: String, // String to handle large decimal values
    pub total_pools: i64,
    pub total_pool_funding: String,
    pub total_credit_requests: i64,
    pub active_credit_lines: i64,
}

/// Request for bulk KYC approval
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkKycApprovalRequest {
    pub user_ids: Vec<String>, // Array of user IDs to approve
    pub notes: Option<String>,
}

/// Response for bulk KYC approval
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkKycApprovalResponse {
    pub approved_count: usize,
    pub failed_count: usize,
    pub failed_user_ids: Vec<String>,
    pub message: String,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to parse UUID from string
fn parse_user_id(user_id: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(user_id).map_err(|_| format!("Invalid user ID format: {}", user_id))
}

/// List pending KYC applications (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/kyc/pending",
    params(ListPendingKycQuery),
    responses(
        (status = 200, description = "List of pending KYC applications", body = ListPendingKycResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_pending_kyc_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListPendingKycQuery>,
) -> Result<Json<ListPendingKycResponse>, (StatusCode, String)> {
    info!("🔍 API: Listing pending KYC applications");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match list_pending_kyc_applications(&state.pool, Some(limit as i64), Some(offset as i64)).await
    {
        Ok(applications) => {
            let total_count = applications.len();
            let has_more = total_count >= limit as usize;

            let application_responses: Vec<UserDetailsResponse> = applications
                .into_iter()
                .map(identity_to_user_details)
                .collect();

            let response = ListPendingKycResponse {
                applications: application_responses,
                total_count,
                has_more,
            };

            info!(
                "✅ API: Found {} pending KYC applications",
                response.total_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list pending KYC applications: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve pending KYC applications".to_string(),
            ))
        }
    }
}

/// Get KYC statistics (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/kyc/stats",
    responses(
        (status = 200, description = "KYC statistics", body = KycStatisticsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_kyc_statistics_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<KycStatisticsResponse>, (StatusCode, String)> {
    info!("📊 API: Getting KYC statistics");

    match get_kyc_statistics(&state.pool).await {
        Ok(stats) => {
            let total_processed = stats.approved_count + stats.rejected_count;
            let approval_rate = if total_processed > 0 {
                (stats.approved_count as f64 / total_processed as f64) * 100.0
            } else {
                0.0
            };

            let response = KycStatisticsResponse {
                total_users: stats.total_users,
                approved_count: stats.approved_count,
                pending_count: stats.pending_count,
                rejected_count: stats.rejected_count,
                not_submitted_count: stats.not_submitted_count,
                approval_rate,
            };

            info!(
                "✅ API: KYC stats - Total: {}, Approved: {}, Pending: {}, Rejected: {}, Not Submitted: {}, Approval Rate: {:.1}%",
                response.total_users,
                response.approved_count,
                response.pending_count,
                response.rejected_count,
                response.not_submitted_count,
                response.approval_rate
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to get KYC statistics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve KYC statistics".to_string(),
            ))
        }
    }
}

/// Bulk approve KYC applications (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/admin/kyc/bulk-approve",
    request_body = BulkKycApprovalRequest,
    responses(
        (status = 200, description = "Bulk KYC approval completed", body = BulkKycApprovalResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
pub async fn bulk_kyc_approval_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<BulkKycApprovalRequest>,
) -> Result<Json<BulkKycApprovalResponse>, (StatusCode, String)> {
    info!(
        "🔄 API: Bulk approving {} KYC applications",
        request.user_ids.len()
    );

    if request.user_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "At least one user ID must be provided".to_string(),
        ));
    }

    let mut approved_count = 0;
    let mut failed_user_ids = Vec::new();

    for user_id_str in &request.user_ids {
        match parse_user_id(user_id_str) {
            Ok(user_id) => {
                match update_kyc_status(&state.pool, user_id, "approved", request.notes.as_deref())
                    .await
                {
                    Ok(_) => {
                        approved_count += 1;
                        info!("✅ Approved KYC for user: {}", user_id);
                    }
                    Err(e) => {
                        error!("❌ Failed to approve KYC for user {}: {}", user_id, e);
                        failed_user_ids.push(user_id_str.clone());
                    }
                }
            }
            Err(_) => {
                warn!("⚠️ Invalid user ID format: {}", user_id_str);
                failed_user_ids.push(user_id_str.clone());
            }
        }
    }

    let failed_count = failed_user_ids.len();
    let message = if failed_count == 0 {
        format!("Successfully approved {} KYC applications", approved_count)
    } else {
        format!(
            "Approved {} KYC applications, {} failed",
            approved_count, failed_count
        )
    };

    let response = BulkKycApprovalResponse {
        approved_count,
        failed_count,
        failed_user_ids,
        message,
    };

    info!(
        "✅ API: Bulk KYC approval completed - Approved: {}, Failed: {}",
        approved_count, failed_count
    );
    Ok(Json(response))
}

/// Get platform metrics (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/platform-metrics",
    responses(
        (status = 200, description = "Platform metrics", body = PlatformMetricsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_platform_metrics_endpoint(
    State(_state): State<ApiState>,
) -> Result<Json<PlatformMetricsResponse>, (StatusCode, String)> {
    info!("📊 API: Getting platform metrics");

    // For now, return mock data to avoid SQLx cache issues
    // TODO: Replace with proper database queries once SQLx cache is updated
    let response = PlatformMetricsResponse {
        total_loans: 55, // We know from direct query this is correct
        active_loans: 55,
        total_loan_volume: "0.00".to_string(), // TODO: Calculate from credit lines
        total_pools: 20,
        total_pool_funding: "448500000".to_string(), // From populate script: $448.5M total
        total_credit_requests: 120,                  // From populate script
        active_credit_lines: 55,
    };

    info!(
        "✅ API: Platform metrics - Credit Lines: {}, Pools: {}, Requests: {}, Funding: {}",
        response.active_credit_lines,
        response.total_pools,
        response.total_credit_requests,
        response.total_pool_funding
    );
    Ok(Json(response))
}

// Additional admin endpoints can be added here as needed
// For now, keeping this file focused on core admin operations
