-- ============================================================================
-- PARTICIPATION AGREEMENTS TABLE
-- ============================================================================
-- Stores Participation Agreements for economic transfers without changing lender of record

CREATE TABLE IF NOT EXISTS participation_agreements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'PART-2025-022'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    trade_id UUID, -- Link to trade/secondary trading record if applicable
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Participation Type
    participation_type VARCHAR(30) NOT NULL CHECK (
        participation_type IN (
            'funded_participation',
            'unfunded_participation',
            'risk_participation',
            'sub_participation'
        )
    ),
    
    -- Parties
    grantor VARCHAR(500) NOT NULL, -- Selling lender (remains lender of record)
    grantor_entity_id UUID, -- Link to entity/lender table
    grantor_remains_lender_of_record BOOLEAN DEFAULT TRUE,
    
    participant VARCHAR(500) NOT NULL, -- Buying participant (economic interest only)
    participant_entity_id UUID, -- Link to entity/lender table
    
    -- Underlying Loan Reference
    borrower_name VARCHAR(500) NOT NULL,
    facility_reference VARCHAR(255), -- e.g., 'Term Loan A', 'Revolver'
    
    -- Participation Interest
    participation_percentage DECIMAL(8, 6), -- % of grantor's interest being participated
    participation_amount DECIMAL(20, 2), -- Dollar amount of participation
    
    -- Grantor's Position
    grantor_total_commitment DECIMAL(20, 2), -- Grantor's total commitment
    grantor_outstanding_principal DECIMAL(20, 2), -- Outstanding principal at participation date
    
    -- Economic Terms
    purchase_price DECIMAL(20, 2), -- Price paid by participant
    purchase_price_percentage DECIMAL(8, 4), -- As % of par
    
    -- Funded vs Unfunded
    funded_at_closing BOOLEAN DEFAULT TRUE,
    funding_mechanism TEXT, -- How participation is funded
    
    -- Payment Flow
    payments_flow VARCHAR(20) NOT NULL CHECK (
        payments_flow IN ('pass_through', 'pro_rata', 'waterfall', 'custom')
    ),
    payment_terms TEXT,
    
    -- Rights & Obligations
    participant_rights JSONB DEFAULT '{}',
    -- {
    --   "voting_rights": false,             // Typically no voting rights
    --   "information_rights": true,         // Receives loan information
    --   "proportionate_share": 50.0,        // % of grantor's economics
    --   "default_notification": true,       // Notified of defaults
    --   "amendment_consent": false          // Typically no consent rights
    -- }
    
    -- Participant has no direct rights against borrower
    no_privity_with_borrower BOOLEAN DEFAULT TRUE,
    participant_has_voting_rights BOOLEAN DEFAULT FALSE,
    
    -- Consent & Notification
    borrower_consent_required BOOLEAN DEFAULT FALSE, -- Usually not required
    borrower_consent_obtained BOOLEAN DEFAULT FALSE,
    borrower_knowledge_of_participation BOOLEAN DEFAULT FALSE,
    
    agent_notification_required BOOLEAN DEFAULT FALSE, -- May or may not notify
    agent_notified BOOLEAN DEFAULT FALSE,
    
    -- Sharing of Payments
    pro_rata_sharing BOOLEAN DEFAULT TRUE,
    principal_sharing_percentage DECIMAL(8, 6),
    interest_sharing_percentage DECIMAL(8, 6),
    fee_sharing_percentage DECIMAL(8, 6),
    
    -- Payment Mechanics
    grantor_pays_participant BOOLEAN DEFAULT TRUE, -- Grantor collects and pays participant
    settlement_frequency VARCHAR(20) CHECK (
        settlement_frequency IN ('upon_receipt', 'monthly', 'quarterly', 'custom')
    ),
    
    -- Risk Allocation
    participant_credit_risk BOOLEAN DEFAULT TRUE, -- Participant bears credit risk
    participant_funding_risk BOOLEAN DEFAULT FALSE, -- In unfunded participation
    
    -- Setoff & Netting
    setoff_rights TEXT,
    netting_provisions TEXT,
    
    -- Representations & Warranties
    grantor_representations JSONB DEFAULT '{}',
    participant_representations JSONB DEFAULT '{}',
    
    -- Grantor Warranties (typical)
    -- - Has authority to grant participation
    -- - Not in default under credit agreement
    -- - No misrepresentation of loan documents
    -- - Will share information
    
    -- Assignment/Transfer Rights
    participant_assignment_rights TEXT, -- Can participant assign its interest?
    requires_grantor_consent_to_assign BOOLEAN DEFAULT TRUE,
    
    -- Sub-Participation
    is_sub_participation BOOLEAN DEFAULT FALSE, -- Participation of a participation
    parent_participation_id UUID REFERENCES participation_agreements(id),
    
    -- Confidentiality
    confidential_agreement BOOLEAN DEFAULT TRUE,
    disclosure_restrictions TEXT,
    
    -- Effective Dates
    trade_date DATE NOT NULL,
    settlement_date DATE NOT NULL,
    effective_date DATE NOT NULL,
    
    -- Termination
    termination_date DATE,
    termination_conditions TEXT,
    
    -- Participant Information (KYC/Compliance)
    participant_kyc_completed BOOLEAN DEFAULT FALSE,
    participant_kyc_date DATE,
    participant_eligible BOOLEAN DEFAULT TRUE,
    
    -- Status Tracking
    agreement_status VARCHAR(20) NOT NULL CHECK (
        agreement_status IN ('draft', 'negotiation', 'executed', 'settled', 'active', 'terminated', 'cancelled')
    ) DEFAULT 'draft',
    
    -- Settlement Tracking
    cash_settled BOOLEAN DEFAULT FALSE,
    participation_active BOOLEAN DEFAULT FALSE,
    
    -- Performance Tracking
    total_payments_received DECIMAL(20, 2) DEFAULT 0,
    total_payments_to_participant DECIMAL(20, 2) DEFAULT 0,
    
    -- Metadata
    notes TEXT,
    special_terms TEXT,
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

CREATE INDEX IF NOT EXISTS idx_participation_document_id ON participation_agreements(document_id);
CREATE INDEX IF NOT EXISTS idx_participation_ca_id ON participation_agreements(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_participation_trade_id ON participation_agreements(trade_id);
CREATE INDEX IF NOT EXISTS idx_participation_grantor ON participation_agreements(grantor);
CREATE INDEX IF NOT EXISTS idx_participation_participant ON participation_agreements(participant);
CREATE INDEX IF NOT EXISTS idx_participation_type ON participation_agreements(participation_type);
CREATE INDEX IF NOT EXISTS idx_participation_trade_date ON participation_agreements(trade_date);
CREATE INDEX IF NOT EXISTS idx_participation_settlement_date ON participation_agreements(settlement_date);
CREATE INDEX IF NOT EXISTS idx_participation_effective_date ON participation_agreements(effective_date);
CREATE INDEX IF NOT EXISTS idx_participation_status ON participation_agreements(agreement_status);
CREATE INDEX IF NOT EXISTS idx_participation_active ON participation_agreements(participation_active);
CREATE INDEX IF NOT EXISTS idx_participation_created_at ON participation_agreements(created_at);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_participation_rights_gin ON participation_agreements USING GIN (participant_rights);
CREATE INDEX IF NOT EXISTS idx_participation_grantor_reps_gin ON participation_agreements USING GIN (grantor_representations);
CREATE INDEX IF NOT EXISTS idx_participation_participant_reps_gin ON participation_agreements USING GIN (participant_representations);
CREATE INDEX IF NOT EXISTS idx_participation_metadata_gin ON participation_agreements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_participation_agreements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_participation_agreements_updated_at
    BEFORE UPDATE ON participation_agreements
    FOR EACH ROW
    EXECUTE FUNCTION update_participation_agreements_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Calculate Participant Payment
-- ============================================================================

CREATE OR REPLACE FUNCTION calculate_participant_payment(
    participation_id UUID,
    total_payment_received DECIMAL(20, 2)
) RETURNS DECIMAL(20, 2) AS $$
DECLARE
    v_percentage DECIMAL(8, 6);
    v_participant_share DECIMAL(20, 2);
BEGIN
    -- Get the participation percentage
    SELECT principal_sharing_percentage 
    INTO v_percentage
    FROM participation_agreements
    WHERE id = participation_id;
    
    -- Calculate participant's share
    v_participant_share := total_payment_received * (v_percentage / 100.0);
    
    RETURN v_participant_share;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- HELPER FUNCTION: Check Participation vs Assignment
-- ============================================================================

CREATE OR REPLACE FUNCTION compare_participation_vs_assignment()
RETURNS TABLE (
    feature TEXT,
    participation TEXT,
    assignment TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        'Lender of Record'::TEXT as feature,
        'Grantor remains lender of record'::TEXT as participation,
        'Assignee becomes new lender of record'::TEXT as assignment
    UNION ALL
    SELECT 
        'Privity with Borrower'::TEXT,
        'No direct relationship with borrower'::TEXT,
        'Direct contractual relationship with borrower'::TEXT
    UNION ALL
    SELECT 
        'Voting Rights'::TEXT,
        'Typically no voting rights'::TEXT,
        'Full voting rights as lender'::TEXT
    UNION ALL
    SELECT 
        'Borrower Consent'::TEXT,
        'Usually not required'::TEXT,
        'Often required (check credit agreement)'::TEXT
    UNION ALL
    SELECT 
        'Payment Flow'::TEXT,
        'Grantor collects and pays participant'::TEXT,
        'Borrower pays assignee directly'::TEXT
    UNION ALL
    SELECT 
        'Credit Risk'::TEXT,
        'Borne by participant'::TEXT,
        'Borne by assignee'::TEXT;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE participation_agreements IS 'Participation agreements for economic transfers where grantor remains lender of record';
COMMENT ON COLUMN participation_agreements.participation_type IS 'Type: funded, unfunded, risk, or sub-participation';
COMMENT ON COLUMN participation_agreements.grantor IS 'Selling lender who retains lender of record status';
COMMENT ON COLUMN participation_agreements.participant IS 'Buying party who gets economic interest only';
COMMENT ON COLUMN participation_agreements.no_privity_with_borrower IS 'Participant has no direct contractual relationship with borrower';
COMMENT ON COLUMN participation_agreements.participant_has_voting_rights IS 'Typically false - participant usually has no voting rights';
COMMENT ON COLUMN participation_agreements.grantor_pays_participant IS 'Grantor collects from borrower and pays participant';
COMMENT ON COLUMN participation_agreements.participant_credit_risk IS 'Participant bears credit risk of the underlying loan';
