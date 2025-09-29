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
    pub sub: String,                // Subject (user_id)
    pub email: String,              // User email
    pub company_id: Option<String>, // Company ID if user belongs to a company
    pub system_admin: bool,         // System-wide admin privileges
    pub exp: i64,                   // Expiration time
    pub iat: i64,                   // Issued at
    pub jti: String,                // JWT ID (for token revocation if needed)
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

    /// Generate a JWT token for a user with company context
    pub fn generate_token(
        &self,
        user_id: Uuid,
        email: &str,
        company_id: Option<Uuid>,
        system_admin: bool,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.token_expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            company_id: company_id.map(|id| id.to_string()),
            system_admin,
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
        // Must not start or end with @ or .
        if email.contains('@')
            && email.contains('.')
            && email.len() >= 5
            && !email.starts_with('@')
            && !email.starts_with('.')
            && !email.ends_with('@')
            && !email.ends_with('.')
            && email.split('@').count() == 2
        // Exactly one @
        {
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
// COMPANY-BASED AUTHORIZATION HELPERS
// ============================================================================

/// Check if user has company access
pub fn has_company_access(claims: &Claims, required_company_id: Uuid) -> bool {
    // System admins have access to all companies
    if claims.system_admin {
        return true;
    }

    // Check if user belongs to the required company
    if let Some(user_company_id) = &claims.company_id {
        if let Ok(parsed_company_id) = Uuid::parse_str(user_company_id) {
            return parsed_company_id == required_company_id;
        }
    }

    false
}

/// Check if user is system admin
pub fn is_system_admin(claims: &Claims) -> bool {
    claims.system_admin
}

/// Get user's company ID from claims
pub fn get_user_company_id(claims: &Claims) -> Option<Uuid> {
    claims
        .company_id
        .as_ref()
        .and_then(|id| Uuid::parse_str(id).ok())
}

/// Authorization context for requests
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub email: String,
    pub company_id: Option<Uuid>,
    pub system_admin: bool,
}

impl AuthContext {
    pub fn from_claims(claims: Claims) -> Result<Self, AuthError> {
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)?;

        let company_id = if let Some(company_str) = &claims.company_id {
            Some(Uuid::parse_str(company_str).map_err(|_| AuthError::InvalidToken)?)
        } else {
            None
        };

        Ok(AuthContext {
            user_id,
            email: claims.email,
            company_id,
            system_admin: claims.system_admin,
        })
    }

    /// Check if this user has access to the specified company
    pub fn has_company_access(&self, company_id: Uuid) -> bool {
        self.system_admin || self.company_id == Some(company_id)
    }

    /// Check if this user is system admin
    pub fn is_system_admin(&self) -> bool {
        self.system_admin
    }

    /// Get user's company ID if they belong to one
    pub fn get_company_id(&self) -> Option<Uuid> {
        self.company_id
    }
}

/// Extract AuthContext from request headers (set by session middleware)
pub fn auth_context_from_headers(
    headers: &axum::http::HeaderMap,
) -> Result<AuthContext, AuthError> {
    let user_id_str = headers
        .get("x-user-id")
        .ok_or(AuthError::InvalidToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken)?;

    let user_id = uuid::Uuid::parse_str(user_id_str).map_err(|_| AuthError::InvalidToken)?;

    let email = headers
        .get("x-user-email")
        .ok_or(AuthError::InvalidToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken)?
        .to_string();

    let company_id = headers
        .get("x-company-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| uuid::Uuid::parse_str(s).ok());

    let system_admin = headers
        .get("x-system-admin")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    Ok(AuthContext {
        user_id,
        email,
        company_id,
        system_admin,
    })
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
        let company_id = Some(Uuid::new_v4());
        let email = "test@example.com";

        let token = auth
            .generate_token(user_id, email, company_id, false)
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
