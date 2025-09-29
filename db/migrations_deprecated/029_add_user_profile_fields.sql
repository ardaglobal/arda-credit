-- Add user profile fields to identities table
-- This migration adds first_name, last_name fields for user profiles

-- ============================================================================
-- ADD USER PROFILE FIELDS TO IDENTITIES TABLE
-- ============================================================================

-- Add first_name and last_name fields
ALTER TABLE identities ADD COLUMN first_name VARCHAR(100);
ALTER TABLE identities ADD COLUMN last_name VARCHAR(100);

-- ============================================================================
-- ADD INDEXES FOR USER PROFILE FIELDS
-- ============================================================================

-- Index for first_name searches (optional but useful for admin searches)
CREATE INDEX idx_identities_first_name ON identities(first_name)
WHERE first_name IS NOT NULL;

-- Index for last_name searches (optional but useful for admin searches)
CREATE INDEX idx_identities_last_name ON identities(last_name)
WHERE last_name IS NOT NULL;

-- Index for full name searches (composite index)
CREATE INDEX idx_identities_full_name ON identities(first_name, last_name)
WHERE first_name IS NOT NULL OR last_name IS NOT NULL;

-- ============================================================================
-- ADD CONSTRAINTS AND VALIDATION
-- ============================================================================

-- Add constraint: first_name and last_name must be reasonable length
ALTER TABLE identities ADD CONSTRAINT identities_first_name_length
CHECK (first_name IS NULL OR length(trim(first_name)) > 0);

ALTER TABLE identities ADD CONSTRAINT identities_last_name_length
CHECK (last_name IS NULL OR length(trim(last_name)) > 0);

-- ============================================================================
-- UPDATE TRIGGER FOR UPDATED_AT
-- ============================================================================

-- The existing trigger will automatically handle these new columns

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON COLUMN identities.first_name IS 'User first name for profile display';
COMMENT ON COLUMN identities.last_name IS 'User last name for profile display';

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- IMPORTANT: After this migration:
-- 1. Existing users will have NULL first_name and last_name - they can be updated later
-- 2. New registrations can optionally set these fields
-- 3. The wallet_address field already exists from migration 028
-- 4. Update the Identity struct in Rust code to include these new fields
