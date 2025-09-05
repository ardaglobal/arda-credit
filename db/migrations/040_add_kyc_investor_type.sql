-- Add KYC_INVESTOR_TYPE enum and column to identities table
-- This migration adds investor type classification for regulatory compliance

-- ============================================================================
-- KYC INVESTOR TYPE ENUM
-- ============================================================================

-- Create enum type for KYC investor classifications
CREATE TYPE kyc_investor_type AS ENUM (
    'QIB',  -- Qualified Institutional Buyer
    'QP',   -- Qualified Purchaser  
    'AI'    -- Accredited Investor
);

-- ============================================================================
-- UPDATE IDENTITIES TABLE
-- ============================================================================

-- Add kyc_investor_type column to identities table
ALTER TABLE identities 
ADD COLUMN kyc_investor_type kyc_investor_type;

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Index for kyc_investor_type filtering
CREATE INDEX idx_identities_kyc_investor_type ON identities(kyc_investor_type);

-- Composite index for investor users with KYC status and investor type
CREATE INDEX idx_identities_investor_kyc_status_type ON identities(user_type, kyc_status, kyc_investor_type) 
    WHERE user_type = 'investor' AND kyc_status = 'approved' AND kyc_investor_type IS NOT NULL;

-- ============================================================================
-- DROP AND RECREATE FUNCTIONS (to handle return type changes)
-- ============================================================================

-- Drop existing functions that will have return type changes
DROP FUNCTION IF EXISTS get_investor_profile_with_identity(UUID);
DROP FUNCTION IF EXISTS get_investor_pools_with_profile(UUID);

-- ============================================================================
-- UPDATE FUNCTIONS AND CONSTRAINTS
-- ============================================================================

-- Update investor profile validation to consider KYC investor type
CREATE OR REPLACE FUNCTION validate_investor_profile_creation(
    profile_user_id UUID
)
RETURNS BOOLEAN AS $$
DECLARE
    user_type_val VARCHAR(20);
    user_exists BOOLEAN := FALSE;
    kyc_status_val VARCHAR(50);
    kyc_investor_type_val kyc_investor_type;
BEGIN
    -- Check if user exists, is an investor, and has appropriate KYC status
    SELECT i.user_type, i.kyc_status, i.kyc_investor_type 
    INTO user_type_val, kyc_status_val, kyc_investor_type_val
    FROM identities i 
    WHERE i.id = profile_user_id 
        AND i.deleted_at IS NULL;
    
    IF FOUND THEN
        user_exists := TRUE;
    END IF;
    
    -- Return true if user exists, is an investor, and has KYC approved with investor type
    RETURN user_exists 
        AND user_type_val = 'investor' 
        AND kyc_status_val = 'approved'
        AND kyc_investor_type_val IS NOT NULL;
END;
$$ LANGUAGE plpgsql;

-- Update investor profile function to include KYC investor type
CREATE OR REPLACE FUNCTION get_investor_profile_with_identity(
    profile_user_id UUID
)
RETURNS TABLE(
    profile_id UUID,
    user_id UUID,
    user_email VARCHAR(255),
    user_type VARCHAR(20),
    kyc_status VARCHAR(50),
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    kyc_investor_type kyc_investor_type,
    accredited_investor BOOLEAN,
    investment_capacity DECIMAL(20, 6),
    regulatory_jurisdiction VARCHAR(10),
    profile_created_at TIMESTAMP WITH TIME ZONE,
    profile_updated_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ip.id,
        ip.user_id,
        i.email,
        i.user_type,
        i.kyc_status,
        i.kyc_verified_at,
        i.kyc_investor_type,
        ip.accredited_investor,
        ip.investment_capacity,
        ip.regulatory_jurisdiction,
        ip.created_at,
        ip.updated_at
    FROM investor_profiles ip
    JOIN identities i ON ip.user_id = i.id
    WHERE ip.user_id = profile_user_id
        AND i.deleted_at IS NULL;
END;
$$ LANGUAGE plpgsql;

-- Function to get investors by KYC investor type
CREATE OR REPLACE FUNCTION get_investors_by_type(
    investor_type kyc_investor_type
)
RETURNS TABLE(
    user_id UUID,
    email VARCHAR(255),
    user_type VARCHAR(20),
    kyc_status VARCHAR(50),
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    kyc_investor_type kyc_investor_type,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        i.id,
        i.email,
        i.user_type,
        i.kyc_status,
        i.kyc_verified_at,
        i.kyc_investor_type,
        i.created_at
    FROM identities i
    WHERE i.user_type = 'investor'
        AND i.kyc_status = 'approved'
        AND i.kyc_investor_type = investor_type
        AND i.deleted_at IS NULL
    ORDER BY i.kyc_verified_at DESC;
END;
$$ LANGUAGE plpgsql;

-- Function to validate KYC investor type assignment
CREATE OR REPLACE FUNCTION validate_kyc_investor_type_assignment(
    user_id_val UUID,
    investor_type_val kyc_investor_type,
    current_kyc_status VARCHAR(50)
)
RETURNS BOOLEAN AS $$
DECLARE
    user_type_val VARCHAR(20);
BEGIN
    -- Get user type
    SELECT i.user_type INTO user_type_val
    FROM identities i 
    WHERE i.id = user_id_val;
    
    -- Only investors can have KYC investor type
    IF user_type_val != 'investor' THEN
        RETURN FALSE;
    END IF;
    
    -- KYC investor type can only be set when KYC is approved
    IF current_kyc_status != 'approved' AND investor_type_val IS NOT NULL THEN
        RETURN FALSE;
    END IF;
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- UPDATE INVESTOR POOLS FUNCTION
-- ============================================================================

-- Update investor pools function to include KYC investor type
CREATE OR REPLACE FUNCTION get_investor_pools_with_profile(
    profile_user_id UUID
)
RETURNS TABLE(
    pool_id UUID,
    pool_name VARCHAR(255),
    total_deposits DECIMAL(20, 6),
    available_liquidity DECIMAL(20, 6),
    utilized_liquidity DECIMAL(20, 6),
    max_ltv_ratio DECIMAL(5, 4),
    max_interest_rate DECIMAL(7, 4),
    repayment_period_days INTEGER,
    pool_status VARCHAR(20),
    pool_created_at TIMESTAMP WITH TIME ZONE,
    pool_updated_at TIMESTAMP WITH TIME ZONE,
    kyc_investor_type kyc_investor_type,
    accredited_investor BOOLEAN,
    investment_capacity DECIMAL(20, 6),
    regulatory_jurisdiction VARCHAR(10)
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lp.id,
        lp.name,
        lp.total_deposits,
        lp.available_liquidity,
        lp.utilized_liquidity,
        lp.max_ltv_ratio,
        lp.max_interest_rate,
        lp.repayment_period_days,
        lp.status,
        lp.created_at,
        lp.updated_at,
        i.kyc_investor_type,
        ip.accredited_investor,
        ip.investment_capacity,
        ip.regulatory_jurisdiction
    FROM liquidity_pools lp
    JOIN investor_profiles ip ON lp.investor_id = ip.user_id
    JOIN identities i ON ip.user_id = i.id
    WHERE ip.user_id = profile_user_id
        AND i.deleted_at IS NULL;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TYPE kyc_investor_type IS 'KYC investor type classifications for regulatory compliance: QIB (Qualified Institutional Buyer), QP (Qualified Purchaser), AI (Accredited Investor)';
COMMENT ON COLUMN identities.kyc_investor_type IS 'KYC investor type classification (QIB, QP, AI) - required for approved investor accounts';