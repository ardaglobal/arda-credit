use arda_credit_db::db::{Identity, MagicLink, Session};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::middleware::middleware::RateLimiter;
use crate::services::email_service::{EmailError, EmailService};
use crate::services::session_service::{SessionError, SessionService};

/// Errors that can occur during magic link operations
#[derive(Debug, thiserror::Error)]
pub enum MagicLinkError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Email service error: {0}")]
    Email(#[from] EmailError),
    #[error("Session service error: {0}")]
    Session(#[from] SessionError),
    #[error("Rate limit exceeded for client: {client_id} (limit: {limit} req/hour)")]
    RateLimit {
        client_id: String,
        limit: u32,
        window_seconds: u64,
        retry_after_seconds: u64,
    },
    #[error("Magic link not found or expired")]
    NotFound,
    #[error("Magic link already consumed")]
    AlreadyConsumed,
    #[error("Invalid token format")]
    InvalidToken,
    #[error("Invalid state parameter (CSRF protection)")]
    InvalidState,
    #[error("User not found: {0}")]
    UserNotFound(String),
    #[error("Magic link expired")]
    Expired,
}

/// Magic link service configuration
#[derive(Debug, Clone)]
pub struct MagicLinkConfig {
    pub token_ttl_minutes: i64,
    pub base_url: String,
    pub max_tokens_per_user: i32,
}

impl Default for MagicLinkConfig {
    fn default() -> Self {
        Self {
            token_ttl_minutes: 10,
            base_url: std::env::var("MAGIC_BASE_URL")
                .unwrap_or_else(|_| "https://app.arda.xyz/auth/verify".to_string()),
            max_tokens_per_user: 3, // Max 3 concurrent tokens per user
        }
    }
}

/// Magic link service for email-based authentication
#[derive(Clone)]
pub struct MagicLinkService {
    pool: PgPool,
    config: MagicLinkConfig,
    email_service: Arc<EmailService>,
    email_rate_limiter: Arc<RateLimiter>,
    session_service: Arc<SessionService>,
}

impl MagicLinkService {
    pub fn new(
        pool: PgPool,
        config: MagicLinkConfig,
        email_service: Arc<EmailService>,
        email_rate_limiter: Arc<RateLimiter>,
        session_service: Arc<SessionService>,
    ) -> Self {
        Self {
            pool,
            config,
            email_service,
            email_rate_limiter,
            session_service,
        }
    }

    /// Generate and send a magic link to a user's email
    #[instrument(skip(self, client_ip, user_agent), level = "info")]
    pub async fn generate_magic_link(
        &self,
        email: &str,
        redirect_uri: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<String, MagicLinkError> {
        info!("🔗 Generating magic link for email: {}", email);

        // Check email rate limit (3 emails per hour)
        if let Err(rate_limit_err) = self.email_rate_limiter.check_rate_limit(email, 1) {
            return Err(MagicLinkError::RateLimit {
                client_id: rate_limit_err.client_id,
                limit: rate_limit_err.limit,
                window_seconds: rate_limit_err.window_seconds,
                retry_after_seconds: rate_limit_err.retry_after_seconds,
            });
        }

        // Find user by email
        let user = self.get_user_by_email(email).await?;

        // Clean up expired tokens for this user first
        self.cleanup_expired_tokens_for_user(user.id).await?;

        // Check if user has too many active tokens
        let active_count = self.count_active_tokens_for_user(user.id).await?;
        if active_count >= self.config.max_tokens_per_user {
            warn!(
                "User {} has too many active magic links ({})",
                email, active_count
            );
            return Err(MagicLinkError::RateLimit {
                client_id: email.to_string(),
                limit: self.config.max_tokens_per_user as u32,
                window_seconds: 3600,
                retry_after_seconds: 600,
            });
        }

        // Generate secure token and state
        let token = Self::generate_token();
        let state = Self::generate_state();
        let token_hash = Self::hash_token(&token);

        // Store magic link in database
        let magic_link = self
            .store_magic_link(
                user.id,
                token_hash,
                state.clone(),
                redirect_uri.clone(),
                client_ip,
                user_agent,
            )
            .await?;

        // Create magic link URL
        let magic_link_url = self.create_magic_link_url(&token, &state);

        // Send email
        let message_id = self
            .email_service
            .send_magic_link_email(email, magic_link_url, self.config.token_ttl_minutes)
            .await?;

        info!(
            "✅ Magic link generated and sent: email={}, message_id={}",
            email, message_id
        );
        Ok(message_id)
    }

    /// Verify a magic link token and create a session
    #[instrument(skip(self, client_ip, user_agent), level = "info")]
    pub async fn verify_magic_link(
        &self,
        token: &str,
        state: &str,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, Session), MagicLinkError> {
        info!("🔓 Verifying magic link token");

        // Validate token format
        if token.len() != 43 {
            // 32 bytes base64url = 43 chars
            return Err(MagicLinkError::InvalidToken);
        }

        // Hash the provided token
        let token_hash = Self::hash_token(token);

        // Find and consume the magic link
        let magic_link = self.find_and_consume_magic_link(token_hash, state).await?;

        // Get user identity
        let user = self.get_user_by_id(magic_link.user_id).await?;

        // Convert IpNetwork to String for session service
        let client_ip_str = client_ip.map(|ip| ip.to_string());

        // Create session for the user
        let (session_token, session) = self
            .session_service
            .create_session(
                user.id,
                client_ip_str,
                user_agent,
                30, // 30 days expiry
            )
            .await?;

        info!("✅ Magic link verified successfully: user={}", user.email);
        Ok((session_token, session))
    }

    /// Generate a cryptographically secure token (32 bytes -> base64url)
    pub fn generate_token() -> String {
        let mut token_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut token_bytes);
        URL_SAFE_NO_PAD.encode(token_bytes)
    }

    /// Generate a CSRF state parameter (16 bytes -> base64url)
    pub fn generate_state() -> String {
        let mut state_bytes = [0u8; 16];
        rand::thread_rng().fill(&mut state_bytes);
        URL_SAFE_NO_PAD.encode(state_bytes)
    }

    /// Hash a token using SHA-256
    pub fn hash_token(token: &str) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Create the complete magic link URL
    fn create_magic_link_url(&self, token: &str, state: &str) -> String {
        format!("{}?token={}&state={}", self.config.base_url, token, state)
    }

    /// Get user by email address
    #[instrument(skip(self), level = "debug")]
    async fn get_user_by_email(&self, email: &str) -> Result<Identity, MagicLinkError> {
        let user = sqlx::query_as::<_, Identity>(
            "SELECT * FROM identities WHERE email = $1 AND deleted_at IS NULL",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        user.ok_or_else(|| MagicLinkError::UserNotFound(email.to_string()))
    }

    /// Get user by ID
    #[instrument(skip(self), level = "debug")]
    async fn get_user_by_id(&self, user_id: Uuid) -> Result<Identity, MagicLinkError> {
        let user = sqlx::query_as::<_, Identity>(
            "SELECT * FROM identities WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        user.ok_or_else(|| MagicLinkError::UserNotFound(user_id.to_string()))
    }

    /// Store magic link in database
    #[instrument(skip(self, token_hash, client_ip, user_agent), level = "debug")]
    async fn store_magic_link(
        &self,
        user_id: Uuid,
        token_hash: Vec<u8>,
        state: String,
        redirect_uri: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<MagicLink, MagicLinkError> {
        let expires_at = Utc::now() + Duration::minutes(self.config.token_ttl_minutes);

        let magic_link = sqlx::query_as::<_, MagicLink>(
            r#"
            INSERT INTO magic_links (user_id, token_hash, state, redirect_uri, client_ip, user_agent, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(&state)
        .bind(redirect_uri)
        .bind(client_ip)
        .bind(user_agent)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        debug!("Magic link stored: id={}", magic_link.id);
        Ok(magic_link)
    }

    /// Find magic link by token hash and consume it (mark as used)
    #[instrument(skip(self, token_hash), level = "debug")]
    async fn find_and_consume_magic_link(
        &self,
        token_hash: Vec<u8>,
        state: &str,
    ) -> Result<MagicLink, MagicLinkError> {
        let now = Utc::now();

        // Find the magic link and mark it as consumed in a single transaction
        let mut tx = self.pool.begin().await?;

        // First, find the magic link
        let magic_link = sqlx::query_as::<_, MagicLink>(
            r#"
            SELECT * FROM magic_links
            WHERE token_hash = $1
            AND state = $2
            AND expires_at > $3
            AND consumed_at IS NULL
            "#,
        )
        .bind(&token_hash)
        .bind(state)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;

        let magic_link = match magic_link {
            Some(link) => link,
            None => {
                // Check if token exists but is expired/consumed for better error message
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM magic_links WHERE token_hash = $1)",
                )
                .bind(&token_hash)
                .fetch_one(&mut *tx)
                .await?;

                tx.rollback().await?;

                if exists {
                    return Err(MagicLinkError::Expired);
                } else {
                    return Err(MagicLinkError::NotFound);
                }
            }
        };

        // Validate state parameter (CSRF protection)
        if magic_link.state != state {
            tx.rollback().await?;
            return Err(MagicLinkError::InvalidState);
        }

        // Mark as consumed
        sqlx::query("UPDATE magic_links SET consumed_at = $1 WHERE id = $2")
            .bind(now)
            .bind(magic_link.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        debug!("Magic link consumed: id={}", magic_link.id);
        Ok(magic_link)
    }

    /// Count active (non-expired, non-consumed) tokens for a user
    #[instrument(skip(self), level = "debug")]
    async fn count_active_tokens_for_user(&self, user_id: Uuid) -> Result<i32, MagicLinkError> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM magic_links
            WHERE user_id = $1
            AND expires_at > NOW()
            AND consumed_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count as i32)
    }

    /// Clean up expired or consumed tokens for a user
    #[instrument(skip(self), level = "debug")]
    async fn cleanup_expired_tokens_for_user(&self, user_id: Uuid) -> Result<u64, MagicLinkError> {
        let result = sqlx::query(
            r#"
            DELETE FROM magic_links
            WHERE user_id = $1
            AND (expires_at < NOW() OR consumed_at IS NOT NULL)
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            debug!(
                "Cleaned up {} expired/consumed tokens for user {}",
                count, user_id
            );
        }
        Ok(count)
    }

    /// Clean up all expired magic links (for background job)
    #[instrument(skip(self), level = "debug")]
    pub async fn cleanup_expired_magic_links(&self) -> Result<u64, MagicLinkError> {
        let result = sqlx::query(
            r#"
            DELETE FROM magic_links
            WHERE (expires_at < NOW() OR consumed_at IS NOT NULL)
            AND created_at < (NOW() - INTERVAL '24 hours')
            "#,
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            debug!("Cleaned up {} expired/consumed magic links", count);
        }
        Ok(count)
    }
}
