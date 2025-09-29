-- ============================================================================
-- USERS TABLE
-- User management with company relationship and system admin flag
-- Consolidates identities table functionality
-- ============================================================================

-- KYC investor type enumeration
CREATE TYPE kyc_investor_type AS ENUM ('QIB', 'QP', 'AI');

-- Main users table (consolidates identities)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID REFERENCES companies(id) ON DELETE SET NULL,
    system_admin BOOLEAN DEFAULT FALSE,

    -- User identification and role
    user_type VARCHAR(20) NOT NULL CHECK (user_type IN ('investor', 'originator', 'admin')),
    email VARCHAR(255) NOT NULL UNIQUE,

    -- User profile information
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    jurisdiction VARCHAR(100),

    -- KYC status and verification
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted' CHECK (kyc_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    kyc_submitted_at TIMESTAMP WITH TIME ZONE,
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    kyc_investor_type kyc_investor_type, -- QIB, QP, AI - only for investors

    -- Authentication fields (magic link system)
    last_login_at TIMESTAMP WITH TIME ZONE,
    email_verified BOOLEAN DEFAULT FALSE,
    email_verification_token VARCHAR(255),

    -- Wallet integration
    wallet_address VARCHAR(42), -- Ethereum address

    -- Status and lifecycle
    deleted_at TIMESTAMP WITH TIME ZONE, -- Soft delete
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- USER ROLES AND TEAM MEMBERSHIP
-- ============================================================================

-- Table for team member roles within companies
CREATE TABLE team_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('admin', 'write', 'read')),

    -- Role-specific permissions
    can_upload_documents BOOLEAN DEFAULT FALSE,
    can_download_documents BOOLEAN DEFAULT TRUE,
    can_delete_documents BOOLEAN DEFAULT FALSE,
    can_approve_documents BOOLEAN DEFAULT FALSE,
    can_manage_team BOOLEAN DEFAULT FALSE,
    can_view_audit_logs BOOLEAN DEFAULT FALSE,

    -- Status and lifecycle
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'suspended')),
    invited_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    joined_at TIMESTAMP WITH TIME ZONE,
    last_active_at TIMESTAMP WITH TIME ZONE,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(company_id, user_id) -- One role per user per company
);

-- ============================================================================
-- AUTHENTICATION TABLES
-- ============================================================================

-- Magic link for email-based authentication
CREATE TABLE magic_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL, -- SHA-256 hash of the actual token
    state VARCHAR(32) NOT NULL, -- CSRF protection nonce
    redirect_uri VARCHAR(500), -- Where to redirect after successful auth
    client_ip INET, -- For security logging
    user_agent TEXT, -- For security logging
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    consumed_at TIMESTAMP WITH TIME ZONE, -- NULL = unused, set when consumed
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Sessions for cookie-based authentication after magic link verification
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token VARCHAR(64) NOT NULL UNIQUE, -- Random session identifier
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    client_ip TEXT, -- For security logging
    user_agent TEXT, -- For security logging
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INVESTOR PROFILES
-- ============================================================================

-- Investor profile for enhanced investor account functionality
CREATE TABLE investor_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    accredited_investor BOOLEAN DEFAULT FALSE,
    investment_capacity DECIMAL(18, 6), -- Maximum investment amount in USDC
    regulatory_jurisdiction VARCHAR(100), -- e.g., 'US', 'EU', 'UK'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(user_id) -- One profile per user
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- User indexes
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_company_id ON users(company_id) WHERE company_id IS NOT NULL;
CREATE INDEX idx_users_user_type ON users(user_type);
CREATE INDEX idx_users_kyc_status ON users(kyc_status);
CREATE INDEX idx_users_created_at ON users(created_at);
CREATE INDEX idx_users_wallet_address ON users(wallet_address) WHERE wallet_address IS NOT NULL;
CREATE INDEX idx_users_deleted_at ON users(deleted_at) WHERE deleted_at IS NOT NULL;

-- Team member indexes
CREATE INDEX idx_team_members_company_id ON team_members(company_id);
CREATE INDEX idx_team_members_user_id ON team_members(user_id);
CREATE INDEX idx_team_members_role ON team_members(role);
CREATE INDEX idx_team_members_status ON team_members(status);
CREATE INDEX idx_team_members_company_role ON team_members(company_id, role);

-- Authentication indexes
CREATE UNIQUE INDEX idx_magic_links_token_hash ON magic_links(token_hash);
CREATE INDEX idx_magic_links_user_id ON magic_links(user_id);
CREATE INDEX idx_magic_links_expires_at ON magic_links(expires_at);
CREATE INDEX idx_magic_links_consumed_at ON magic_links(consumed_at);

CREATE UNIQUE INDEX idx_sessions_token ON sessions(session_token);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_sessions_last_accessed ON sessions(last_accessed_at);

-- Investor profile indexes
CREATE INDEX idx_investor_profiles_user_id ON investor_profiles(user_id);
CREATE INDEX idx_investor_profiles_jurisdiction ON investor_profiles(regulatory_jurisdiction);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Triggers for updated_at timestamps
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_team_members_updated_at BEFORE UPDATE ON team_members FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_investor_profiles_updated_at BEFORE UPDATE ON investor_profiles FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- ROLE-BASED PERMISSION FUNCTIONS
-- ============================================================================

-- Function to set role-based permissions automatically
CREATE OR REPLACE FUNCTION set_role_permissions()
RETURNS TRIGGER AS $$
BEGIN
    -- Set permissions based on role
    CASE NEW.role
        WHEN 'admin' THEN
            NEW.can_upload_documents := TRUE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := TRUE;
            NEW.can_approve_documents := TRUE;
            NEW.can_manage_team := TRUE;
            NEW.can_view_audit_logs := TRUE;
        WHEN 'write' THEN
            NEW.can_upload_documents := TRUE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := FALSE;
            NEW.can_approve_documents := TRUE;
            NEW.can_manage_team := FALSE;
            NEW.can_view_audit_logs := FALSE;
        WHEN 'read' THEN
            NEW.can_upload_documents := FALSE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := FALSE;
            NEW.can_approve_documents := FALSE;
            NEW.can_manage_team := FALSE;
            NEW.can_view_audit_logs := FALSE;
    END CASE;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically set permissions based on role
CREATE TRIGGER set_team_member_permissions
    BEFORE INSERT OR UPDATE ON team_members
    FOR EACH ROW
    EXECUTE FUNCTION set_role_permissions();

-- ============================================================================
-- AUTHENTICATION HELPER FUNCTIONS
-- ============================================================================

-- Function to clean up expired magic links
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

-- Function to clean up expired sessions
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
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE users IS 'User management with company relationships and system admin capabilities (consolidates identities)';
COMMENT ON TABLE team_members IS 'Team member roles within companies with granular permissions for document operations';
COMMENT ON TABLE magic_links IS 'Stores hashed magic link tokens for email-based authentication';
COMMENT ON TABLE sessions IS 'User sessions for cookie-based authentication after magic link verification';
COMMENT ON TABLE investor_profiles IS 'Enhanced investor account functionality and regulatory compliance';

COMMENT ON COLUMN users.company_id IS 'Optional company relationship for team-based access control';
COMMENT ON COLUMN users.system_admin IS 'System-wide administrative privileges';
COMMENT ON COLUMN users.user_type IS 'User role: investor, originator, or admin';
COMMENT ON COLUMN users.kyc_status IS 'KYC compliance status: not_submitted, pending, approved, rejected';
COMMENT ON COLUMN users.kyc_investor_type IS 'Investor classification: QIB (Qualified Institutional Buyer), QP (Qualified Purchaser), AI (Accredited Investor)';
COMMENT ON COLUMN magic_links.token_hash IS 'SHA-256 hash of the actual magic link token - never store plaintext tokens';
COMMENT ON COLUMN magic_links.state IS 'CSRF protection nonce to prevent request forgery attacks';
COMMENT ON COLUMN magic_links.consumed_at IS 'Timestamp when token was used - NULL means unused';
COMMENT ON COLUMN sessions.session_token IS 'Random session identifier stored in encrypted cookie';
COMMENT ON COLUMN sessions.last_accessed_at IS 'Updated on each authenticated request for activity tracking';
COMMENT ON COLUMN team_members.role IS 'Team member role: admin (full access), write (upload/approve), read (view only)';