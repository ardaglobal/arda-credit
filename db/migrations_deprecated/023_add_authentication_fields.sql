-- Add authentication fields to identities table
-- This migration adds password storage and authentication-related fields

-- ============================================================================
-- ADD AUTHENTICATION FIELDS TO IDENTITIES TABLE
-- ============================================================================

-- Add password and authentication tracking fields
ALTER TABLE identities ADD COLUMN password_hash VARCHAR(255);
ALTER TABLE identities ADD COLUMN last_login_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE identities ADD COLUMN email_verified BOOLEAN DEFAULT FALSE;
ALTER TABLE identities ADD COLUMN email_verification_token VARCHAR(255);
ALTER TABLE identities ADD COLUMN password_reset_token VARCHAR(255);
ALTER TABLE identities ADD COLUMN password_reset_expires_at TIMESTAMP WITH TIME ZONE;

-- ============================================================================
-- ADD AUTHENTICATION INDEXES
-- ============================================================================

-- Index for email + email_verified (login queries)
CREATE INDEX idx_identities_email_verified ON identities(email, email_verified);

-- Index for password reset token lookup
CREATE INDEX idx_identities_password_reset_token 
ON identities(password_reset_token) 
WHERE password_reset_token IS NOT NULL;

-- Index for email verification token lookup
CREATE INDEX idx_identities_email_verification_token 
ON identities(email_verification_token) 
WHERE email_verification_token IS NOT NULL;

-- Index for last login tracking (analytics)
CREATE INDEX idx_identities_last_login_at ON identities(last_login_at) 
WHERE last_login_at IS NOT NULL;

-- ============================================================================
-- ADD AUTHENTICATION HELPER FUNCTIONS
-- ============================================================================

-- Function to generate secure random tokens
CREATE OR REPLACE FUNCTION generate_auth_token()
RETURNS VARCHAR(64) AS $$
BEGIN
    -- Generate 32-byte (256-bit) random token encoded as hex (64 chars)
    RETURN encode(gen_random_bytes(32), 'hex');
END;
$$ LANGUAGE plpgsql;

-- Function to check if password reset token is valid (not expired)
CREATE OR REPLACE FUNCTION is_password_reset_token_valid(
    user_email VARCHAR(255),
    token VARCHAR(255)
)
RETURNS BOOLEAN AS $$
DECLARE
    token_expires_at TIMESTAMP WITH TIME ZONE;
BEGIN
    -- Get token expiration time
    SELECT password_reset_expires_at
    INTO token_expires_at
    FROM identities
    WHERE email = user_email 
    AND password_reset_token = token;
    
    -- Return true if token exists and hasn't expired
    RETURN (token_expires_at IS NOT NULL AND token_expires_at > NOW());
END;
$$ LANGUAGE plpgsql;

-- Function to clean up expired tokens (can be called periodically)
CREATE OR REPLACE FUNCTION cleanup_expired_auth_tokens()
RETURNS INTEGER AS $$
DECLARE
    cleaned_count INTEGER := 0;
BEGIN
    -- Clear expired password reset tokens
    UPDATE identities 
    SET 
        password_reset_token = NULL,
        password_reset_expires_at = NULL
    WHERE password_reset_expires_at < NOW()
    AND password_reset_token IS NOT NULL;
    
    GET DIAGNOSTICS cleaned_count = ROW_COUNT;
    
    -- Clear old email verification tokens (older than 7 days)
    UPDATE identities 
    SET email_verification_token = NULL
    WHERE email_verification_token IS NOT NULL
    AND created_at < (NOW() - INTERVAL '7 days')
    AND email_verified = FALSE;
    
    RETURN cleaned_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- UPDATE EXISTING TRIGGER FOR UPDATED_AT
-- ============================================================================

-- The trigger for updated_at already exists from migration 014, so we just need to ensure
-- it continues to work with the new columns (it will automatically)

-- ============================================================================
-- ADD CONSTRAINTS AND VALIDATION
-- ============================================================================

-- Add constraint: password_hash required for active users (except for migration period)
-- We'll add this constraint after the migration is complete and passwords are set
-- ALTER TABLE identities ADD CONSTRAINT identities_password_required 
-- CHECK (password_hash IS NOT NULL OR created_at < '2024-01-01'::timestamp);

-- Add constraint: email verification token must be unique when present
CREATE UNIQUE INDEX idx_identities_email_verification_token_unique
ON identities(email_verification_token) 
WHERE email_verification_token IS NOT NULL;

-- Add constraint: password reset token must be unique when present  
CREATE UNIQUE INDEX idx_identities_password_reset_token_unique
ON identities(password_reset_token)
WHERE password_reset_token IS NOT NULL;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON COLUMN identities.password_hash IS 'Argon2 hashed password for authentication';
COMMENT ON COLUMN identities.last_login_at IS 'Timestamp of last successful login';
COMMENT ON COLUMN identities.email_verified IS 'Whether the email address has been verified';
COMMENT ON COLUMN identities.email_verification_token IS 'Token for email verification (expires after 7 days)';
COMMENT ON COLUMN identities.password_reset_token IS 'Token for password reset requests';
COMMENT ON COLUMN identities.password_reset_expires_at IS 'Expiration time for password reset token (typically 1 hour)';

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- IMPORTANT: After this migration:
-- 1. Existing users will have NULL password_hash - they'll need to set passwords
-- 2. New registrations should set password_hash immediately
-- 3. Consider sending email verification tokens to existing users
-- 4. Run cleanup_expired_auth_tokens() periodically (e.g., daily cron job)
-- 5. Enable password requirements constraint after transition period
