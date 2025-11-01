-- ============================================================================
-- FEE LETTER TABLE
-- ============================================================================
-- Stores fee letters with fee components (kept confidential from main CA)

CREATE TABLE IF NOT EXISTS fee_letters (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'FEE-2025-001'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    
    -- Document Hashing
    doc_hash_fee_letter VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Fee Components (stored as JSONB for flexibility and confidentiality)
    fee_components JSONB NOT NULL DEFAULT '{}',
    -- Example structure:
    -- {
    --   "upfront": 1.0,          // % of facility
    --   "oid": 2.0,              // Original Issue Discount %
    --   "unused": 0.25,          // Unused commitment fee %
    --   "agency_fee": 50000,     // Annual agency fee
    --   "amendment_fee": 25000,  // Per amendment
    --   "other_fees": []
    -- }
    
    -- Confidentiality
    confidentiality_flag BOOLEAN NOT NULL DEFAULT TRUE,
    
    -- Fee Schedules (more detailed breakdowns)
    upfront_fee_pct DECIMAL(5, 4), -- % of committed amount
    original_issue_discount_pct DECIMAL(5, 4), -- OID %
    unused_commitment_fee_bps INTEGER, -- Unused fee in bps
    agency_fee_amount DECIMAL(15, 2), -- Annual agency fee
    amendment_fee_amount DECIMAL(15, 2), -- Per amendment
    
    -- Payment Terms
    fee_payment_schedule TEXT,
    fee_calculation_basis TEXT,
    
    -- Effective Dates
    effective_date DATE,
    expiry_date DATE,
    
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

CREATE INDEX IF NOT EXISTS idx_fee_letter_document_id ON fee_letters(document_id);
CREATE INDEX IF NOT EXISTS idx_fee_letter_ca_id ON fee_letters(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_fee_letter_created_at ON fee_letters(created_at);
CREATE INDEX IF NOT EXISTS idx_fee_letter_effective_date ON fee_letters(effective_date);

-- GIN index for JSONB
CREATE INDEX IF NOT EXISTS idx_fee_letter_components_gin ON fee_letters USING GIN (fee_components);
CREATE INDEX IF NOT EXISTS idx_fee_letter_metadata_gin ON fee_letters USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_fee_letters_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_fee_letters_updated_at
    BEFORE UPDATE ON fee_letters
    FOR EACH ROW
    EXECUTE FUNCTION update_fee_letters_updated_at();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE fee_letters IS 'Confidential fee letters with detailed fee structures not disclosed in credit agreements';
COMMENT ON COLUMN fee_letters.fee_components IS 'JSONB structure containing all fee types and amounts';
COMMENT ON COLUMN fee_letters.confidentiality_flag IS 'Marks this fee letter as confidential (default true)';
COMMENT ON COLUMN fee_letters.doc_hash_fee_letter IS 'SHA-256 hash of executed fee letter PDF';
