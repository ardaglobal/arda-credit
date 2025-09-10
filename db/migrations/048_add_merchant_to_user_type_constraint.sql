-- Add 'merchant' to the user_type constraint to support merchant registration
-- Expanding from 'user', 'investor', 'lender', 'originator', 'admin' to include 'merchant'

-- ============================================================================
-- UPDATE USER_TYPE CONSTRAINT
-- ============================================================================

-- Drop the existing constraint
ALTER TABLE identities DROP CONSTRAINT IF EXISTS identities_user_type_check;

-- Add the updated constraint with 'merchant' included
ALTER TABLE identities ADD CONSTRAINT identities_user_type_check
    CHECK (user_type IN ('user', 'investor', 'lender', 'originator', 'admin', 'merchant'));

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
-- INSERT INTO identities (user_type, email) VALUES ('merchant', 'test6@example.com');

-- This should fail:
-- INSERT INTO identities (user_type, email) VALUES ('invalid_type', 'test7@example.com');

-- Clean up test data:
-- DELETE FROM identities WHERE email LIKE 'test%@example.com';
