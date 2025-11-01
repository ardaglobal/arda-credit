-- ============================================================================
-- LEGAL OPINIONS TABLE
-- ============================================================================
-- Stores legal opinions from counsel regarding enforceability and authority

CREATE TABLE IF NOT EXISTS legal_opinions (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'OPN-2025-001'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    
    -- Document Hashing (array for multiple opinion letters in a set)
    doc_hash_legal_opinion TEXT[] NOT NULL, -- Array of SHA-256 hashes: ['sha256:...', 'sha256:...']
    
    -- Opinion Firms
    opinion_firms TEXT[] NOT NULL, -- Array of issuing law firms: ['ABC LLP', 'XYZ Partners']
    lead_opinion_firm VARCHAR(500),
    
    -- Opinion Scopes
    scopes TEXT[] NOT NULL, -- Array of scope types
    -- Common scopes: 'Enforceability', 'Authorization', 'Corporate Authority', 
    --                'Perfection/Priority', 'No Conflicts', 'Binding Obligation'
    
    -- Governing Laws Covered
    governing_laws TEXT[] NOT NULL, -- Array of jurisdictions: ['NY', 'DE', 'DE UCC']
    primary_governing_law VARCHAR(50),
    
    -- Reliance & Beneficiaries
    reliance_language_present BOOLEAN NOT NULL DEFAULT FALSE,
    beneficiaries TEXT[], -- Parties who can rely on the opinion
    
    -- Opinion Details
    opinion_date DATE NOT NULL,
    effective_date DATE,
    limitations TEXT, -- Standard opinion limitations and qualifications
    assumptions TEXT, -- Standard assumptions made in opinion
    
    -- Deliverables
    full_opinion_delivered BOOLEAN NOT NULL DEFAULT FALSE,
    disclosure_schedules_attached BOOLEAN DEFAULT FALSE,
    
    -- Update & Bring-Down
    bring_down_opinion_id UUID REFERENCES legal_opinions(id), -- Links to bring-down/update opinion
    is_bring_down BOOLEAN DEFAULT FALSE,
    original_opinion_id UUID, -- Links back to original if this is a bring-down
    
    -- Coverage Assessment
    coverage_status VARCHAR(20) CHECK (
        coverage_status IN ('complete', 'partial', 'pending', 'waived')
    ) DEFAULT 'complete',
    gaps_or_qualifications TEXT,
    
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

CREATE INDEX IF NOT EXISTS idx_legal_opinion_document_id ON legal_opinions(document_id);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_ca_id ON legal_opinions(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_date ON legal_opinions(opinion_date);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_created_at ON legal_opinions(created_at);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_coverage_status ON legal_opinions(coverage_status);

-- Array indexes for efficient searches
CREATE INDEX IF NOT EXISTS idx_legal_opinion_firms_gin ON legal_opinions USING GIN (opinion_firms);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_scopes_gin ON legal_opinions USING GIN (scopes);
CREATE INDEX IF NOT EXISTS idx_legal_opinion_governing_laws_gin ON legal_opinions USING GIN (governing_laws);

-- JSONB index
CREATE INDEX IF NOT EXISTS idx_legal_opinion_metadata_gin ON legal_opinions USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_legal_opinions_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_legal_opinions_updated_at
    BEFORE UPDATE ON legal_opinions
    FOR EACH ROW
    EXECUTE FUNCTION update_legal_opinions_updated_at();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE legal_opinions IS 'Legal opinions from counsel covering enforceability, authority, and perfection';
COMMENT ON COLUMN legal_opinions.doc_hash_legal_opinion IS 'Array of SHA-256 hashes for all opinion letters in the set';
COMMENT ON COLUMN legal_opinions.scopes IS 'Array of opinion scopes (enforceability, authorization, perfection, etc.)';
COMMENT ON COLUMN legal_opinions.governing_laws IS 'Array of jurisdictions covered by the opinions';
COMMENT ON COLUMN legal_opinions.reliance_language_present IS 'Whether reliance/beneficiary language is included';
COMMENT ON COLUMN legal_opinions.is_bring_down IS 'Indicates if this is a bring-down or update opinion';
