-- ============================================================================
-- COMPANIES TABLE
-- Company/organization management with type field for investor/originator
-- ============================================================================

-- Table for company/organization management
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    type VARCHAR(20) NOT NULL CHECK (type IN ('originator', 'investor')),
    kyb_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted' CHECK (kyb_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    kyb_submitted_at TIMESTAMP WITH TIME ZONE,
    kyb_verified_at TIMESTAMP WITH TIME ZONE,

    -- Company contact and legal information
    legal_name VARCHAR(500),
    tax_id VARCHAR(50),
    registration_number VARCHAR(100),
    jurisdiction VARCHAR(100),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state_province VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(100),

    -- Document storage configuration
    s3_bucket_prefix VARCHAR(100),
    encryption_key_id VARCHAR(255),
    document_retention_days INTEGER DEFAULT 2555, -- ~7 years default retention

    -- Audit and compliance
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(name, type) -- Prevent duplicate company names within same type
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

CREATE INDEX idx_companies_type ON companies(type);
CREATE INDEX idx_companies_kyb_status ON companies(kyb_status);
CREATE INDEX idx_companies_created_at ON companies(created_at);
CREATE INDEX idx_companies_name_type ON companies(name, type);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC TIMESTAMP UPDATES
-- ============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at
CREATE TRIGGER update_companies_updated_at BEFORE UPDATE ON companies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE companies IS 'Company/organization management with KYB compliance and document storage configuration';
COMMENT ON COLUMN companies.type IS 'Company type: originator (creates deals), investor (provides capital)';
COMMENT ON COLUMN companies.kyb_status IS 'Know Your Business verification status: not_submitted, pending, approved, rejected';
COMMENT ON COLUMN companies.s3_bucket_prefix IS 'Company-specific S3 bucket prefix for document organization';
COMMENT ON COLUMN companies.encryption_key_id IS 'AWS KMS key ID for company-specific document encryption';