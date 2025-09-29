//! Company Invitation API Handlers
//!
//! REST API endpoints for the company invitation and onboarding system.
//! Provides invite creation, retrieval, acceptance, and management.

use crate::auth::AuthContext;
use crate::company_invitation_service::{
    AcceptInvitationRequest, CompanyInvitationService, CreateInvitationRequest, InvitationError,
};
use crate::types::{ApiState, AppError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use uuid::Uuid;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequestBody {
    pub email: String,
    pub role: String,
    pub personal_message: Option<String>,
    pub custom_role_title: Option<String>,
    pub expires_in_days: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInvitationRequestBody {
    // Empty for now - user ID comes from auth context
}

#[derive(Debug, Deserialize)]
pub struct InvitationListQuery {
    pub status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CreateInvitationResponse {
    pub invitation_id: Uuid,
    pub invite_code: String,
    pub invite_url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AcceptInvitationResponse {
    pub team_member_id: Uuid,
    pub company_id: Uuid,
    pub role: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct InvitationInfoResponse {
    pub invitation: serde_json::Value, // InvitationDetails serialized
    pub can_accept: bool,
    pub reasons: Vec<String>, // Reasons why invitation cannot be accepted (if any)
}

#[derive(Debug, Serialize)]
pub struct RevokeInvitationResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

impl From<InvitationError> for AppError {
    fn from(err: InvitationError) -> Self {
        match err {
            InvitationError::NotFound(msg) => AppError::NotFound(msg),
            InvitationError::Expired => AppError::BadRequest("Invitation has expired".to_string()),
            InvitationError::AlreadyAccepted => {
                AppError::BadRequest("Invitation already accepted".to_string())
            }
            InvitationError::Revoked => AppError::BadRequest("Invitation was revoked".to_string()),
            InvitationError::AlreadyMember => {
                AppError::BadRequest("User is already a member of this company".to_string())
            }
            InvitationError::InvalidRole(role) => {
                AppError::BadRequest(format!("Invalid role: {}", role))
            }
            InvitationError::InvalidEmail(email) => {
                AppError::BadRequest(format!("Invalid email format: {}", email))
            }
            InvitationError::PermissionDenied(msg) => AppError::Forbidden(msg),
            InvitationError::CompanyNotFound(id) => {
                AppError::NotFound(format!("Company not found: {}", id))
            }
            InvitationError::UserNotFound(id) => {
                AppError::NotFound(format!("User not found: {}", id))
            }
            InvitationError::ValidationError(msg) => AppError::BadRequest(msg),
            InvitationError::Database(e) => AppError::DatabaseError(e.to_string()),
        }
    }
}

/// Extract auth context from headers
fn extract_auth_context(headers: &axum::http::HeaderMap) -> Result<AuthContext, AppError> {
    crate::auth::auth_context_from_headers(headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))
}

/// Generate invitation URL from invite code
fn generate_invite_url(invite_code: &str) -> String {
    // In production, this would use the actual frontend URL
    format!("https://app.arda.xyz/invites/{}", invite_code)
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// Create a new company invitation
/// POST /companies/{company_id}/invites
#[instrument(skip(api_state), level = "debug")]
pub async fn create_company_invitation(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(request): Json<CreateInvitationRequestBody>,
) -> Result<Json<CreateInvitationResponse>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    info!(
        "User {} creating invitation for {} to join company {}",
        auth_context.user_id, request.email, company_id
    );

    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    let create_request = CreateInvitationRequest {
        email: request.email,
        role: request.role,
        personal_message: request.personal_message,
        custom_role_title: request.custom_role_title,
        expires_in_days: request.expires_in_days,
        custom_permissions: None, // Use default role permissions for now
    };

    let invitation = service
        .create_invitation(company_id, auth_context.user_id, create_request)
        .await?;

    let invite_url = generate_invite_url(&invitation.invite_code);

    let response = CreateInvitationResponse {
        invitation_id: invitation.id,
        invite_code: invitation.invite_code.clone(),
        invite_url,
        expires_at: invitation.expires_at,
        message: format!("Invitation sent to {}", invitation.email),
    };

    // TODO: Send email notification here
    info!(
        "Created invitation {} for {} to join company {}",
        invitation.id, invitation.email, company_id
    );

    Ok(Json(response))
}

/// Get invitation details by invite code
/// GET /invites/{invite_code}
#[instrument(skip(api_state), level = "debug")]
pub async fn get_invitation_info(
    State(api_state): State<ApiState>,
    Path(invite_code): Path<String>,
) -> Result<Json<InvitationInfoResponse>, AppError> {
    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    let invitation_details = service.get_invitation_by_code(&invite_code).await?;

    // Check if invitation can be accepted
    let mut can_accept = true;
    let mut reasons = Vec::new();

    match invitation_details.invitation.status.as_str() {
        "pending" => {
            if invitation_details.invitation.expires_at < chrono::Utc::now() {
                can_accept = false;
                reasons.push("Invitation has expired".to_string());
            }
        }
        "accepted" => {
            can_accept = false;
            reasons.push("Invitation has already been accepted".to_string());
        }
        "expired" => {
            can_accept = false;
            reasons.push("Invitation has expired".to_string());
        }
        "revoked" => {
            can_accept = false;
            reasons.push("Invitation has been revoked".to_string());
        }
        _ => {
            can_accept = false;
            reasons.push("Invitation is in an invalid state".to_string());
        }
    }

    let response = InvitationInfoResponse {
        invitation: serde_json::to_value(&invitation_details)
            .map_err(|e| AppError::InternalServerError(format!("Serialization error: {}", e)))?,
        can_accept,
        reasons,
    };

    Ok(Json(response))
}

/// Accept an invitation
/// POST /invites/{invite_code}/accept
#[instrument(skip(api_state), level = "debug")]
pub async fn accept_invitation(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(invite_code): Path<String>,
    Json(_request): Json<AcceptInvitationRequestBody>,
) -> Result<Json<AcceptInvitationResponse>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    info!(
        "User {} accepting invitation {}",
        auth_context.user_id, invite_code
    );

    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    // First get invitation details to return in response
    let invitation_details = service.get_invitation_by_code(&invite_code).await?;

    let team_member_id = service
        .accept_invitation(&invite_code, auth_context.user_id)
        .await?;

    let response = AcceptInvitationResponse {
        team_member_id,
        company_id: invitation_details.invitation.company_id,
        role: invitation_details.invitation.role,
        message: format!(
            "Successfully joined {} as {}",
            invitation_details.company_name, invitation_details.invitation.role
        ),
    };

    info!(
        "User {} successfully accepted invitation and joined company {}",
        auth_context.user_id, invitation_details.invitation.company_id
    );

    Ok(Json(response))
}

/// Get all invitations for a company
/// GET /companies/{company_id}/invites
#[instrument(skip(api_state), level = "debug")]
pub async fn list_company_invitations(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<InvitationListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    // Check if user has permission to view company invitations
    // This requires admin or team management permissions
    let has_permission = sqlx::query_scalar!(
        r#"
        SELECT can_manage_team FROM team_members
        WHERE company_id = $1 AND user_id = $2 AND status = 'active'
        "#,
        company_id,
        auth_context.user_id
    )
    .fetch_optional(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if !has_permission.unwrap_or(false) {
        return Err(AppError::Forbidden(
            "Permission denied: requires team management access".to_string(),
        ));
    }

    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    let invitations = service
        .get_company_invitations(company_id, query.status, query.limit, query.offset)
        .await?;

    Ok(Json(serde_json::to_value(&invitations).map_err(|e| {
        AppError::InternalServerError(format!("Serialization error: {}", e))
    })?))
}

/// Revoke an invitation
/// DELETE /invites/{invite_code}
#[instrument(skip(api_state), level = "debug")]
pub async fn revoke_invitation(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(invite_code): Path<String>,
) -> Result<Json<RevokeInvitationResponse>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    service
        .revoke_invitation(&invite_code, auth_context.user_id)
        .await?;

    let response = RevokeInvitationResponse {
        success: true,
        message: format!("Invitation {} has been revoked", invite_code),
    };

    info!(
        "User {} revoked invitation {}",
        auth_context.user_id, invite_code
    );

    Ok(Json(response))
}

/// Get invitation statistics for a company (admin endpoint)
/// GET /companies/{company_id}/invites/stats
#[instrument(skip(api_state), level = "debug")]
pub async fn get_company_invitation_stats(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    // Check if user has permission to view company statistics
    let has_permission = sqlx::query_scalar!(
        r#"
        SELECT can_manage_team FROM team_members
        WHERE company_id = $1 AND user_id = $2 AND status = 'active'
        "#,
        company_id,
        auth_context.user_id
    )
    .fetch_optional(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if !has_permission.unwrap_or(false) {
        return Err(AppError::Forbidden(
            "Permission denied: requires team management access".to_string(),
        ));
    }

    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total_invitations,
            COUNT(*) FILTER (WHERE status = 'pending') as pending_invitations,
            COUNT(*) FILTER (WHERE status = 'accepted') as accepted_invitations,
            COUNT(*) FILTER (WHERE status = 'expired') as expired_invitations,
            COUNT(*) FILTER (WHERE status = 'revoked') as revoked_invitations,
            COUNT(*) FILTER (WHERE role = 'admin') as admin_invitations,
            COUNT(*) FILTER (WHERE role = 'write') as write_invitations,
            COUNT(*) FILTER (WHERE role = 'read') as read_invitations,
            AVG(EXTRACT(EPOCH FROM (COALESCE(accepted_at, expires_at) - created_at))/3600) as avg_response_time_hours
        FROM company_invitations
        WHERE company_id = $1
        "#,
        company_id
    )
    .fetch_one(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let response = serde_json::json!({
        "company_id": company_id,
        "total_invitations": stats.total_invitations,
        "status_breakdown": {
            "pending": stats.pending_invitations,
            "accepted": stats.accepted_invitations,
            "expired": stats.expired_invitations,
            "revoked": stats.revoked_invitations
        },
        "role_breakdown": {
            "admin": stats.admin_invitations,
            "write": stats.write_invitations,
            "read": stats.read_invitations
        },
        "metrics": {
            "average_response_time_hours": stats.avg_response_time_hours.unwrap_or(0.0)
        }
    });

    Ok(Json(response))
}

/// Utility endpoint to expire old invitations (admin/cron job)
/// POST /admin/invitations/expire-old
#[instrument(skip(api_state), level = "debug")]
pub async fn expire_old_invitations(
    State(api_state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_context = extract_auth_context(&headers)?;

    // This should be a system admin only operation
    if !auth_context.is_system_admin() {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    let service = CompanyInvitationService::new(api_state.db_pool.clone());

    let expired_count = service.expire_old_invitations().await?;

    let response = serde_json::json!({
        "success": true,
        "expired_count": expired_count,
        "message": format!("Expired {} old invitations", expired_count)
    });

    Ok(Json(response))
}
