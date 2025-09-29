-- Create invite codes table for user registration
-- This table stores invite codes that can be used during user onboarding

-- ============================================================================
-- CREATE INVITE_CODES TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS invite_codes (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) NOT NULL UNIQUE,
    created_by UUID, -- Optional: who created this invite code
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP WITH TIME ZONE, -- Optional: expiration date
    is_active BOOLEAN DEFAULT TRUE,
    used_by UUID, -- Track who used the code
    used_at TIMESTAMP WITH TIME ZONE, -- When it was used
    max_uses INTEGER DEFAULT 1, -- How many times this code can be used
    current_uses INTEGER DEFAULT 0, -- How many times it has been used
    description TEXT, -- Optional: description of what this invite is for
    
    -- Constraints
    CHECK (current_uses >= 0),
    CHECK (max_uses >= 1),
    CHECK (current_uses <= max_uses)
);

-- ============================================================================
-- CREATE INDEXES FOR PERFORMANCE
-- ============================================================================

-- Index for quick code lookup (main use case)
CREATE INDEX idx_invite_codes_code ON invite_codes(code);

-- Index for active and non-expired codes
CREATE INDEX idx_invite_codes_active_unexpired 
ON invite_codes(is_active, expires_at) 
WHERE is_active = TRUE;

-- Index for tracking usage
CREATE INDEX idx_invite_codes_usage ON invite_codes(current_uses, max_uses);

-- Index for created_by queries (if we want to see who created what)
CREATE INDEX idx_invite_codes_created_by ON invite_codes(created_by)
WHERE created_by IS NOT NULL;

-- ============================================================================
-- ADD HELPER FUNCTIONS
-- ============================================================================

-- Function to check if invite code is valid and available
CREATE OR REPLACE FUNCTION is_invite_code_valid(invite_code VARCHAR(64))
RETURNS BOOLEAN AS $$
DECLARE
    code_record RECORD;
BEGIN
    -- Get the invite code record
    SELECT * INTO code_record 
    FROM invite_codes 
    WHERE code = invite_code;
    
    -- Return false if code doesn't exist
    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;
    
    -- Check if code is active
    IF NOT code_record.is_active THEN
        RETURN FALSE;
    END IF;
    
    -- Check if code has expired
    IF code_record.expires_at IS NOT NULL AND code_record.expires_at < CURRENT_TIMESTAMP THEN
        RETURN FALSE;
    END IF;
    
    -- Check if code has reached max uses
    IF code_record.current_uses >= code_record.max_uses THEN
        RETURN FALSE;
    END IF;
    
    -- Code is valid
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to use an invite code (marks it as used and increments counter)
CREATE OR REPLACE FUNCTION use_invite_code(invite_code VARCHAR(64), user_id UUID DEFAULT NULL)
RETURNS BOOLEAN AS $$
DECLARE
    code_record RECORD;
BEGIN
    -- First check if code is valid
    IF NOT is_invite_code_valid(invite_code) THEN
        RETURN FALSE;
    END IF;
    
    -- Update the code usage
    UPDATE invite_codes 
    SET 
        current_uses = current_uses + 1,
        used_by = COALESCE(user_id, used_by), -- Only set if provided and not already set
        used_at = CASE 
            WHEN current_uses = 0 THEN CURRENT_TIMESTAMP 
            ELSE used_at 
        END
    WHERE code = invite_code;
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to create a new invite code
CREATE OR REPLACE FUNCTION create_invite_code(
    invite_code VARCHAR(64),
    creator_id UUID DEFAULT NULL,
    expiry_date TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    max_usage INTEGER DEFAULT 1,
    code_description TEXT DEFAULT NULL
)
RETURNS BOOLEAN AS $$
BEGIN
    INSERT INTO invite_codes (
        code, 
        created_by, 
        expires_at, 
        max_uses, 
        description
    ) VALUES (
        invite_code,
        creator_id,
        expiry_date,
        max_usage,
        code_description
    );
    
    RETURN TRUE;
EXCEPTION
    WHEN unique_violation THEN
        -- Code already exists
        RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INSERT SOME SAMPLE INVITE CODES FOR TESTING
-- ============================================================================

-- Insert a few test invite codes
INSERT INTO invite_codes (code, description, max_uses, expires_at) VALUES 
    ('WELCOME2024', 'General welcome code', 100, CURRENT_TIMESTAMP + INTERVAL '1 year'),
    ('BETA_ACCESS', 'Beta tester access', 50, CURRENT_TIMESTAMP + INTERVAL '6 months'),
    ('SINGLE_USE_TEST', 'Single use test code', 1, CURRENT_TIMESTAMP + INTERVAL '30 days'),
    ('EXPIRED_TEST', 'Already expired code for testing', 1, CURRENT_TIMESTAMP - INTERVAL '1 day');