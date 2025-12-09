-- Migration: Drop intentional double pledge columns
-- Description: Remove intentional_double_pledge and double_pledge_notes columns
--              as this logic belongs in arda-collateral-intel service per
--              architectural separation of concerns.
--
-- Reasoning: User intentions about double-pledging are business logic that should
--            be managed by the duplicate detection service, not the core deal system.

-- ============================================================================
-- STEP 1: Drop the index
-- ============================================================================

DROP INDEX IF EXISTS idx_collateral_intentional_double_pledge;

-- ============================================================================
-- STEP 2: Drop the view (will be recreated without the columns)
-- ============================================================================

DROP VIEW IF EXISTS collateral_with_deal_count;

-- ============================================================================
-- STEP 3: Drop the columns from collateral table
-- ============================================================================

ALTER TABLE collateral
    DROP COLUMN IF EXISTS intentional_double_pledge,
    DROP COLUMN IF EXISTS double_pledge_notes;

-- ============================================================================
-- STEP 4: Recreate the view without the dropped columns
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
         c.created_at, c.updated_at, c.created_by, c.metadata;

-- ============================================================================
-- STEP 5: Add comment explaining the change
-- ============================================================================

COMMENT ON VIEW collateral_with_deal_count IS 'View showing collateral with count and list of associated deals. User intentions about double-pledging are managed by arda-collateral-intel service.';

