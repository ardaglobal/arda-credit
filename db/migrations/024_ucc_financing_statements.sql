-- ============================================================================
-- UCC-1 FINANCING STATEMENTS TABLE
-- ============================================================================
-- Stores UCC-1 financing statement filings for perfecting security interests

CREATE TABLE IF NOT EXISTS ucc_financing_statements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'UCC-2025-DE-001'
    security_agreement_id UUID REFERENCES security_agreements(id) ON DELETE CASCADE,
    credit_agreement_id UUID REFERENCES credit_agreements(id),
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash of filing receipt/PDF
    
    -- Filing Information
    file_number VARCHAR(100) UNIQUE NOT NULL, -- State-assigned filing number: '2025-1234567-89'
    filing_office VARCHAR(100) NOT NULL, -- 'Delaware SOS', 'New York DOS', etc.
    jurisdiction VARCHAR(50) NOT NULL, -- State/country code: 'DE', 'NY', 'CA'
    
    -- Filing Dates
    file_datetime TIMESTAMP WITH TIME ZONE NOT NULL,
    expiration_date DATE, -- Typically 5 years from filing
    lapse_date DATE,
    
    -- Filing Type
    filing_type VARCHAR(20) NOT NULL CHECK (
        filing_type IN ('initial', 'continuation', 'amendment', 'termination', 'assignment')
    ) DEFAULT 'initial',
    
    -- Related Filings (for amendments, continuations, etc.)
    original_file_number VARCHAR(100), -- Links to original UCC-1 if this is amendment/continuation
    amends_file_number VARCHAR(100), -- UCC-3 amendment reference
    
    -- Parties
    debtor_name VARCHAR(500) NOT NULL, -- Legal name of debtor (grantor)
    debtor_type VARCHAR(20) CHECK (
        debtor_type IN ('organization', 'individual')
    ) DEFAULT 'organization',
    debtor_jurisdiction VARCHAR(50), -- For organizations: state of organization
    debtor_organization_id VARCHAR(100), -- EIN or other org ID
    debtor_address TEXT,
    
    secured_party VARCHAR(500) NOT NULL, -- Secured party of record
    secured_party_address TEXT,
    
    -- Additional Debtors (for multiple debtors)
    additional_debtors JSONB DEFAULT '[]',
    -- Array of debtor objects: [{"name": "...", "type": "...", "address": "..."}]
    
    -- Additional Secured Parties
    additional_secured_parties JSONB DEFAULT '[]',
    
    -- Collateral Description
    collateral_description TEXT NOT NULL,
    collateral_desc_short VARCHAR(500), -- Short summary for indexing
    
    -- Collateral Indicators (for quick filtering)
    all_assets_indicator BOOLEAN DEFAULT FALSE,
    specific_collateral BOOLEAN DEFAULT TRUE,
    
    -- Filing Status
    filing_status VARCHAR(20) NOT NULL CHECK (
        filing_status IN ('active', 'lapsed', 'terminated', 'amended', 'assigned')
    ) DEFAULT 'active',
    
    -- Continuation & Termination Tracking
    continuation_filed BOOLEAN DEFAULT FALSE,
    continuation_file_number VARCHAR(100),
    continuation_date DATE,
    
    terminated BOOLEAN DEFAULT FALSE,
    termination_date DATE,
    termination_reason TEXT,
    
    -- Agricultural Lien / Special Provisions
    is_agricultural_lien BOOLEAN DEFAULT FALSE,
    is_fixture_filing BOOLEAN DEFAULT FALSE,
    is_manufactured_home BOOLEAN DEFAULT FALSE,
    real_property_description TEXT, -- If fixture filing
    
    -- Fee Information
    filing_fee DECIMAL(10, 2),
    
    -- Search/Verification
    search_verified BOOLEAN DEFAULT FALSE,
    search_verification_date DATE,
    search_results TEXT,
    
    -- Metadata
    notes TEXT,
    metadata JSONB DEFAULT '{}',
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    -- Audit
    created_by UUID,
    updated_by UUID
);

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_ucc_document_id ON ucc_financing_statements(document_id);
CREATE INDEX IF NOT EXISTS idx_ucc_file_number ON ucc_financing_statements(file_number);
CREATE INDEX IF NOT EXISTS idx_ucc_security_agreement_id ON ucc_financing_statements(security_agreement_id);
CREATE INDEX IF NOT EXISTS idx_ucc_debtor_name ON ucc_financing_statements(debtor_name);
CREATE INDEX IF NOT EXISTS idx_ucc_secured_party ON ucc_financing_statements(secured_party);
CREATE INDEX IF NOT EXISTS idx_ucc_jurisdiction ON ucc_financing_statements(jurisdiction);
CREATE INDEX IF NOT EXISTS idx_ucc_filing_office ON ucc_financing_statements(filing_office);
CREATE INDEX IF NOT EXISTS idx_ucc_file_datetime ON ucc_financing_statements(file_datetime);
CREATE INDEX IF NOT EXISTS idx_ucc_expiration_date ON ucc_financing_statements(expiration_date);
CREATE INDEX IF NOT EXISTS idx_ucc_filing_status ON ucc_financing_statements(filing_status);
CREATE INDEX IF NOT EXISTS idx_ucc_filing_type ON ucc_financing_statements(filing_type);
CREATE INDEX IF NOT EXISTS idx_ucc_created_at ON ucc_financing_statements(created_at);

-- Text search index for collateral description
CREATE INDEX IF NOT EXISTS idx_ucc_collateral_text ON ucc_financing_statements USING GIN (to_tsvector('english', collateral_description));

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_ucc_additional_debtors_gin ON ucc_financing_statements USING GIN (additional_debtors);
CREATE INDEX IF NOT EXISTS idx_ucc_metadata_gin ON ucc_financing_statements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_ucc_financing_statements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_ucc_financing_statements_updated_at
    BEFORE UPDATE ON ucc_financing_statements
    FOR EACH ROW
    EXECUTE FUNCTION update_ucc_financing_statements_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Check UCC Expiration Status
-- ============================================================================

CREATE OR REPLACE FUNCTION check_ucc_expiration_status(ucc_id UUID)
RETURNS TABLE (
    id UUID,
    file_number VARCHAR(100),
    days_until_expiration INTEGER,
    needs_continuation BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        u.id,
        u.file_number,
        EXTRACT(DAY FROM (u.expiration_date - CURRENT_DATE))::INTEGER as days_until_expiration,
        (u.expiration_date - CURRENT_DATE <= INTERVAL '6 months' 
         AND u.filing_status = 'active' 
         AND NOT u.continuation_filed) as needs_continuation
    FROM ucc_financing_statements u
    WHERE u.id = ucc_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE ucc_financing_statements IS 'UCC-1 financing statements filed to perfect security interests';
COMMENT ON COLUMN ucc_financing_statements.file_number IS 'State-assigned filing number for the UCC-1';
COMMENT ON COLUMN ucc_financing_statements.filing_office IS 'Office where UCC-1 was filed (e.g., Delaware SOS)';
COMMENT ON COLUMN ucc_financing_statements.expiration_date IS 'Date when filing expires (typically 5 years)';
COMMENT ON COLUMN ucc_financing_statements.collateral_description IS 'Full UCC collateral description';
COMMENT ON COLUMN ucc_financing_statements.all_assets_indicator IS 'True if filing covers all assets';
