-- Migration: Change deals to reference companies instead of users
-- This migration changes originator_id and investor_id from user references to company references

-- ============================================================================
-- STEP 1: Drop existing foreign key constraints
-- ============================================================================

ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_originator_id_fkey;
ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_investor_id_fkey;

-- ============================================================================
-- STEP 2: Drop the constraint that originator_id != investor_id
-- (This made sense for users, but companies can have different roles)
-- ============================================================================

ALTER TABLE deals DROP CONSTRAINT IF EXISTS originator_investor_different;

-- ============================================================================
-- STEP 3: For existing data, map user IDs to their company IDs
-- This ensures we don't lose any data during migration
-- ============================================================================

-- Update originator_id to map to the user's company
UPDATE deals d
SET originator_id = cm.company_id
FROM company_memberships cm
WHERE d.originator_id = cm.user_id
  AND cm.status = 'active'
  AND cm.company_id IS NOT NULL;

-- Update investor_id to map to the user's company (where investor exists)
UPDATE deals d
SET investor_id = cm.company_id
FROM company_memberships cm
WHERE d.investor_id = cm.user_id
  AND cm.status = 'active'
  AND cm.company_id IS NOT NULL;

-- ============================================================================
-- STEP 4: Add new foreign key constraints referencing companies
-- ============================================================================

ALTER TABLE deals
    ADD CONSTRAINT deals_originator_id_fkey
    FOREIGN KEY (originator_id) REFERENCES companies(id) ON DELETE CASCADE;

ALTER TABLE deals
    ADD CONSTRAINT deals_investor_id_fkey
    FOREIGN KEY (investor_id) REFERENCES companies(id) ON DELETE SET NULL;

-- ============================================================================
-- STEP 5: Remove the company_id column since it's now redundant
-- originator_id already identifies the originating company
-- ============================================================================

-- First, drop the foreign key constraint
ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_company_id_fkey;

-- Drop the index
DROP INDEX IF EXISTS idx_deals_company_id;

-- Drop the column
ALTER TABLE deals DROP COLUMN IF EXISTS company_id;

-- ============================================================================
-- STEP 6: Update comments for clarity
-- ============================================================================

COMMENT ON COLUMN deals.originator_id IS 'Company ID of the deal originator (company requesting credit)';
COMMENT ON COLUMN deals.investor_id IS 'Company ID of the investor (company providing capital), NULL for draft deals';

-- ============================================================================
-- STEP 7: Add new constraint to prevent a company from being both originator and investor
-- (A company should not create a deal with itself)
-- ============================================================================

ALTER TABLE deals
    ADD CONSTRAINT originator_investor_different
    CHECK (originator_id != investor_id);

-- ============================================================================
-- Migration Complete
-- ============================================================================

COMMENT ON TABLE deals IS 'Deals between companies: originator_id and investor_id now reference companies(id) instead of users(id)';
