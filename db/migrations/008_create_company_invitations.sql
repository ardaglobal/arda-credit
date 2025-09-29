-- ============================================================================
-- COMPANY INVITATION AND ONBOARDING SYSTEM
-- Create comprehensive invite system for company user onboarding
-- ============================================================================

-- Create invitation status enum if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'invitation_status') THEN
        CREATE TYPE invitation_status AS ENUM (
            'pending',     -- Invitation sent, waiting for response
            'accepted',    -- User accepted and registered
            'expired',     -- Invitation expired
            'revoked'      -- Invitation was cancelled/revoked
        );
    END IF;
END $$;

-- Create company invitations table
CREATE TABLE IF NOT EXISTS company_invitations (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invite_code VARCHAR(32) NOT NULL UNIQUE, -- Short, URL-safe invite code

    -- Company and role information
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('admin', 'write', 'read')),

    -- Invitation details
    email VARCHAR(255) NOT NULL, -- Email address being invited
    invited_by UUID NOT NULL REFERENCES users(id), -- Who sent the invite
    invited_user_id UUID REFERENCES users(id), -- Set when user accepts (if they register)

    -- Invitation status and lifecycle
    status invitation_status NOT NULL DEFAULT 'pending',
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (NOW() + INTERVAL '7 days'),
    accepted_at TIMESTAMP WITH TIME ZONE,

    -- Customization options
    personal_message TEXT, -- Optional message from inviter
    custom_role_title VARCHAR(100), -- Custom job title/role display name

    -- Permissions to be granted (copied from team_members structure)
    can_upload_documents BOOLEAN DEFAULT false,
    can_download_documents BOOLEAN DEFAULT true,
    can_delete_documents BOOLEAN DEFAULT false,
    can_approve_documents BOOLEAN DEFAULT false,
    can_manage_team BOOLEAN DEFAULT false,
    can_view_audit_logs BOOLEAN DEFAULT false,

    -- Audit fields
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),

    -- Metadata
    metadata JSONB DEFAULT '{}' -- Store additional invitation context
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

CREATE INDEX idx_company_invitations_invite_code ON company_invitations(invite_code);
CREATE INDEX idx_company_invitations_company_id ON company_invitations(company_id);
CREATE INDEX idx_company_invitations_email ON company_invitations(email);
CREATE INDEX idx_company_invitations_status ON company_invitations(status);
CREATE INDEX idx_company_invitations_expires_at ON company_invitations(expires_at);
CREATE INDEX idx_company_invitations_invited_by ON company_invitations(invited_by);
CREATE INDEX idx_company_invitations_invited_user_id ON company_invitations(invited_user_id) WHERE invited_user_id IS NOT NULL;

-- Composite indexes for common query patterns
CREATE INDEX idx_company_invitations_company_status ON company_invitations(company_id, status);
CREATE INDEX idx_company_invitations_email_status ON company_invitations(email, status);

-- ============================================================================
-- FUNCTIONS AND TRIGGERS
-- ============================================================================

-- Function to generate unique invite codes
CREATE OR REPLACE FUNCTION generate_invite_code()
RETURNS VARCHAR(32) AS $$
DECLARE
    chars TEXT := 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    result VARCHAR(32) := '';
    i INTEGER;
    code_exists BOOLEAN := TRUE;
BEGIN
    -- Generate codes until we get a unique one
    WHILE code_exists LOOP
        result := '';
        -- Generate 8-character code
        FOR i IN 1..8 LOOP
            result := result || substr(chars, floor(random() * length(chars) + 1)::INTEGER, 1);
        END LOOP;

        -- Check if code already exists
        SELECT EXISTS(SELECT 1 FROM company_invitations WHERE invite_code = result) INTO code_exists;
    END LOOP;

    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Function to set role-based permissions automatically
CREATE OR REPLACE FUNCTION set_invitation_permissions()
RETURNS TRIGGER AS $$
BEGIN
    -- Set permissions based on role
    CASE NEW.role
        WHEN 'admin' THEN
            NEW.can_upload_documents := true;
            NEW.can_download_documents := true;
            NEW.can_delete_documents := true;
            NEW.can_approve_documents := true;
            NEW.can_manage_team := true;
            NEW.can_view_audit_logs := true;
        WHEN 'write' THEN
            NEW.can_upload_documents := true;
            NEW.can_download_documents := true;
            NEW.can_delete_documents := false;
            NEW.can_approve_documents := false;
            NEW.can_manage_team := false;
            NEW.can_view_audit_logs := false;
        WHEN 'read' THEN
            NEW.can_upload_documents := false;
            NEW.can_download_documents := true;
            NEW.can_delete_documents := false;
            NEW.can_approve_documents := false;
            NEW.can_manage_team := false;
            NEW.can_view_audit_logs := false;
    END CASE;

    -- Generate invite code if not provided
    IF NEW.invite_code IS NULL OR NEW.invite_code = '' THEN
        NEW.invite_code := generate_invite_code();
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_invitation_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to automatically expire old invitations
CREATE OR REPLACE FUNCTION expire_old_invitations()
RETURNS VOID AS $$
BEGIN
    UPDATE company_invitations
    SET status = 'expired', updated_at = NOW()
    WHERE status = 'pending' AND expires_at < NOW();
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

-- Trigger to set permissions and generate invite code on insert/update
CREATE TRIGGER set_invitation_permissions_trigger
    BEFORE INSERT OR UPDATE ON company_invitations
    FOR EACH ROW
    EXECUTE FUNCTION set_invitation_permissions();

-- Trigger to update updated_at timestamp
CREATE TRIGGER update_invitation_updated_at_trigger
    BEFORE UPDATE ON company_invitations
    FOR EACH ROW
    EXECUTE FUNCTION update_invitation_updated_at();

-- ============================================================================
-- CONSTRAINTS AND VALIDATION
-- ============================================================================

-- Note: CHECK constraint with subquery not allowed in PostgreSQL
-- This validation will be enforced at application level instead
-- when creating invitations to prevent duplicate team members

-- Prevent duplicate pending invitations for same email/company
CREATE UNIQUE INDEX unique_pending_company_invitation
ON company_invitations(company_id, email)
WHERE status = 'pending';

-- Ensure expires_at is in the future for pending invitations
ALTER TABLE company_invitations ADD CONSTRAINT check_expires_at_future
CHECK (status != 'pending' OR expires_at > NOW());

-- ============================================================================
-- UTILITY VIEWS
-- ============================================================================

-- View for active invitations with company and inviter details
CREATE OR REPLACE VIEW active_company_invitations AS
SELECT
    ci.id,
    ci.invite_code,
    ci.email,
    ci.role,
    ci.status,
    ci.expires_at,
    ci.personal_message,
    ci.custom_role_title,
    ci.created_at,

    -- Company information
    c.name as company_name,
    c.type as company_type,

    -- Inviter information
    i.first_name as invited_by_first_name,
    i.last_name as invited_by_last_name,
    i.email as invited_by_email,

    -- Permission summary
    CASE ci.role
        WHEN 'admin' THEN 'Full access to company resources'
        WHEN 'write' THEN 'Can upload and download documents'
        WHEN 'read' THEN 'Can view and download documents'
        ELSE 'Custom permissions'
    END as permission_summary

FROM company_invitations ci
JOIN companies c ON ci.company_id = c.id
JOIN users i ON ci.invited_by = i.id
WHERE ci.status = 'pending' AND ci.expires_at > NOW();

-- ============================================================================
-- INITIAL DATA AND CLEANUP
-- ============================================================================

-- Clean up any expired invitations (run periodically)
SELECT expire_old_invitations();

-- ============================================================================
-- COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE company_invitations IS 'Company invitation system for user onboarding';
COMMENT ON COLUMN company_invitations.invite_code IS 'Short, URL-safe code for invitation links';
COMMENT ON COLUMN company_invitations.role IS 'Role to be assigned when user accepts invitation';
COMMENT ON COLUMN company_invitations.expires_at IS 'When the invitation expires (default 7 days)';
COMMENT ON COLUMN company_invitations.metadata IS 'Additional context and customization options';

COMMENT ON FUNCTION generate_invite_code() IS 'Generates unique 8-character alphanumeric invite codes';
COMMENT ON FUNCTION set_invitation_permissions() IS 'Automatically sets permissions based on role and generates invite code';
COMMENT ON FUNCTION expire_old_invitations() IS 'Utility function to expire old pending invitations';

COMMENT ON VIEW active_company_invitations IS 'Active invitations with company and inviter details for API responses';