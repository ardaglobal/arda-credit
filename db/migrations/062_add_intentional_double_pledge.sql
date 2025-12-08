-- Migration: Add intentional double pledge support
-- Description: Add flag and metadata to support intentional double pledging where
--              the same collateral is knowingly used across multiple deals

-- ============================================================================
-- STEP 1: Add intentional_double_pledge flag to collateral table
-- ============================================================================

ALTER TABLE collateral
    ADD COLUMN intentional_double_pledge BOOLEAN DEFAULT FALSE,
    ADD COLUMN double_pledge_notes TEXT;

-- ============================================================================
-- STEP 2: Create index for querying intentionally double-pledged collateral
-- ============================================================================

CREATE INDEX idx_collateral_intentional_double_pledge 
    ON collateral(intentional_double_pledge) 
    WHERE intentional_double_pledge = TRUE;

-- ============================================================================
-- STEP 3: Create helper view for collateral with deal counts
-- ============================================================================

CREATE OR REPLACE VIEW collateral_with_deal_count AS
SELECT 
    c.*,
    COUNT(d.id) as deal_count,
    ARRAY_AGG(d.id) FILTER (WHERE d.id IS NOT NULL) as deal_ids
FROM collateral c
LEFT JOIN deals d ON d.collateral_id = c.id
GROUP BY c.id, c.name, c.collateral_type, c.description, c.address, 
         c.custom_id, c.double_pledged, c.has_potential_duplicates,
         c.intentional_double_pledge, c.double_pledge_notes,
         c.created_at, c.updated_at, c.created_by, c.metadata;

-- ============================================================================
-- STEP 4: Add comments
-- ============================================================================

COMMENT ON COLUMN collateral.intentional_double_pledge IS 'Flag indicating user has intentionally marked this collateral as used across multiple deals';
COMMENT ON COLUMN collateral.double_pledge_notes IS 'Optional notes explaining why collateral is intentionally double-pledged';
COMMENT ON VIEW collateral_with_deal_count IS 'View showing collateral with count and list of associated deals';
