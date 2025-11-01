-- ============================================================================
-- SECONDARY TRADING TABLE
-- ============================================================================
-- Stores secondary market trades for credit instruments and deals
-- Tracks trade execution, pricing, settlement, and transfer details

CREATE TABLE IF NOT EXISTS secondary_trades (
    -- Core Identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id VARCHAR(255) UNIQUE NOT NULL, -- Unique trade identifier (e.g., 'TRD-2025-0045')
    credit_agreement_id UUID REFERENCES credit_agreements(id) ON DELETE CASCADE,

    -- Document Hashing
    trade_confirm_hash VARCHAR(66) NOT NULL, -- SHA-256 hash of trade confirmation

    -- Instrument Details
    instrument_id VARCHAR(255) NOT NULL, -- Reference to tradable instrument (loan, credit line, etc.)
    instrument_type VARCHAR(50) CHECK (
        instrument_type IN (
            'term_loan', 'revolver', 'delayed_draw', 'bridge_loan',
            'credit_line', 'participation', 'assignment', 'syndication', 'other'
        )
    ) DEFAULT 'term_loan',

    -- =================================================================
    -- PARTIES
    -- =================================================================

    -- Seller Information
    seller_party_id UUID NOT NULL, -- Reference to seller company/user
    seller_legal_name VARCHAR(500) NOT NULL,
    seller_role VARCHAR(50) CHECK (
        seller_role IN ('originator', 'investor', 'agent', 'participant', 'arranger')
    ),

    -- Buyer Information
    buyer_party_id UUID NOT NULL, -- Reference to buyer company/user
    buyer_legal_name VARCHAR(500) NOT NULL,
    buyer_role VARCHAR(50) CHECK (
        buyer_role IN ('investor', 'participant', 'syndicate_member', 'secondary_buyer')
    ),

    -- Broker/Intermediary (if applicable)
    broker_party_id UUID,
    broker_name VARCHAR(500),
    broker_fee_amount DECIMAL(20, 2),
    broker_fee_currency VARCHAR(3) DEFAULT 'USD',

    -- =================================================================
    -- TRADE DETAILS & TIMING
    -- =================================================================

    -- Trade Execution
    trade_datetime TIMESTAMP WITH TIME ZONE NOT NULL, -- Exact trade execution time
    trade_date DATE NOT NULL, -- Trade date (T)
    trade_type VARCHAR(30) NOT NULL CHECK (
        trade_type IN (
            'par_trade',           -- Trading at par (100%)
            'discount_trade',      -- Trading below par
            'premium_trade',       -- Trading above par
            'distressed_trade',    -- Distressed credit
            'participation',       -- Participation interest
            'assignment',          -- Full assignment
            'novation',            -- Novation of contract
            'syndication'          -- Syndication participation
        )
    ),

    -- Trade Status
    trade_status VARCHAR(30) NOT NULL CHECK (
        trade_status IN (
            'pending',             -- Trade pending confirmation
            'confirmed',           -- Trade confirmed by both parties
            'allocated',           -- Trade allocated
            'settling',            -- In settlement process
            'settled',             -- Fully settled
            'failed',              -- Settlement failed
            'cancelled',           -- Trade cancelled
            'rejected'             -- Trade rejected
        )
    ) DEFAULT 'pending',

    -- Settlement Timing
    effective_date DATE NOT NULL, -- When trade becomes effective
    settlement_date DATE NOT NULL, -- Expected settlement date (T+X)
    settlement_days INTEGER NOT NULL DEFAULT 3, -- Settlement cycle (T+3 standard)
    actual_settlement_date DATE, -- Actual settlement completion

    -- =================================================================
    -- FINANCIAL TERMS
    -- =================================================================

    -- Notional Amount
    notional_amount DECIMAL(20, 2) NOT NULL, -- Face value being traded
    notional_currency VARCHAR(3) NOT NULL DEFAULT 'USD',

    -- Pricing
    price_type VARCHAR(30) NOT NULL CHECK (
        price_type IN (
            'percentage_of_par',   -- e.g., 95% of par
            'dollar_price',        -- e.g., $950,000
            'yield_based',         -- Based on yield
            'discount_to_par',     -- e.g., 5% discount
            'premium_to_par'       -- e.g., 2% premium
        )
    ),
    price DECIMAL(12, 6) NOT NULL, -- Price (e.g., 95.500 for 95.5% of par, or absolute dollar amount)
    price_in_dollars DECIMAL(20, 2), -- Calculated dollar price

    -- Accrued Interest
    accrued_interest_amount DECIMAL(20, 2) DEFAULT 0.00,
    accrued_interest_currency VARCHAR(3) DEFAULT 'USD',
    accrued_interest_calculation_method VARCHAR(50), -- e.g., "ACT/360", "30/360"
    accrued_interest_from_date DATE,
    accrued_interest_to_date DATE,

    -- Total Consideration
    total_consideration DECIMAL(20, 2) NOT NULL, -- price_in_dollars + accrued_interest
    total_consideration_currency VARCHAR(3) DEFAULT 'USD',

    -- Transfer Fees & Expenses
    transfer_fee_amount DECIMAL(20, 2) DEFAULT 0.00,
    transfer_fee_currency VARCHAR(3) DEFAULT 'USD',
    transfer_fee_paid_by VARCHAR(20) CHECK (
        transfer_fee_paid_by IN ('seller', 'buyer', 'shared', 'broker')
    ) DEFAULT 'buyer',

    -- Other Fees
    other_fees JSONB DEFAULT '{}',
    -- Example structure:
    -- {
    --   "agency_fee": 5000,
    --   "legal_fee": 15000,
    --   "diligence_fee": 10000
    -- }

    -- =================================================================
    -- ELIGIBILITY & COMPLIANCE
    -- =================================================================

    -- Eligibility Checks
    eligibility_checks JSONB DEFAULT '{}',
    -- Example structure:
    -- {
    --   "kyc_verified": true,
    --   "accredited_investor": true,
    --   "sanctions_clear": true,
    --   "transfer_restrictions_waived": false,
    --   "consent_required": true,
    --   "consent_obtained": true
    -- }

    kyc_checks_passed BOOLEAN DEFAULT FALSE,
    aml_checks_passed BOOLEAN DEFAULT FALSE,
    sanctions_screening_passed BOOLEAN DEFAULT FALSE,

    -- Transfer Restrictions
    transfer_restrictions_apply BOOLEAN DEFAULT TRUE,
    transfer_consent_required BOOLEAN DEFAULT TRUE,
    transfer_consent_obtained BOOLEAN DEFAULT FALSE,
    transfer_consent_date DATE,
    transfer_consent_obtained_from VARCHAR(500), -- Agent/Lender who granted consent

    -- =================================================================
    -- ALLOCATION & DOCUMENTATION
    -- =================================================================

    -- Allocation Details
    allocation_id VARCHAR(255), -- If part of a larger allocation
    allocation_percentage DECIMAL(5, 4), -- % of total facility/instrument
    pro_rata_share BOOLEAN DEFAULT FALSE,

    -- Required Documentation
    docs_required JSONB DEFAULT '{}',
    -- Example structure:
    -- {
    --   "assignment_agreement": "pending",
    --   "buyer_kyc": "received",
    --   "seller_rep_letter": "received",
    --   "legal_opinion": "waived",
    --   "credit_agreement": "executed"
    -- }

    docs_received_count INTEGER DEFAULT 0,
    docs_required_count INTEGER DEFAULT 0,
    docs_complete BOOLEAN DEFAULT FALSE,

    -- Trade Confirmation
    trade_confirm_sent BOOLEAN DEFAULT FALSE,
    trade_confirm_sent_at TIMESTAMP WITH TIME ZONE,
    trade_confirm_received BOOLEAN DEFAULT FALSE,
    trade_confirm_received_at TIMESTAMP WITH TIME ZONE,

    -- =================================================================
    -- SPECIAL TERMS & NOTES
    -- =================================================================

    -- Special Terms
    special_terms TEXT,
    representations_and_warranties TEXT,
    conditions_precedent TEXT,
    post_closing_obligations TEXT,

    -- Trade Notes
    internal_notes TEXT,
    trader_notes TEXT,
    compliance_notes TEXT,

    -- =================================================================
    -- MARKET DATA & ANALYTICS
    -- =================================================================

    -- Market Context
    market_segment VARCHAR(50), -- e.g., "Broadly Syndicated", "Middle Market", "Direct Lending"
    credit_rating VARCHAR(10), -- e.g., "BB+", "B-"
    sector VARCHAR(100), -- Industry sector

    -- Yield & Spread
    yield_to_maturity DECIMAL(6, 4), -- Yield as decimal (0.0850 = 8.50%)
    spread_to_benchmark_bps INTEGER, -- Spread over benchmark rate

    -- =================================================================
    -- SETTLEMENT INSTRUCTIONS
    -- =================================================================

    -- SSI References
    seller_ssi_id UUID, -- REFERENCES ssi_records(id)
    buyer_ssi_id UUID, -- REFERENCES ssi_records(id)

    settlement_method VARCHAR(30) CHECK (
        settlement_method IN ('dvp', 'fop', 'wire', 'blockchain', 'escrow')
    ) DEFAULT 'dvp', -- Delivery vs. Payment, Free of Payment, etc.

    settlement_account_seller VARCHAR(255),
    settlement_account_buyer VARCHAR(255),

    -- =================================================================
    -- METADATA & AUDIT
    -- =================================================================

    -- Metadata
    metadata JSONB DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    -- Audit Trail
    created_by UUID,
    updated_by UUID,
    confirmed_by UUID,
    settled_by UUID
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_secondary_trade_id ON secondary_trades(trade_id);
CREATE INDEX IF NOT EXISTS idx_secondary_ca_id ON secondary_trades(credit_agreement_id);
CREATE INDEX IF NOT EXISTS idx_secondary_instrument_id ON secondary_trades(instrument_id);
CREATE INDEX IF NOT EXISTS idx_secondary_seller_party ON secondary_trades(seller_party_id);
CREATE INDEX IF NOT EXISTS idx_secondary_buyer_party ON secondary_trades(buyer_party_id);
CREATE INDEX IF NOT EXISTS idx_secondary_broker_party ON secondary_trades(broker_party_id);
CREATE INDEX IF NOT EXISTS idx_secondary_trade_datetime ON secondary_trades(trade_datetime);
CREATE INDEX IF NOT EXISTS idx_secondary_trade_date ON secondary_trades(trade_date);
CREATE INDEX IF NOT EXISTS idx_secondary_trade_type ON secondary_trades(trade_type);
CREATE INDEX IF NOT EXISTS idx_secondary_trade_status ON secondary_trades(trade_status);
CREATE INDEX IF NOT EXISTS idx_secondary_settlement_date ON secondary_trades(settlement_date);
CREATE INDEX IF NOT EXISTS idx_secondary_effective_date ON secondary_trades(effective_date);
CREATE INDEX IF NOT EXISTS idx_secondary_allocation_id ON secondary_trades(allocation_id);
CREATE INDEX IF NOT EXISTS idx_secondary_created_at ON secondary_trades(created_at);

-- JSONB indexes
CREATE INDEX IF NOT EXISTS idx_secondary_eligibility_gin ON secondary_trades USING GIN (eligibility_checks);
CREATE INDEX IF NOT EXISTS idx_secondary_docs_gin ON secondary_trades USING GIN (docs_required);
CREATE INDEX IF NOT EXISTS idx_secondary_fees_gin ON secondary_trades USING GIN (other_fees);
CREATE INDEX IF NOT EXISTS idx_secondary_metadata_gin ON secondary_trades USING GIN (metadata);

-- ============================================================================
-- TRIGGER FOR UPDATED_AT
-- ============================================================================

CREATE OR REPLACE FUNCTION update_secondary_trades_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_secondary_trades_updated_at
    BEFORE UPDATE ON secondary_trades
    FOR EACH ROW
    EXECUTE FUNCTION update_secondary_trades_updated_at();

-- ============================================================================
-- HELPER FUNCTION: Calculate Trade Settlement Amount
-- ============================================================================

CREATE OR REPLACE FUNCTION calculate_trade_settlement_amount(trade_uuid UUID)
RETURNS TABLE (
    trade_id VARCHAR(255),
    notional_amount DECIMAL(20, 2),
    price_in_dollars DECIMAL(20, 2),
    accrued_interest DECIMAL(20, 2),
    transfer_fee DECIMAL(20, 2),
    total_settlement DECIMAL(20, 2)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        t.trade_id,
        t.notional_amount,
        t.price_in_dollars,
        t.accrued_interest_amount,
        t.transfer_fee_amount,
        (t.price_in_dollars + t.accrued_interest_amount +
         CASE WHEN t.transfer_fee_paid_by = 'buyer' THEN t.transfer_fee_amount ELSE 0 END) as total_settlement
    FROM secondary_trades t
    WHERE t.id = trade_uuid;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- HELPER FUNCTION: Check Trade Eligibility
-- ============================================================================

CREATE OR REPLACE FUNCTION check_trade_eligibility(trade_uuid UUID)
RETURNS BOOLEAN AS $$
DECLARE
    is_eligible BOOLEAN;
BEGIN
    SELECT
        kyc_checks_passed AND
        aml_checks_passed AND
        sanctions_screening_passed AND
        (NOT transfer_consent_required OR transfer_consent_obtained)
    INTO is_eligible
    FROM secondary_trades
    WHERE id = trade_uuid;

    RETURN COALESCE(is_eligible, FALSE);
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE secondary_trades IS 'Secondary market trades for credit instruments and deal participations';
COMMENT ON COLUMN secondary_trades.trade_type IS 'Type of trade: par, discount, premium, distressed, participation, assignment, novation, or syndication';
COMMENT ON COLUMN secondary_trades.price_type IS 'Pricing method: percentage_of_par, dollar_price, yield_based, discount_to_par, or premium_to_par';
COMMENT ON COLUMN secondary_trades.eligibility_checks IS 'JSONB containing all eligibility check results (KYC, AML, sanctions, transfer restrictions)';
COMMENT ON COLUMN secondary_trades.docs_required IS 'JSONB tracking required documentation status for trade settlement';
COMMENT ON COLUMN secondary_trades.special_terms IS 'Any special terms, conditions, or representations specific to this trade';
COMMENT ON COLUMN secondary_trades.settlement_method IS 'Settlement method: dvp (delivery vs payment), fop (free of payment), wire, blockchain, or escrow';
COMMENT ON COLUMN secondary_trades.transfer_consent_required IS 'Whether lender/agent consent is required for this transfer';
