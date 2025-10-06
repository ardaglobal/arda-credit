-- ============================================================================
-- REFACTOR COMPANY ACCESS CONTROL
-- ============================================================================
-- This migration replaces the old company structure with a simplified
-- company-centric access control model.
--
-- Old structure:
--   - companies (with company_type, kyb_status, etc.)
--   - team_members (with granular document permissions)
--
-- New structure:
--   - companies (simplified: just name, description, status)
--   - company_memberships (role-based: admin or user)
--   - company_invitations (invitation workflow)
--   - company_audit (audit trail)
--
-- Key changes:
--   - Users can belong to multiple companies
--   - A user can be an investor in one company and originator in another
--   - Deals are scoped to companies via company_id
-- ============================================================================

-- ============================================================================
-- STEP 1: Drop old company structure
-- ============================================================================

-- Drop dependent tables first (due to foreign keys)
DROP TABLE IF EXISTS team_members CASCADE;
DROP TABLE IF EXISTS company_documents CASCADE;
DROP TABLE IF EXISTS companies CASCADE;

-- ============================================================================
-- STEP 2: Create new simplified company structure
-- ============================================================================

-- Core company information
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'inactive')),

    -- Metadata
    metadata JSONB DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STEP 3: Company memberships (links users to companies with roles)
-- ============================================================================

CREATE TABLE company_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,

    -- Role-based access control (starting with admin and user)
    role VARCHAR(20) NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),

    -- Membership status
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'active', 'suspended', 'removed')),

    -- Invitation tracking
    invited_by UUID REFERENCES identities(id),
    invited_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    accepted_at TIMESTAMP WITH TIME ZONE,

    -- Permissions (JSON object for granular control)
    -- Example: {"can_create_deals": true, "can_approve_deals": false, "can_reject_deals": false}
    permissions JSONB DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Constraints
    UNIQUE(company_id, user_id) -- User can only have one membership per company
);

-- ============================================================================
-- STEP 4: Company invitations
-- ============================================================================

CREATE TABLE company_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,

    -- Invitation details
    email VARCHAR(255) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),

    -- Who sent the invitation
    invited_by UUID NOT NULL REFERENCES identities(id),

    -- Token for accepting invitation
    invitation_token VARCHAR(255) NOT NULL UNIQUE,

    -- Status tracking
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected', 'expired', 'cancelled')),

    -- Expiration
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (NOW() + INTERVAL '7 days'),

    -- Acceptance tracking
    accepted_by UUID REFERENCES identities(id),
    accepted_at TIMESTAMP WITH TIME ZONE,

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STEP 5: Add company_id to deals table
-- ============================================================================

-- Add company_id column (nullable initially, will be made NOT NULL in migration 005)
ALTER TABLE deals ADD COLUMN IF NOT EXISTS company_id UUID REFERENCES companies(id) ON DELETE SET NULL;

-- Create index for efficient company-based deal queries
CREATE INDEX IF NOT EXISTS idx_deals_company_id ON deals(company_id);

-- ============================================================================
-- STEP 6: Audit table for company operations
-- ============================================================================

CREATE TABLE company_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    operation_type VARCHAR(50) NOT NULL CHECK (operation_type IN (
        'COMPANY_CREATED', 'COMPANY_UPDATED', 'COMPANY_SUSPENDED', 'COMPANY_REACTIVATED',
        'MEMBER_INVITED', 'MEMBER_JOINED', 'MEMBER_REMOVED', 'MEMBER_ROLE_CHANGED',
        'PERMISSIONS_UPDATED', 'DEAL_APPROVED', 'DEAL_REJECTED', 'DOCUMENT_APPROVED', 'DOCUMENT_REJECTED'
    )),
    operator_id UUID REFERENCES identities(id), -- Who performed the operation
    target_user_id UUID REFERENCES identities(id), -- Target user if applicable
    old_values JSONB,
    new_values JSONB,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STEP 7: Create indexes for performance
-- ============================================================================

-- Company lookups
CREATE INDEX idx_companies_status ON companies(status);
CREATE INDEX idx_companies_created_at ON companies(created_at DESC);

-- Membership lookups
CREATE INDEX idx_company_memberships_company_id ON company_memberships(company_id);
CREATE INDEX idx_company_memberships_user_id ON company_memberships(user_id);
CREATE INDEX idx_company_memberships_status ON company_memberships(status);
CREATE INDEX idx_company_memberships_role ON company_memberships(role);

-- Compound index for authorization checks
CREATE INDEX idx_company_memberships_company_user_status
    ON company_memberships(company_id, user_id, status);

-- Invitation lookups
CREATE INDEX idx_company_invitations_company_id ON company_invitations(company_id);
CREATE INDEX idx_company_invitations_email ON company_invitations(email);
CREATE INDEX idx_company_invitations_token ON company_invitations(invitation_token);
CREATE INDEX idx_company_invitations_status ON company_invitations(status);
CREATE INDEX idx_company_invitations_expires_at ON company_invitations(expires_at);

-- Audit lookups
CREATE INDEX idx_company_audit_company_id ON company_audit(company_id);
CREATE INDEX idx_company_audit_operation_type ON company_audit(operation_type);
CREATE INDEX idx_company_audit_created_at ON company_audit(created_at DESC);

-- ============================================================================
-- STEP 8: Create helper functions
-- ============================================================================

-- Function to check if user is admin of a company
CREATE OR REPLACE FUNCTION is_company_admin(p_user_id UUID, p_company_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM company_memberships
        WHERE user_id = p_user_id
          AND company_id = p_company_id
          AND role = 'admin'
          AND status = 'active'
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if user has specific permission
CREATE OR REPLACE FUNCTION has_company_permission(
    p_user_id UUID,
    p_company_id UUID,
    p_permission VARCHAR
)
RETURNS BOOLEAN AS $$
DECLARE
    v_is_admin BOOLEAN;
    v_permissions JSONB;
BEGIN
    -- Admins have all permissions
    v_is_admin := is_company_admin(p_user_id, p_company_id);
    IF v_is_admin THEN
        RETURN TRUE;
    END IF;

    -- Check specific permission for regular users
    SELECT permissions INTO v_permissions
    FROM company_memberships
    WHERE user_id = p_user_id
      AND company_id = p_company_id
      AND status = 'active';

    IF v_permissions IS NULL THEN
        RETURN FALSE;
    END IF;

    RETURN (v_permissions->>p_permission)::BOOLEAN = TRUE;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get user's role in a company
CREATE OR REPLACE FUNCTION get_user_company_role(p_user_id UUID, p_company_id UUID)
RETURNS VARCHAR AS $$
DECLARE
    v_role VARCHAR;
BEGIN
    SELECT role INTO v_role
    FROM company_memberships
    WHERE user_id = p_user_id
      AND company_id = p_company_id
      AND status = 'active';

    RETURN v_role;
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- STEP 9: Create triggers
-- ============================================================================

-- Trigger to update updated_at timestamp on companies
CREATE OR REPLACE FUNCTION update_company_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_company_timestamp
    BEFORE UPDATE ON companies
    FOR EACH ROW
    EXECUTE FUNCTION update_company_updated_at();

-- Trigger to update updated_at timestamp on company_memberships
CREATE TRIGGER trigger_update_membership_timestamp
    BEFORE UPDATE ON company_memberships
    FOR EACH ROW
    EXECUTE FUNCTION update_company_updated_at();

-- Trigger to update updated_at timestamp on company_invitations
CREATE TRIGGER trigger_update_invitation_timestamp
    BEFORE UPDATE ON company_invitations
    FOR EACH ROW
    EXECUTE FUNCTION update_company_updated_at();

-- Trigger to audit company membership changes
CREATE OR REPLACE FUNCTION audit_company_membership_changes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO company_audit (
            company_id,
            operation_type,
            operator_id,
            target_user_id,
            new_values
        ) VALUES (
            NEW.company_id,
            'MEMBER_JOINED',
            NEW.invited_by,
            NEW.user_id,
            jsonb_build_object(
                'role', NEW.role,
                'status', NEW.status,
                'permissions', NEW.permissions
            )
        );
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.role != NEW.role THEN
            INSERT INTO company_audit (
                company_id,
                operation_type,
                operator_id,
                target_user_id,
                old_values,
                new_values
            ) VALUES (
                NEW.company_id,
                'MEMBER_ROLE_CHANGED',
                NULL,
                NEW.user_id,
                jsonb_build_object('role', OLD.role),
                jsonb_build_object('role', NEW.role)
            );
        END IF;

        IF OLD.status != NEW.status AND NEW.status = 'removed' THEN
            INSERT INTO company_audit (
                company_id,
                operation_type,
                operator_id,
                target_user_id,
                old_values
            ) VALUES (
                NEW.company_id,
                'MEMBER_REMOVED',
                NULL,
                NEW.user_id,
                jsonb_build_object('role', OLD.role, 'status', OLD.status)
            );
        END IF;

        IF OLD.permissions::text != NEW.permissions::text THEN
            INSERT INTO company_audit (
                company_id,
                operation_type,
                operator_id,
                target_user_id,
                old_values,
                new_values
            ) VALUES (
                NEW.company_id,
                'PERMISSIONS_UPDATED',
                NULL,
                NEW.user_id,
                jsonb_build_object('permissions', OLD.permissions),
                jsonb_build_object('permissions', NEW.permissions)
            );
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_audit_membership_changes
    AFTER INSERT OR UPDATE ON company_memberships
    FOR EACH ROW
    EXECUTE FUNCTION audit_company_membership_changes();

-- ============================================================================
-- STEP 10: Add comments
-- ============================================================================

COMMENT ON TABLE companies IS 'Simplified company/organization table for managing teams';
COMMENT ON TABLE company_memberships IS 'Links users to companies with role-based access control';
COMMENT ON TABLE company_invitations IS 'Tracks pending invitations to join companies';
COMMENT ON TABLE company_audit IS 'Audit trail for all company-related operations';

COMMENT ON COLUMN company_memberships.role IS 'User role: admin (full permissions) or user (limited permissions)';
COMMENT ON COLUMN company_memberships.permissions IS 'Granular permissions JSON object for fine-grained access control';
COMMENT ON COLUMN companies.status IS 'Company status: active, suspended, or inactive';
COMMENT ON COLUMN company_memberships.status IS 'Membership status: pending, active, suspended, or removed';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
