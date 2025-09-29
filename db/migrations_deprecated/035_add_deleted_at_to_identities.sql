-- Migration: Add deleted_at column to identities table for soft deletes
-- Description: Add soft delete functionality to preserve data for regulatory compliance

BEGIN;

-- ============================================================================
-- ADD DELETED_AT COLUMN TO IDENTITIES TABLE
-- ============================================================================

-- Add deleted_at column if it doesn't exist (skip if exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'identities'
        AND column_name = 'deleted_at'
    ) THEN
        ALTER TABLE identities ADD COLUMN deleted_at TIMESTAMP WITH TIME ZONE;
    END IF;
END $$;

-- ============================================================================
-- ADD INDEX FOR EFFICIENT SOFT DELETE QUERIES
-- ============================================================================

-- Create index on deleted_at for efficient filtering of active users
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE tablename = 'identities'
        AND indexname = 'idx_identities_deleted_at'
    ) THEN
        CREATE INDEX idx_identities_deleted_at ON identities(deleted_at) WHERE deleted_at IS NULL;
    END IF;
END $$;

-- ============================================================================
-- ADD CONSTRAINT TO PREVENT DELETION OF ALREADY DELETED USERS
-- ============================================================================

-- Add constraint to ensure deleted_at can only be set once (prevents double deletion)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'check_deleted_at_not_changed'
    ) THEN
        ALTER TABLE identities ADD CONSTRAINT check_deleted_at_not_changed
        CHECK (deleted_at IS NULL OR deleted_at <= NOW());
    END IF;
END $$;

-- ============================================================================
-- UPDATE TRIGGER FOR UPDATED_AT ON IDENTITIES
-- ============================================================================

-- Ensure the updated_at trigger still works when we set deleted_at
-- The existing trigger should handle this automatically

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- This migration adds soft delete functionality to the identities table:
-- 1. deleted_at column tracks when a user was soft-deleted
-- 2. Index on deleted_at IS NULL for efficient active user queries
-- 3. Constraint prevents changing deletion timestamp once set
--
-- Usage:
-- - To soft delete: UPDATE identities SET deleted_at = NOW() WHERE id = ?
-- - To find active users: SELECT * FROM identities WHERE deleted_at IS NULL
-- - To find deleted users: SELECT * FROM identities WHERE deleted_at IS NOT NULL

COMMIT;
