-- Create merchant registration and profile tables
-- Enables merchants to onboard, create profiles, and request credit lines

-- ============================================================================
-- UPDATE USER_TYPE CONSTRAINT TO INCLUDE MERCHANT
-- ============================================================================

-- Drop the existing constraint
ALTER TABLE identities DROP CONSTRAINT IF EXISTS identities_user_type_check;

-- Add the updated constraint with merchant support
ALTER TABLE identities ADD CONSTRAINT identities_user_type_check 
    CHECK (user_type IN ('user', 'investor', 'lender', 'originator', 'admin', 'merchant'));

-- ============================================================================
-- CREATE MERCHANTS TABLE
-- ============================================================================

CREATE TABLE merchants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) UNIQUE,
    legal_name VARCHAR(255) NOT NULL,
    dba VARCHAR(255), -- Doing Business As name
    tax_id VARCHAR(50) UNIQUE,
    mcc VARCHAR(10), -- Merchant Category Code
    country VARCHAR(3) NOT NULL,
    kyc_status VARCHAR(20) DEFAULT 'NOT_SUBMITTED',
    business_type VARCHAR(50), -- LLC, CORP, SOLE_PROP, etc.
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance
CREATE INDEX idx_merchants_user_id ON merchants(user_id);
CREATE INDEX idx_merchants_tax_id ON merchants(tax_id);
CREATE INDEX idx_merchants_kyc_status ON merchants(kyc_status);
CREATE INDEX idx_merchants_country ON merchants(country);
CREATE INDEX idx_merchants_mcc ON merchants(mcc);

-- ============================================================================
-- CREATE MERCHANT PROFILES TABLE  
-- ============================================================================

CREATE TABLE merchant_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) UNIQUE,
    industry VARCHAR(100),
    years_in_business INTEGER,
    employee_count INTEGER,
    monthly_revenue_estimate DECIMAL(20,6),
    business_description TEXT,
    profile_completed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance  
CREATE INDEX idx_merchant_profiles_merchant_id ON merchant_profiles(merchant_id);
CREATE INDEX idx_merchant_profiles_industry ON merchant_profiles(industry);
CREATE INDEX idx_merchant_profiles_profile_completed ON merchant_profiles(profile_completed);

-- ============================================================================
-- ADD CONSTRAINTS
-- ============================================================================

-- Ensure valid KYC status values
ALTER TABLE merchants ADD CONSTRAINT merchants_kyc_status_check 
    CHECK (kyc_status IN ('NOT_SUBMITTED', 'PENDING', 'APPROVED', 'REJECTED'));

-- Ensure valid business types
ALTER TABLE merchants ADD CONSTRAINT merchants_business_type_check
    CHECK (business_type IN ('LLC', 'CORP', 'SOLE_PROP', 'PARTNERSHIP', 'OTHER'));

-- Ensure positive values for business metrics
ALTER TABLE merchant_profiles ADD CONSTRAINT merchant_profiles_years_in_business_check
    CHECK (years_in_business >= 0);

ALTER TABLE merchant_profiles ADD CONSTRAINT merchant_profiles_employee_count_check
    CHECK (employee_count >= 0);

ALTER TABLE merchant_profiles ADD CONSTRAINT merchant_profiles_monthly_revenue_check
    CHECK (monthly_revenue_estimate >= 0);

-- ============================================================================
-- CREATE TRIGGER FOR UPDATED_AT TIMESTAMPS
-- ============================================================================

-- Function to update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for merchants table
CREATE TRIGGER update_merchants_updated_at 
    BEFORE UPDATE ON merchants 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Triggers for merchant_profiles table
CREATE TRIGGER update_merchant_profiles_updated_at 
    BEFORE UPDATE ON merchant_profiles 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- VERIFICATION QUERIES (for manual testing)
-- ============================================================================

-- Test merchant registration
-- INSERT INTO identities (user_type, email) VALUES ('merchant', 'test_merchant@example.com');
-- INSERT INTO merchants (user_id, legal_name, tax_id, mcc, country, business_type) 
-- VALUES (
--     (SELECT id FROM identities WHERE email = 'test_merchant@example.com'),
--     'Test Merchant LLC', 
--     '123456789', 
--     '5999', 
--     'USA', 
--     'LLC'
-- );

-- Test profile creation  
-- INSERT INTO merchant_profiles (merchant_id, industry, years_in_business, employee_count)
-- VALUES (
--     (SELECT id FROM merchants WHERE legal_name = 'Test Merchant LLC'),
--     'E-commerce',
--     5,
--     10
-- );

-- Clean up test data:
-- DELETE FROM merchant_profiles WHERE merchant_id IN (SELECT id FROM merchants WHERE legal_name = 'Test Merchant LLC');
-- DELETE FROM merchants WHERE legal_name = 'Test Merchant LLC';
-- DELETE FROM identities WHERE email = 'test_merchant@example.com';