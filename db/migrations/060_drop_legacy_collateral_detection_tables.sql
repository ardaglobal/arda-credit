-- Migration: Drop legacy collateral detection tables
-- Description: Remove SQL-based duplicate detection tables now that functionality
--              has been moved to arda-collateral-intel microservice with SurrealDB

-- ============================================================================
-- Background
-- ============================================================================
-- The old detection system used these PostgreSQL tables:
-- - collateral_check_queue: Queue of collaterals pending duplicate check
-- - collateral_check_config: Configuration for the checker (intervals, thresholds)
-- - collateral_checks: History of duplicate checks performed
-- - collateral_matches: Detected matches/duplicates
--
-- The new arda-collateral-intel service stores all detection data in its own
-- SurrealDB instance, making these tables obsolete.
--
-- Note: The `collateral` table itself is retained as it's the main collateral data table.

-- ============================================================================
-- STEP 1: Drop dependent tables first (foreign key constraints)
-- ============================================================================

-- Drop collateral_matches (references collateral table)
DROP TABLE IF EXISTS collateral_matches CASCADE;

-- Drop collateral_check_queue (references collateral table)
DROP TABLE IF EXISTS collateral_check_queue CASCADE;

-- Drop collateral_checks (references collateral table)
DROP TABLE IF EXISTS collateral_checks CASCADE;

-- ============================================================================
-- STEP 2: Drop configuration table
-- ============================================================================

-- Drop collateral_check_config (standalone config table)
DROP TABLE IF EXISTS collateral_check_config CASCADE;

-- ============================================================================
-- STEP 3: Clean up any remaining indexes (if any were created separately)
-- ============================================================================

-- These should be automatically dropped with CASCADE, but we'll be explicit
DROP INDEX IF EXISTS idx_collateral_matches_collateral_id;
DROP INDEX IF EXISTS idx_collateral_matches_matched_id;
DROP INDEX IF EXISTS idx_collateral_matches_score;

DROP INDEX IF EXISTS idx_collateral_check_queue_status;
DROP INDEX IF EXISTS idx_collateral_check_queue_pending;
DROP INDEX IF EXISTS idx_collateral_check_queue_failed;

DROP INDEX IF EXISTS idx_collateral_checks_collateral_id;
DROP INDEX IF EXISTS idx_collateral_checks_status;

-- ============================================================================
-- STEP 4: Verify cleanup
-- ============================================================================

-- The collateral table remains with columns:
-- - double_pledged (UUID) - kept for backward compatibility
-- - has_potential_duplicates (BOOLEAN) - kept for backward compatibility
--
-- These columns can be populated by the new arda-collateral-intel service
-- via API calls if needed.

COMMENT ON TABLE collateral IS 'Collateral assets - duplicate detection now handled by arda-collateral-intel microservice';
