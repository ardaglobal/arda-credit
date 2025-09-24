-- ============================================================================
-- MAGIC LINK AUTHENTICATION MIGRATION
-- ============================================================================
-- This migration transforms the authentication system from password-based to magic link authentication
-- Removes password-related columns from identities table
-- Adds magic_links and sessions tables for new authentication flow

-- ============================================================================
-- REMOVE PASSWORD-RELATED COLUMNS FROM IDENTITIES TABLE
-- ============================================================================

-- Remove password-related columns from identities table
ALTER TABLE identities DROP COLUMN IF EXISTS password_hash;
ALTER TABLE identities DROP COLUMN IF EXISTS password_reset_token;
ALTER TABLE identities DROP COLUMN IF EXISTS password_reset_expires_at;

-- Drop related indexes
DROP INDEX IF EXISTS idx_identities_password_reset_token;
DROP INDEX IF EXISTS idx_identities_password_reset_token_unique;

-- ============================================================================
-- CREATE MAGIC LINKS TABLE
-- ============================================================================

CREATE TABLE magic_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL, -- SHA-256 hash of the actual token (never store plaintext)
    state VARCHAR(32) NOT NULL, -- CSRF protection nonce
    redirect_uri VARCHAR(500), -- Where to redirect after successful auth
    client_ip INET, -- For security logging
    user_agent TEXT, -- For security logging
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    consumed_at TIMESTAMP WITH TIME ZONE, -- NULL = unused, set when consumed
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for magic_links performance and security
CREATE UNIQUE INDEX idx_magic_links_token_hash ON magic_links(token_hash);
CREATE INDEX idx_magic_links_user_id ON magic_links(user_id);
CREATE INDEX idx_magic_links_expires_at ON magic_links(expires_at);
CREATE INDEX idx_magic_links_consumed_at ON magic_links(consumed_at);

-- ============================================================================
-- CREATE SESSIONS TABLE
-- ============================================================================

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    session_token VARCHAR(64) NOT NULL UNIQUE, -- Random session identifier
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    client_ip TEXT, -- For security logging
    user_agent TEXT, -- For security logging
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for sessions performance and cleanup
CREATE UNIQUE INDEX idx_sessions_token ON sessions(session_token);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_sessions_last_accessed ON sessions(last_accessed_at);

-- ============================================================================
-- UPDATE AUTHENTICATION HELPER FUNCTIONS
-- ============================================================================

-- Remove password-related functions (no longer needed)
DROP FUNCTION IF EXISTS is_password_reset_token_valid(VARCHAR, VARCHAR);
DROP FUNCTION IF EXISTS cleanup_expired_auth_tokens();

-- Add new magic link helper functions
CREATE OR REPLACE FUNCTION cleanup_expired_magic_links()
RETURNS INTEGER AS $$
DECLARE
    cleaned_count INTEGER := 0;
BEGIN
    -- Clean up expired and consumed magic links (older than 24 hours)
    DELETE FROM magic_links
    WHERE (expires_at < NOW() OR consumed_at IS NOT NULL)
    AND created_at < (NOW() - INTERVAL '24 hours');

    GET DIAGNOSTICS cleaned_count = ROW_COUNT;
    RETURN cleaned_count;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION cleanup_expired_sessions()
RETURNS INTEGER AS $$
DECLARE
    cleaned_count INTEGER := 0;
BEGIN
    -- Clean up expired sessions (older than 7 days)
    DELETE FROM sessions
    WHERE expires_at < NOW()
    AND created_at < (NOW() - INTERVAL '7 days');

    GET DIAGNOSTICS cleaned_count = ROW_COUNT;
    RETURN cleaned_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- ADD TABLE COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE magic_links IS 'Stores hashed magic link tokens for email-based authentication';
COMMENT ON COLUMN magic_links.token_hash IS 'SHA-256 hash of the actual magic link token - never store plaintext tokens';
COMMENT ON COLUMN magic_links.state IS 'CSRF protection nonce to prevent request forgery attacks';
COMMENT ON COLUMN magic_links.consumed_at IS 'Timestamp when token was used - NULL means unused';

COMMENT ON TABLE sessions IS 'User sessions for cookie-based authentication after magic link verification';
COMMENT ON COLUMN sessions.session_token IS 'Random session identifier stored in encrypted cookie';
COMMENT ON COLUMN sessions.last_accessed_at IS 'Updated on each authenticated request for activity tracking';

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================
--
-- IMPORTANT: After this migration:
-- 1. All existing password-based authentication is removed
-- 2. Users will need to use magic link authentication going forward
-- 3. Consider sending welcome emails to existing users with magic link instructions
-- 4. Run cleanup functions periodically (cleanup_expired_magic_links, cleanup_expired_sessions)
-- 5. Update application code to use new authentication flow
