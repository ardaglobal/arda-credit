///! Company DTOs with profile_type support
///!
///! This module provides updated DTOs for company operations that include
///! the profile_type field (originator/investor).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to invite a user to a company with a specific profile type via magic link
///
/// This request triggers a team invitation email with an embedded magic link.
/// The invitee will receive a professional email and can sign in to accept the invitation.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct InviteMemberRequest {
    /// Email address of the invitee (must be valid email format)
    #[validate(email)]
    #[schema(example = "user@example.com")]
    pub email: String,

    /// Role: 'admin' (full permissions) or 'user' (limited permissions)
    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    /// Profile type: 'originator' (creates deals) or 'investor' (provides capital)
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

/// Response for a company invitation with magic link
///
/// This response confirms that an invitation was created and a magic link email was sent.
/// The invitation_token is used internally for accepting the invitation via magic link.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InvitationResponse {
    /// Unique identifier for this invitation
    pub id: Uuid,

    /// Company the user is invited to join
    pub company_id: Uuid,

    /// Email address of the invitee
    #[schema(example = "user@example.com")]
    pub email: String,

    /// Role: 'admin' or 'user'
    #[schema(example = "admin")]
    pub role: String, // "admin" or "user"

    /// Profile type: 'originator' or 'investor'
    #[schema(example = "originator")]
    pub profile_type: String, // "originator" or "investor"

    /// User ID of the admin who sent the invitation
    pub invited_by: Uuid,

    /// Invitation token (used internally in magic link for auto-acceptance)
    pub invitation_token: String,

    /// Status: 'pending', 'accepted', 'rejected', 'expired', or 'cancelled'
    #[schema(example = "pending")]
    pub status: String, // "pending", "accepted", "rejected", "expired", "cancelled"

    /// When the invitation expires (UTC timestamp)
    pub expires_at: DateTime<Utc>,

    /// User ID who accepted the invitation (null if pending)
    pub accepted_by: Option<Uuid>,

    /// Timestamp when invitation was accepted (null if pending)
    pub accepted_at: Option<DateTime<Utc>>,

    /// Timestamp when invitation was created
    pub created_at: DateTime<Utc>,

    /// Timestamp when invitation was last updated
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
