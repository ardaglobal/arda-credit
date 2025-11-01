-- ============================================================================
-- CREDIT AGREEMENT TABLE
-- ============================================================================
-- Stores executed credit agreements with full document attributes

CREATE TABLE IF NOT EXISTS credit_agreements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'CA-2025-001'
    version VARCHAR(50) NOT NULL DEFAULT '1.0',
    execution_status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK (
        execution_status IN ('draft', 'executed', 'amended', 'superseded')
    ),
    
    -- Document Hashing
    executed_pdf_hash VARCHAR(66) NOT NULL, -- SHA-256 hash: 'sha256:...'
    bundle_hash VARCHAR(66), -- Hash of evidence bundle
    
    -- Dates
    effective_date DATE NOT NULL,
    maturity_date DATE NOT NULL,
    closing_date DATE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    
    -- Governing Law
    governing_law VARCHAR(50) NOT NULL DEFAULT 'New York' CHECK (
        governing_law IN ('New York', 'Delaware', 'England', 'Other')
    ),
    
    -- Parties & Roles
    borrower_legal_name VARCHAR(500) NOT NULL,
    borrower_external_id VARCHAR(255),
    agent_or_lender VARCHAR(500) NOT NULL,
    collateral_agent VARCHAR(500),
    guarantors_present BOOLEAN NOT NULL DEFAULT FALSE,
    guarantor_names TEXT[], -- Array of guarantor legal names
    
    -- Facility Structure & Economics
    facility_type VARCHAR(50) NOT NULL CHECK (
        facility_type IN ('Term Loan', 'Revolver', 'Delayed Draw', 'Bridge', 'Unitranche', 'Other')
    ),
    committed_amount DECIMAL(20, 2) NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD' CHECK (
        currency IN ('USD', 'EUR', 'GBP', 'JPY', 'Other')
    ),
    draw_rules TEXT,
    
    -- Repayment Structure
    repayment_type VARCHAR(20) NOT NULL CHECK (
        repayment_type IN ('Bullet', 'Amortizing', 'Custom')
    ),
    amortization_schedule JSONB, -- Stores amortization schedule details
    
    -- Pricing
    reference_rate VARCHAR(50) NOT NULL CHECK (
        reference_rate IN ('SOFR', 'Term SOFR', 'Base Rate', 'LIBOR', 'EURIBOR', 'Other')
    ),
    margin_bps INTEGER NOT NULL, -- Spread over reference rate in basis points
    floor_bps INTEGER DEFAULT 0, -- Rate floor in basis points
    
    -- Security & Collateral
    secured_flag BOOLEAN NOT NULL DEFAULT FALSE,
    collateral_scope_summary TEXT,
    
    -- Financial Covenants (stored as JSONB for flexibility)
    financial_covenants JSONB DEFAULT '{}',
    
    -- Use of Proceeds
    use_of_proceeds TEXT,
    
    -- Additional Terms
    accordion_provision TEXT,
    prepayment_terms TEXT,
    default_provisions TEXT,
    
    -- State Management
    deal_state VARCHAR(50) NOT NULL DEFAULT 'draft' CHECK (
        deal_state IN (
            'draft', 'under_review', 'negotiation', 'finalized', 
            'executed', 'cp_complete', 'pending_funding', 'active', 
            'amendment', 'suspended', 'matured', 'defaulted'
        )
    ),
    
    -- Metadata
    metadata JSONB DEFAULT '{}',
    
    -- Audit Trail
    created_by UUID,
    updated_by UUID
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_ca_document_id ON credit_agreements(document_id);
CREATE INDEX IF NOT EXISTS idx_ca_borrower ON credit_agreements(borrower_legal_name);
CREATE INDEX IF NOT EXISTS idx_ca_effective_date ON credit_agreements(effective_date);
CREATE INDEX IF NOT EXISTS idx_ca_maturity_date ON credit_agreements(maturity_date);
CREATE INDEX IF NOT EXISTS idx_ca_deal_state ON credit_agreements(deal_state);
CREATE INDEX IF NOT EXISTS idx_ca_execution_status ON credit_agreements(execution_status);
CREATE INDEX IF NOT EXISTS idx_ca_facility_type ON credit_agreements(facility_type);
CREATE INDEX IF NOT EXISTS idx_ca_created_at ON credit_agreements(created_at);
CREATE INDEX IF NOT EXISTS idx_ca_updated_at ON credit_agreements(updated_at);

-- GIN index for JSONB fields
CREATE INDEX IF NOT EXISTS idx_ca_financial_covenants_gin ON credit_agreements USING GIN (financial_covenants);
CREATE INDEX IF NOT EXISTS idx_ca_metadata_gin ON credit_agreements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_credit_agreements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_credit_agreements_updated_at
    BEFORE UPDATE ON credit_agreements
    FOR EACH ROW
    EXECUTE FUNCTION update_credit_agreements_updated_at();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE credit_agreements IS 'Stores executed credit agreements with full document attributes from the MVP';
COMMENT ON COLUMN credit_agreements.document_id IS 'Unique document identifier (e.g., CA-2025-001)';
COMMENT ON COLUMN credit_agreements.executed_pdf_hash IS 'SHA-256 hash of the executed PDF for tamper evidence';
COMMENT ON COLUMN credit_agreements.bundle_hash IS 'Hash of the complete evidence bundle (CA + ancillaries)';
COMMENT ON COLUMN credit_agreements.financial_covenants IS 'JSONB storage for flexible covenant structures';
COMMENT ON COLUMN credit_agreements.metadata IS 'Additional metadata and custom fields';
COMMENT ON COLUMN credit_agreements.deal_state IS 'Current lifecycle state of the credit agreement';
