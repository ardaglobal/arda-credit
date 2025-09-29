-- ============================================================================
-- DEALS TABLE
-- Core deal management with proper naming conventions
-- Consolidates deals, payments, listings, and transfers
-- ============================================================================

-- Main deals table (fixed naming: deal.status instead of deal.deal_status)
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    originator_id UUID NOT NULL REFERENCES users(id),
    investor_id UUID NOT NULL REFERENCES users(id),

    -- Deal financial terms
    principal_amount DECIMAL(18, 6) NOT NULL CHECK (principal_amount > 0),
    outstanding_balance DECIMAL(18, 6) NOT NULL CHECK (outstanding_balance >= 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0), -- Basis points (1/100th of 1%)

    -- Deal lifecycle and status (fixed: single status field instead of loan_status + status)
    status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'paid_off', 'closed', 'defaulted', 'for_sale', 'transfer_pending')),
    originated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    maturity_date TIMESTAMP WITH TIME ZONE,

    -- Deal metadata
    custom_deal_id VARCHAR(100), -- Optional user-defined identifier
    title VARCHAR(255),
    description TEXT,
    industry VARCHAR(100), -- Industry sector classification
    document_ids JSONB, -- JSON array of document UUIDs

    -- Risk and investment categorization
    risk_level VARCHAR(10) CHECK (risk_level IN ('low', 'medium', 'high')),
    asset_class VARCHAR(100),
    region VARCHAR(100),
    minimum_investment DECIMAL(15, 2),

    -- Audit timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT originator_investor_different CHECK (originator_id != investor_id)
);

-- ============================================================================
-- DEAL PAYMENTS
-- ============================================================================

-- Table for tracking payments made against deals
CREATE TABLE deal_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id) ON DELETE CASCADE,
    payer_id UUID NOT NULL REFERENCES users(id),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    payment_type VARCHAR(20) NOT NULL DEFAULT 'regular' CHECK (payment_type IN ('regular', 'principal', 'interest', 'penalty')),
    transaction_hash VARCHAR(66), -- Ethereum tx hash if on-chain
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- DEAL MARKETPLACE
-- ============================================================================

-- Table for deal listings in marketplace
CREATE TABLE deal_listings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id) ON DELETE CASCADE,
    seller_id UUID NOT NULL REFERENCES users(id),
    asking_price DECIMAL(15, 2) NOT NULL CHECK (asking_price > 0),
    description TEXT,
    listed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE, -- Optional expiration for listings
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'sold', 'cancelled', 'expired', 'delisted')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Business constraints
    UNIQUE(deal_id) -- Only one active listing per deal
);

-- ============================================================================
-- DEAL TRANSFERS
-- ============================================================================

-- Table for deal ownership transfers
CREATE TABLE deal_transfers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id) ON DELETE CASCADE,
    from_investor_id UUID NOT NULL REFERENCES users(id),
    to_investor_id UUID NOT NULL REFERENCES users(id),
    transfer_price DECIMAL(15, 2) NOT NULL CHECK (transfer_price > 0),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed', 'cancelled')),
    transfer_date TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE, -- When transfer actually completed
    notes TEXT, -- Optional transfer notes
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT no_self_transfer CHECK (from_investor_id != to_investor_id) -- Cannot transfer to self
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Deal indexes
CREATE INDEX idx_deals_originator_id ON deals(originator_id);
CREATE INDEX idx_deals_investor_id ON deals(investor_id);
CREATE INDEX idx_deals_status ON deals(status);
CREATE INDEX idx_deals_originated_at ON deals(originated_at);
CREATE INDEX idx_deals_outstanding_balance ON deals(outstanding_balance) WHERE outstanding_balance > 0;
CREATE INDEX idx_deals_marketplace ON deals(status, originated_at) WHERE status = 'for_sale';
CREATE INDEX idx_deals_investor_status ON deals(investor_id, status);
CREATE INDEX idx_deals_industry ON deals(industry) WHERE industry IS NOT NULL;
CREATE INDEX idx_deals_custom_id ON deals(custom_deal_id) WHERE custom_deal_id IS NOT NULL;
CREATE INDEX idx_deals_document_ids ON deals USING GIN(document_ids) WHERE document_ids IS NOT NULL;

-- Deal payment indexes
CREATE INDEX idx_deal_payments_deal_id ON deal_payments(deal_id);
CREATE INDEX idx_deal_payments_payer_id ON deal_payments(payer_id);
CREATE INDEX idx_deal_payments_processed_at ON deal_payments(processed_at);
CREATE INDEX idx_deal_payments_transaction_hash ON deal_payments(transaction_hash) WHERE transaction_hash IS NOT NULL;

-- Deal listing indexes
CREATE INDEX idx_deal_listings_status_listed ON deal_listings(status, listed_at);
CREATE INDEX idx_deal_listings_seller ON deal_listings(seller_id);
CREATE INDEX idx_deal_listings_price ON deal_listings(asking_price);
CREATE INDEX idx_deal_listings_expires ON deal_listings(expires_at) WHERE expires_at IS NOT NULL;

-- Deal transfer indexes
CREATE INDEX idx_deal_transfers_deal ON deal_transfers(deal_id);
CREATE INDEX idx_deal_transfers_from_investor ON deal_transfers(from_investor_id);
CREATE INDEX idx_deal_transfers_to_investor ON deal_transfers(to_investor_id);
CREATE INDEX idx_deal_transfers_status ON deal_transfers(status, transfer_date);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC TIMESTAMP UPDATES
-- ============================================================================

-- Triggers for updated_at
CREATE TRIGGER update_deals_updated_at BEFORE UPDATE ON deals FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_deal_listings_updated_at BEFORE UPDATE ON deal_listings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- DEAL PROCESSING FUNCTIONS
-- ============================================================================

-- Function to update deal balance after payment
CREATE OR REPLACE FUNCTION update_deal_balance_on_payment()
RETURNS TRIGGER AS $$
BEGIN
    -- Update outstanding balance on the deal
    UPDATE deals
    SET
        outstanding_balance = outstanding_balance - NEW.amount,
        status = CASE
            WHEN (outstanding_balance - NEW.amount) <= 0 THEN 'paid_off'
            ELSE status
        END,
        updated_at = NOW()
    WHERE id = NEW.deal_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update deal balances when payments are made
CREATE TRIGGER update_deal_on_payment
    AFTER INSERT ON deal_payments
    FOR EACH ROW
    EXECUTE FUNCTION update_deal_balance_on_payment();

-- ============================================================================
-- MARKETPLACE VIEW
-- ============================================================================

-- Create marketplace view for deals
CREATE VIEW marketplace_deals AS
SELECT
    -- Listing information
    dl.id as listing_id,
    dl.asking_price,
    dl.description as listing_description,
    dl.listed_at,
    dl.expires_at,
    dl.status as listing_status,

    -- Deal information
    d.id as deal_id,
    d.originator_id,
    d.investor_id,
    d.principal_amount,
    d.outstanding_balance,
    d.interest_rate_bps,
    d.status,
    d.originated_at,
    d.title,
    d.description as deal_description,
    d.risk_level,
    d.asset_class,
    d.region,
    d.minimum_investment,
    d.industry,

    -- Seller information
    u.email as seller_email,
    u.kyc_status as seller_kyc_status
FROM deal_listings dl
JOIN deals d ON dl.deal_id = d.id
JOIN users u ON dl.seller_id = u.id
WHERE dl.status = 'active'
  AND d.status = 'for_sale'
  AND u.kyc_status IN ('approved', 'pending');

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE deals IS 'Deal records with originator/investor relationships and lifecycle management';
COMMENT ON TABLE deal_payments IS 'Payment history and transaction records for deal servicing';
COMMENT ON TABLE deal_listings IS 'Marketplace listings for deals available for sale';
COMMENT ON TABLE deal_transfers IS 'Record of all deal ownership transfers';
COMMENT ON VIEW marketplace_deals IS 'Combined view of deals available in marketplace with seller info';

COMMENT ON COLUMN deals.status IS 'Deal lifecycle status: draft, active, paid_off, closed, defaulted, for_sale, transfer_pending';
COMMENT ON COLUMN deals.originator_id IS 'User who originates the deal (provides the asset)';
COMMENT ON COLUMN deals.investor_id IS 'User who provides capital for the deal';
COMMENT ON COLUMN deals.industry IS 'Industry sector classification for the deal';
COMMENT ON COLUMN deals.document_ids IS 'JSON array of document UUIDs associated with the deal';
COMMENT ON COLUMN deals.interest_rate_bps IS 'Interest rate in basis points (1/100th of 1%)';
COMMENT ON COLUMN deals.outstanding_balance IS 'Current remaining balance to be paid';
COMMENT ON COLUMN deal_transfers.from_investor_id IS 'Original investor transferring the deal';
COMMENT ON COLUMN deal_transfers.to_investor_id IS 'New investor receiving the deal';