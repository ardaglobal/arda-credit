-- Add user_type field to invite codes table
-- This allows invite codes to specify what type of user they're intended for

-- ============================================================================
-- ADD USER_TYPE FIELD TO INVITE_CODES TABLE
-- ============================================================================

-- Add user_type column to existing invite_codes table
ALTER TABLE invite_codes 
ADD COLUMN user_type VARCHAR(20) DEFAULT 'regular' NOT NULL;

-- Add constraint to ensure valid user types
ALTER TABLE invite_codes 
ADD CONSTRAINT chk_invite_codes_user_type 
CHECK (user_type IN ('admin', 'regular'));

-- Add index for user_type queries
CREATE INDEX idx_invite_codes_user_type ON invite_codes(user_type);

-- ============================================================================
-- UPDATE HELPER FUNCTIONS TO INCLUDE USER_TYPE
-- ============================================================================

-- Updated function to check if invite code is valid and available
-- Now also returns the user_type information
CREATE OR REPLACE FUNCTION get_invite_code_info(invite_code VARCHAR(64))
RETURNS TABLE (
    is_valid BOOLEAN,
    user_type VARCHAR(20),
    expires_at TIMESTAMP WITH TIME ZONE,
    description TEXT
) AS $$
DECLARE
    code_record RECORD;
    code_valid BOOLEAN := FALSE;
BEGIN
    -- Get the invite code record
    SELECT * INTO code_record 
    FROM invite_codes 
    WHERE code = invite_code;
    
    -- Return false if code doesn't exist
    IF NOT FOUND THEN
        RETURN QUERY SELECT FALSE, NULL::VARCHAR(20), NULL::TIMESTAMP WITH TIME ZONE, NULL::TEXT;
        RETURN;
    END IF;
    
    -- Check all validity conditions
    code_valid := TRUE;
    
    -- Check if code is active
    IF NOT code_record.is_active THEN
        code_valid := FALSE;
    END IF;
    
    -- Check if code has expired
    IF code_record.expires_at IS NOT NULL AND code_record.expires_at < CURRENT_TIMESTAMP THEN
        code_valid := FALSE;
    END IF;
    
    -- Check if code has reached max uses
    IF code_record.current_uses >= code_record.max_uses THEN
        code_valid := FALSE;
    END IF;
    
    -- Return validity status and code info
    RETURN QUERY SELECT 
        code_valid,
        code_record.user_type,
        code_record.expires_at,
        code_record.description;
END;
$$ LANGUAGE plpgsql;

-- Keep the original function for backwards compatibility
-- but update it to use the new logic
CREATE OR REPLACE FUNCTION is_invite_code_valid(invite_code VARCHAR(64))
RETURNS BOOLEAN AS $$
DECLARE
    result BOOLEAN;
BEGIN
    SELECT is_valid INTO result
    FROM get_invite_code_info(invite_code);
    
    RETURN COALESCE(result, FALSE);
END;
$$ LANGUAGE plpgsql;

-- Updated function to create invite codes with user_type
CREATE OR REPLACE FUNCTION create_invite_code(
    invite_code VARCHAR(64),
    creator_id UUID DEFAULT NULL,
    expiry_date TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    max_usage INTEGER DEFAULT 1,
    code_description TEXT DEFAULT NULL,
    invite_user_type VARCHAR(20) DEFAULT 'regular'
)
RETURNS BOOLEAN AS $$
BEGIN
    -- Validate user_type
    IF invite_user_type NOT IN ('admin', 'regular') THEN
        RAISE EXCEPTION 'Invalid user_type: %. Must be admin or regular', invite_user_type;
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
-- UPDATE EXISTING TEST DATA WITH USER TYPES
-- ============================================================================

-- Update existing invite codes to have appropriate user types
-- Set one of them as admin for testing
UPDATE invite_codes 
SET user_type = 'admin' 
WHERE code = 'BETA_ACCESS';

-- The rest remain as 'regular' due to the DEFAULT constraint

-- Insert a new admin-specific invite code for testing
INSERT INTO invite_codes (code, description, max_uses, expires_at, user_type) VALUES 
    ('ADMIN_INVITE_2024', 'Admin access invite code', 5, CURRENT_TIMESTAMP + INTERVAL '1 year', 'admin')
ON CONFLICT (code) DO NOTHING;

-- ============================================================================
-- ADD PERMANENT ADMIN INVITE CODE
-- ============================================================================

-- Insert the ARDAADMIN25 admin invite code
INSERT INTO invite_codes (
    code, 
    description, 
    max_uses, 
    expires_at, 
    user_type
) VALUES (
    'ARDAADMIN25', 
    'Admin access invite code - expires 12/31/2025', 
    100, -- Allow multiple admin registrations
    '2025-12-31 23:59:00'::TIMESTAMP WITH TIME ZONE,
    'admin'
)
ON CONFLICT (code) DO NOTHING;
