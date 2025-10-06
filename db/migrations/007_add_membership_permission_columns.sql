-- ============================================================================
-- ADD INDIVIDUAL PERMISSION COLUMNS TO COMPANY_MEMBERSHIPS
-- ============================================================================
-- This migration adds individual permission boolean columns to company_memberships
-- for easier querying. The permissions JSONB field is kept for flexibility.
-- ============================================================================

-- ============================================================================
-- STEP 1: Add permission columns
-- ============================================================================

ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_upload_documents BOOLEAN DEFAULT FALSE;
ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_download_documents BOOLEAN DEFAULT TRUE;
ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_delete_documents BOOLEAN DEFAULT FALSE;
ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_approve_documents BOOLEAN DEFAULT FALSE;
ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_manage_team BOOLEAN DEFAULT FALSE;
ALTER TABLE company_memberships ADD COLUMN IF NOT EXISTS can_view_audit_logs BOOLEAN DEFAULT FALSE;

-- ============================================================================
-- STEP 2: Set default permissions based on role
-- ============================================================================

-- Admins get all permissions
UPDATE company_memberships
SET
    can_upload_documents = true,
    can_download_documents = true,
    can_delete_documents = true,
    can_approve_documents = true,
    can_manage_team = true,
    can_view_audit_logs = true
WHERE role = 'admin';

-- Regular users get basic permissions
UPDATE company_memberships
SET
    can_upload_documents = false,
    can_download_documents = true,
    can_delete_documents = false,
    can_approve_documents = false,
    can_manage_team = false,
    can_view_audit_logs = false
WHERE role = 'user' AND can_upload_documents IS NULL;

-- ============================================================================
-- STEP 3: Create function to sync permissions to JSONB
-- ============================================================================

CREATE OR REPLACE FUNCTION sync_membership_permissions_to_jsonb()
RETURNS TRIGGER AS $$
BEGIN
    -- Sync individual columns to permissions JSONB
    NEW.permissions = jsonb_build_object(
        'can_upload_documents', COALESCE(NEW.can_upload_documents, false),
        'can_download_documents', COALESCE(NEW.can_download_documents, true),
        'can_delete_documents', COALESCE(NEW.can_delete_documents, false),
        'can_approve_documents', COALESCE(NEW.can_approve_documents, false),
        'can_manage_team', COALESCE(NEW.can_manage_team, false),
        'can_view_audit_logs', COALESCE(NEW.can_view_audit_logs, false)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_sync_membership_permissions
    BEFORE INSERT OR UPDATE ON company_memberships
    FOR EACH ROW
    EXECUTE FUNCTION sync_membership_permissions_to_jsonb();

-- ============================================================================
-- STEP 4: Sync existing permissions to JSONB
-- ============================================================================

UPDATE company_memberships
SET permissions = jsonb_build_object(
    'can_upload_documents', COALESCE(can_upload_documents, false),
    'can_download_documents', COALESCE(can_download_documents, true),
    'can_delete_documents', COALESCE(can_delete_documents, false),
    'can_approve_documents', COALESCE(can_approve_documents, false),
    'can_manage_team', COALESCE(can_manage_team, false),
    'can_view_audit_logs', COALESCE(can_view_audit_logs, false)
);

-- ============================================================================
-- STEP 5: Add comments
-- ============================================================================

COMMENT ON COLUMN company_memberships.can_upload_documents IS 'Permission to upload documents';
COMMENT ON COLUMN company_memberships.can_download_documents IS 'Permission to download documents';
COMMENT ON COLUMN company_memberships.can_delete_documents IS 'Permission to delete documents';
COMMENT ON COLUMN company_memberships.can_approve_documents IS 'Permission to approve documents';
COMMENT ON COLUMN company_memberships.can_manage_team IS 'Permission to manage team members';
COMMENT ON COLUMN company_memberships.can_view_audit_logs IS 'Permission to view audit logs';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
