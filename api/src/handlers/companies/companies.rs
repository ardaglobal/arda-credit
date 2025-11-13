//! Company management API handlers
//!
//! This module provides REST API endpoints for company registration,
//! KYB verification, and team management functionality.

use crate::{
    api::router::ApiState,
    services::auth::Claims,
    services::company_service::{
        CompanyRegistrationRequest, CompanyService, KybSubmissionRequest, TeamInvitationRequest,
        TeamMemberPermissions,
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

/// Helper function to extract session from headers and create Claims
///
/// This function extracts the session token from the cookie header,
/// validates it, and converts the user identity to Claims for compatibility
/// with existing code that expects JWT Claims.
async fn extract_claims_from_session(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<Claims, (StatusCode, String)> {
    // Extract session token from cookie header
    let session_token = extract_session_token_from_headers(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Authentication required. No session cookie found.".to_string(),
        )
    })?;

    // Validate session and get user identity
    let identity = state
        .session_service
        .validate_session(&session_token)
        .await
        .map_err(|e| {
            error!("Failed to validate session: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                "Authentication required. Invalid or expired session.".to_string(),
            )
        })?;

    // Convert identity to Claims
    Ok(Claims {
        sub: identity.id.to_string(),
        email: identity.email,
        user_type: identity.user_type,
        exp: 0,             // Not used for session-based auth
        iat: 0,             // Not used for session-based auth
        jti: session_token, // Use session token as JTI
    })
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

/// Register a new company
#[utoipa::path(
    post,
    path = "/api/v1/companies/register",
    request_body = CompanyRegistrationRequest,
    responses(
        (status = 201, description = "Company registered successfully", body = crate::services::company_service::CompanyResponseDto),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Company already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn register_company_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CompanyRegistrationRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service.register_company(&claims, request).await {
        Ok(company) => {
            info!(
                company_id = %company.id,
                company_name = %company.company_name,
                "Company registered successfully"
            );
            (StatusCode::CREATED, Json(company)).into_response()
        }
        Err(err) => {
            error!(error = %err, "Failed to register company");
            match err {
                crate::services::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Get company details
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    responses(
        (status = 200, description = "Company details retrieved", body = crate::services::company_service::CompanyResponseDto),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn get_company_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service.get_company(&claims, company_id).await {
        Ok(company) => Json(company).into_response(),
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to get company");
            match err {
                crate::services::company_service::CompanyServiceError::CompanyNotFound {
                    ..
                } => (StatusCode::NOT_FOUND, Json("Company not found")).into_response(),
                crate::services::company_service::CompanyServiceError::TeamMemberNotFound {
                    ..
                } => (StatusCode::FORBIDDEN, Json("Access denied")).into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Submit KYB documentation
#[utoipa::path(
    post,
    path = "/api/v1/companies/{company_id}/kyb",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = KybSubmissionRequest,
    responses(
        (status = 200, description = "KYB submitted successfully", body = crate::services::company_service::CompanyResponseDto),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn submit_kyb_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<KybSubmissionRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service
        .submit_kyb(&claims, company_id, request)
        .await
    {
        Ok(company) => {
            info!(
                company_id = %company.id,
                "KYB documentation submitted"
            );
            Json(company).into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to submit KYB");
            match err {
                crate::services::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::services::company_service::CompanyServiceError::CompanyNotFound {
                    ..
                } => (StatusCode::NOT_FOUND, Json("Company not found")).into_response(),
                crate::services::company_service::CompanyServiceError::AccessDenied { .. } => {
                    (StatusCode::FORBIDDEN, Json("Access denied")).into_response()
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Get KYB status
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct KybStatusResponse {
    pub company_id: Uuid,
    pub kyb_status: String,
    pub kyb_submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub kyb_verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Get KYB status
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/kyb/status",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    responses(
        (status = 200, description = "KYB status retrieved", body = KybStatusResponse),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn get_kyb_status_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service.get_company(&claims, company_id).await {
        Ok(company) => {
            let response = KybStatusResponse {
                company_id: company.id,
                kyb_status: company.kyb_status,
                kyb_submitted_at: company.kyb_submitted_at,
                kyb_verified_at: company.kyb_verified_at,
            };
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to get KYB status");
            match err {
                crate::services::company_service::CompanyServiceError::CompanyNotFound {
                    ..
                } => (StatusCode::NOT_FOUND, Json("Company not found")).into_response(),
                crate::services::company_service::CompanyServiceError::TeamMemberNotFound {
                    ..
                } => (StatusCode::FORBIDDEN, Json("Access denied")).into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Invite team member
#[utoipa::path(
    post,
    path = "/api/v1/companies/{company_id}/team/invite",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = TeamInvitationRequest,
    responses(
        (status = 201, description = "Team member invited", body = crate::services::company_service::TeamMemberResponseDto),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied"),
        (status = 409, description = "Team member already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn invite_team_member_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<TeamInvitationRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service
        .invite_team_member(&claims, company_id, request)
        .await
    {
        Ok(team_member) => {
            info!(
                company_id = %company_id,
                user_id = %team_member.user_id,
                role = %team_member.role,
                "Team member invited"
            );
            (StatusCode::CREATED, Json(team_member)).into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to invite team member");
            match err {
                crate::services::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::services::company_service::CompanyServiceError::TeamMemberAlreadyExists { .. } => {
                    (StatusCode::CONFLICT, Json("Team member already exists")).into_response()
                }
                crate::services::company_service::CompanyServiceError::AccessDenied { .. } => {
                    (StatusCode::FORBIDDEN, Json("Access denied")).into_response()
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// List team members
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/team",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    responses(
        (status = 200, description = "Team members list retrieved", body = Vec<crate::services::company_service::TeamMemberResponseDto>),
        (status = 403, description = "Access denied"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn list_team_members_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service.list_team_members(&claims, company_id).await {
        Ok(team_members) => Json(team_members).into_response(),
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to list team members");
            match err {
                crate::services::company_service::CompanyServiceError::TeamMemberNotFound {
                    ..
                } => (StatusCode::FORBIDDEN, Json("Access denied")).into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Update team member role request
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateTeamMemberRoleRequest {
    #[validate(length(min = 1, max = 20))]
    pub role: String,
    pub permissions: Option<TeamMemberPermissions>,
}

/// Update team member role
#[utoipa::path(
    put,
    path = "/api/v1/companies/{company_id}/team/{user_id}/role",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    request_body = UpdateTeamMemberRoleRequest,
    responses(
        (status = 200, description = "Team member role updated", body = crate::services::company_service::TeamMemberResponseDto),
        (status = 400, description = "Validation error or cannot remove last admin"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Team member not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn update_team_member_role_endpoint(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<UpdateTeamMemberRoleRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    match company_service
        .update_team_member_role(
            &claims,
            company_id,
            user_id,
            request.role,
            request.permissions,
        )
        .await
    {
        Ok(team_member) => {
            info!(
                company_id = %company_id,
                user_id = %user_id,
                role = %team_member.role,
                "Team member role updated"
            );
            Json(team_member).into_response()
        }
        Err(err) => {
            error!(
                error = %err,
                company_id = %company_id,
                user_id = %user_id,
                "Failed to update team member role"
            );
            match err {
                crate::services::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::services::company_service::CompanyServiceError::CannotRemoveLastAdmin {
                    ..
                } => (
                    StatusCode::BAD_REQUEST,
                    Json("Cannot remove the last admin"),
                )
                    .into_response(),
                crate::services::company_service::CompanyServiceError::AccessDenied { .. } => {
                    (StatusCode::FORBIDDEN, Json("Access denied")).into_response()
                }
                crate::services::company_service::CompanyServiceError::TeamMemberNotFound {
                    ..
                } => (StatusCode::NOT_FOUND, Json("Team member not found")).into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            }
        }
    }
}

/// Remove team member
#[utoipa::path(
    delete,
    path = "/api/v1/companies/{company_id}/team/{user_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 204, description = "Team member removed"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Team member not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn remove_team_member_endpoint(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // For now, we'll implement this by setting status to 'inactive'
    // In a full implementation, you might want to add this method to the CompanyService

    let _company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    // Check if the requester has team management access
    let requester_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json("Invalid user ID")).into_response();
        }
    };

    // This is a simplified version - in a full implementation, you'd add proper validation
    match sqlx::query!(
        r#"
        UPDATE company_memberships
        SET status = 'removed', updated_at = NOW()
        WHERE user_id = $1 AND company_id = $2
        AND EXISTS (
            SELECT 1 FROM company_memberships tm
            WHERE tm.user_id = $3 AND tm.company_id = $2
            AND tm.status = 'active' AND tm.can_manage_team = true
        )
        "#,
        user_id,
        company_id,
        requester_id
    )
    .execute(&state.pool)
    .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                info!(
                    company_id = %company_id,
                    user_id = %user_id,
                    removed_by = %requester_id,
                    "Team member removed"
                );
                StatusCode::NO_CONTENT.into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json("Team member not found or access denied"),
                )
                    .into_response()
            }
        }
        Err(err) => {
            error!(error = %err, "Failed to remove team member");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Internal server error"),
            )
                .into_response()
        }
    }
}

/// Company dashboard stats
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CompanyDashboardResponse {
    pub company: crate::services::company_service::CompanyResponseDto,
    pub team_stats: TeamStats,
    pub document_stats: DocumentStats,
    pub recent_activity: Vec<ActivityItem>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TeamStats {
    pub total_members: i64,
    pub active_members: i64,
    pub pending_invitations: i64,
    pub admin_count: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DocumentStats {
    pub total_documents: i64,
    pub pending_approval: i64,
    pub approved_documents: i64,
    pub storage_size_mb: f64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ActivityItem {
    pub id: Uuid,
    pub activity_type: String,
    pub description: String,
    pub user_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Get company dashboard
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/dashboard",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    responses(
        (status = 200, description = "Company dashboard retrieved", body = CompanyDashboardResponse),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn get_company_dashboard_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Extract claims from session cookie
    let claims = match extract_claims_from_session(&state, &headers).await {
        Ok(claims) => claims,
        Err((status, message)) => {
            error!("Authentication failed: {}", message);
            return (status, Json(message)).into_response();
        }
    };

    // Get company details
    let company = match company_service.get_company(&claims, company_id).await {
        Ok(company) => company,
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to get company for dashboard");
            return match err {
                crate::services::company_service::CompanyServiceError::CompanyNotFound {
                    ..
                } => (StatusCode::NOT_FOUND, Json("Company not found")).into_response(),
                crate::services::company_service::CompanyServiceError::TeamMemberNotFound {
                    ..
                } => (StatusCode::FORBIDDEN, Json("Access denied")).into_response(),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal server error"),
                )
                    .into_response(),
            };
        }
    };

    // Get team stats
    let team_stats = match get_team_stats(&state.pool, company_id).await {
        Ok(stats) => stats,
        Err(err) => {
            error!(error = %err, "Failed to get team stats");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Internal server error"),
            )
                .into_response();
        }
    };

    // Get document stats
    let document_stats = match get_document_stats(&state.pool, company_id).await {
        Ok(stats) => stats,
        Err(err) => {
            error!(error = %err, "Failed to get document stats");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Internal server error"),
            )
                .into_response();
        }
    };

    // Get recent activity (simplified)
    let recent_activity = match get_recent_activity(&state.pool, company_id).await {
        Ok(activity) => activity,
        Err(err) => {
            error!(error = %err, "Failed to get recent activity");
            Vec::new() // Return empty activity on error
        }
    };

    let dashboard = CompanyDashboardResponse {
        company,
        team_stats,
        document_stats,
        recent_activity,
    };

    Json(dashboard).into_response()
}

// Helper functions for dashboard

async fn get_team_stats(
    db_pool: &sqlx::PgPool,
    company_id: Uuid,
) -> Result<TeamStats, sqlx::Error> {
    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total_members,
            COUNT(*) FILTER (WHERE status = 'active') as active_members,
            COUNT(*) FILTER (WHERE status = 'invited') as pending_invitations,
            COUNT(*) FILTER (WHERE role = 'admin' AND status = 'active') as admin_count
        FROM company_memberships
        WHERE company_id = $1
        "#,
        company_id
    )
    .fetch_one(db_pool)
    .await?;

    Ok(TeamStats {
        total_members: stats.total_members.unwrap_or(0),
        active_members: stats.active_members.unwrap_or(0),
        pending_invitations: stats.pending_invitations.unwrap_or(0),
        admin_count: stats.admin_count.unwrap_or(0),
    })
}

async fn get_document_stats(
    db_pool: &sqlx::PgPool,
    company_id: Uuid,
) -> Result<DocumentStats, sqlx::Error> {
    let stats = sqlx::query(
        r#"
        SELECT
            COUNT(*) as total_documents,
            COUNT(*) FILTER (WHERE approval_status = 'pending') as pending_approval,
            COUNT(*) FILTER (WHERE approval_status = 'approved') as approved_documents,
            COALESCE(SUM(file_size_bytes), 0) as total_size_bytes
        FROM documents
        WHERE company_id = $1 AND status != 'deleted'
        "#,
    )
    .bind(company_id)
    .fetch_one(db_pool)
    .await?;

    let total_documents: i64 = stats.try_get("total_documents")?;
    let pending_approval: i64 = stats.try_get("pending_approval")?;
    let approved_documents: i64 = stats.try_get("approved_documents")?;
    let total_size_bytes: i64 = stats.try_get("total_size_bytes")?;

    Ok(DocumentStats {
        total_documents,
        pending_approval,
        approved_documents,
        storage_size_mb: (total_size_bytes as f64) / (1024.0 * 1024.0),
    })
}

async fn get_recent_activity(
    db_pool: &sqlx::PgPool,
    company_id: Uuid,
) -> Result<Vec<ActivityItem>, sqlx::Error> {
    let activities = sqlx::query(
        r#"
        SELECT
            dal.id,
            dal.action as activity_type,
            CONCAT('Document ', dal.action, ': ', d.filename) as description,
            dal.user_id,
            dal.created_at
        FROM document_audit_logs dal
        JOIN documents d ON dal.document_id = d.id
        WHERE dal.company_id = $1
        ORDER BY dal.created_at DESC
        LIMIT 10
        "#,
    )
    .bind(company_id)
    .fetch_all(db_pool)
    .await?;

    Ok(activities
        .into_iter()
        .map(|row| ActivityItem {
            id: row.try_get("id").unwrap_or_default(),
            activity_type: row.try_get("activity_type").unwrap_or_default(),
            description: row.try_get("description").unwrap_or_default(),
            user_id: row.try_get("user_id").ok(),
            created_at: row.try_get("created_at").unwrap_or_default(),
        })
        .collect())
}

// ============================================================================
// INVESTOR PROFILE ENDPOINTS
// ============================================================================

use arda_credit_db::db::{
    get_investor_profile_by_user_id, get_investor_profile_by_user_id_or_email,
    get_investor_profile_with_identity, register_or_reregister_user, update_investor_profile,
};
use axum::extract::Query;
use axum::http::HeaderMap;

/// Query parameters for investor profile lookup
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct InvestorProfileQuery {
    /// User ID to look up
    pub user_id: Option<Uuid>,
    /// Email to look up
    pub email: Option<String>,
}

/// Investor profile response
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct InvestorProfileResponse {
    pub user_id: Uuid,
    pub email: String,
    pub user_type: String,
    pub kyc_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Request to register a new investor
#[derive(Debug, serde::Deserialize, utoipa::ToSchema, Validate)]
pub struct RegisterInvestorRequest {
    #[validate(email)]
    pub email: String,
    pub invite_code: Option<String>,
}

/// Response after registering investor
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct RegisterInvestorResponse {
    pub user_id: Uuid,
    pub email: String,
    pub user_type: String,
    pub message: String,
}

/// Request to update investor profile
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateInvestorProfileRequest {
    pub email: Option<String>,
}

/// Get investor profile by user ID or email
#[utoipa::path(
    get,
    path = "/api/v1/investor/profile",
    params(InvestorProfileQuery),
    responses(
        (status = 200, description = "Investor profile retrieved", body = InvestorProfileResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Investor not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn get_investor_profile_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<InvestorProfileQuery>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    match (&query.user_id, &query.email) {
        (Some(_), Some(_)) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Provide either user_id or email, not both".to_string(),
            ))
        }
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Must provide either user_id or email".to_string(),
            ))
        }
        _ => {}
    }

    let user_id_option = query.user_id;
    let email_option = query.email.as_deref();

    match get_investor_profile_by_user_id_or_email(&state.pool, user_id_option, email_option).await
    {
        Ok(Some((_profile, identity))) => {
            let response = InvestorProfileResponse {
                user_id: identity.id,
                email: identity.email,
                user_type: identity.user_type,
                kyc_status: identity.kyc_status,
                created_at: identity.created_at,
                updated_at: identity.updated_at,
            };
            Ok(Json(response))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Investor not found".to_string())),
        Err(e) => {
            error!("Failed to get investor profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get investor profile: {}", e),
            ))
        }
    }
}

/// Update investor profile
#[utoipa::path(
    put,
    path = "/api/v1/investor/profile",
    request_body = UpdateInvestorProfileRequest,
    responses(
        (status = 200, description = "Investor profile updated", body = InvestorProfileResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Investor not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn update_investor_profile_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(_request): Json<UpdateInvestorProfileRequest>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    let user_id_str = headers
        .get("X-User-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing X-User-ID header".to_string(),
            )
        })?;

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    let db_request = arda_credit_db::db::UpdateInvestorProfileRequest {
        accredited_investor: None,
        investment_capacity: None,
        regulatory_jurisdiction: None,
    };

    match update_investor_profile(&state.pool, user_id, &db_request).await {
        Ok(_) => match get_investor_profile_with_identity(&state.pool, user_id).await {
            Ok(Some((_profile, identity))) => {
                let response = InvestorProfileResponse {
                    user_id: identity.id,
                    email: identity.email,
                    user_type: identity.user_type,
                    kyc_status: identity.kyc_status,
                    created_at: identity.created_at,
                    updated_at: identity.updated_at,
                };
                Ok(Json(response))
            }
            Ok(None) => Err((
                StatusCode::NOT_FOUND,
                "Investor profile not found after update".to_string(),
            )),
            Err(e) => {
                error!("Failed to fetch updated investor profile: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to fetch updated profile: {}", e),
                ))
            }
        },
        Err(e) => {
            error!("Failed to update investor profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update investor profile: {}", e),
            ))
        }
    }
}

/// Register a new investor
#[utoipa::path(
    post,
    path = "/api/v1/investor/register",
    request_body = RegisterInvestorRequest,
    responses(
        (status = 201, description = "Investor registered successfully", body = RegisterInvestorResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Companies"
)]
pub async fn register_investor_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<RegisterInvestorRequest>,
) -> Result<Json<RegisterInvestorResponse>, (StatusCode, String)> {
    info!("💼 API: Registering new investor: email={}", request.email);

    if let Err(e) = request.validate() {
        return Err((StatusCode::BAD_REQUEST, format!("Validation error: {}", e)));
    }

    let result = match register_or_reregister_user(&state.pool, "investor", &request.email).await {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to register investor: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to register investor: {}", e),
            ));
        }
    };

    let identity = match result {
        arda_credit_db::db::RegisterResult::Created(identity) => identity,
        arda_credit_db::db::RegisterResult::Reregistered(identity) => identity,
        arda_credit_db::db::RegisterResult::AlreadyExists => {
            return Err((StatusCode::CONFLICT, "Investor already exists".to_string()));
        }
    };

    info!(
        "✅ Investor registered successfully: user_id={}",
        identity.id
    );

    match get_investor_profile_by_user_id(&state.pool, identity.id).await {
        Ok(Some(_profile)) => {
            let response = RegisterInvestorResponse {
                user_id: identity.id,
                email: identity.email.clone(),
                user_type: identity.user_type.clone(),
                message: "Investor registered successfully".to_string(),
            };
            Ok(Json(response))
        }
        Ok(None) => {
            let response = RegisterInvestorResponse {
                user_id: identity.id,
                email: identity.email.clone(),
                user_type: identity.user_type.clone(),
                message: "Investor registered successfully (no profile created yet)".to_string(),
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to fetch investor profile after registration: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Registration succeeded but failed to fetch profile: {}", e),
            ))
        }
    }
}
