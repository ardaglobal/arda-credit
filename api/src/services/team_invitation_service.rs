use arda_credit_db::business::company::{
    create_invitation, get_invitation_by_token, is_company_admin, CompanyInvitation, CompanyRole,
    InviteUserRequest, ProfileType,
};
use arda_credit_db::db::User;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::services::email_service::{EmailError, EmailService};
use crate::services::magic_link_service::{MagicLinkConfig, MagicLinkError, MagicLinkService};

/// Errors that can occur during team invitation operations
#[derive(Debug, thiserror::Error)]
pub enum TeamInvitationError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Email service error: {0}")]
    Email(#[from] EmailError),
    #[error("Magic link error: {0}")]
    MagicLink(#[from] MagicLinkError),
    #[error("Not authorized: only company admins can send invitations")]
    NotAuthorized,
    #[error("User already exists with email: {0}")]
    UserAlreadyExists(String),
    #[error("Company not found: {0}")]
    CompanyNotFound(String),
    #[error("Invitation not found or expired")]
    InvitationNotFound,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Team invitation service for inviting users to companies
#[derive(Clone)]
pub struct TeamInvitationService {
    pool: PgPool,
    magic_link_service: Arc<MagicLinkService>,
    email_service: Arc<EmailService>,
    magic_link_config: MagicLinkConfig,
}

impl TeamInvitationService {
    pub fn new(
        pool: PgPool,
        magic_link_service: Arc<MagicLinkService>,
        email_service: Arc<EmailService>,
        magic_link_config: MagicLinkConfig,
    ) -> Self {
        Self {
            pool,
            magic_link_service,
            email_service,
            magic_link_config,
        }
    }

    /// Send a team invitation to a new or existing user
    #[instrument(skip(self), level = "info")]
    pub async fn send_invitation(
        &self,
        company_id: Uuid,
        inviter_id: Uuid,
        invitee_email: &str,
        role: CompanyRole,
        profile_type: ProfileType,
    ) -> Result<String, TeamInvitationError> {
        info!(
            "📧 Sending team invitation: company={}, inviter={}, invitee={}, role={}, profile={}",
            company_id, inviter_id, invitee_email, role, profile_type
        );

        // Check if inviter is an admin of the company
        let is_admin = is_company_admin(&self.pool, inviter_id, company_id)
            .await
            .map_err(|e| TeamInvitationError::Database(sqlx::Error::Decode(Box::new(e))))?;

        if !is_admin {
            warn!(
                "User {} attempted to invite but is not admin of company {}",
                inviter_id, company_id
            );
            return Err(TeamInvitationError::NotAuthorized);
        }

        // Get company details
        let company = arda_credit_db::business::company::get_company_by_id(&self.pool, company_id)
            .await
            .map_err(|_| TeamInvitationError::CompanyNotFound(company_id.to_string()))?;

        // Get inviter details
        let inviter = self.get_user_by_id(inviter_id).await?;

        // Create invitation record in database
        let request = InviteUserRequest {
            email: invitee_email.to_string(),
            role,
            profile_type,
            invited_by: inviter_id,
        };

        let invitation = create_invitation(&self.pool, &request, company_id)
            .await
            .map_err(|e| TeamInvitationError::Database(sqlx::Error::Decode(Box::new(e))))?;

        // Check if user already exists - if so, send magic link directly
        // If not, they'll need to register first
        let user_exists = self.check_user_exists(invitee_email).await?;

        // Create magic link URL with invitation token as redirect parameter
        let redirect_uri = format!("/invitation/accept?token={}", invitation.invitation_token);

        // Generate magic link (this will create a user if they don't exist)
        let magic_link_url = if user_exists {
            // User exists, generate magic link for sign-in
            self.generate_magic_link_for_existing_user(invitee_email, Some(redirect_uri))
                .await?
        } else {
            // New user, they need to complete registration
            // We'll create a placeholder user and send them a magic link
            self.create_placeholder_user_and_magic_link(invitee_email, Some(redirect_uri))
                .await?
        };

        // Send invitation email
        let message_id = self
            .email_service
            .send_team_invitation_email(
                invitee_email,
                company.name.clone(),
                inviter.email.clone(),
                role.to_string(),
                profile_type.to_string(),
                magic_link_url,
                self.magic_link_config.token_ttl_minutes,
            )
            .await?;

        info!(
            "✅ Team invitation sent successfully: invitation_id={}, message_id={}",
            invitation.id, message_id
        );

        Ok(message_id)
    }

    /// Accept an invitation (called after magic link verification)
    #[instrument(skip(self), level = "info")]
    pub async fn accept_invitation(
        &self,
        invitation_token: &str,
        user_id: Uuid,
    ) -> Result<CompanyInvitation, TeamInvitationError> {
        info!(
            "✅ Accepting invitation: token={}, user={}",
            invitation_token, user_id
        );

        // Get invitation
        let invitation = get_invitation_by_token(&self.pool, invitation_token)
            .await
            .map_err(|_| TeamInvitationError::InvitationNotFound)?;

        // Accept the invitation using the existing database function
        let request = arda_credit_db::business::company::AcceptInvitationRequest {
            invitation_token: invitation_token.to_string(),
            user_id,
        };

        let membership = arda_credit_db::business::company::accept_invitation(&self.pool, &request)
            .await
            .map_err(|e| TeamInvitationError::Database(sqlx::Error::Decode(Box::new(e))))?;

        info!(
            "✅ Invitation accepted successfully: membership_id={}",
            membership.id
        );

        Ok(invitation)
    }

    /// Helper: Check if user exists
    async fn check_user_exists(&self, email: &str) -> Result<bool, TeamInvitationError> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND deleted_at IS NULL)",
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    /// Helper: Get user by ID
    async fn get_user_by_id(&self, user_id: Uuid) -> Result<User, TeamInvitationError> {
        let user =
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;

        user.ok_or_else(|| TeamInvitationError::Internal(format!("User not found: {}", user_id)))
    }

    /// Helper: Generate magic link for existing user
    async fn generate_magic_link_for_existing_user(
        &self,
        email: &str,
        redirect_uri: Option<String>,
    ) -> Result<String, TeamInvitationError> {
        // Generate magic link token and state
        let token = MagicLinkService::generate_token();
        let state = MagicLinkService::generate_state();

        // Find user
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| TeamInvitationError::Internal(format!("User not found: {}", email)))?;

        // Store magic link
        let token_hash = MagicLinkService::hash_token(&token);
        let expires_at = chrono::Utc::now()
            + chrono::Duration::minutes(self.magic_link_config.token_ttl_minutes);

        sqlx::query(
            r#"
            INSERT INTO magic_links (user_id, token_hash, state, redirect_uri, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(user.id)
        .bind(&token_hash)
        .bind(&state)
        .bind(&redirect_uri)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        // Create magic link URL
        let magic_link_url = format!(
            "{}?token={}&state={}",
            self.magic_link_config.base_url, token, state
        );

        Ok(magic_link_url)
    }

    /// Helper: Create placeholder user and generate magic link
    async fn create_placeholder_user_and_magic_link(
        &self,
        email: &str,
        redirect_uri: Option<String>,
    ) -> Result<String, TeamInvitationError> {
        // Check if user already exists
        let existing_user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        let user = if let Some(user) = existing_user {
            user
        } else {
            // Create new user with pending status
            sqlx::query_as::<_, User>(
                r#"
                INSERT INTO users (email, user_type, kyc_status, email_verified)
                VALUES ($1, 'investor', 'not_submitted', false)
                RETURNING *
                "#,
            )
            .bind(email)
            .fetch_one(&self.pool)
            .await?
        };

        // Generate magic link
        let token = MagicLinkService::generate_token();
        let state = MagicLinkService::generate_state();
        let token_hash = MagicLinkService::hash_token(&token);
        let expires_at = chrono::Utc::now()
            + chrono::Duration::minutes(self.magic_link_config.token_ttl_minutes);

        sqlx::query(
            r#"
            INSERT INTO magic_links (user_id, token_hash, state, redirect_uri, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(user.id)
        .bind(&token_hash)
        .bind(&state)
        .bind(&redirect_uri)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        // Create magic link URL
        let magic_link_url = format!(
            "{}?token={}&state={}",
            self.magic_link_config.base_url, token, state
        );

        Ok(magic_link_url)
    }
}
