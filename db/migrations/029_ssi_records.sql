-- ============================================================================
-- STANDARD SETTLEMENT INSTRUCTIONS (SSI) TABLE
-- ============================================================================
-- Stores SSI records for deal settlement between parties
-- Supports both traditional banking rails and digital asset custody

CREATE TABLE IF NOT EXISTS ssi_records (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id VARCHAR(255) UNIQUE NOT NULL, -- e.g., 'SSI-2025-001'
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,
    deal_id UUID, -- Future reference to actual deal/trade

    -- Document Hashing
    doc_hash VARCHAR(66) NOT NULL, -- SHA-256 hash

    -- SSI Type
    ssi_type VARCHAR(20) NOT NULL CHECK (
        ssi_type IN ('full_bilateral', 'seller_only', 'buyer_only')
    ) DEFAULT 'full_bilateral',

    -- =================================================================
    -- SELLER SSI DETAILS (Outgoing Payment/Asset Instructions)
    -- =================================================================

    -- Seller Banking Details
    seller_ssi_id VARCHAR(255) UNIQUE NOT NULL, -- Unique SSI identifier
    seller_beneficiary_name VARCHAR(500) NOT NULL, -- Legal name on account
    seller_bank_name VARCHAR(500), -- Bank institution name
    seller_bank_identifier VARCHAR(255), -- SWIFT/BIC/ABA routing number
    seller_swift_code VARCHAR(11), -- ISO 9362 SWIFT/BIC code
    seller_aba_routing VARCHAR(9), -- US ABA routing number
    seller_iban VARCHAR(34), -- International Bank Account Number
    seller_account_number VARCHAR(100), -- Bank account number
    seller_reference_text TEXT, -- Payment reference/memo text
    seller_bank_address TEXT, -- Full bank address
    seller_intermediary_bank VARCHAR(500), -- Intermediary/correspondent bank
    seller_intermediary_swift VARCHAR(11), -- Intermediary bank SWIFT code

    -- Seller Digital Asset Custody
    seller_custody_type VARCHAR(30) CHECK (
        seller_custody_type IN ('bank', 'self_custody', 'custodian', 'exchange', 'hybrid')
    ) DEFAULT 'bank',
    seller_custodian_name VARCHAR(500), -- Custody provider name (e.g., Coinbase Custody, Anchorage)
    seller_custodian_account_ref VARCHAR(255), -- Account reference at custodian
    seller_wallet_address VARCHAR(255), -- Blockchain wallet address
    seller_wallet_chain VARCHAR(50), -- Blockchain network (e.g., Ethereum, Polygon, Arbitrum)
    seller_wallet_chain_id INTEGER, -- Chain ID (1 = Ethereum, 137 = Polygon, etc.)
    seller_wallet_type VARCHAR(20) CHECK (
        seller_wallet_type IN ('hot', 'cold', 'multisig', 'smart_contract')
    ),

    -- =================================================================
    -- BUYER SSI DETAILS (Incoming Payment/Asset Instructions)
    -- =================================================================

    -- Buyer Banking Details
    buyer_ssi_id VARCHAR(255) UNIQUE NOT NULL, -- Unique SSI identifier
    buyer_beneficiary_name VARCHAR(500) NOT NULL, -- Legal name on account
    buyer_bank_name VARCHAR(500), -- Bank institution name
    buyer_bank_identifier VARCHAR(255), -- SWIFT/BIC/ABA routing number
    buyer_swift_code VARCHAR(11), -- ISO 9362 SWIFT/BIC code
    buyer_aba_routing VARCHAR(9), -- US ABA routing number
    buyer_iban VARCHAR(34), -- International Bank Account Number
    buyer_account_number VARCHAR(100), -- Bank account number
    buyer_reference_text TEXT, -- Payment reference/memo text
    buyer_bank_address TEXT, -- Full bank address
    buyer_intermediary_bank VARCHAR(500), -- Intermediary/correspondent bank
    buyer_intermediary_swift VARCHAR(11), -- Intermediary bank SWIFT code

    -- Buyer Digital Asset Custody
    buyer_custody_type VARCHAR(30) CHECK (
        buyer_custody_type IN ('bank', 'self_custody', 'custodian', 'exchange', 'hybrid')
    ) DEFAULT 'bank',
    buyer_custodian_name VARCHAR(500), -- Custody provider name
    buyer_custodian_account_ref VARCHAR(255), -- Account reference at custodian
    buyer_wallet_address VARCHAR(255), -- Blockchain wallet address
    buyer_wallet_chain VARCHAR(50), -- Blockchain network
    buyer_wallet_chain_id INTEGER, -- Chain ID
    buyer_wallet_type VARCHAR(20) CHECK (
        buyer_wallet_type IN ('hot', 'cold', 'multisig', 'smart_contract')
    ),

    -- =================================================================
    -- SSI STATUS & VALIDATION
    -- =================================================================

    -- Verification Status
    seller_ssi_verified BOOLEAN DEFAULT FALSE,
    seller_ssi_verified_at TIMESTAMP WITH TIME ZONE,
    seller_ssi_verified_by VARCHAR(255), -- KYC/compliance officer

    buyer_ssi_verified BOOLEAN DEFAULT FALSE,
    buyer_ssi_verified_at TIMESTAMP WITH TIME ZONE,
    buyer_ssi_verified_by VARCHAR(255),

    -- SSI Status
    ssi_status VARCHAR(20) NOT NULL CHECK (
        ssi_status IN ('draft', 'pending_verification', 'active', 'expired', 'revoked')
    ) DEFAULT 'draft',

    -- Effective Dates
    effective_date DATE NOT NULL,
    expiry_date DATE, -- SSI expiration (for security)

    -- Usage Tracking
    last_used_date DATE,
    usage_count INTEGER DEFAULT 0,

    -- =================================================================
    -- COMPLIANCE & SECURITY
    -- =================================================================

    -- Sanctions Screening
    sanctions_screened BOOLEAN DEFAULT FALSE,
    sanctions_screen_date TIMESTAMP WITH TIME ZONE,
    sanctions_screen_result VARCHAR(20) CHECK (
        sanctions_screen_result IN ('clear', 'flagged', 'blocked', 'pending')
    ),

    -- AML/KYC Compliance
    aml_check_passed BOOLEAN DEFAULT FALSE,
    aml_check_date TIMESTAMP WITH TIME ZONE,

    -- Confidentiality
    confidential BOOLEAN DEFAULT TRUE,
    access_restricted_to TEXT[], -- Array of authorized user IDs

    -- =================================================================
    -- METADATA & AUDIT
    -- =================================================================

    -- Settlement Preferences
    settlement_currency VARCHAR(3) DEFAULT 'USD', -- Preferred settlement currency
    settlement_method VARCHAR(30) CHECK (
        settlement_method IN ('wire', 'ach', 'sepa', 'swift', 'blockchain', 'stablecoin')
    ) DEFAULT 'wire',

    -- Notes & Instructions
    special_instructions TEXT,
    internal_notes TEXT,

    -- Metadata
    metadata JSONB DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    -- Audit
    created_by UUID,
    updated_by UUID
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_ssi_document_id ON ssi_records(document_id);
CREATE INDEX IF NOT EXISTS idx_ssi_ca_id ON ssi_records(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_ssi_deal_id ON ssi_records(deal_id);
CREATE INDEX IF NOT EXISTS idx_ssi_seller_id ON ssi_records(seller_ssi_id);
CREATE INDEX IF NOT EXISTS idx_ssi_buyer_id ON ssi_records(buyer_ssi_id);
CREATE INDEX IF NOT EXISTS idx_ssi_status ON ssi_records(ssi_status);
CREATE INDEX IF NOT EXISTS idx_ssi_effective_date ON ssi_records(effective_date);
CREATE INDEX IF NOT EXISTS idx_ssi_expiry_date ON ssi_records(expiry_date);
CREATE INDEX IF NOT EXISTS idx_ssi_seller_verified ON ssi_records(seller_ssi_verified);
CREATE INDEX IF NOT EXISTS idx_ssi_buyer_verified ON ssi_records(buyer_ssi_verified);
CREATE INDEX IF NOT EXISTS idx_ssi_created_at ON ssi_records(created_at);

-- Wallet address indexes for blockchain settlement
CREATE INDEX IF NOT EXISTS idx_ssi_seller_wallet ON ssi_records(seller_wallet_address) WHERE seller_wallet_address IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ssi_buyer_wallet ON ssi_records(buyer_wallet_address) WHERE buyer_wallet_address IS NOT NULL;

-- Chain indexes
CREATE INDEX IF NOT EXISTS idx_ssi_seller_chain ON ssi_records(seller_wallet_chain) WHERE seller_wallet_chain IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ssi_buyer_chain ON ssi_records(buyer_wallet_chain) WHERE buyer_wallet_chain IS NOT NULL;

-- Array index for access control
CREATE INDEX IF NOT EXISTS idx_ssi_access_restricted_gin ON ssi_records USING GIN (access_restricted_to);

-- JSONB index
CREATE INDEX IF NOT EXISTS idx_ssi_metadata_gin ON ssi_records USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_ssi_records_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_ssi_records_updated_at
    BEFORE UPDATE ON ssi_records
    FOR EACH ROW
    EXECUTE FUNCTION update_ssi_records_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Validate SSI Expiry
-- ============================================================================

CREATE OR REPLACE FUNCTION check_ssi_expiry(ssi_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    is_expired BOOLEAN;
BEGIN
    SELECT
        CASE
            WHEN expiry_date IS NULL THEN FALSE
            WHEN expiry_date < CURRENT_DATE THEN TRUE
            ELSE FALSE
        END INTO is_expired
    FROM ssi_records
    WHERE id = ssi_id;

    RETURN COALESCE(is_expired, TRUE);
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- HELPER FUNCTION: Track SSI Usage
-- ============================================================================

CREATE OR REPLACE FUNCTION increment_ssi_usage(ssi_id UUID)
RETURNS VOID AS $$
BEGIN
    UPDATE ssi_records
    SET
        usage_count = usage_count + 1,
        last_used_date = CURRENT_DATE
    WHERE id = ssi_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE ssi_records IS 'Standard Settlement Instructions for deal settlements, supporting both traditional banking and digital asset custody';
COMMENT ON COLUMN ssi_records.ssi_type IS 'Type of SSI: full_bilateral (both parties), seller_only, or buyer_only';
COMMENT ON COLUMN ssi_records.seller_custody_type IS 'Custody method for seller: bank, self_custody, custodian, exchange, or hybrid';
COMMENT ON COLUMN ssi_records.buyer_custody_type IS 'Custody method for buyer: bank, self_custody, custodian, exchange, or hybrid';
COMMENT ON COLUMN ssi_records.seller_wallet_chain IS 'Blockchain network for seller wallet (e.g., Ethereum, Polygon, Arbitrum)';
COMMENT ON COLUMN ssi_records.buyer_wallet_chain IS 'Blockchain network for buyer wallet';
COMMENT ON COLUMN ssi_records.settlement_method IS 'Preferred settlement method: wire, ach, sepa, swift, blockchain, or stablecoin';
COMMENT ON COLUMN ssi_records.sanctions_screened IS 'Whether SSI has passed sanctions screening (OFAC, EU, UN lists)';
COMMENT ON COLUMN ssi_records.aml_check_passed IS 'Whether SSI has passed AML/KYC verification';
COMMENT ON COLUMN ssi_records.confidential IS 'Whether SSI is confidential (restricted access)';
