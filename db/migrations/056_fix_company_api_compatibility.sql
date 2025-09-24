-- Fix Company API Compatibility
-- This migration fixes compatibility issues between the database schema
-- and the company service API implementation.

-- ============================================================================
-- COMPANY TABLE FIXES
-- ============================================================================

-- Add missing columns that the API expects
ALTER TABLE companies
ADD COLUMN IF NOT EXISTS company_name VARCHAR(255),
ADD COLUMN IF NOT EXISTS description TEXT,
ADD COLUMN IF NOT EXISTS industry VARCHAR(100),
ADD COLUMN IF NOT EXISTS business_email VARCHAR(255),
ADD COLUMN IF NOT EXISTS business_phone VARCHAR(20);

-- Add KYB-related fields that API expects
ALTER TABLE companies
ADD COLUMN IF NOT EXISTS legal_entity_name VARCHAR(500),
ADD COLUMN IF NOT EXISTS entity_type VARCHAR(100),
ADD COLUMN IF NOT EXISTS beneficial_owners JSONB,
ADD COLUMN IF NOT EXISTS supporting_documents JSONB,
ADD COLUMN IF NOT EXISTS kyb_notes TEXT;

-- Update existing companies to have company_name = name if company_name is NULL
UPDATE companies
SET company_name = name
WHERE company_name IS NULL AND name IS NOT NULL;

-- ============================================================================
-- TEAM MEMBERS TABLE FIXES
-- ============================================================================

-- Add invitation-related fields
ALTER TABLE team_members
ADD COLUMN IF NOT EXISTS invited_by UUID,
ADD COLUMN IF NOT EXISTS invitation_message TEXT;

-- Add invited status to the check constraint if not already there
DO $$
BEGIN
    -- First check if 'invited' is already in the constraint using modern approach
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints cc
        JOIN information_schema.constraint_column_usage ccu
            ON cc.constraint_name = ccu.constraint_name
        WHERE ccu.table_name = 'team_members'
        AND ccu.column_name = 'status'
        AND cc.check_clause LIKE '%invited%'
    ) THEN
        -- Drop old constraint and recreate with invited status
        ALTER TABLE team_members DROP CONSTRAINT IF EXISTS team_members_status_check;
        ALTER TABLE team_members ADD CONSTRAINT team_members_status_check
            CHECK (status IN ('active', 'inactive', 'suspended', 'invited'));
    END IF;
END $$;

-- ============================================================================
-- USERS TABLE COMPATIBILITY
-- ============================================================================

-- Create users table if it doesn't exist (for API compatibility)
-- This creates a simple users table that maps to identities
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    user_type VARCHAR(20) NOT NULL DEFAULT 'investor' CHECK (user_type IN ('investor', 'lender', 'merchant', 'admin')),
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted' CHECK (kyc_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Map to identities table for compatibility
    FOREIGN KEY (id) REFERENCES identities(id) ON DELETE CASCADE
);

-- Create trigger to sync users table with identities
CREATE OR REPLACE FUNCTION sync_users_with_identities()
RETURNS TRIGGER AS $$
BEGIN
    -- Handle INSERT
    IF TG_OP = 'INSERT' THEN
        INSERT INTO users (id, email, user_type, kyc_status, created_at, updated_at)
        VALUES (
            NEW.id,
            NEW.email,
            CASE
                WHEN NEW.user_type IN ('investor', 'lender', 'merchant', 'admin') THEN NEW.user_type
                ELSE 'investor'
            END,
            NEW.kyc_status,
            NEW.created_at,
            NEW.updated_at
        )
        ON CONFLICT (id) DO UPDATE SET
            email = EXCLUDED.email,
            user_type = EXCLUDED.user_type,
            kyc_status = EXCLUDED.kyc_status,
            updated_at = EXCLUDED.updated_at;
        RETURN NEW;
    END IF;

    -- Handle UPDATE
    IF TG_OP = 'UPDATE' THEN
        UPDATE users SET
            email = NEW.email,
            user_type = CASE
                WHEN NEW.user_type IN ('investor', 'lender', 'merchant', 'admin') THEN NEW.user_type
                ELSE COALESCE(OLD.user_type, 'investor')
            END,
            kyc_status = NEW.kyc_status,
            updated_at = NEW.updated_at
        WHERE id = NEW.id;
        RETURN NEW;
    END IF;

    -- Handle DELETE
    IF TG_OP = 'DELETE' THEN
        DELETE FROM users WHERE id = OLD.id;
        RETURN OLD;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger if it doesn't exist
DROP TRIGGER IF EXISTS sync_users_trigger ON identities;
CREATE TRIGGER sync_users_trigger
    AFTER INSERT OR UPDATE OR DELETE ON identities
    FOR EACH ROW
    EXECUTE FUNCTION sync_users_with_identities();

-- Populate users table with existing identities
INSERT INTO users (id, email, user_type, kyc_status, created_at, updated_at)
SELECT
    id,
    email,
    CASE
        WHEN user_type IN ('investor', 'lender', 'merchant', 'admin') THEN user_type
        ELSE 'investor'
    END as user_type,
    kyc_status,
    created_at,
    updated_at
FROM identities
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- ADDITIONAL INDEXES
-- ============================================================================

-- Add indexes for new columns
CREATE INDEX IF NOT EXISTS idx_companies_company_name ON companies(company_name);
CREATE INDEX IF NOT EXISTS idx_companies_business_email ON companies(business_email);
CREATE INDEX IF NOT EXISTS idx_team_members_invited_by ON team_members(invited_by);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_user_type ON users(user_type);
CREATE INDEX IF NOT EXISTS idx_users_kyc_status ON users(kyc_status);

-- ============================================================================
-- UPDATE TRIGGERS
-- ============================================================================

-- Add trigger for users table
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE users IS 'User table for API compatibility - synced with identities table';
COMMENT ON COLUMN companies.company_name IS 'Company display name (API compatibility)';
COMMENT ON COLUMN companies.business_email IS 'Primary business contact email';
COMMENT ON COLUMN companies.business_phone IS 'Primary business contact phone';
COMMENT ON COLUMN companies.beneficial_owners IS 'JSON array of beneficial ownership information for KYB';
COMMENT ON COLUMN companies.supporting_documents IS 'JSON array of document IDs supporting KYB submission';
COMMENT ON COLUMN team_members.invited_by IS 'UUID of the user who invited this team member';
COMMENT ON COLUMN team_members.invitation_message IS 'Optional message included with the team invitation';