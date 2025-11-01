-- ============================================================================
-- ACCOUNT CONTROL AGREEMENTS (DACA/ACA) TABLE
-- ============================================================================
-- Stores Deposit Account Control Agreements and Account Control Agreements

CREATE TABLE IF NOT EXISTS account_control_agreements (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'DACA-2025-002' or 'ACA-2025-003'
    security_agreement_id UUID REFERENCES security_agreements(id) ON DELETE CASCADE,
    credit_agreement_id UUID REFERENCES credit_agreements(id),
    
    -- Document Type
    agreement_type VARCHAR(10) NOT NULL CHECK (
        agreement_type IN ('DACA', 'ACA', 'SACA')
    ), -- DACA = Deposit Account, ACA = Securities Account, SACA = Commodity Account
    
    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash
    
    -- Parties
    debtor_name VARCHAR(500) NOT NULL, -- Account owner (borrower/grantor)
    secured_party VARCHAR(500) NOT NULL, -- Secured party / Collateral Agent
    bank_name VARCHAR(500) NOT NULL, -- Depository/control bank or securities intermediary
    bank_identifier VARCHAR(100), -- SWIFT/BIC or other identifier
    
    -- Account Information (masked for security)
    accounts_covered JSONB NOT NULL DEFAULT '[]',
    -- Array of account objects (masked account numbers for security):
    -- [
    --   {
    --     "account_number_masked": "••1234",
    --     "account_name": "Operating Account",
    --     "account_type": "checking",
    --     "currency": "USD",
    --     "approximate_balance": 500000,
    --     "control_status": "active"
    --   }
    -- ]
    
    total_accounts_count INTEGER NOT NULL DEFAULT 0,
    
    -- Control Type & Level
    control_level VARCHAR(30) NOT NULL CHECK (
        control_level IN (
            'exclusive_control',      -- Secured party has exclusive control
            'springing_control',      -- Control springs upon trigger event
            'tri_party_control',      -- Three-party agreement with conditions
            'notification_only',      -- Bank notified but no control until notice
            'blocked_account'         -- Account blocked for debtor withdrawals
        )
    ),
    
    control_trigger_event TEXT, -- Event that triggers control (for springing)
    
    -- Activation & Control Status
    control_activated BOOLEAN NOT NULL DEFAULT FALSE,
    activation_date DATE,
    activation_trigger TEXT,
    
    -- Account Operations
    debtor_withdrawal_rights TEXT, -- Description of debtor's rights to withdraw
    sweeps_permitted BOOLEAN DEFAULT FALSE,
    sweep_terms TEXT,
    setoff_rights_waived BOOLEAN DEFAULT FALSE,
    
    -- Notice Requirements
    notice_required_for_control BOOLEAN DEFAULT TRUE,
    notice_period_days INTEGER,
    notice_delivery_method TEXT,
    
    -- UCC-9 Compliance
    ucc9_control_perfected BOOLEAN NOT NULL DEFAULT FALSE,
    perfection_date DATE,
    control_method VARCHAR(50), -- 'control_agreement', 'account_in_secured_party_name', etc.
    
    -- Effective Dates
    effective_date DATE NOT NULL,
    termination_date DATE,
    
    -- Termination Provisions
    termination_conditions TEXT,
    automatic_termination BOOLEAN DEFAULT FALSE,
    
    -- Bank Obligations
    bank_acknowledgment_received BOOLEAN DEFAULT FALSE,
    bank_compliance_confirmed BOOLEAN DEFAULT FALSE,
    bank_fees DECIMAL(10, 2),
    
    -- Priority & Liens
    prior_liens_exist BOOLEAN DEFAULT FALSE,
    prior_liens_description TEXT,
    subordination_agreements TEXT,
    
    -- Special Provisions
    collateral_account BOOLEAN DEFAULT FALSE, -- Is this a designated collateral account?
    lockbox_account BOOLEAN DEFAULT FALSE,
    collections_account BOOLEAN DEFAULT FALSE,
    
    -- Integration with Other Systems
    linked_to_cash_management BOOLEAN DEFAULT FALSE,
    cash_management_system VARCHAR(100),
    
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

CREATE INDEX IF NOT EXISTS idx_aca_document_id ON account_control_agreements(document_id);
CREATE INDEX IF NOT EXISTS idx_aca_security_agreement_id ON account_control_agreements(security_agreement_id);
CREATE INDEX IF NOT EXISTS idx_aca_agreement_type ON account_control_agreements(agreement_type);
CREATE INDEX IF NOT EXISTS idx_aca_debtor_name ON account_control_agreements(debtor_name);
CREATE INDEX IF NOT EXISTS idx_aca_bank_name ON account_control_agreements(bank_name);
CREATE INDEX IF NOT EXISTS idx_aca_control_level ON account_control_agreements(control_level);
CREATE INDEX IF NOT EXISTS idx_aca_control_activated ON account_control_agreements(control_activated);
CREATE INDEX IF NOT EXISTS idx_aca_effective_date ON account_control_agreements(effective_date);
CREATE INDEX IF NOT EXISTS idx_aca_created_at ON account_control_agreements(created_at);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_aca_accounts_covered_gin ON account_control_agreements USING GIN (accounts_covered);
CREATE INDEX IF NOT EXISTS idx_aca_metadata_gin ON account_control_agreements USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_account_control_agreements_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_account_control_agreements_updated_at
    BEFORE UPDATE ON account_control_agreements
    FOR EACH ROW
    EXECUTE FUNCTION update_account_control_agreements_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Activate Control
-- ============================================================================

CREATE OR REPLACE FUNCTION activate_account_control(
    aca_id UUID,
    trigger_description TEXT
) RETURNS BOOLEAN AS $$
DECLARE
    control_type VARCHAR(30);
BEGIN
    -- Get the control level
    SELECT control_level INTO control_type
    FROM account_control_agreements
    WHERE id = aca_id;
    
    -- Only activate if it's a springing control type
    IF control_type = 'springing_control' THEN
        UPDATE account_control_agreements
        SET 
            control_activated = TRUE,
            activation_date = CURRENT_DATE,
            activation_trigger = trigger_description,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = aca_id;
        
        RETURN TRUE;
    END IF;
    
    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE account_control_agreements IS 'Deposit and securities account control agreements (DACA/ACA) for perfecting security interests';
COMMENT ON COLUMN account_control_agreements.agreement_type IS 'Type of control agreement: DACA (deposit), ACA (securities), SACA (commodity)';
COMMENT ON COLUMN account_control_agreements.control_level IS 'Level of control: exclusive, springing, tri-party, notification, or blocked';
COMMENT ON COLUMN account_control_agreements.accounts_covered IS 'JSONB array of controlled accounts (with masked account numbers)';
COMMENT ON COLUMN account_control_agreements.control_activated IS 'Whether control has been activated (relevant for springing control)';
COMMENT ON COLUMN account_control_agreements.ucc9_control_perfected IS 'Whether UCC-9 control perfection requirements are met';
