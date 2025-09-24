use arda_credit_db::db::{Identity, Session};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sqlx::PgPool;
use std::net::SocketAddr;
use tracing::{debug, instrument, warn};

/// Errors that can occur during session management
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Session not found or expired")]
    NotFound,
    #[error("Invalid session token")]
    InvalidToken,
    #[error("Session expired")]
    Expired,
}

/// Session management service
#[derive(Clone)]
pub struct SessionService {
    pool: PgPool,
}

impl SessionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new session for a user
    #[instrument(skip(self, user_id), level = "debug")]
    pub async fn create_session(
        &self,
        user_id: uuid::Uuid,
        client_ip: Option<String>,
        user_agent: Option<String>,
        expires_in_days: i64,
    ) -> Result<(String, Session), SessionError> {
        let session_token = Self::generate_session_token();
        let expires_at = Utc::now() + Duration::days(expires_in_days);

        // Execute insert without returning the full struct to avoid IpNetwork issues
        sqlx::query(
            r#"
            INSERT INTO sessions (user_id, session_token, expires_at, client_ip, user_agent)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(user_id)
        .bind(&session_token)
        .bind(expires_at)
        .bind(client_ip.as_deref())
        .bind(user_agent.as_deref())
        .execute(&self.pool)
        .await?;

        // Fetch the created session
        let session =
            sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE session_token = $1")
                .bind(&session_token)
                .fetch_one(&self.pool)
                .await?;

        debug!("Created session for user {}", user_id);
        Ok((session_token, session))
    }

    /// Validate session token and return user identity
    #[instrument(skip(self, session_token), level = "debug")]
    pub async fn validate_session(&self, session_token: &str) -> Result<Identity, SessionError> {
        // First update the session access time
        sqlx::query(
            r#"
            UPDATE sessions
            SET last_accessed_at = NOW()
            WHERE session_token = $1 AND expires_at > NOW()
            "#,
        )
        .bind(session_token)
        .execute(&self.pool)
        .await?;

        // Then get the user identity
        let identity = sqlx::query_as::<_, Identity>(
            r#"
            SELECT i.*
            FROM identities i
            JOIN sessions s ON s.user_id = i.id
            WHERE s.session_token = $1 AND s.expires_at > NOW()
            "#,
        )
        .bind(session_token)
        .fetch_optional(&self.pool)
        .await?;

        match identity {
            Some(identity) => {
                debug!("Valid session for user: {}", identity.email);
                Ok(identity)
            }
            None => {
                warn!("Invalid or expired session token");
                Err(SessionError::NotFound)
            }
        }
    }

    /// Invalidate a specific session
    #[instrument(skip(self, session_token), level = "debug")]
    pub async fn invalidate_session(&self, session_token: &str) -> Result<(), SessionError> {
        let result = sqlx::query("DELETE FROM sessions WHERE session_token = $1")
            .bind(session_token)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() > 0 {
            debug!("Session invalidated");
            Ok(())
        } else {
            Err(SessionError::NotFound)
        }
    }

    /// Invalidate all sessions for a user (logout from all devices)
    #[instrument(skip(self, user_id), level = "debug")]
    pub async fn invalidate_user_sessions(&self, user_id: uuid::Uuid) -> Result<u64, SessionError> {
        let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        let count = result.rows_affected();
        debug!("Invalidated {} sessions for user {}", count, user_id);
        Ok(count)
    }

    /// Clean up expired sessions (for background job)
    #[instrument(skip(self), level = "debug")]
    pub async fn cleanup_expired_sessions(&self) -> Result<u64, SessionError> {
        let result = sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE expires_at < NOW()
            AND created_at < (NOW() - INTERVAL '7 days')
            "#,
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            debug!("Cleaned up {} expired sessions", count);
        }
        Ok(count)
    }

    /// Generate a cryptographically secure session token
    fn generate_session_token() -> String {
        let mut token_bytes = [0u8; 32]; // 256 bits
        rand::thread_rng().fill(&mut token_bytes);
        URL_SAFE_NO_PAD.encode(token_bytes)
    }

    /// Extract client IP from socket address
    pub fn extract_client_ip(addr: Option<std::net::SocketAddr>) -> Option<String> {
        addr.map(|a| a.ip().to_string())
    }
}
