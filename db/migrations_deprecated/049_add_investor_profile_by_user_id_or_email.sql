-- ============================================================================
-- Add function to get investor profile by user_id OR email
-- ============================================================================

-- Create a new function that accepts either user_id or email for getting investor profile
CREATE OR REPLACE FUNCTION get_investor_profile_by_user_id_or_email(
    profile_user_id UUID DEFAULT NULL,
    lookup_email VARCHAR(255) DEFAULT NULL
)
RETURNS TABLE(
    profile_id UUID,
    user_id UUID,
    user_email VARCHAR(255),
    user_type VARCHAR(20),
    kyc_status VARCHAR(50),
    kyc_submitted_at TIMESTAMP WITH TIME ZONE,
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    kyc_investor_type kyc_investor_type,
    accredited_investor BOOLEAN,
    investment_capacity DECIMAL(20, 6),
    regulatory_jurisdiction VARCHAR(10),
    profile_created_at TIMESTAMP WITH TIME ZONE,
    profile_updated_at TIMESTAMP WITH TIME ZONE,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    jurisdiction VARCHAR(10),
    wallet_address VARCHAR(42)
) AS $$
BEGIN
    -- Validate input: exactly one of user_id or email must be provided
    IF (profile_user_id IS NULL AND lookup_email IS NULL) OR (profile_user_id IS NOT NULL AND lookup_email IS NOT NULL) THEN
        RAISE EXCEPTION 'Exactly one of user_id or email must be provided';
    END IF;

    RETURN QUERY
    SELECT 
        ip.id,
        ip.user_id,
        i.email,
        i.user_type,
        i.kyc_status,
        i.kyc_submitted_at,
        i.kyc_verified_at,
        i.kyc_investor_type,
        ip.accredited_investor,
        ip.investment_capacity,
        ip.regulatory_jurisdiction,
        ip.created_at,
        ip.updated_at,
        i.first_name,
        i.last_name,
        i.jurisdiction,
        i.wallet_address
    FROM investor_profiles ip
    INNER JOIN identities i ON ip.user_id = i.id
    WHERE (profile_user_id IS NOT NULL AND i.id = profile_user_id)
       OR (lookup_email IS NOT NULL AND LOWER(i.email) = LOWER(lookup_email));
END;
$$ LANGUAGE plpgsql;