//! Company Invitation Service
//!
//! Service layer for managing company invitations and onboarding flows.
//! Handles invite creation, validation, acceptance, and role-based permissions.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// Email service imports
use crate::email_service::EmailService;
use crate::email_templates::{CompanyInvitationEmailData, EmailContent};

// ============================================================================
// TYPES AND ENUMS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvitationStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "revoked")]
    Revoked,
}

impl std::fmt::Display for InvitationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvitationStatus::Pending => write!(f, "pending"),
            InvitationStatus::Accepted => write!(f, "accepted"),
            InvitationStatus::Expired => write!(f, "expired"),
            InvitationStatus::Revoked => write!(f, "revoked"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompanyRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "read")]
    Read,
}

impl std::fmt::Display for CompanyRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompanyRole::Admin => write!(f, "admin"),
            CompanyRole::Write => write!(f, "write"),
            CompanyRole::Read => write!(f, "read"),
        }
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyInvitation {
    pub id: Uuid,
    pub invite_code: String,
    pub company_id: Uuid,
    pub role: String,
    pub email: String,
    pub invited_by: Uuid,
    pub invited_user_id: Option<Uuid>,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub personal_message: Option<String>,
    pub custom_role_title: Option<String>,
    pub permissions: CompanyPermissions,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyPermissions {
    pub can_upload_documents: bool,
    pub can_download_documents: bool,
    pub can_delete_documents: bool,
    pub can_approve_documents: bool,
    pub can_manage_team: bool,
    pub can_view_audit_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: String,
    pub personal_message: Option<String>,
    pub custom_role_title: Option<String>,
    pub expires_in_days: Option<i32>, // Default 7 days
    pub custom_permissions: Option<CompanyPermissions>, // Override default role permissions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptInvitationRequest {
    pub user_id: Uuid, // User who is accepting the invitation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationDetails {
    pub invitation: CompanyInvitation,
    pub company_name: String,
    pub company_type: String,
    pub inviter_name: String,
    pub inviter_email: String,
    pub permission_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationListResponse {
    pub invitations: Vec<InvitationDetails>,
    pub total_count: i64,
    pub pending_count: i64,
    pub accepted_count: i64,
    pub expired_count: i64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum InvitationError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Invitation not found: {0}")]
    NotFound(String),

    #[error("Invitation has expired")]
    Expired,

    #[error("Invitation already accepted")]
    AlreadyAccepted,

    #[error("Invitation was revoked")]
    Revoked,

    #[error("User is already a member of this company")]
    AlreadyMember,

    #[error("Invalid role: {0}")]
    InvalidRole(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid email format: {0}")]
    InvalidEmail(String),

    #[error("Company not found: {0}")]
    CompanyNotFound(Uuid),

    #[error("User not found: {0}")]
    UserNotFound(Uuid),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

// ============================================================================
// SERVICE IMPLEMENTATION
// ============================================================================

pub struct CompanyInvitationService {
    pool: PgPool,
    email_service: Option<Arc<EmailService>>,
}

impl CompanyInvitationService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            email_service: None,
        }
    }

    pub fn with_email_service(pool: PgPool, email_service: Arc<EmailService>) -> Self {
        Self {
            pool,
            email_service: Some(email_service),
        }
    }

    // ========================================================================
    // INVITATION CREATION
    // ========================================================================

    /// Create a new company invitation
    #[instrument(skip(self, inviter_id), level = "debug")]
    pub async fn create_invitation(
        &self,
        company_id: Uuid,
        inviter_id: Uuid,
        request: CreateInvitationRequest,
    ) -> Result<CompanyInvitation, InvitationError> {
        info!(
            "Creating invitation for {} to join company {} as {}",
            request.email, company_id, request.role
        );

        // Validate role
        if !self.is_valid_role(&request.role) {
            return Err(InvitationError::InvalidRole(request.role));
        }

        // Validate email format
        if !self.is_valid_email(&request.email) {
            return Err(InvitationError::InvalidEmail(request.email));
        }

        // Check if company exists
        let company_exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM companies WHERE id = $1)",
            company_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(false);

        if !company_exists {
            return Err(InvitationError::CompanyNotFound(company_id));
        }

        // Check if inviter has permission to invite users
        if !self.can_user_invite(company_id, inviter_id).await? {
            return Err(InvitationError::PermissionDenied(
                "User does not have permission to invite others".to_string(),
            ));
        }

        // Check if email is already a member
        if self
            .is_email_already_member(company_id, &request.email)
            .await?
        {
            return Err(InvitationError::AlreadyMember);
        }

        // Calculate expiration date
        let expires_in_days = request.expires_in_days.unwrap_or(7);
        let expires_at = Utc::now() + Duration::days(expires_in_days as i64);

        // Create invitation
        let invitation = sqlx::query!(
            r#"
            INSERT INTO company_invitations (
                company_id, email, role, invited_by, expires_at,
                personal_message, custom_role_title, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id, invite_code, company_id, role, email, invited_by, invited_user_id,
                status as "status: String", expires_at, accepted_at, personal_message,
                custom_role_title, can_upload_documents, can_download_documents,
                can_delete_documents, can_approve_documents, can_manage_team,
                can_view_audit_logs, created_at, updated_at, metadata
            "#,
            company_id,
            request.email,
            request.role,
            inviter_id,
            expires_at,
            request.personal_message,
            request.custom_role_title,
            serde_json::json!({
                "custom_permissions": request.custom_permissions,
                "expires_in_days": expires_in_days
            })
        )
        .fetch_one(&self.pool)
        .await?;

        let result = CompanyInvitation {
            id: invitation.id,
            invite_code: invitation.invite_code,
            company_id: invitation.company_id,
            role: invitation.role,
            email: invitation.email,
            invited_by: invitation.invited_by,
            invited_user_id: invitation.invited_user_id,
            status: invitation.status,
            expires_at: invitation.expires_at,
            accepted_at: invitation.accepted_at,
            personal_message: invitation.personal_message,
            custom_role_title: invitation.custom_role_title,
            permissions: CompanyPermissions {
                can_upload_documents: invitation.can_upload_documents,
                can_download_documents: invitation.can_download_documents,
                can_delete_documents: invitation.can_delete_documents,
                can_approve_documents: invitation.can_approve_documents,
                can_manage_team: invitation.can_manage_team,
                can_view_audit_logs: invitation.can_view_audit_logs,
            },
            created_at: invitation.created_at,
            updated_at: invitation.updated_at,
            metadata: invitation.metadata,
        };

        info!(
            "Created invitation {} with code {}",
            result.id, result.invite_code
        );

        // Send invitation email if email service is available
        if let Some(email_service) = &self.email_service {
            match self.send_invitation_email(&result, company_id).await {
                Ok(()) => {
                    info!("Invitation email sent successfully to {}", result.email);
                }
                Err(e) => {
                    warn!("Failed to send invitation email to {}: {}", result.email, e);
                    // Don't fail the invitation creation if email fails
                }
            }
        } else {
            debug!("Email service not configured, skipping invitation email");
        }

        Ok(result)
    }

    // ========================================================================
    // INVITATION RETRIEVAL
    // ========================================================================

    /// Get invitation by invite code
    #[instrument(skip(self), level = "debug")]
    pub async fn get_invitation_by_code(
        &self,
        invite_code: &str,
    ) -> Result<InvitationDetails, InvitationError> {
        let invitation_data = sqlx::query!(
            r#"
            SELECT
                ci.id, ci.invite_code, ci.company_id, ci.role, ci.email,
                ci.invited_by, ci.invited_user_id, ci.status as "status: String",
                ci.expires_at, ci.accepted_at, ci.personal_message, ci.custom_role_title,
                ci.can_upload_documents, ci.can_download_documents, ci.can_delete_documents,
                ci.can_approve_documents, ci.can_manage_team, ci.can_view_audit_logs,
                ci.created_at, ci.updated_at, ci.metadata,
                c.name as company_name, c.company_type,
                i.first_name as inviter_first_name, i.last_name as inviter_last_name,
                i.email as inviter_email
            FROM company_invitations ci
            JOIN companies c ON ci.company_id = c.id
            JOIN identities i ON ci.invited_by = i.id
            WHERE ci.invite_code = $1
            "#,
            invite_code
        )
        .fetch_optional(&self.pool)
        .await?;

        let invitation_data =
            invitation_data.ok_or_else(|| InvitationError::NotFound(invite_code.to_string()))?;

        let invitation = CompanyInvitation {
            id: invitation_data.id,
            invite_code: invitation_data.invite_code,
            company_id: invitation_data.company_id,
            role: invitation_data.role.clone(),
            email: invitation_data.email,
            invited_by: invitation_data.invited_by,
            invited_user_id: invitation_data.invited_user_id,
            status: invitation_data.status.clone(),
            expires_at: invitation_data.expires_at,
            accepted_at: invitation_data.accepted_at,
            personal_message: invitation_data.personal_message,
            custom_role_title: invitation_data.custom_role_title,
            permissions: CompanyPermissions {
                can_upload_documents: invitation_data.can_upload_documents,
                can_download_documents: invitation_data.can_download_documents,
                can_delete_documents: invitation_data.can_delete_documents,
                can_approve_documents: invitation_data.can_approve_documents,
                can_manage_team: invitation_data.can_manage_team,
                can_view_audit_logs: invitation_data.can_view_audit_logs,
            },
            created_at: invitation_data.created_at,
            updated_at: invitation_data.updated_at,
            metadata: invitation_data.metadata,
        };

        let inviter_name = format!(
            "{} {}",
            invitation_data.inviter_first_name.unwrap_or_default(),
            invitation_data.inviter_last_name.unwrap_or_default()
        )
        .trim()
        .to_string();

        let permission_summary = self.get_permission_summary(&invitation.role);

        Ok(InvitationDetails {
            invitation,
            company_name: invitation_data.company_name,
            company_type: invitation_data.company_type,
            inviter_name,
            inviter_email: invitation_data.inviter_email,
            permission_summary,
        })
    }

    /// Get all invitations for a company
    #[instrument(skip(self), level = "debug")]
    pub async fn get_company_invitations(
        &self,
        company_id: Uuid,
        status_filter: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<InvitationListResponse, InvitationError> {
        let limit = limit.unwrap_or(50).min(100);
        let offset = offset.unwrap_or(0);

        let mut query_conditions = vec!["ci.company_id = $1".to_string()];
        let mut param_count = 1;

        if let Some(status) = &status_filter {
            param_count += 1;
            query_conditions.push(format!("ci.status = ${}", param_count));
        }

        let where_clause = query_conditions.join(" AND ");

        let query_str = format!(
            r#"
            SELECT
                ci.id, ci.invite_code, ci.company_id, ci.role, ci.email,
                ci.invited_by, ci.invited_user_id, ci.status as status,
                ci.expires_at, ci.accepted_at, ci.personal_message, ci.custom_role_title,
                ci.can_upload_documents, ci.can_download_documents, ci.can_delete_documents,
                ci.can_approve_documents, ci.can_manage_team, ci.can_view_audit_logs,
                ci.created_at, ci.updated_at, ci.metadata,
                c.name as company_name, c.company_type,
                i.first_name as inviter_first_name, i.last_name as inviter_last_name,
                i.email as inviter_email
            FROM company_invitations ci
            JOIN companies c ON ci.company_id = c.id
            JOIN identities i ON ci.invited_by = i.id
            WHERE {}
            ORDER BY ci.created_at DESC
            LIMIT {} OFFSET {}
            "#,
            where_clause, limit, offset
        );

        let mut query = sqlx::query(&query_str).bind(company_id);

        if let Some(status) = &status_filter {
            query = query.bind(status);
        }

        let invitation_rows = query.fetch_all(&self.pool).await?;

        let mut invitations = Vec::new();

        for row in invitation_rows {
            let invitation = CompanyInvitation {
                id: row.get("id"),
                invite_code: row.get("invite_code"),
                company_id: row.get("company_id"),
                role: row.get::<String, _>("role").clone(),
                email: row.get("email"),
                invited_by: row.get("invited_by"),
                invited_user_id: row.get("invited_user_id"),
                status: row.get::<String, _>("status").clone(),
                expires_at: row.get("expires_at"),
                accepted_at: row.get("accepted_at"),
                personal_message: row.get("personal_message"),
                custom_role_title: row.get("custom_role_title"),
                permissions: CompanyPermissions {
                    can_upload_documents: row.get("can_upload_documents"),
                    can_download_documents: row.get("can_download_documents"),
                    can_delete_documents: row.get("can_delete_documents"),
                    can_approve_documents: row.get("can_approve_documents"),
                    can_manage_team: row.get("can_manage_team"),
                    can_view_audit_logs: row.get("can_view_audit_logs"),
                },
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                metadata: row.get("metadata"),
            };

            let inviter_name = format!(
                "{} {}",
                row.get::<Option<String>, _>("inviter_first_name")
                    .unwrap_or_default(),
                row.get::<Option<String>, _>("inviter_last_name")
                    .unwrap_or_default()
            )
            .trim()
            .to_string();

            let permission_summary = self.get_permission_summary(&invitation.role);

            invitations.push(InvitationDetails {
                invitation,
                company_name: row.get("company_name"),
                company_type: row.get("company_type"),
                inviter_name,
                inviter_email: row.get("inviter_email"),
                permission_summary,
            });
        }

        // Get counts
        let counts = sqlx::query!(
            r#"
            SELECT
                COUNT(*) as total_count,
                COUNT(*) FILTER (WHERE status = 'pending') as pending_count,
                COUNT(*) FILTER (WHERE status = 'accepted') as accepted_count,
                COUNT(*) FILTER (WHERE status = 'expired') as expired_count
            FROM company_invitations
            WHERE company_id = $1
            "#,
            company_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(InvitationListResponse {
            invitations,
            total_count: counts.total_count.unwrap_or(0),
            pending_count: counts.pending_count.unwrap_or(0),
            accepted_count: counts.accepted_count.unwrap_or(0),
            expired_count: counts.expired_count.unwrap_or(0),
        })
    }

    // ========================================================================
    // INVITATION ACCEPTANCE
    // ========================================================================

    /// Accept an invitation and create team membership
    #[instrument(skip(self), level = "debug")]
    pub async fn accept_invitation(
        &self,
        invite_code: &str,
        user_id: Uuid,
    ) -> Result<Uuid, InvitationError> {
        info!("User {} accepting invitation {}", user_id, invite_code);

        let mut tx = self.pool.begin().await?;

        // Get and validate invitation
        let invitation = sqlx::query!(
            r#"
            SELECT id, company_id, role, email, status as "status: String",
                   expires_at, invited_user_id,
                   can_upload_documents, can_download_documents, can_delete_documents,
                   can_approve_documents, can_manage_team, can_view_audit_logs
            FROM company_invitations
            WHERE invite_code = $1
            FOR UPDATE
            "#,
            invite_code
        )
        .fetch_optional(&mut *tx)
        .await?;

        let invitation =
            invitation.ok_or_else(|| InvitationError::NotFound(invite_code.to_string()))?;

        // Validate invitation status
        match invitation.status.as_str() {
            "pending" => {}
            "accepted" => return Err(InvitationError::AlreadyAccepted),
            "expired" => return Err(InvitationError::Expired),
            "revoked" => return Err(InvitationError::Revoked),
            _ => {
                return Err(InvitationError::ValidationError(
                    "Invalid invitation status".to_string(),
                ))
            }
        }

        // Check expiration
        if invitation.expires_at < Utc::now() {
            // Update status to expired
            sqlx::query!(
                "UPDATE company_invitations SET status = 'expired' WHERE id = $1",
                invitation.id
            )
            .execute(&mut *tx)
            .await?;

            return Err(InvitationError::Expired);
        }

        // Verify user email matches invitation
        let user_email = sqlx::query_scalar!("SELECT email FROM identities WHERE id = $1", user_id)
            .fetch_optional(&mut *tx)
            .await?;

        let user_email = user_email.ok_or_else(|| InvitationError::UserNotFound(user_id))?;

        if user_email != invitation.email {
            return Err(InvitationError::ValidationError(
                "User email does not match invitation email".to_string(),
            ));
        }

        // Check if user is already a member
        let existing_membership = sqlx::query_scalar!(
            "SELECT id FROM team_members WHERE company_id = $1 AND user_id = $2",
            invitation.company_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if existing_membership.is_some() {
            return Err(InvitationError::AlreadyMember);
        }

        // Create team membership
        let team_member_id = sqlx::query_scalar!(
            r#"
            INSERT INTO team_members (
                company_id, user_id, role, status,
                can_upload_documents, can_download_documents, can_delete_documents,
                can_approve_documents, can_manage_team, can_view_audit_logs,
                invited_at, joined_at, invited_by
            )
            VALUES ($1, $2, $3, 'active', $4, $5, $6, $7, $8, $9, NOW(), NOW(),
                    (SELECT invited_by FROM company_invitations WHERE id = $10))
            RETURNING id
            "#,
            invitation.company_id,
            user_id,
            invitation.role,
            invitation.can_upload_documents,
            invitation.can_download_documents,
            invitation.can_delete_documents,
            invitation.can_approve_documents,
            invitation.can_manage_team,
            invitation.can_view_audit_logs,
            invitation.id
        )
        .fetch_one(&mut *tx)
        .await?;

        // Update invitation status
        sqlx::query!(
            r#"
            UPDATE company_invitations
            SET status = 'accepted', accepted_at = NOW(), invited_user_id = $2
            WHERE id = $1
            "#,
            invitation.id,
            user_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        info!(
            "User {} successfully joined company {} as {}",
            user_id, invitation.company_id, invitation.role
        );

        Ok(team_member_id)
    }

    // ========================================================================
    // INVITATION MANAGEMENT
    // ========================================================================

    /// Revoke an invitation
    #[instrument(skip(self), level = "debug")]
    pub async fn revoke_invitation(
        &self,
        invite_code: &str,
        revoker_id: Uuid,
    ) -> Result<(), InvitationError> {
        // Check if revoker has permission
        let invitation = sqlx::query!(
            "SELECT company_id, invited_by FROM company_invitations WHERE invite_code = $1",
            invite_code
        )
        .fetch_optional(&self.pool)
        .await?;

        let invitation =
            invitation.ok_or_else(|| InvitationError::NotFound(invite_code.to_string()))?;

        // Allow invitation creator or company admin to revoke
        if invitation.invited_by != revoker_id
            && !self
                .can_user_invite(invitation.company_id, revoker_id)
                .await?
        {
            return Err(InvitationError::PermissionDenied(
                "User does not have permission to revoke this invitation".to_string(),
            ));
        }

        // Revoke the invitation
        sqlx::query!(
            "UPDATE company_invitations SET status = 'revoked', updated_at = NOW() WHERE invite_code = $1",
            invite_code
        )
        .execute(&self.pool)
        .await?;

        info!("Invitation {} revoked by {}", invite_code, revoker_id);

        Ok(())
    }

    /// Expire old invitations (utility function)
    pub async fn expire_old_invitations(&self) -> Result<i64, InvitationError> {
        let result = sqlx::query!(
            r#"
            UPDATE company_invitations
            SET status = 'expired', updated_at = NOW()
            WHERE status = 'pending' AND expires_at < NOW()
            "#
        )
        .execute(&self.pool)
        .await?;

        let expired_count = result.rows_affected() as i64;
        if expired_count > 0 {
            info!("Expired {} old invitations", expired_count);
        }

        Ok(expired_count)
    }

    // ========================================================================
    // EMAIL FUNCTIONALITY
    // ========================================================================

    /// Send invitation email to the invitee
    async fn send_invitation_email(
        &self,
        invitation: &CompanyInvitation,
        company_id: Uuid,
    ) -> Result<(), InvitationError> {
        // Get company and inviter information
        let company_info = sqlx::query!(
            r#"
            SELECT c.name as company_name, c.company_type,
                   i.first_name, i.last_name, i.email as inviter_email
            FROM companies c
            JOIN identities i ON i.id = $1
            WHERE c.id = $2
            "#,
            invitation.invited_by,
            company_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let company_info =
            company_info.ok_or_else(|| InvitationError::CompanyNotFound(company_id))?;

        // Generate invitation URL
        let invitation_url = format!("https://app.arda.xyz/invites/{}", invitation.invite_code);

        // Format inviter name
        let inviter_name = format!(
            "{} {}",
            company_info.first_name.unwrap_or_default(),
            company_info.last_name.unwrap_or_default()
        )
        .trim()
        .to_string();

        let inviter_name = if inviter_name.is_empty() {
            "A team member".to_string()
        } else {
            inviter_name
        };

        // Calculate expiry days
        let expiry_days = (invitation.expires_at - invitation.created_at).num_days();

        // Create email data
        let email_data = CompanyInvitationEmailData::new(
            invitation.email.clone(),
            invitation_url,
            company_info.company_name,
            invitation.role.clone(),
            inviter_name,
            company_info.inviter_email,
            invitation.personal_message.clone(),
            expiry_days as i32,
        );

        // Get email service
        let email_service = self.email_service.as_ref().ok_or_else(|| {
            InvitationError::ValidationError("Email service not configured".to_string())
        })?;

        // Generate email content
        let email_content = EmailContent::company_invitation_email(email_data).map_err(|e| {
            InvitationError::ValidationError(format!("Email template error: {}", e))
        })?;

        // Send email
        email_service
            .send_email(
                &invitation.email,
                &email_content.subject,
                &email_content.text_body,
                &email_content.html_body,
            )
            .await
            .map_err(|e| {
                InvitationError::ValidationError(format!("Email sending failed: {}", e))
            })?;

        Ok(())
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    fn is_valid_role(&self, role: &str) -> bool {
        matches!(role, "admin" | "write" | "read")
    }

    fn is_valid_email(&self, email: &str) -> bool {
        email.contains('@') && email.len() > 5 && email.len() < 256
    }

    async fn can_user_invite(
        &self,
        company_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, InvitationError> {
        let can_invite = sqlx::query_scalar!(
            r#"
            SELECT can_manage_team FROM team_members
            WHERE company_id = $1 AND user_id = $2 AND status = 'active'
            "#,
            company_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(can_invite.unwrap_or(false))
    }

    async fn is_email_already_member(
        &self,
        company_id: Uuid,
        email: &str,
    ) -> Result<bool, InvitationError> {
        let is_member = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM team_members tm
                JOIN identities i ON tm.user_id = i.id
                WHERE tm.company_id = $1 AND i.email = $2 AND tm.status = 'active'
            )
            "#,
            company_id,
            email
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(is_member.unwrap_or(false))
    }

    fn get_permission_summary(&self, role: &str) -> String {
        match role {
            "admin" => "Full access to company resources and team management".to_string(),
            "write" => {
                "Can upload and download documents, limited administrative access".to_string()
            }
            "read" => "Can view and download documents, read-only access".to_string(),
            _ => "Custom permissions".to_string(),
        }
    }
}
