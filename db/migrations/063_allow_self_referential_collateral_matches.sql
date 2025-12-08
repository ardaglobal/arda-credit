-- Migration: Allow self-referential collateral matches
-- Description: Fix constraint to allow same collateral UUID to match itself
--              when it's used in multiple active deals (purest form of double-pledge)
--
-- Background: The constraint `CHECK (collateral_id != matched_collateral_id)` 
-- was too restrictive. It blocked detection of the most critical case:
-- when the exact same collateral is used across multiple active deals.
--
-- This migration allows self-referential matches for the 'self_double_pledge' type.

-- ============================================================================
-- STEP 1: Drop the overly restrictive constraint
-- ============================================================================

ALTER TABLE collateral_matches 
DROP CONSTRAINT IF EXISTS collateral_matches_check;

-- ============================================================================
-- STEP 2: Add new constraint that allows self-reference for specific types
-- ============================================================================

ALTER TABLE collateral_matches
ADD CONSTRAINT collateral_matches_check 
CHECK (
    collateral_id != matched_collateral_id 
    OR match_type IN ('self_double_pledge', 'exact_name')
);

-- ============================================================================
-- STEP 3: Add documentation
-- ============================================================================

COMMENT ON CONSTRAINT collateral_matches_check ON collateral_matches IS 
'Allows self-referential matches for self_double_pledge detection (same collateral in multiple active deals) and exact_name matches (duplicate collateral records with same name)';

-- ============================================================================
-- STEP 4: Update table comment
-- ============================================================================

COMMENT ON TABLE collateral_matches IS 
'Stores duplicate detection results from arda-collateral-intel service. Includes self-referential matches when same collateral is used in multiple active deals.';

