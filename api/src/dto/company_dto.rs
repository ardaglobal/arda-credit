///! Company DTOs with profile_type support
///!
///! This module provides updated DTOs for company operations that include
///! the profile_type field (originator/investor).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to invite a user to a company with a specific profile type
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct InviteMemberRequest {
    #[validate(email)]
    #[schema(example = "user@example.com")]
    pub email: String,

    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    #[schema(example = "originator")]
    pub profile_type: String, // "originator" or "investor"
}

/// Response for a company membership
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MembershipResponse {
    pub id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,

    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    #[schema(example = "originator")]
    pub profile_type: String, // "originator" or "investor"

    #[schema(example = "active")]
    pub status: String, // "pending", "active", "suspended", "removed"

    pub invited_by: Option<Uuid>,
    pub invited_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response for a company membership with user details
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MembershipWithUserResponse {
    // Membership fields
    pub id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,

    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    #[schema(example = "originator")]
    pub profile_type: String, // "originator" or "investor"

    #[schema(example = "active")]
    pub status: String, // "pending", "active", "suspended", "removed"

    pub invited_by: Option<Uuid>,
    pub invited_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // User fields
    pub user_email: String,
    pub user_type: String,
    pub kyc_status: String,
}

/// Request to update a membership
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateMembershipRequest {
    #[schema(example = "admin")]
    pub role: Option<String>, // "admin" or "user"

    #[schema(example = "active")]
    pub status: Option<String>, // "pending", "active", "suspended", "removed"

    pub permissions: Option<serde_json::Value>,
}

/// Response for a company invitation
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InvitationResponse {
    pub id: Uuid,
    pub company_id: Uuid,
    pub email: String,

    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    #[schema(example = "originator")]
    pub profile_type: String, // "originator" or "investor"

    pub invited_by: Uuid,
    pub invitation_token: String,

    #[schema(example = "pending")]
    pub status: String, // "pending", "accepted", "rejected", "expired", "cancelled"

    pub expires_at: DateTime<Utc>,
    pub accepted_by: Option<Uuid>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Conversion helpers
impl From<arda_credit_db::business::company::CompanyMembership> for MembershipResponse {
    fn from(membership: arda_credit_db::business::company::CompanyMembership) -> Self {
        Self {
            id: membership.id,
            company_id: membership.company_id,
            user_id: membership.user_id,
            role: membership.role.to_string(),
            profile_type: membership.profile_type.to_string(),
            status: membership.status.to_string(),
            invited_by: membership.invited_by,
            invited_at: membership.invited_at,
            accepted_at: membership.accepted_at,
            created_at: membership.created_at,
            updated_at: membership.updated_at,
        }
    }
}

impl From<arda_credit_db::business::company::MembershipWithUser> for MembershipWithUserResponse {
    fn from(membership: arda_credit_db::business::company::MembershipWithUser) -> Self {
        Self {
            id: membership.id,
            company_id: membership.company_id,
            user_id: membership.user_id,
            role: membership.role.to_string(),
            profile_type: membership.profile_type.to_string(),
            status: membership.status.to_string(),
            invited_by: membership.invited_by,
            invited_at: membership.invited_at,
            accepted_at: membership.accepted_at,
            created_at: membership.created_at,
            updated_at: membership.updated_at,
            user_email: membership.user_email,
            user_type: membership.user_type,
            kyc_status: membership.kyc_status,
        }
    }
}

impl From<arda_credit_db::business::company::CompanyInvitation> for InvitationResponse {
    fn from(invitation: arda_credit_db::business::company::CompanyInvitation) -> Self {
        Self {
            id: invitation.id,
            company_id: invitation.company_id,
            email: invitation.email,
            role: invitation.role.to_string(),
            profile_type: invitation.profile_type.to_string(),
            invited_by: invitation.invited_by,
            invitation_token: invitation.invitation_token,
            status: invitation.status.to_string(),
            expires_at: invitation.expires_at,
            accepted_by: invitation.accepted_by,
            accepted_at: invitation.accepted_at,
            created_at: invitation.created_at,
            updated_at: invitation.updated_at,
        }
    }
}
