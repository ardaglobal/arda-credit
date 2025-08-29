-- Update user_type constraint to support additional user types
-- Expanding from just 'investor', 'lender' to include 'user', 'originator', 'admin'

-- ============================================================================
-- UPDATE USER_TYPE CONSTRAINT
-- ============================================================================

-- Drop the existing constraint
ALTER TABLE identities DROP CONSTRAINT IF EXISTS identities_user_type_check;

-- Add the updated constraint with all supported user types
ALTER TABLE identities ADD CONSTRAINT identities_user_type_check 
    CHECK (user_type IN ('user', 'investor', 'lender', 'originator', 'admin'));

-- ============================================================================
-- VERIFICATION QUERY (for manual testing)
-- ============================================================================

-- Test query to verify the constraint works
-- This should succeed for all valid user types:
-- INSERT INTO identities (user_type, email) VALUES ('user', 'test1@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('investor', 'test2@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('lender', 'test3@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('originator', 'test4@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('admin', 'test5@example.com');

-- This should fail:
-- INSERT INTO identities (user_type, email) VALUES ('invalid_type', 'test6@example.com');

-- Clean up test data:
-- DELETE FROM identities WHERE email LIKE 'test%@example.com';
