-- Migration: Add collateral detection metadata
-- Description: Adds detection tracking fields to collateral table for storing
--              minimal detection status from arda-collateral-intel service

-- ============================================================================
-- UP MIGRATION
-- ============================================================================

-- Add detection metadata columns to collateral table
ALTER TABLE collateral
    ADD COLUMN has_detection BOOLEAN DEFAULT FALSE NOT NULL,
    ADD COLUMN detection_count INTEGER DEFAULT 0 NOT NULL,
    ADD COLUMN last_detection_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN detection_status VARCHAR(50);

-- Add index for filtering by detection status
CREATE INDEX idx_collateral_has_detection ON collateral(has_detection);

-- Add comments
COMMENT ON COLUMN collateral.has_detection IS 'Flag indicating detection has been performed by arda-collateral-intel';
COMMENT ON COLUMN collateral.detection_count IS 'Number of matches found in last detection';
COMMENT ON COLUMN collateral.last_detection_at IS 'Timestamp when detection was last performed';
COMMENT ON COLUMN collateral.detection_status IS 'Status from intel service (e.g., no_matches, potential_duplicates, high_confidence_match)';

-- ============================================================================
-- DOWN MIGRATION
-- ============================================================================

-- To rollback this migration, run:
-- DROP INDEX IF EXISTS idx_collateral_has_detection;
-- ALTER TABLE collateral
--     DROP COLUMN IF EXISTS has_detection,
--     DROP COLUMN IF EXISTS detection_count,
--     DROP COLUMN IF EXISTS last_detection_at,
--     DROP COLUMN IF EXISTS detection_status;

