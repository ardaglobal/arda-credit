-- Create investor profiles table for enhanced investor account functionality
-- This migration adds investor-specific profile management and accredited investor tracking

-- ============================================================================
-- INVESTOR PROFILES TABLE
-- ============================================================================

-- Table for investor-specific profile information and regulatory status
CREATE TABLE investor_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE UNIQUE,
    accredited_investor BOOLEAN NOT NULL DEFAULT FALSE,
    investment_capacity DECIMAL(20, 6), -- optional maximum investment amount in USDC
    regulatory_jurisdiction VARCHAR(10) NOT NULL DEFAULT 'US', -- e.g., 'US', 'EU', 'UK'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INVESTOR PROFILE AUDIT TABLE
-- ============================================================================

-- Audit trail for all investor profile operations
CREATE TABLE investor_profile_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES investor_profiles(id) ON DELETE CASCADE,
    operation_type VARCHAR(30) NOT NULL CHECK (operation_type IN ('PROFILE_CREATED', 'PROFILE_UPDATED', 'ACCREDITATION_CHANGED', 'CAPACITY_UPDATED', 'JURISDICTION_CHANGED')),
    old_values JSONB,
    new_values JSONB,
    operator_id UUID REFERENCES identities(id),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- CONSTRAINTS AND VALIDATIONS
-- ============================================================================

-- Investment capacity must be positive if specified
ALTER TABLE investor_profiles ADD CONSTRAINT investor_profiles_positive_capacity 
    CHECK (investment_capacity IS NULL OR investment_capacity > 0);

-- Regulatory jurisdiction must be a valid code
ALTER TABLE investor_profiles ADD CONSTRAINT investor_profiles_valid_jurisdiction
    CHECK (regulatory_jurisdiction ~ '^[A-Z]{2,10}$');

-- Ensure user_id references an investor-type user (application layer will enforce)
-- PostgreSQL doesn't support subqueries in CHECK constraints, so this is handled in application

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Primary access patterns
CREATE INDEX idx_investor_profiles_user_id ON investor_profiles(user_id);
CREATE INDEX idx_investor_profiles_accredited ON investor_profiles(accredited_investor);
CREATE INDEX idx_investor_profiles_jurisdiction ON investor_profiles(regulatory_jurisdiction);
CREATE INDEX idx_investor_profiles_created_at ON investor_profiles(created_at);

-- Investment capacity filtering
CREATE INDEX idx_investor_profiles_capacity ON investor_profiles(investment_capacity) WHERE investment_capacity IS NOT NULL;

-- Composite index for accredited investors with capacity
CREATE INDEX idx_investor_profiles_accredited_capacity ON investor_profiles(accredited_investor, investment_capacity) 
    WHERE accredited_investor = true AND investment_capacity IS NOT NULL;

-- Audit table indexes
CREATE INDEX idx_investor_profile_audit_profile_id ON investor_profile_audit(profile_id);
CREATE INDEX idx_investor_profile_audit_operation_type ON investor_profile_audit(operation_type);
CREATE INDEX idx_investor_profile_audit_created_at ON investor_profile_audit(created_at);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Trigger for updating updated_at timestamp
CREATE TRIGGER update_investor_profiles_updated_at 
    BEFORE UPDATE ON investor_profiles 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Function to create audit records for profile operations
CREATE OR REPLACE FUNCTION create_investor_profile_audit_record()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO investor_profile_audit (profile_id, operation_type, new_values, operator_id)
        VALUES (NEW.id, 'PROFILE_CREATED', to_jsonb(NEW), NEW.user_id);
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        -- Check what changed and create appropriate audit record
        IF OLD.accredited_investor != NEW.accredited_investor THEN
            INSERT INTO investor_profile_audit (profile_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'ACCREDITATION_CHANGED', 
                    jsonb_build_object('accredited_investor', OLD.accredited_investor), 
                    jsonb_build_object('accredited_investor', NEW.accredited_investor), 
                    NEW.user_id);
        ELSIF OLD.investment_capacity != NEW.investment_capacity THEN
            INSERT INTO investor_profile_audit (profile_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'CAPACITY_UPDATED', 
                    jsonb_build_object('investment_capacity', OLD.investment_capacity), 
                    jsonb_build_object('investment_capacity', NEW.investment_capacity), 
                    NEW.user_id);
        ELSIF OLD.regulatory_jurisdiction != NEW.regulatory_jurisdiction THEN
            INSERT INTO investor_profile_audit (profile_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'JURISDICTION_CHANGED', 
                    jsonb_build_object('regulatory_jurisdiction', OLD.regulatory_jurisdiction), 
                    jsonb_build_object('regulatory_jurisdiction', NEW.regulatory_jurisdiction), 
                    NEW.user_id);
        ELSE
            INSERT INTO investor_profile_audit (profile_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'PROFILE_UPDATED', to_jsonb(OLD), to_jsonb(NEW), NEW.user_id);
        END IF;
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically create audit records
CREATE TRIGGER investor_profile_audit_trigger
    AFTER INSERT OR UPDATE ON investor_profiles
    FOR EACH ROW
    EXECUTE FUNCTION create_investor_profile_audit_record();

-- ============================================================================
-- HELPER FUNCTIONS FOR PROFILE MANAGEMENT
-- ============================================================================

-- Function to get investor profile with identity information
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

-- Function to validate investor profile creation
CREATE OR REPLACE FUNCTION validate_investor_profile_creation(
    profile_user_id UUID
)
RETURNS BOOLEAN AS $$
DECLARE
    user_type_val VARCHAR(20);
    user_exists BOOLEAN := FALSE;
BEGIN
    -- Check if user exists and is an investor
    SELECT i.user_type INTO user_type_val
    FROM identities i 
    WHERE i.id = profile_user_id 
        AND i.deleted_at IS NULL;
    
    IF FOUND THEN
        user_exists := TRUE;
    END IF;
    
    -- Return true if user exists and is an investor
    RETURN user_exists AND user_type_val = 'investor';
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INTEGRATION WITH EXISTING LIQUIDITY POOLS
-- ============================================================================

-- Update liquidity pools to reference investor profiles for enhanced validation
-- (The existing liquidity_pools table already references identities.id as investor_id)
-- Add index to improve joins between investor profiles and liquidity pools
CREATE INDEX idx_liquidity_pools_investor_profile_join ON liquidity_pools(investor_id);

-- Function to get investor pools with profile information
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
        ip.accredited_investor,
        ip.investment_capacity,
        ip.regulatory_jurisdiction
    FROM liquidity_pools lp
    JOIN investor_profiles ip ON lp.investor_id = ip.user_id
    WHERE ip.user_id = profile_user_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE investor_profiles IS 'Investor-specific profile information including accreditation status and investment capacity';
COMMENT ON TABLE investor_profile_audit IS 'Audit trail for all investor profile operations and changes';

COMMENT ON COLUMN investor_profiles.accredited_investor IS 'Whether the investor is accredited under regulatory definitions';
COMMENT ON COLUMN investor_profiles.investment_capacity IS 'Maximum investment amount in USDC (6 decimals)';
COMMENT ON COLUMN investor_profiles.regulatory_jurisdiction IS 'Regulatory jurisdiction (e.g., US, EU, UK)';