-- ============================================================================
-- GUARANTIES TABLE
-- ============================================================================
-- Stores guaranty agreements where third parties guarantee borrower obligations

CREATE TABLE IF NOT EXISTS guaranties (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'GUA-2025-003'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Guarantor Information
    guarantor_name VARCHAR(500) NOT NULL, -- Legal name of guarantor
    guarantor_type VARCHAR(20) CHECK (
        guarantor_type IN ('corporate', 'individual', 'parent', 'subsidiary', 'affiliate', 'sponsor')
    ),
    guarantor_jurisdiction VARCHAR(100),
    guarantor_relationship VARCHAR(100), -- Relationship to borrower: 'Parent', 'Subsidiary', 'Affiliate', 'Sponsor'
    
    -- Multiple Guarantors (if part of group guaranty)
    is_joint_guaranty BOOLEAN DEFAULT FALSE,
    other_guarantors TEXT[], -- Array of other guarantor names if joint
    
    -- Beneficiary
    beneficiary VARCHAR(500) NOT NULL, -- Lender/Agent benefiting from guaranty
    
    -- Guarantee Type & Scope
    guarantee_type VARCHAR(30) NOT NULL CHECK (
        guarantee_type IN (
            'full_unconditional',
            'limited',
            'capped',
            'payment',
            'collection',
            'performance',
            'completion'
        )
    ),
    
    -- Coverage Scope
    coverage_scope TEXT NOT NULL, -- Summary of obligations covered
    covers_principal BOOLEAN NOT NULL DEFAULT TRUE,
    covers_interest BOOLEAN NOT NULL DEFAULT TRUE,
    covers_fees BOOLEAN NOT NULL DEFAULT TRUE,
    covers_expenses BOOLEAN NOT NULL DEFAULT TRUE,
    covers_amendments BOOLEAN DEFAULT TRUE, -- Covers future amendments to credit agreement
    
    -- Cap/Limitation (if applicable)
    is_capped BOOLEAN DEFAULT FALSE,
    cap_amount DECIMAL(20, 2), -- Maximum guaranteed amount
    cap_currency VARCHAR(3) DEFAULT 'USD',
    cap_type VARCHAR(20) CHECK (
        cap_type IN ('absolute', 'per_guarantor', 'aggregate', 'formula_based')
    ),
    cap_formula TEXT, -- If formula-based cap
    
    -- Payment/Collection Type
    is_payment_guaranty BOOLEAN DEFAULT TRUE, -- Pay on demand vs. collection only
    is_collection_guaranty BOOLEAN DEFAULT FALSE,
    
    -- Continuing Guaranty
    is_continuing BOOLEAN DEFAULT TRUE, -- Covers future advances/obligations
    revolving_guaranty BOOLEAN DEFAULT FALSE,
    
    -- Defenses & Waivers
    waivers_of_defenses TEXT,
    suretyship_defenses_waived BOOLEAN DEFAULT TRUE,
    notice_of_default_waived BOOLEAN DEFAULT FALSE,
    notice_of_acceptance_waived BOOLEAN DEFAULT TRUE,
    
    -- Subrogation & Subordination
    subrogation_rights TEXT,
    subrogation_subordinated BOOLEAN DEFAULT TRUE, -- Subordinated until obligations paid
    subordination_provisions TEXT,
    
    -- Security (if guaranteed obligations are secured by guarantor's assets)
    secured_guaranty BOOLEAN DEFAULT FALSE,
    guarantor_security_description TEXT,
    
    -- Financial Covenants (if guarantor has separate covenants)
    guarantor_covenants JSONB DEFAULT '{}',
    
    -- Release Provisions
    release_conditions TEXT,
    automatic_release_trigger TEXT,
    partial_release_permitted BOOLEAN DEFAULT FALSE,
    
    -- Effectiveness & Term
    effective_date DATE NOT NULL,
    termination_date DATE,
    
    -- Governing Law
    governing_law VARCHAR(50) NOT NULL,
    
    -- Enforceability
    enforceability_opinion_received BOOLEAN DEFAULT FALSE,
    opinion_reference VARCHAR(255), -- Link to legal opinion
    
    -- Corporate Authority
    corporate_authority_documents JSONB DEFAULT '{}',
    -- Contains: board resolutions, officer certificates, good standings, etc.
    -- {
    --   "board_resolution": true,
    --   "secretary_certificate": true,
    --   "good_standing": true,
    --   "authorization_date": "2025-11-01"
    -- }
    
    -- Guaranty Status
    guaranty_status VARCHAR(20) NOT NULL CHECK (
        guaranty_status IN ('pending', 'effective', 'terminated', 'released', 'defaulted')
    ) DEFAULT 'pending',
    
    -- Claims & Defaults
    claims_filed BOOLEAN DEFAULT FALSE,
    amount_claimed DECIMAL(20, 2),
    amount_recovered DECIMAL(20, 2),
    
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

CREATE INDEX IF NOT EXISTS idx_guaranty_document_id ON guaranties(document_id);
CREATE INDEX IF NOT EXISTS idx_guaranty_ca_id ON guaranties(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_guaranty_guarantor_name ON guaranties(guarantor_name);
CREATE INDEX IF NOT EXISTS idx_guaranty_guarantor_type ON guaranties(guarantor_type);
CREATE INDEX IF NOT EXISTS idx_guaranty_guarantee_type ON guaranties(guarantee_type);
CREATE INDEX IF NOT EXISTS idx_guaranty_status ON guaranties(guaranty_status);
CREATE INDEX IF NOT EXISTS idx_guaranty_effective_date ON guaranties(effective_date);
CREATE INDEX IF NOT EXISTS idx_guaranty_is_capped ON guaranties(is_capped);
CREATE INDEX IF NOT EXISTS idx_guaranty_created_at ON guaranties(created_at);

-- Array index
CREATE INDEX IF NOT EXISTS idx_guaranty_other_guarantors_gin ON guaranties USING GIN (other_guarantors);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_guaranty_covenants_gin ON guaranties USING GIN (guarantor_covenants);
CREATE INDEX IF NOT EXISTS idx_guaranty_corporate_authority_gin ON guaranties USING GIN (corporate_authority_documents);
CREATE INDEX IF NOT EXISTS idx_guaranty_metadata_gin ON guaranties USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_guaranties_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_guaranties_updated_at
    BEFORE UPDATE ON guaranties
    FOR EACH ROW
    EXECUTE FUNCTION update_guaranties_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Check Guaranty Cap Utilization
-- ============================================================================

CREATE OR REPLACE FUNCTION check_guaranty_cap_utilization(guaranty_id UUID)
RETURNS TABLE (
    id UUID,
    guarantor_name VARCHAR(500),
    cap_amount DECIMAL(20, 2),
    amount_claimed DECIMAL(20, 2),
    available_capacity DECIMAL(20, 2),
    utilization_percent DECIMAL(5, 2)
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        g.id,
        g.guarantor_name,
        g.cap_amount,
        COALESCE(g.amount_claimed, 0) as amount_claimed,
        COALESCE(g.cap_amount - COALESCE(g.amount_claimed, 0), 0) as available_capacity,
        CASE 
            WHEN g.cap_amount > 0 THEN 
                (COALESCE(g.amount_claimed, 0) / g.cap_amount * 100)
            ELSE 0
        END as utilization_percent
    FROM guaranties g
    WHERE g.id = guaranty_id AND g.is_capped = TRUE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE guaranties IS 'Guaranty agreements where third parties guarantee borrower obligations';
COMMENT ON COLUMN guaranties.guarantee_type IS 'Type of guarantee: full unconditional, limited, capped, payment, collection, performance';
COMMENT ON COLUMN guaranties.coverage_scope IS 'Summary of obligations covered by the guaranty';
COMMENT ON COLUMN guaranties.is_capped IS 'Whether the guaranty has a maximum amount cap';
COMMENT ON COLUMN guaranties.cap_amount IS 'Maximum amount guaranteed (if capped)';
COMMENT ON COLUMN guaranties.is_payment_guaranty IS 'True if payment guaranty (pay on demand), false if collection only';
COMMENT ON COLUMN guaranties.subrogation_subordinated IS 'Whether subrogation rights are subordinated until full payment';
COMMENT ON COLUMN guaranties.guaranty_status IS 'Current status: pending, effective, terminated, released, or defaulted';
