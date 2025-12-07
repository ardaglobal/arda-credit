-- Migration: Recreate collateral_matches table
-- Description: Recreate collateral_matches table to store detection results from arda-collateral-intel
--
-- Background: Migration 060 dropped collateral_matches table assuming detection was fully external.
-- However, arda-credit needs to persist detection results it receives from arda-collateral-intel
-- for quick querying and to populate the double_pledged flag.

-- ============================================================================
-- STEP 1: Recreate collateral_matches table
-- ============================================================================

CREATE TABLE collateral_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    matched_collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    match_score DECIMAL(5, 2) NOT NULL CHECK (match_score >= 0 AND match_score <= 100),
    match_type VARCHAR(50) NOT NULL, -- 'exact_name', 'custom_id', 'address', 'fuzzy_name', 'semantic'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Prevent duplicate matches
    UNIQUE(collateral_id, matched_collateral_id),
    
    -- Prevent self-matches
    CHECK (collateral_id != matched_collateral_id)
);

-- ============================================================================
-- STEP 2: Create indexes for efficient querying
-- ============================================================================

-- Index for looking up matches by collateral
CREATE INDEX idx_collateral_matches_collateral_id ON collateral_matches(collateral_id);

-- Index for reverse lookups (what collaterals match this one)
CREATE INDEX idx_collateral_matches_matched_id ON collateral_matches(matched_collateral_id);

-- Index for filtering by match score
CREATE INDEX idx_collateral_matches_score ON collateral_matches(match_score DESC);

-- ============================================================================
-- STEP 3: Add comments
-- ============================================================================

COMMENT ON TABLE collateral_matches IS 'Stores duplicate detection results received from arda-collateral-intel service';
COMMENT ON COLUMN collateral_matches.match_score IS 'Similarity score from 0-100 provided by detection service';
COMMENT ON COLUMN collateral_matches.match_type IS 'Type of match: exact_name, custom_id, address, fuzzy_name, semantic';
COMMENT ON COLUMN collateral_matches.created_at IS 'When this match was detected and stored';

