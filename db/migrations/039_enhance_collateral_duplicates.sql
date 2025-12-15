-- Migration: Enhance collateral duplicate detection
-- Description: Add multiple match tracking with scores, queue system, and configurable thresholds

-- ============================================================================
-- STEP 1: Add collateral_matches table for multiple matches with scores
-- ============================================================================

CREATE TABLE collateral_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    matched_collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    match_score DECIMAL(5, 2) NOT NULL CHECK (match_score >= 0 AND match_score <= 100),
    match_type VARCHAR(50) NOT NULL, -- 'exact_name', 'custom_id', 'address', 'fuzzy_name'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Prevent duplicate matches
    UNIQUE(collateral_id, matched_collateral_id),
    
    -- Prevent self-matches
    CHECK (collateral_id != matched_collateral_id)
);

-- Indexes for collateral_matches
CREATE INDEX idx_collateral_matches_collateral_id ON collateral_matches(collateral_id);
CREATE INDEX idx_collateral_matches_matched_id ON collateral_matches(matched_collateral_id);
CREATE INDEX idx_collateral_matches_score ON collateral_matches(match_score DESC);

-- ============================================================================
-- STEP 2: Add queue table for duplicate checks
-- ============================================================================

CREATE TABLE collateral_check_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    status VARCHAR(20) DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    priority INT DEFAULT 0, -- Higher priority processed first
    retry_count INT DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    
    -- Prevent duplicate queue entries for same collateral
    UNIQUE(collateral_id)
);

-- Indexes for queue processing
CREATE INDEX idx_collateral_check_queue_status ON collateral_check_queue(status, priority DESC, created_at);
CREATE INDEX idx_collateral_check_queue_pending ON collateral_check_queue(created_at) WHERE status = 'pending';
CREATE INDEX idx_collateral_check_queue_failed ON collateral_check_queue(retry_count, created_at) WHERE status = 'failed';

-- ============================================================================
-- STEP 3: Add configuration table for thresholds
-- ============================================================================

CREATE TABLE collateral_check_config (
    id SERIAL PRIMARY KEY,
    match_threshold DECIMAL(5, 2) DEFAULT 50.0 CHECK (match_threshold >= 0 AND match_threshold <= 100),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default config
INSERT INTO collateral_check_config (match_threshold) VALUES (50.0);

-- ============================================================================
-- STEP 4: Update collateral table
-- ============================================================================

-- Add has_potential_duplicates flag (keep double_pledged for backward compatibility)
ALTER TABLE collateral
    ADD COLUMN has_potential_duplicates BOOLEAN DEFAULT FALSE;

-- Index for filtering collateral with duplicates
CREATE INDEX idx_collateral_has_duplicates ON collateral(has_potential_duplicates) WHERE has_potential_duplicates = TRUE;

-- ============================================================================
-- STEP 5: Add comments
-- ============================================================================

COMMENT ON TABLE collateral_matches IS 'Stores all potential duplicate matches with individual scores per match';
COMMENT ON COLUMN collateral_matches.match_score IS 'Similarity score from 0-100 (threshold default: 50)';
COMMENT ON COLUMN collateral_matches.match_type IS 'Type of match: exact_name, custom_id, address, fuzzy_name';

COMMENT ON TABLE collateral_check_queue IS 'Queue for background duplicate detection processing';
COMMENT ON COLUMN collateral_check_queue.status IS 'Processing status: pending, processing, completed, failed';
COMMENT ON COLUMN collateral_check_queue.priority IS 'Higher priority processed first (default: 0)';
COMMENT ON COLUMN collateral_check_queue.retry_count IS 'Number of retry attempts for failed checks';

COMMENT ON TABLE collateral_check_config IS 'Configuration for duplicate detection thresholds';
COMMENT ON COLUMN collateral_check_config.match_threshold IS 'Minimum score required for a match (default: 50.0)';

COMMENT ON COLUMN collateral.has_potential_duplicates IS 'Quick flag indicating if this collateral has any potential duplicates';

