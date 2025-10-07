///! Company membership management API handlers with profile_type support
///!
///! This module provides REST API endpoints for managing company memberships
///! with support for originator and investor profiles.
use crate::{api::router::ApiState, dto::company_dto::*};
use arda_credit_db::business::company::{
    self, CompanyRole, InviteUserRequest, MembershipStatus, ProfileType,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

/// Query parameters for listing members
#[derive(Debug, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct ListMembersQuery {
    /// Filter by status
    #[param(example = "active")]
    pub status: Option<String>,

    /// Filter by role
    #[param(example = "admin")]
    pub role: Option<String>,

    /// Filter by profile type
    #[param(example = "originator")]
    pub profile_type: Option<String>,

    /// Limit number of results
    #[param(example = 50)]
    pub limit: Option<i64>,

    /// Offset for pagination
    #[param(example = 0)]
    pub offset: Option<i64>,
}

/// Invite a user to join a company with a specific profile type
#[utoipa::path(
    post,
    path = "/api/v1/companies/{company_id}/members/invite",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = InviteMemberRequest,
    responses(
        (status = 201, description = "Invitation sent successfully", body = InvitationResponse),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn invite_member(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    Json(request): Json<InviteMemberRequest>,
) -> impl IntoResponse {
    // TODO: Extract claims from request when auth is properly implemented
    let inviter_id = Uuid::parse_str("2e918cf4-3d02-4eba-970c-a9f2630ecbde").expect("Valid UUID"); // chad@arda.xyz

    // Parse role and profile_type
    let role: CompanyRole = match request.role.parse() {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(format!("Invalid role: {}", e)),
            )
                .into_response()
        }
    };

    let profile_type: ProfileType = match request.profile_type.parse() {
        Ok(pt) => pt,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(format!("Invalid profile_type: {}", e)),
            )
                .into_response()
        }
    };

    let invite_request = InviteUserRequest {
        email: request.email,
        role,
        profile_type,
        invited_by: inviter_id,
    };

    match company::create_invitation(&state.pool, &invite_request, company_id).await {
        Ok(invitation) => {
            info!(
                company_id = %company_id,
                email = %invitation.email,
                profile_type = %invitation.profile_type,
                "Member invited successfully"
            );
            (
                StatusCode::CREATED,
                Json(InvitationResponse::from(invitation)),
            )
                .into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to invite member");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to invite member"),
            )
                .into_response()
        }
    }
}

/// List company members with optional filters
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/members",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ListMembersQuery
    ),
    responses(
        (status = 200, description = "Members retrieved successfully", body = Vec<MembershipWithUserResponse>),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn list_members(
    Path(company_id): Path<Uuid>,
    Query(query): Query<ListMembersQuery>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    // Parse optional filters
    let status: Option<MembershipStatus> = query.status.and_then(|s| s.parse().ok());

    let role: Option<CompanyRole> = query.role.and_then(|r| r.parse().ok());

    match company::list_company_members(
        &state.pool,
        company_id,
        status,
        role,
        query.limit,
        query.offset,
    )
    .await
    {
        Ok(members) => {
            let response: Vec<MembershipWithUserResponse> =
                members.into_iter().map(|m| m.into()).collect();
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to list members");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to list members"),
            )
                .into_response()
        }
    }
}

/// Get a specific membership
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/members/{membership_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("membership_id" = Uuid, Path, description = "Membership ID")
    ),
    responses(
        (status = 200, description = "Membership retrieved successfully", body = MembershipResponse),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Membership not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn get_membership(
    Path((company_id, membership_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    match company::get_membership_by_id(&state.pool, membership_id).await {
        Ok(membership) => {
            if membership.company_id != company_id {
                return (
                    StatusCode::NOT_FOUND,
                    Json("Membership not found in this company"),
                )
                    .into_response();
            }
            Json(MembershipResponse::from(membership)).into_response()
        }
        Err(err) => {
            error!(error = %err, membership_id = %membership_id, "Failed to get membership");
            (StatusCode::NOT_FOUND, Json("Membership not found")).into_response()
        }
    }
}

/// Get all memberships for a user in a company (both profiles if they exist)
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/users/{user_id}/memberships",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User memberships retrieved successfully", body = Vec<MembershipResponse>),
        (status = 403, description = "Access denied"),
        (status = 404, description = "User has no memberships in this company"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn get_user_memberships(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    match company::get_user_memberships_in_company(&state.pool, company_id, user_id).await {
        Ok(memberships) => {
            let response: Vec<MembershipResponse> =
                memberships.into_iter().map(|m| m.into()).collect();
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, user_id = %user_id, company_id = %company_id, "Failed to get user memberships");
            (
                StatusCode::NOT_FOUND,
                Json("User has no memberships in this company"),
            )
                .into_response()
        }
    }
}

/// Update a membership (role, status, permissions)
#[utoipa::path(
    put,
    path = "/api/v1/companies/{company_id}/members/{membership_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("membership_id" = Uuid, Path, description = "Membership ID")
    ),
    request_body = UpdateMembershipRequest,
    responses(
        (status = 200, description = "Membership updated successfully", body = MembershipResponse),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Membership not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn update_membership(
    Path((company_id, membership_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
    Json(request): Json<UpdateMembershipRequest>,
) -> impl IntoResponse {
    // First verify membership belongs to this company
    let existing = match company::get_membership_by_id(&state.pool, membership_id).await {
        Ok(m) => m,
        Err(_) => return (StatusCode::NOT_FOUND, Json("Membership not found")).into_response(),
    };

    if existing.company_id != company_id {
        return (
            StatusCode::NOT_FOUND,
            Json("Membership not found in this company"),
        )
            .into_response();
    }

    // Parse optional role and status
    let role: Option<CompanyRole> = request.role.and_then(|r| r.parse().ok());
    let status: Option<MembershipStatus> = request.status.and_then(|s| s.parse().ok());

    let update_request = arda_credit_db::business::company::UpdateMembershipRequest {
        role,
        status,
        permissions: request.permissions,
    };

    match company::update_membership(&state.pool, membership_id, &update_request).await {
        Ok(membership) => {
            info!(membership_id = %membership_id, "Membership updated successfully");
            Json(MembershipResponse::from(membership)).into_response()
        }
        Err(err) => {
            error!(error = %err, membership_id = %membership_id, "Failed to update membership");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to update membership"),
            )
                .into_response()
        }
    }
}

/// Remove a member from a company
#[utoipa::path(
    delete,
    path = "/api/v1/companies/{company_id}/members/{membership_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("membership_id" = Uuid, Path, description = "Membership ID")
    ),
    responses(
        (status = 204, description = "Member removed successfully"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Membership not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Memberships"
)]
pub async fn remove_member(
    Path((company_id, membership_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    // First verify membership belongs to this company
    let existing = match company::get_membership_by_id(&state.pool, membership_id).await {
        Ok(m) => m,
        Err(_) => return (StatusCode::NOT_FOUND, Json("Membership not found")).into_response(),
    };

    if existing.company_id != company_id {
        return (
            StatusCode::NOT_FOUND,
            Json("Membership not found in this company"),
        )
            .into_response();
    }

    match company::remove_membership(&state.pool, membership_id).await {
        Ok(_) => {
            info!(membership_id = %membership_id, "Member removed successfully");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(err) => {
            error!(error = %err, membership_id = %membership_id, "Failed to remove member");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to remove member"),
            )
                .into_response()
        }
    }
}
