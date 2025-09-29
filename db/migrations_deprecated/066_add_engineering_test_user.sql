-- Add engineering test user with approved KYC status
-- This migration creates a test user account that is always available for development and testing
-- User: engineering@arda.xyz (originator type with approved KYC)

-- ============================================================================
-- INSERT ENGINEERING TEST USER
-- ============================================================================

-- Insert the engineering test user with approved KYC status
INSERT INTO identities (
    id,
    user_type,
    email,
    kyc_status,
    kyc_verified_at,
    created_at,
    updated_at
) VALUES (
    '369783f3-81c0-429c-a66f-cefb1259cb3c'::UUID,
    'originator',
    'engineering@arda.xyz',
    'approved',
    NOW(),
    NOW(),
    NOW()
)
ON CONFLICT (email) DO UPDATE SET
    user_type = EXCLUDED.user_type,
    kyc_status = EXCLUDED.kyc_status,
    kyc_verified_at = CASE
        WHEN identities.kyc_status != 'approved' THEN EXCLUDED.kyc_verified_at
        ELSE identities.kyc_verified_at
    END,
    updated_at = NOW()
WHERE identities.email = 'engineering@arda.xyz';

-- ============================================================================
-- VERIFICATION QUERY
-- ============================================================================

-- You can verify this user exists with:
-- SELECT id, user_type, email, kyc_status, kyc_verified_at FROM identities WHERE email = 'engineering@arda.xyz';
