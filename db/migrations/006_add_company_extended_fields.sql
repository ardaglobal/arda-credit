-- ============================================================================
-- ADD EXTENDED COMPANY FIELDS
-- ============================================================================
-- This migration adds back extended company fields that are used by the
-- company service but were removed in the simplified structure.
--
-- Added fields:
-- - company_type (originator/investor)
-- - kyb_status (KYB verification status)
-- - kyb_submitted_at, kyb_verified_at (KYB timestamps)
-- - Additional company details (industry, jurisdiction, etc.)
-- ============================================================================

-- ============================================================================
-- STEP 1: Add extended fields to companies table
-- ============================================================================

ALTER TABLE companies ADD COLUMN IF NOT EXISTS company_name VARCHAR(255);
ALTER TABLE companies ADD COLUMN IF NOT EXISTS company_type VARCHAR(20) CHECK (company_type IN ('originator', 'investor'));
ALTER TABLE companies ADD COLUMN IF NOT EXISTS industry VARCHAR(100);
ALTER TABLE companies ADD COLUMN IF NOT EXISTS jurisdiction VARCHAR(100);
ALTER TABLE companies ADD COLUMN IF NOT EXISTS registration_number VARCHAR(100);
ALTER TABLE companies ADD COLUMN IF NOT EXISTS business_email VARCHAR(255);
ALTER TABLE companies ADD COLUMN IF NOT EXISTS business_phone VARCHAR(20);

-- KYB (Know Your Business) fields
ALTER TABLE companies ADD COLUMN IF NOT EXISTS kyb_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted'
    CHECK (kyb_status IN ('not_submitted', 'pending', 'approved', 'rejected'));
ALTER TABLE companies ADD COLUMN IF NOT EXISTS kyb_submitted_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE companies ADD COLUMN IF NOT EXISTS kyb_verified_at TIMESTAMP WITH TIME ZONE;

-- Document retention and audit
ALTER TABLE companies ADD COLUMN IF NOT EXISTS document_retention_days INTEGER DEFAULT 2555; -- ~7 years

-- Track which user created the company (for admin queries)
-- Note: This is informational only, actual admin status is in company_memberships
ALTER TABLE companies ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id);

-- ============================================================================
-- STEP 2: Add indexes for new columns
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_companies_company_type ON companies(company_type);
CREATE INDEX IF NOT EXISTS idx_companies_kyb_status ON companies(kyb_status);
CREATE INDEX IF NOT EXISTS idx_companies_created_by ON companies(created_by);

-- ============================================================================
-- STEP 3: Add comments
-- ============================================================================

COMMENT ON COLUMN companies.company_name IS 'Legal company name';
COMMENT ON COLUMN companies.company_type IS 'Type of company: originator or investor';
COMMENT ON COLUMN companies.kyb_status IS 'KYB verification status: not_submitted, pending, approved, rejected';
COMMENT ON COLUMN companies.created_by IS 'User who created the company (informational, actual admin status in company_memberships)';
COMMENT ON COLUMN companies.document_retention_days IS 'Number of days to retain documents (default ~7 years)';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
