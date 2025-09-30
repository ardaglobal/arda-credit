-- Remove Duplicate Users Table
-- This migration removes the redundant users table that was created for API compatibility
-- All functionality should use the identities table directly

-- ============================================================================
-- DROP SYNC TRIGGER AND FUNCTION
-- ============================================================================

-- Drop the trigger that syncs users with identities
DROP TRIGGER IF EXISTS sync_users_trigger ON identities;

-- Drop the sync function
DROP FUNCTION IF EXISTS sync_users_with_identities();

-- ============================================================================
-- DROP USERS TABLE
-- ============================================================================

-- Drop all indexes on users table
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_user_type;
DROP INDEX IF EXISTS idx_users_kyc_status;

-- Drop the users table (CASCADE will handle any dependent views)
DROP TABLE IF EXISTS users CASCADE;

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================
--
-- This migration removes the duplicate users table that was syncing with identities.
-- The identities table is the source of truth and should be used directly.
--
-- Changes required in application code:
-- 1. Update queries from "users" to "identities"
-- 2. Update JOIN clauses to use identities table
-- 3. Regenerate SQLx cache after migration
--
-- Benefits:
-- - Single source of truth for user data
-- - Eliminates sync overhead and potential inconsistencies
-- - Simpler schema with fewer tables to maintain
-- - No more trigger overhead on every identities INSERT/UPDATE/DELETE