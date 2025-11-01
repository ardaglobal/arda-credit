-- ============================================================================
-- ASSIGNMENT AND ASSUMPTION AGREEMENTS TABLE
-- ============================================================================
-- Stores Assignment & Assumption Agreements for transferring loan interests

CREATE TABLE IF NOT EXISTS assignment_assumption_agreements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'A&A-2025-015'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    trade_id UUID, -- Link to trade/secondary trading record if applicable
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Assignment Type
    assignment_type VARCHAR(30) NOT NULL CHECK (
        assignment_type IN ('full_assignment', 'partial_assignment', 'pro_rata_assignment')
    ),
    
    -- Parties
    assignor VARCHAR(500) NOT NULL, -- Selling lender (original or current holder)
    assignor_entity_id UUID, -- Link to entity/lender table
    
    assignee VARCHAR(500) NOT NULL, -- Buying lender (new holder)
    assignee_entity_id UUID, -- Link to entity/lender table
    
    administrative_agent VARCHAR(500), -- Administrative agent under credit agreement
    
    -- Borrower (for notice/consent purposes)
    borrower_name VARCHAR(500) NOT NULL,
    
    -- Assignment Details
    facility_assigned VARCHAR(255), -- e.g., 'Term Loan A', 'Revolver'
    
    -- Assigned Interest Amounts
    assigned_commitment_amount DECIMAL(20, 2), -- Commitment being assigned
    assigned_outstanding_principal DECIMAL(20, 2), -- Outstanding principal at assignment date
    assigned_percentage DECIMAL(8, 6), -- % of total facility being assigned
    
    -- Pro Rata Share
    pro_rata_share_before DECIMAL(8, 6), -- Assignor's pro rata share before assignment
    pro_rata_share_after_assignor DECIMAL(8, 6), -- Assignor's remaining share after
    pro_rata_share_assignee DECIMAL(8, 6), -- Assignee's new pro rata share
    
    -- Economic Terms
    purchase_price DECIMAL(20, 2), -- Purchase price paid by assignee
    price_per_unit DECIMAL(12, 6), -- Price per dollar/unit of principal
    accrued_interest DECIMAL(15, 2), -- Accrued interest being transferred
    fees_assigned DECIMAL(15, 2), -- Fees assigned (if any)
    
    -- Effective Date
    trade_date DATE NOT NULL,
    settlement_date DATE NOT NULL,
    effective_date DATE NOT NULL, -- Date assignee becomes lender of record
    
    -- Consent & Approval
    borrower_consent_required BOOLEAN NOT NULL DEFAULT FALSE,
    borrower_consent_obtained BOOLEAN DEFAULT FALSE,
    borrower_consent_date DATE,
    
    agent_consent_required BOOLEAN NOT NULL DEFAULT TRUE,
    agent_consent_obtained BOOLEAN DEFAULT FALSE,
    agent_consent_date DATE,
    
    other_consents_required TEXT, -- Other required consents
    all_consents_obtained BOOLEAN DEFAULT FALSE,
    
    -- Transfer Restrictions Compliance
    minimum_assignment_amount DECIMAL(20, 2), -- Minimum per credit agreement
    meets_minimum_threshold BOOLEAN DEFAULT TRUE,
    
    eligible_assignee BOOLEAN DEFAULT TRUE, -- Meets eligibility requirements
    eligibility_criteria_met TEXT, -- Description of criteria satisfied
    
    -- Disqualified Lender List
    disqualified_lender_check BOOLEAN DEFAULT FALSE,
    not_on_dql BOOLEAN DEFAULT TRUE, -- Not on disqualified lender list
    
    -- Representations & Warranties
    assignor_representations JSONB DEFAULT '{}',
    assignee_representations JSONB DEFAULT '{}',
    -- Typical reps: authority, binding obligation, no conflicts, etc.
    
    -- Processing Fee
    processing_fee DECIMAL(10, 2), -- Fee paid to agent for processing
    processing_fee_paid_by VARCHAR(50) CHECK (
        processing_fee_paid_by IN ('assignor', 'assignee', 'shared', 'other')
    ),
    
    -- Register/Record Update
    recorded_in_register BOOLEAN DEFAULT FALSE,
    register_update_date DATE,
    register_notation TEXT,
    
    -- Assignee Information (KYC/Compliance)
    assignee_kyc_completed BOOLEAN DEFAULT FALSE,
    assignee_kyc_date DATE,
    assignee_tax_forms_received BOOLEAN DEFAULT FALSE, -- W-9/W-8 forms
    
    -- Participation vs Assignment
    is_assignment BOOLEAN DEFAULT TRUE, -- TRUE for assignment, FALSE for participation
    changes_lender_of_record BOOLEAN DEFAULT TRUE,
    
    -- Ancillary Documents
    joinder_agreement_required BOOLEAN DEFAULT FALSE,
    joinder_executed BOOLEAN DEFAULT FALSE,
    
    -- Status Tracking
    agreement_status VARCHAR(20) NOT NULL CHECK (
        agreement_status IN ('draft', 'pending_consent', 'approved', 'executed', 'settled', 'cancelled')
    ) DEFAULT 'draft',
    
    -- Settlement Tracking
    cash_settled BOOLEAN DEFAULT FALSE,
    asset_transferred BOOLEAN DEFAULT FALSE,
    settlement_confirmation TEXT,
    
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

CREATE INDEX IF NOT EXISTS idx_aaa_document_id ON assignment_assumption_agreements(document_id);
CREATE INDEX IF NOT EXISTS idx_aaa_ca_id ON assignment_assumption_agreements(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_aaa_trade_id ON assignment_assumption_agreements(trade_id);
CREATE INDEX IF NOT EXISTS idx_aaa_assignor ON assignment_assumption_agreements(assignor);
CREATE INDEX IF NOT EXISTS idx_aaa_assignee ON assignment_assumption_agreements(assignee);
CREATE INDEX IF NOT EXISTS idx_aaa_trade_date ON assignment_assumption_agreements(trade_date);
CREATE INDEX IF NOT EXISTS idx_aaa_settlement_date ON assignment_assumption_agreements(settlement_date);
CREATE INDEX IF NOT EXISTS idx_aaa_effective_date ON assignment_assumption_agreements(effective_date);
CREATE INDEX IF NOT EXISTS idx_aaa_status ON assignment_assumption_agreements(agreement_status);
CREATE INDEX IF NOT EXISTS idx_aaa_created_at ON assignment_assumption_agreements(created_at);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_aaa_assignor_reps_gin ON assignment_assumption_agreements USING GIN (assignor_representations);
CREATE INDEX IF NOT EXISTS idx_aaa_assignee_reps_gin ON assignment_assumption_agreements USING GIN (assignee_representations);
CREATE INDEX IF NOT EXISTS idx_aaa_metadata_gin ON assignment_assumption_agreements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_assignment_assumption_agreements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_assignment_assumption_agreements_updated_at
    BEFORE UPDATE ON assignment_assumption_agreements
    FOR EACH ROW
    EXECUTE FUNCTION update_assignment_assumption_agreements_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Check Assignment Readiness
-- ============================================================================

CREATE OR REPLACE FUNCTION check_assignment_readiness(assignment_id UUID)
RETURNS TABLE (
    id UUID,
    document_id VARCHAR(255),
    ready_for_settlement BOOLEAN,
    pending_items TEXT[]
) AS $$
DECLARE
    v_pending_items TEXT[] := '{}';
    v_ready BOOLEAN := TRUE;
BEGIN
    SELECT 
        ARRAY_APPEND(v_pending_items, 'Borrower consent required')
    INTO v_pending_items
    FROM assignment_assumption_agreements
    WHERE id = assignment_id 
        AND borrower_consent_required = TRUE 
        AND borrower_consent_obtained = FALSE;
    
    SELECT 
        ARRAY_APPEND(v_pending_items, 'Agent consent required')
    INTO v_pending_items
    FROM assignment_assumption_agreements
    WHERE id = assignment_id 
        AND agent_consent_required = TRUE 
        AND agent_consent_obtained = FALSE;
    
    SELECT 
        ARRAY_APPEND(v_pending_items, 'Assignee KYC not completed')
    INTO v_pending_items
    FROM assignment_assumption_agreements
    WHERE id = assignment_id 
        AND assignee_kyc_completed = FALSE;
    
    SELECT 
        ARRAY_APPEND(v_pending_items, 'Assignee tax forms not received')
    INTO v_pending_items
    FROM assignment_assumption_agreements
    WHERE id = assignment_id 
        AND assignee_tax_forms_received = FALSE;
    
    -- Check if ready
    v_ready := (array_length(v_pending_items, 1) IS NULL OR array_length(v_pending_items, 1) = 0);
    
    RETURN QUERY
    SELECT 
        a.id,
        a.document_id,
        v_ready as ready_for_settlement,
        v_pending_items as pending_items
    FROM assignment_assumption_agreements a
    WHERE a.id = assignment_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE assignment_assumption_agreements IS 'Assignment & Assumption Agreements for transferring loan commitments and outstanding balances';
COMMENT ON COLUMN assignment_assumption_agreements.assignment_type IS 'Type: full assignment, partial assignment, or pro-rata assignment';
COMMENT ON COLUMN assignment_assumption_agreements.assignor IS 'Selling lender transferring its interest';
COMMENT ON COLUMN assignment_assumption_agreements.assignee IS 'Buying lender assuming the interest';
COMMENT ON COLUMN assignment_assumption_agreements.assigned_percentage IS 'Percentage of total facility being assigned';
COMMENT ON COLUMN assignment_assumption_agreements.effective_date IS 'Date when assignee becomes lender of record';
COMMENT ON COLUMN assignment_assumption_agreements.changes_lender_of_record IS 'True for assignments (changes record holder), false for participations';
