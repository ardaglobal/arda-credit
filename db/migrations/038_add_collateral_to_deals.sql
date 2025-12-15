-- Migration: Add collateral_id to deals table
-- Description: Links deals to collateral assets

-- ============================================================================
-- STEP 1: Add collateral_id column to deals table
-- ============================================================================

ALTER TABLE deals
    ADD COLUMN collateral_id UUID REFERENCES collateral(id) ON DELETE SET NULL;

-- ============================================================================
-- STEP 2: Create index for collateral_id queries
-- ============================================================================

CREATE INDEX idx_deals_collateral_id ON deals(collateral_id);

-- ============================================================================
-- STEP 3: Add comment
-- ============================================================================

COMMENT ON COLUMN deals.collateral_id IS 'UUID of collateral asset associated with this deal (optional)';
