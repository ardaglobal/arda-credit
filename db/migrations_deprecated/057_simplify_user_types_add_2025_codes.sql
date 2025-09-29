-- Simplify user types and add 2025 invite codes
-- Remove support for 'user', 'merchant', 'lender', 'regular' user types
-- Keep only: 'investor', 'originator', 'admin'
-- Add new 2025 invite codes for the three supported user types

-- ============================================================================
-- DROP EXISTING CONSTRAINTS FIRST
-- ============================================================================

-- Drop the existing invite_codes constraint before making data changes
ALTER TABLE invite_codes DROP CONSTRAINT IF EXISTS chk_invite_codes_user_type;

-- ============================================================================
-- UPDATE EXISTING DATA
-- ============================================================================

-- Update any existing invite codes with old user types to valid ones
-- Map 'regular' to 'investor' as the most common user type
UPDATE invite_codes
SET user_type = 'investor'
WHERE user_type = 'regular';

-- Any other unsupported user types should be updated to 'investor' as default
UPDATE invite_codes
SET user_type = 'investor'
WHERE user_type NOT IN ('investor', 'originator', 'admin');

-- ============================================================================
-- UPDATE IDENTITIES TABLE USER_TYPE CONSTRAINT
-- ============================================================================

-- Drop the existing constraint
ALTER TABLE identities DROP CONSTRAINT IF EXISTS identities_user_type_check;

-- Add the updated constraint with only the three supported user types
ALTER TABLE identities ADD CONSTRAINT identities_user_type_check
    CHECK (user_type IN ('investor', 'originator', 'admin'));

-- ============================================================================
-- UPDATE INVITE_CODES TABLE USER_TYPE CONSTRAINT
-- ============================================================================

-- Drop the existing constraint
ALTER TABLE invite_codes DROP CONSTRAINT IF EXISTS chk_invite_codes_user_type;

-- Add the updated constraint with only the three supported user types
ALTER TABLE invite_codes ADD CONSTRAINT chk_invite_codes_user_type
    CHECK (user_type IN ('investor', 'originator', 'admin'));

-- ============================================================================
-- ADD 2025 INVITE CODES
-- ============================================================================

-- Insert the three new 2025 invite codes
INSERT INTO invite_codes (
    code,
    description,
    max_uses,
    expires_at,
    user_type,
    is_active
) VALUES
    (
        'INVESTOR2025',
        'Investor access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'investor',
        true
    ),
    (
        'ORIGINATOR2025',
        'Originator access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'originator',
        true
    ),
    (
        'ADMIN2025',
        'Admin access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'admin',
        true
    )
ON CONFLICT (code) DO NOTHING;

-- ============================================================================
-- UPDATE DATABASE FUNCTIONS FOR NEW USER TYPES
-- ============================================================================

-- Update the create_invite_code function to only accept the new user types
CREATE OR REPLACE FUNCTION create_invite_code(
    invite_code VARCHAR(64),
    creator_id UUID DEFAULT NULL,
    expiry_date TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    max_usage INTEGER DEFAULT 1,
    code_description TEXT DEFAULT NULL,
    invite_user_type VARCHAR(20) DEFAULT 'investor'
)
RETURNS BOOLEAN AS $$
BEGIN
    -- Validate user_type - only allow the three supported types
    IF invite_user_type NOT IN ('investor', 'originator', 'admin') THEN
        RAISE EXCEPTION 'Invalid user_type: %. Must be one of: investor, originator, admin', invite_user_type;
    END IF;

    INSERT INTO invite_codes (
        code,
        created_by,
        expires_at,
        max_uses,
        description,
        user_type
    ) VALUES (
        invite_code,
        creator_id,
        expiry_date,
        max_usage,
        code_description,
        invite_user_type
    );

    RETURN TRUE;
EXCEPTION
    WHEN unique_violation THEN
        -- Code already exists
        RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- VERIFICATION QUERIES (for manual testing)
-- ============================================================================

-- Test queries to verify the constraints work correctly
-- These should succeed for all valid user types:
-- INSERT INTO identities (user_type, email) VALUES ('investor', 'test1@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('originator', 'test2@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('admin', 'test3@example.com');

-- These should fail:
-- INSERT INTO identities (user_type, email) VALUES ('user', 'test4@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('merchant', 'test5@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('lender', 'test6@example.com');
-- INSERT INTO identities (user_type, email) VALUES ('regular', 'test7@example.com');

-- Clean up test data:
-- DELETE FROM identities WHERE email LIKE 'test%@example.com';

-- Test invite code functions:
-- SELECT * FROM get_invite_code_info('INVESTOR2025');
-- SELECT * FROM get_invite_code_info('ORIGINATOR2025');
-- SELECT * FROM get_invite_code_info('ADMIN2025');
