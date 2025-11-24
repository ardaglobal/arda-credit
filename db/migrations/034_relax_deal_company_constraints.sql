-- ============================================================================
-- RELAX DEAL COMPANY CONSTRAINTS
-- ============================================================================
-- This migration removes foreign key constraints on deals table to allow
-- flexible company references while maintaining data integrity checks.
--
-- Changes:
--   1. Remove FK constraints for originator_id, investor_id, and borrower_id
--   2. Keep the deal_parties_different check constraint
--   3. Add unique index on company_name to enforce unique company names
--   4. Add index on name for faster lookups
--
-- Rationale:
--   - Allow deal creation without pre-existing companies
--   - API layer will handle company creation/lookup atomically
--   - Unique company_name prevents duplicate companies
--   - Check constraint still enforces business rule that parties must be different
-- ============================================================================

-- ============================================================================
-- STEP 1: Drop foreign key constraints
-- ============================================================================

ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_originator_id_fkey;
ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_investor_id_fkey;
ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_borrower_id_fkey;

-- ============================================================================
-- STEP 2: Add unique index on company_name
-- ============================================================================
-- This ensures we don't create duplicate companies with the same name
-- The index uses a partial index to ignore NULL values

CREATE UNIQUE INDEX IF NOT EXISTS idx_companies_company_name_unique
    ON companies(company_name)
    WHERE company_name IS NOT NULL;

-- ============================================================================
-- STEP 3: Add index on name column for faster lookups
-- ============================================================================
-- The 'name' column is used for display purposes and searches

CREATE INDEX IF NOT EXISTS idx_companies_name ON companies(name);

-- ============================================================================
-- STEP 4: Add comments for clarity
-- ============================================================================

COMMENT ON TABLE deals IS 'Deals between companies. Company references (originator_id, investor_id, borrower_id) are no longer enforced by FK constraints. API layer ensures referential integrity.';

COMMENT ON CONSTRAINT deal_parties_different ON deals IS 'Ensures originator, investor, and borrower (if present) are all different companies';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================
