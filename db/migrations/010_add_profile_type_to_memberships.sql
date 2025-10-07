-- ============================================================================
-- ADD PROFILE TYPE TO COMPANY MEMBERSHIPS
-- ============================================================================
-- This migration adds a profile_type field to company_memberships to support
-- users having different roles in different companies.
--
-- Key features:
--   - A user can be an originator in Company A and an investor in Company B
--   - A user can have BOTH originator and investor profiles in the same company
--   - This is achieved by having separate membership records for each profile type
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: Add profile_type column to company_memberships
-- ============================================================================

ALTER TABLE company_memberships
ADD COLUMN profile_type VARCHAR(20) NOT NULL DEFAULT 'investor'
CHECK (profile_type IN ('originator', 'investor'));

-- ============================================================================
-- STEP 2: Add profile_type column to company_invitations
-- ============================================================================

ALTER TABLE company_invitations
ADD COLUMN profile_type VARCHAR(20) NOT NULL DEFAULT 'investor'
CHECK (profile_type IN ('originator', 'investor'));

-- ============================================================================
-- STEP 3: Update the unique constraint to include profile_type
-- ============================================================================

-- Drop the old unique constraint (company_id, user_id)
ALTER TABLE company_memberships
DROP CONSTRAINT IF EXISTS company_memberships_company_id_user_id_key;

-- Add new unique constraint that includes profile_type
-- This allows a user to have multiple memberships in the same company with different profile types
ALTER TABLE company_memberships
ADD CONSTRAINT company_memberships_company_user_profile_unique
UNIQUE(company_id, user_id, profile_type);

-- ============================================================================
-- STEP 11: Add index for efficient profile_type queries
-- ============================================================================

CREATE INDEX idx_company_memberships_profile_type
ON company_memberships(profile_type);

CREATE INDEX idx_company_memberships_company_profile
ON company_memberships(company_id, profile_type, status);

-- ============================================================================
-- STEP 11: Update audit trigger to track profile_type changes
-- ============================================================================

-- The existing audit_company_membership_changes trigger will automatically
-- capture profile_type in the JSONB fields, but let's add a comment

COMMENT ON COLUMN company_memberships.profile_type IS
'Profile type: originator (creates deals) or investor (provides capital).
A user can have multiple memberships in the same company with different profile types.';

-- ============================================================================
-- STEP 11: Add helper functions for profile-aware queries
-- ============================================================================

-- Function to check if user has specific profile type in a company
CREATE OR REPLACE FUNCTION has_company_profile(
    p_user_id UUID,
    p_company_id UUID,
    p_profile_type VARCHAR
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM company_memberships
        WHERE user_id = p_user_id
          AND company_id = p_company_id
          AND profile_type = p_profile_type
          AND status = 'active'
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get all profile types for a user in a company
CREATE OR REPLACE FUNCTION get_user_profiles_in_company(
    p_user_id UUID,
    p_company_id UUID
)
RETURNS TEXT[] AS $$
BEGIN
    RETURN ARRAY(
        SELECT profile_type::TEXT
        FROM company_memberships
        WHERE user_id = p_user_id
          AND company_id = p_company_id
          AND status = 'active'
        ORDER BY profile_type
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if user is originator admin in a company
CREATE OR REPLACE FUNCTION is_originator_admin(
    p_user_id UUID,
    p_company_id UUID
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM company_memberships
        WHERE user_id = p_user_id
          AND company_id = p_company_id
          AND profile_type = 'originator'
          AND role = 'admin'
          AND status = 'active'
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if user is investor admin in a company
CREATE OR REPLACE FUNCTION is_investor_admin(
    p_user_id UUID,
    p_company_id UUID
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM company_memberships
        WHERE user_id = p_user_id
          AND company_id = p_company_id
          AND profile_type = 'investor'
          AND role = 'admin'
          AND status = 'active'
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- STEP 11: Update company_audit operations to include profile types
-- ============================================================================

ALTER TABLE company_audit
DROP CONSTRAINT IF EXISTS company_audit_operation_type_check;

ALTER TABLE company_audit
ADD CONSTRAINT company_audit_operation_type_check
CHECK (operation_type IN (
    'COMPANY_CREATED', 'COMPANY_UPDATED', 'COMPANY_SUSPENDED', 'COMPANY_REACTIVATED',
    'MEMBER_INVITED', 'MEMBER_JOINED', 'MEMBER_REMOVED', 'MEMBER_ROLE_CHANGED',
    'PERMISSIONS_UPDATED', 'DEAL_APPROVED', 'DEAL_REJECTED',
    'DOCUMENT_APPROVED', 'DOCUMENT_REJECTED',
    'PROFILE_ADDED', 'PROFILE_REMOVED'  -- New operations for profile management
));

-- ============================================================================
-- STEP 11: Update audit trigger to log profile changes
-- ============================================================================

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
            CASE
                WHEN NEW.status = 'pending' THEN 'MEMBER_INVITED'
                ELSE 'MEMBER_JOINED'
            END,
            NEW.invited_by,
            NEW.user_id,
            jsonb_build_object(
                'role', NEW.role,
                'status', NEW.status,
                'profile_type', NEW.profile_type,
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
                jsonb_build_object('role', OLD.role, 'profile_type', OLD.profile_type),
                jsonb_build_object('role', NEW.role, 'profile_type', NEW.profile_type)
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
                jsonb_build_object('role', OLD.role, 'status', OLD.status, 'profile_type', OLD.profile_type)
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
                jsonb_build_object('permissions', OLD.permissions, 'profile_type', OLD.profile_type),
                jsonb_build_object('permissions', NEW.permissions, 'profile_type', NEW.profile_type)
            );
        END IF;

        IF OLD.profile_type != NEW.profile_type THEN
            INSERT INTO company_audit (
                company_id,
                operation_type,
                operator_id,
                target_user_id,
                old_values,
                new_values
            ) VALUES (
                NEW.company_id,
                'PROFILE_REMOVED',
                NULL,
                NEW.user_id,
                jsonb_build_object('profile_type', OLD.profile_type),
                jsonb_build_object('profile_type', NEW.profile_type)
            );
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 11: Add comments
-- ============================================================================

COMMENT ON CONSTRAINT company_memberships_company_user_profile_unique ON company_memberships IS
'A user can have multiple memberships in the same company with different profile types (originator and investor)';

COMMENT ON FUNCTION has_company_profile(UUID, UUID, VARCHAR) IS
'Check if a user has a specific active profile type in a company';

COMMENT ON FUNCTION get_user_profiles_in_company(UUID, UUID) IS
'Get all active profile types for a user in a company';

COMMENT ON FUNCTION is_originator_admin(UUID, UUID) IS
'Check if user is an admin with originator profile in a company';

COMMENT ON FUNCTION is_investor_admin(UUID, UUID) IS
'Check if user is an admin with investor profile in a company';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
