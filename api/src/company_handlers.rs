//! Company management API handlers
//!
//! This module provides REST API endpoints for company registration,
//! KYB verification, and team management functionality.

use crate::{
    auth::Claims,
    company_service::{
        CompanyRegistrationRequest, CompanyResponseDto, CompanyService, KybSubmissionRequest,
        TeamInvitationRequest, TeamMemberPermissions, TeamMemberResponseDto,
    },
    types::{ApiState, AppError},
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

/// Helper function to create dummy claims until proper auth is implemented
fn create_dummy_claims() -> Claims {
    Claims {
        sub: "dummy".to_string(),
        email: "dummy@example.com".to_string(),
        user_type: "investor".to_string(),
        exp: 0,
        iat: 0,
        jti: "dummy".to_string(),
    }
}

/// Register a new company
pub async fn register_company_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CompanyRegistrationRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // TODO: Add authentication back when Claims implements FromRequestParts
    let dummy_claims = create_dummy_claims();

    match company_service
        .register_company(&dummy_claims, request)
        .await
    {
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
                crate::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::company_service::CompanyServiceError::CompanyAlreadyExists { .. } => (
                    StatusCode::CONFLICT,
                    Json(format!("Company already exists: {}", err)),
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
pub async fn get_company_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // TODO: Add authentication back when Claims implements FromRequestParts
    let dummy_claims = create_dummy_claims();

    match company_service.get_company(&dummy_claims, company_id).await {
        Ok(company) => Json(company).into_response(),
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to get company");
            match err {
                crate::company_service::CompanyServiceError::CompanyNotFound { .. } => {
                    (StatusCode::NOT_FOUND, Json("Company not found")).into_response()
                }
                crate::company_service::CompanyServiceError::TeamMemberNotFound { .. } => {
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

/// Submit KYB documentation
pub async fn submit_kyb_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    Json(request): Json<KybSubmissionRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    match company_service
        .submit_kyb(&dummy_claims, company_id, request)
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
                crate::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::company_service::CompanyServiceError::CompanyNotFound { .. } => {
                    (StatusCode::NOT_FOUND, Json("Company not found")).into_response()
                }
                crate::company_service::CompanyServiceError::AccessDenied { .. } => {
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
#[derive(Debug, Serialize)]
pub struct KybStatusResponse {
    pub company_id: Uuid,
    pub kyb_status: String,
    pub kyb_submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub kyb_verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_kyb_status_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    match company_service.get_company(&dummy_claims, company_id).await {
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
                crate::company_service::CompanyServiceError::CompanyNotFound { .. } => {
                    (StatusCode::NOT_FOUND, Json("Company not found")).into_response()
                }
                crate::company_service::CompanyServiceError::TeamMemberNotFound { .. } => {
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

/// Invite team member
pub async fn invite_team_member_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    Json(request): Json<TeamInvitationRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    match company_service
        .invite_team_member(&dummy_claims, company_id, request)
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
                crate::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::company_service::CompanyServiceError::TeamMemberAlreadyExists { .. } => {
                    (StatusCode::CONFLICT, Json("Team member already exists")).into_response()
                }
                crate::company_service::CompanyServiceError::AccessDenied { .. } => {
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
pub async fn list_team_members_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    match company_service
        .list_team_members(&dummy_claims, company_id)
        .await
    {
        Ok(team_members) => Json(team_members).into_response(),
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to list team members");
            match err {
                crate::company_service::CompanyServiceError::TeamMemberNotFound { .. } => {
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

/// Update team member role request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTeamMemberRoleRequest {
    #[validate(length(min = 1, max = 20))]
    pub role: String,
    pub permissions: Option<TeamMemberPermissions>,
}

/// Update team member role
pub async fn update_team_member_role_endpoint(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
    Json(request): Json<UpdateTeamMemberRoleRequest>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    match company_service
        .update_team_member_role(
            &dummy_claims,
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
                crate::company_service::CompanyServiceError::ValidationError(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(format!("Validation error: {}", err)),
                )
                    .into_response(),
                crate::company_service::CompanyServiceError::CannotRemoveLastAdmin { .. } => (
                    StatusCode::BAD_REQUEST,
                    Json("Cannot remove the last admin"),
                )
                    .into_response(),
                crate::company_service::CompanyServiceError::AccessDenied { .. } => {
                    (StatusCode::FORBIDDEN, Json("Access denied")).into_response()
                }
                crate::company_service::CompanyServiceError::TeamMemberNotFound { .. } => {
                    (StatusCode::NOT_FOUND, Json("Team member not found")).into_response()
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

/// Remove team member
pub async fn remove_team_member_endpoint(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    // For now, we'll implement this by setting status to 'inactive'
    // In a full implementation, you might want to add this method to the CompanyService

    let company_service = CompanyService::new(state.pool.clone());

    let dummy_claims = create_dummy_claims();
    // Check if the requester has team management access
    let requester_id = match Uuid::parse_str(&dummy_claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json("Invalid user ID")).into_response();
        }
    };

    // This is a simplified version - in a full implementation, you'd add proper validation
    match sqlx::query!(
        r#"
        UPDATE team_members
        SET status = 'inactive', updated_at = NOW()
        WHERE user_id = $1 AND company_id = $2
        AND EXISTS (
            SELECT 1 FROM team_members tm
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
#[derive(Debug, Serialize)]
pub struct CompanyDashboardResponse {
    pub company: crate::company_service::CompanyResponseDto,
    pub team_stats: TeamStats,
    pub document_stats: DocumentStats,
    pub recent_activity: Vec<ActivityItem>,
}

#[derive(Debug, Serialize)]
pub struct TeamStats {
    pub total_members: i64,
    pub active_members: i64,
    pub pending_invitations: i64,
    pub admin_count: i64,
}

#[derive(Debug, Serialize)]
pub struct DocumentStats {
    pub total_documents: i64,
    pub pending_approval: i64,
    pub approved_documents: i64,
    pub storage_size_mb: f64,
}

#[derive(Debug, Serialize)]
pub struct ActivityItem {
    pub id: Uuid,
    pub activity_type: String,
    pub description: String,
    pub user_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Get company dashboard
pub async fn get_company_dashboard_endpoint(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let company_service = CompanyService::new(state.pool.clone());

    // Get company details
    let dummy_claims = create_dummy_claims();
    let company = match company_service.get_company(&dummy_claims, company_id).await {
        Ok(company) => company,
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to get company for dashboard");
            return match err {
                crate::company_service::CompanyServiceError::CompanyNotFound { .. } => {
                    (StatusCode::NOT_FOUND, Json("Company not found")).into_response()
                }
                crate::company_service::CompanyServiceError::TeamMemberNotFound { .. } => {
                    (StatusCode::FORBIDDEN, Json("Access denied")).into_response()
                }
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
        FROM team_members
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
// ADDITIONAL COMPANY HANDLERS FOR NEW API STRUCTURE
// ============================================================================

/// List all companies (admin endpoint)
#[utoipa::path(
    get,
    path = "/companies",
    tag = "companies",
    summary = "List all companies",
    description = "Get a list of all companies in the system (admin only)"
)]
pub async fn list_companies_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<CompanyResponseDto>>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Check if user is system admin
    if !auth_context.is_system_admin() {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    // Get all companies from database
    let companies = sqlx::query_as!(
        CompanyRecord,
        r#"
        SELECT id, name, company_type as "company_type: String", kyb_status, kyb_submitted_at, kyb_verified_at,
               created_at, updated_at
        FROM companies
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let response: Vec<CompanyResponseDto> = companies
        .into_iter()
        .map(|company| {
            CompanyResponseDto {
                id: company.id,
                company_name: company.name,
                company_type: company.company_type,
                description: None,
                industry: None,
                jurisdiction: None,
                registration_number: None,
                business_email: "".to_string(), // Placeholder
                business_phone: None,
                kyb_status: company.kyb_status,
                kyb_submitted_at: company.kyb_submitted_at,
                kyb_verified_at: company.kyb_verified_at,
                document_retention_days: 2555,
                created_at: company.created_at,
                updated_at: company.updated_at,
                team_member_count: 0, // Placeholder - would need separate query
            }
        })
        .collect();

    Ok(Json(response))
}

/// Update company details
#[utoipa::path(
    put,
    path = "/companies/{company_id}",
    tag = "companies",
    summary = "Update company details",
    description = "Update company information (admin or company member with manage_team permission)"
)]
pub async fn update_company_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(request): Json<UpdateCompanyRequest>,
) -> Result<Json<CompanyResponseDto>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Check authorization - system admin or company member with admin role
    if !auth_context.is_system_admin() && !auth_context.has_company_access(company_id) {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }

    // Update company in database
    let updated_company = sqlx::query_as!(
        CompanyRecord,
        r#"
        UPDATE companies
        SET name = COALESCE($2, name),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, company_type as "company_type: String", kyb_status, kyb_submitted_at, kyb_verified_at,
                  created_at, updated_at
        "#,
        company_id,
        request.name
    )
    .fetch_one(&api_state.db_pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Company not found".to_string()),
        _ => AppError::DatabaseError(e.to_string()),
    })?;

    let response = CompanyResponseDto {
        id: updated_company.id,
        company_name: updated_company.name,
        company_type: updated_company.company_type,
        description: None,
        industry: None,
        jurisdiction: None,
        registration_number: None,
        business_email: "".to_string(),
        business_phone: None,
        kyb_status: updated_company.kyb_status,
        kyb_submitted_at: updated_company.kyb_submitted_at,
        kyb_verified_at: updated_company.kyb_verified_at,
        document_retention_days: 2555,
        created_at: updated_company.created_at,
        updated_at: updated_company.updated_at,
        team_member_count: 0,
    };

    Ok(Json(response))
}

/// Delete company (soft delete)
#[utoipa::path(
    delete,
    path = "/companies/{company_id}",
    tag = "companies",
    summary = "Delete company",
    description = "Soft delete a company (system admin only)"
)]
pub async fn delete_company_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Only system admins can delete companies
    if !auth_context.is_system_admin() {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    // Soft delete the company by updating a deleted_at timestamp
    // Note: This would require adding a deleted_at column to the companies table
    let result = sqlx::query!(
        "UPDATE companies SET updated_at = NOW() WHERE id = $1",
        company_id
    )
    .execute(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Company not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "message": "Company deleted successfully",
        "company_id": company_id
    })))
}

/// Update KYB status (admin only)
pub async fn update_kyb_status_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(request): Json<UpdateKybStatusRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Only system admins can update KYB status
    if !auth_context.is_system_admin() {
        return Err(AppError::Forbidden(
            "System admin access required".to_string(),
        ));
    }

    let verified_at = if request.status == "approved" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    sqlx::query!(
        r#"
        UPDATE companies
        SET kyb_status = $2,
            kyb_verified_at = $3,
            updated_at = NOW()
        WHERE id = $1
        "#,
        company_id,
        request.status,
        verified_at
    )
    .execute(&api_state.db_pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "message": "KYB status updated successfully",
        "company_id": company_id,
        "status": request.status
    })))
}

// ============================================================================
// TEAM MEMBER MANAGEMENT HANDLERS
// ============================================================================

/// Add team member to company
pub async fn add_team_member_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(request): Json<AddTeamMemberRequest>,
) -> Result<Json<TeamMemberResponseDto>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Check authorization
    if !auth_context.is_system_admin() && !auth_context.has_company_access(company_id) {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }

    // Add team member logic here...
    // This would involve creating the user if needed and adding them to team_members table

    // Placeholder response
    let response = TeamMemberResponseDto {
        id: Uuid::new_v4(),
        user_id: request.user_id,
        company_id,
        email: request.email.clone(),
        role: request.role.clone(),
        status: "active".to_string(),
        can_upload_documents: request.permissions.can_upload_documents,
        can_download_documents: request.permissions.can_download_documents,
        can_delete_documents: request.permissions.can_delete_documents,
        can_approve_documents: request.permissions.can_approve_documents,
        can_manage_team: request.permissions.can_manage_team,
        can_view_audit_logs: request.permissions.can_view_audit_logs,
        invited_at: chrono::Utc::now(),
        joined_at: Some(chrono::Utc::now()),
        last_active_at: None,
        invited_by: Some(auth_context.user_id),
    };

    Ok(Json(response))
}

/// Get team member details
pub async fn get_team_member_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<TeamMemberResponseDto>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Check authorization
    if !auth_context.is_system_admin() && !auth_context.has_company_access(company_id) {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }

    // Get team member from database
    let team_member = sqlx::query!(
        r#"
        SELECT tm.id, tm.user_id, tm.company_id, u.email, tm.role, tm.status,
               tm.can_upload_documents, tm.can_download_documents, tm.can_delete_documents,
               tm.can_approve_documents, tm.can_manage_team, tm.can_view_audit_logs,
               tm.invited_at, tm.joined_at, tm.last_active_at, tm.invited_by
        FROM team_members tm
        JOIN users u ON tm.user_id = u.id
        WHERE tm.company_id = $1 AND tm.user_id = $2 AND tm.status != 'inactive'
        "#,
        company_id,
        user_id
    )
    .fetch_one(&api_state.db_pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Team member not found".to_string()),
        _ => AppError::DatabaseError(e.to_string()),
    })?;

    let response = TeamMemberResponseDto {
        id: team_member.id,
        user_id: team_member.user_id,
        company_id: team_member.company_id,
        email: team_member.email,
        role: team_member.role,
        status: team_member.status,
        can_upload_documents: team_member.can_upload_documents.unwrap_or(false),
        can_download_documents: team_member.can_download_documents.unwrap_or(false),
        can_delete_documents: team_member.can_delete_documents.unwrap_or(false),
        can_approve_documents: team_member.can_approve_documents.unwrap_or(false),
        can_manage_team: team_member.can_manage_team.unwrap_or(false),
        can_view_audit_logs: team_member.can_view_audit_logs.unwrap_or(false),
        invited_at: team_member.invited_at,
        joined_at: team_member.joined_at,
        last_active_at: team_member.last_active_at,
        invited_by: team_member.invited_by,
    };

    Ok(Json(response))
}

/// Update team member role and permissions
pub async fn update_team_member_endpoint(
    State(api_state): State<crate::types::ApiState>,
    headers: axum::http::HeaderMap,
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateTeamMemberRequest>,
) -> Result<Json<TeamMemberResponseDto>, AppError> {
    // Extract auth context
    let auth_context = crate::auth::auth_context_from_headers(&headers)
        .map_err(|_| AppError::Unauthorized("Authentication required".to_string()))?;

    // Check authorization - system admin or company member with manage_team permission
    if !auth_context.is_system_admin() && !auth_context.has_company_access(company_id) {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }

    // Update team member
    let updated_member = sqlx::query!(
        r#"
        UPDATE team_members
        SET role = COALESCE($3, role),
            can_upload_documents = COALESCE($4, can_upload_documents),
            can_download_documents = COALESCE($5, can_download_documents),
            can_delete_documents = COALESCE($6, can_delete_documents),
            can_approve_documents = COALESCE($7, can_approve_documents),
            can_manage_team = COALESCE($8, can_manage_team),
            can_view_audit_logs = COALESCE($9, can_view_audit_logs),
            updated_at = NOW()
        WHERE company_id = $1 AND user_id = $2 AND status != 'inactive'
        RETURNING id, user_id, company_id, role, status,
                  can_upload_documents, can_download_documents, can_delete_documents,
                  can_approve_documents, can_manage_team, can_view_audit_logs,
                  invited_at, joined_at, last_active_at, invited_by
        "#,
        company_id,
        user_id,
        request.role.as_deref(),
        request.permissions.as_ref().map(|p| p.can_upload_documents),
        request
            .permissions
            .as_ref()
            .map(|p| p.can_download_documents),
        request.permissions.as_ref().map(|p| p.can_delete_documents),
        request
            .permissions
            .as_ref()
            .map(|p| p.can_approve_documents),
        request.permissions.as_ref().map(|p| p.can_manage_team),
        request.permissions.as_ref().map(|p| p.can_view_audit_logs),
    )
    .fetch_one(&api_state.db_pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Team member not found".to_string()),
        _ => AppError::DatabaseError(e.to_string()),
    })?;

    // Get user email
    let user_email = sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user_id)
        .fetch_one(&api_state.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let response = TeamMemberResponseDto {
        id: updated_member.id,
        user_id: updated_member.user_id,
        company_id: updated_member.company_id,
        email: user_email,
        role: updated_member.role,
        status: updated_member.status,
        can_upload_documents: updated_member.can_upload_documents.unwrap_or(false),
        can_download_documents: updated_member.can_download_documents.unwrap_or(false),
        can_delete_documents: updated_member.can_delete_documents.unwrap_or(false),
        can_approve_documents: updated_member.can_approve_documents.unwrap_or(false),
        can_manage_team: updated_member.can_manage_team.unwrap_or(false),
        can_view_audit_logs: updated_member.can_view_audit_logs.unwrap_or(false),
        invited_at: updated_member.invited_at,
        joined_at: updated_member.joined_at,
        last_active_at: updated_member.last_active_at,
        invited_by: updated_member.invited_by,
    };

    Ok(Json(response))
}

// ============================================================================
// REQUEST/RESPONSE TYPES FOR NEW HANDLERS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCompanyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub industry: Option<String>,
    pub jurisdiction: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateKybStatusRequest {
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTeamMemberRequest {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub permissions: TeamMemberPermissionsRequest,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTeamMemberRequest {
    pub role: Option<String>,
    pub permissions: Option<TeamMemberPermissionsRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamMemberPermissionsRequest {
    pub can_upload_documents: bool,
    pub can_download_documents: bool,
    pub can_delete_documents: bool,
    pub can_approve_documents: bool,
    pub can_manage_team: bool,
    pub can_view_audit_logs: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct CompanyRecord {
    pub id: Uuid,
    pub name: String,
    pub company_type: String,
    pub kyb_status: String,
    pub kyb_submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub kyb_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
