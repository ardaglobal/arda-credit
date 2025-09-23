use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

// ============================================================================
// JWT CLAIMS STRUCTURE
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // Subject (user_id)
    pub email: String,     // User email
    pub user_type: String, // User type (investor, lender)
    pub exp: i64,          // Expiration time
    pub iat: i64,          // Issued at
    pub jti: String,       // JWT ID (for token revocation if needed)
}

// ============================================================================
// VALIDATION STRUCTURES
// ============================================================================

#[derive(Debug, Validate)]
pub struct EmailRequirements {
    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 255, message = "Email must be less than 255 characters"))]
    pub email: String,
}

// ============================================================================
// AUTHENTICATION SERVICE
// ============================================================================

pub struct AuthService {
    jwt_secret: String,
    token_expiry_hours: i64,
}

impl AuthService {
    pub fn new(jwt_secret: String, token_expiry_hours: Option<i64>) -> Self {
        Self {
            jwt_secret,
            token_expiry_hours: token_expiry_hours.unwrap_or(24), // 24 hours default
        }
    }

    // ============================================================================
    // JWT TOKEN OPERATIONS
    // ============================================================================

    /// Generate a JWT token for a user
    pub fn generate_token(
        &self,
        user_id: Uuid,
        email: &str,
        user_type: &str,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.token_expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            user_type: user_type.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Uuid::new_v4().to_string(), // Unique token ID
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(AuthError::JwtError)?;

        Ok(token)
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(AuthError::JwtError)?;

        Ok(token_data.claims)
    }

    /// Extract user ID from a valid token
    pub fn get_user_id_from_token(&self, token: &str) -> Result<Uuid, AuthError> {
        let claims = self.validate_token(token)?;
        Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)
    }

    // ============================================================================
    // TOKEN GENERATION HELPERS
    // ============================================================================

    /// Generate a secure random token (for email verification, password reset, etc.)
    pub fn generate_secure_token() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        const TOKEN_LENGTH: usize = 64;

        let mut rng = rand::rng();
        (0..TOKEN_LENGTH)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    // ============================================================================
    // VALIDATION HELPERS
    // ============================================================================

    /// Validate email format
    pub fn validate_email(&self, email: &str) -> Result<(), AuthError> {
        // Simple email validation: contains @ and . with basic structure
        if email.contains('@') && email.contains('.') && email.len() >= 5 {
            Ok(())
        } else {
            Err(AuthError::ValidationError(
                validator::ValidationErrors::new(),
            ))
        }
    }

    // ============================================================================
    // TOKEN INFO
    // ============================================================================

    /// Get token expiry duration in seconds
    pub fn token_expiry_seconds(&self) -> i64 {
        self.token_expiry_hours * 3600
    }

    /// Check if a token is about to expire (within 1 hour)
    pub fn is_token_expiring_soon(&self, token: &str) -> Result<bool, AuthError> {
        let claims = self.validate_token(token)?;
        let exp_time =
            chrono::DateTime::from_timestamp(claims.exp, 0).ok_or(AuthError::InvalidToken)?;
        let now = Utc::now();
        let one_hour_from_now = now + Duration::hours(1);

        Ok(exp_time <= one_hour_from_now)
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,

    #[error("Token has expired")]
    ExpiredToken,

    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Account not verified")]
    AccountNotVerified,

    #[error("Account locked or disabled")]
    AccountLocked,
}

// ============================================================================
// RESPONSE HELPERS
// ============================================================================

impl AuthError {
    /// Convert to HTTP status code
    pub fn to_status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;

        match self {
            AuthError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AuthError::InvalidToken | AuthError::ExpiredToken => StatusCode::UNAUTHORIZED,
            AuthError::AccountNotVerified | AuthError::AccountLocked => StatusCode::FORBIDDEN,
            AuthError::JwtError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> &'static str {
        match self {
            AuthError::InvalidToken | AuthError::ExpiredToken => "Invalid or expired token",
            AuthError::AccountNotVerified => "Please verify your email address before logging in",
            AuthError::AccountLocked => "Account is temporarily locked",
            AuthError::ValidationError(_) => "Input validation failed",
            AuthError::JwtError(_) => "Authentication service error",
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_auth_service() -> AuthService {
        AuthService::new("test-secret-key-minimum-32-characters".to_string(), Some(1))
    }

    #[test]
    fn test_jwt_token_generation() {
        let auth = create_test_auth_service();
        let user_id = Uuid::new_v4();
        let email = "test@example.com";
        let user_type = "investor";

        let token = auth
            .generate_token(user_id, email, user_type)
            .expect("Failed to generate token");
        assert!(!token.is_empty());

        let claims = auth
            .validate_token(&token)
            .expect("Failed to validate token");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.email, email);
        assert_eq!(claims.user_type, user_type);
    }

    #[test]
    fn test_email_validation() {
        let auth = create_test_auth_service();

        // Valid emails
        assert!(auth.validate_email("user@example.com").is_ok());
        assert!(auth.validate_email("test.user+tag@domain.co.uk").is_ok());

        // Invalid emails
        assert!(auth.validate_email("invalid-email").is_err());
        assert!(auth.validate_email("@example.com").is_err());
        assert!(auth.validate_email("user@").is_err());
    }
}
