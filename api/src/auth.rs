use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use validator::{Validate, ValidationError};

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
pub struct PasswordRequirements {
    #[validate(length(
        min = 8,
        max = 128,
        message = "Password must be between 8 and 128 characters"
    ))]
    #[validate(custom(function = "validate_password_complexity"))]
    pub password: String,
}

#[derive(Debug, Validate)]
pub struct EmailRequirements {
    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 255, message = "Email must be less than 255 characters"))]
    pub email: String,
}

fn validate_password_complexity(password: &str) -> Result<(), ValidationError> {
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_numeric());
    let has_special = password
        .chars()
        .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c));

    if has_upper && has_lower && has_digit && has_special {
        Ok(())
    } else {
        Err(ValidationError::new("password_complexity"))
    }
}

// ============================================================================
// AUTHENTICATION SERVICE
// ============================================================================

pub struct AuthService {
    jwt_secret: String,
    token_expiry_hours: i64,
    argon2: Argon2<'static>,
}

impl AuthService {
    pub fn new(jwt_secret: String, token_expiry_hours: Option<i64>) -> Self {
        Self {
            jwt_secret,
            token_expiry_hours: token_expiry_hours.unwrap_or(24), // 24 hours default
            argon2: Argon2::default(),
        }
    }

    // ============================================================================
    // PASSWORD OPERATIONS
    // ============================================================================

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        // Validate password requirements
        let password_req = PasswordRequirements {
            password: password.to_string(),
        };
        password_req
            .validate()
            .map_err(AuthError::ValidationError)?;

        // Generate salt and hash password
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| AuthError::HashingError)?
            .to_string();

        Ok(password_hash)
    }

    /// Verify a password against its hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::HashingError)?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(_) => Err(AuthError::HashingError),
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
        let email_req = EmailRequirements {
            email: email.to_string(),
        };
        email_req.validate().map_err(AuthError::ValidationError)
    }

    /// Validate password requirements
    pub fn validate_password(&self, password: &str) -> Result<(), AuthError> {
        let password_req = PasswordRequirements {
            password: password.to_string(),
        };
        password_req.validate().map_err(AuthError::ValidationError)
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
    #[error("Invalid password format or complexity requirements not met")]
    InvalidPassword,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token has expired")]
    ExpiredToken,

    #[error("Password hashing error")]
    HashingError,

    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Account not verified")]
    AccountNotVerified,

    #[error("Account locked or disabled")]
    AccountLocked,
}

impl From<argon2::password_hash::Error> for AuthError {
    fn from(_: argon2::password_hash::Error) -> Self {
        AuthError::HashingError
    }
}

// ============================================================================
// RESPONSE HELPERS
// ============================================================================

impl AuthError {
    /// Convert to HTTP status code
    pub fn to_status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;

        match self {
            AuthError::InvalidPassword | AuthError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AuthError::InvalidToken | AuthError::ExpiredToken | AuthError::InvalidCredentials => {
                StatusCode::UNAUTHORIZED
            }
            AuthError::AccountNotVerified | AuthError::AccountLocked => StatusCode::FORBIDDEN,
            AuthError::HashingError | AuthError::JwtError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> &'static str {
        match self {
            AuthError::InvalidPassword => "Password does not meet security requirements",
            AuthError::InvalidToken | AuthError::ExpiredToken => "Invalid or expired token",
            AuthError::InvalidCredentials => "Invalid email or password",
            AuthError::AccountNotVerified => "Please verify your email address before logging in",
            AuthError::AccountLocked => "Account is temporarily locked",
            AuthError::ValidationError(_) => "Input validation failed",
            AuthError::HashingError | AuthError::JwtError(_) => "Authentication service error",
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
    fn test_password_hashing() {
        let auth = create_test_auth_service();
        let password = "SecureP@ssw0rd123";

        let hash = auth
            .hash_password(password)
            .expect("Failed to hash password");
        assert_ne!(hash, password);

        let is_valid = auth
            .verify_password(password, &hash)
            .expect("Failed to verify password");
        assert!(is_valid);

        let is_invalid = auth
            .verify_password("WrongPassword", &hash)
            .expect("Failed to verify password");
        assert!(!is_invalid);
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
    fn test_password_validation() {
        let auth = create_test_auth_service();

        // Valid password
        assert!(auth.validate_password("SecureP@ssw0rd123").is_ok());

        // Too short
        assert!(auth.validate_password("Short1!").is_err());

        // No uppercase
        assert!(auth.validate_password("lowercase123!").is_err());

        // No special character
        assert!(auth.validate_password("NoSpecialChar123").is_err());
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
