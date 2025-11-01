-- ============================================================================
-- SECURITY AGREEMENTS TABLE
-- ============================================================================
-- Stores security/collateral agreements granting lender security interests

CREATE TABLE IF NOT EXISTS security_agreements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'SEC-2025-004'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Parties
    grantor VARCHAR(500) NOT NULL, -- Entity granting security interest
    grantor_jurisdiction VARCHAR(100),
    secured_party VARCHAR(500) NOT NULL, -- Secured party / Collateral Agent
    collateral_agent VARCHAR(500), -- If different from secured party
    
    -- Collateral Scope
    collateral_scope_summary TEXT NOT NULL,
    collateral_description TEXT, -- Detailed UCC description
    
    -- Collateral Categories (boolean flags for quick filtering)
    includes_accounts_receivable BOOLEAN DEFAULT FALSE,
    includes_inventory BOOLEAN DEFAULT FALSE,
    includes_equipment BOOLEAN DEFAULT FALSE,
    includes_intellectual_property BOOLEAN DEFAULT FALSE,
    includes_real_estate BOOLEAN DEFAULT FALSE,
    includes_deposit_accounts BOOLEAN DEFAULT FALSE,
    includes_investment_property BOOLEAN DEFAULT FALSE,
    includes_general_intangibles BOOLEAN DEFAULT FALSE,
    
    -- Collateral Details (JSONB for flexibility)
    collateral_details JSONB DEFAULT '{}',
    -- Example structure:
    -- {
    --   "accounts_receivable": {"estimated_value": 5000000, "aging": "..."},
    --   "equipment": {"categories": ["machinery", "vehicles"], "count": 25},
    --   "intellectual_property": {"patents": 12, "trademarks": 8}
    -- }
    
    -- Perfection Methods
    perfection_methods TEXT[] NOT NULL, -- Array: ['UCC-1', 'DACA', 'Possession', 'Control']
    perfection_status VARCHAR(20) CHECK (
        perfection_status IN ('perfected', 'pending', 'partial', 'not_required')
    ) DEFAULT 'pending',
    perfection_date DATE,
    
    -- Priority
    lien_priority VARCHAR(20) CHECK (
        lien_priority IN ('first_lien', 'second_lien', 'junior', 'pari_passu')
    ),
    intercreditor_agreement_id UUID, -- Link to intercreditor agreement if exists
    
    -- Jurisdictions & Governing Law
    governing_law VARCHAR(50) NOT NULL,
    filing_jurisdictions TEXT[], -- States/countries where UCC/liens filed
    
    -- Exclusions & Carve-Outs
    excluded_collateral TEXT,
    permitted_liens TEXT,
    
    -- Effective Dates
    effective_date DATE NOT NULL,
    termination_date DATE,
    
    -- Release Provisions
    release_conditions TEXT,
    partial_release_permitted BOOLEAN DEFAULT FALSE,
    
    -- Maintenance Obligations
    maintenance_covenants TEXT,
    insurance_requirements TEXT,
    
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

CREATE INDEX IF NOT EXISTS idx_security_agreement_document_id ON security_agreements(document_id);
CREATE INDEX IF NOT EXISTS idx_security_agreement_ca_id ON security_agreements(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_security_agreement_grantor ON security_agreements(grantor);
CREATE INDEX IF NOT EXISTS idx_security_agreement_secured_party ON security_agreements(secured_party);
CREATE INDEX IF NOT EXISTS idx_security_agreement_perfection_status ON security_agreements(perfection_status);
CREATE INDEX IF NOT EXISTS idx_security_agreement_lien_priority ON security_agreements(lien_priority);
CREATE INDEX IF NOT EXISTS idx_security_agreement_effective_date ON security_agreements(effective_date);
CREATE INDEX IF NOT EXISTS idx_security_agreement_created_at ON security_agreements(created_at);

-- Array indexes
CREATE INDEX IF NOT EXISTS idx_security_agreement_perfection_methods_gin ON security_agreements USING GIN (perfection_methods);
CREATE INDEX IF NOT EXISTS idx_security_agreement_filing_jurisdictions_gin ON security_agreements USING GIN (filing_jurisdictions);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_security_agreement_collateral_details_gin ON security_agreements USING GIN (collateral_details);
CREATE INDEX IF NOT EXISTS idx_security_agreement_metadata_gin ON security_agreements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_security_agreements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_security_agreements_updated_at
    BEFORE UPDATE ON security_agreements
    FOR EACH ROW
    EXECUTE FUNCTION update_security_agreements_updated_at();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE security_agreements IS 'Security agreements granting lenders security interests in collateral';
COMMENT ON COLUMN security_agreements.collateral_scope_summary IS 'High-level summary of collateral covered';
COMMENT ON COLUMN security_agreements.perfection_methods IS 'Array of methods used to perfect security interest';
COMMENT ON COLUMN security_agreements.lien_priority IS 'Priority level of this security interest';
COMMENT ON COLUMN security_agreements.collateral_details IS 'JSONB storage for detailed collateral information';
