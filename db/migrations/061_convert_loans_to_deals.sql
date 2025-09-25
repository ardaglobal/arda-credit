-- ============================================================================
-- LOAN TO DEAL CONVERSION MIGRATION
-- Hard cutover conversion - drops old tables, creates new deal tables
-- ============================================================================

-- ============================================================================
-- DROP OLD LOAN TABLES (HARD CUTOVER)
-- ============================================================================

-- Drop old loan tables in dependency order (reverse of creation)
DROP TABLE IF EXISTS loan_transfers CASCADE;
DROP TABLE IF EXISTS loan_listings CASCADE;
DROP TABLE IF EXISTS loan_transactions CASCADE;
DROP TABLE IF EXISTS payments CASCADE;
DROP TABLE IF EXISTS loans CASCADE;

-- Drop old marketplace view
DROP VIEW IF EXISTS marketplace_loans CASCADE;

-- ============================================================================
-- CREATE NEW DEAL TABLES
-- ============================================================================

-- Table for deal records and lifecycle management (converted from loans)
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    originator_id UUID NOT NULL REFERENCES identities(id),  -- was borrower_id
    investor_id UUID NOT NULL REFERENCES identities(id),    -- was lender_id
    principal_amount DECIMAL(18, 6) NOT NULL CHECK (principal_amount > 0),
    outstanding_balance DECIMAL(18, 6) NOT NULL CHECK (outstanding_balance >= 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0), -- Basis points (1/100th of 1%)
    loan_status VARCHAR(20) NOT NULL DEFAULT 'originated' CHECK (loan_status IN ('originated', 'active', 'paid_off', 'closed', 'defaulted', 'for_sale', 'transfer_pending')),
    originated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    maturity_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Marketplace-friendly fields (added in previous migration)
    title VARCHAR(255),
    description TEXT,
    risk_level VARCHAR(10) CHECK (risk_level IN ('low', 'medium', 'high')),
    asset_class VARCHAR(100),
    region VARCHAR(100),
    minimum_investment DECIMAL(15,2),

    CONSTRAINT originator_investor_different CHECK (originator_id != investor_id)
);

-- ============================================================================
-- PAYMENT AND BALANCE TABLES (converted from payments)
-- ============================================================================

-- Table for tracking payments made against deals
CREATE TABLE deal_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),  -- was loan_id
    payer_id UUID NOT NULL REFERENCES identities(id),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    payment_type VARCHAR(20) NOT NULL DEFAULT 'regular' CHECK (payment_type IN ('regular', 'principal', 'interest', 'penalty')),
    transaction_hash VARCHAR(66), -- Ethereum tx hash if on-chain
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STATE AND BATCH INTEGRATION TABLES (converted from loan_transactions)
-- ============================================================================

-- Table for deal transaction records (integrates with existing batch system)
CREATE TABLE deal_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id_hash BYTEA NOT NULL UNIQUE, -- SHA256 hash for zkVM compatibility
    user_id_hash BYTEA NOT NULL, -- SHA256 hash of user UUID
    deal_id_hash BYTEA, -- SHA256 hash of deal UUID, NULL for non-deal transactions (was loan_id_hash)
    transaction_type VARCHAR(30) NOT NULL CHECK (transaction_type IN (
        -- Existing transaction types (keep for backward compatibility)
        'user_registration', 'kyc_submission', 'kyc_verification',
        'user_deposit', 'user_withdrawal',
        'investor_registration', 'pool_creation',
        -- New deal transaction types
        'deal_origination', 'deal_payment', 'deal_closeout'
    )),
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0,

    -- Foreign key references to actual records
    identity_id UUID REFERENCES identities(id),
    deal_id UUID REFERENCES deals(id),  -- was loan_id
    payment_id UUID REFERENCES deal_payments(id),  -- was payment_id

    -- Batch processing integration
    included_in_batch_id INTEGER REFERENCES proof_batches(id),

    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- MARKETPLACE TABLES (converted from loan_listings and loan_transfers)
-- ============================================================================

-- Table for deal listings in marketplace
CREATE TABLE deal_listings (
    listing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),  -- was loan_id
    seller_id UUID NOT NULL REFERENCES identities(id),  -- Note: using identities table
    asking_price DECIMAL(15,2) NOT NULL CHECK (asking_price > 0),
    description TEXT,
    listed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,  -- Optional expiration for listings
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'sold', 'cancelled', 'expired')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT deal_listings_unique_deal UNIQUE(deal_id)  -- Only one active listing per deal
);

-- Table for deal ownership transfers
CREATE TABLE deal_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),  -- was loan_id
    from_investor_id UUID NOT NULL REFERENCES identities(id),  -- was from_lender_id
    to_investor_id UUID NOT NULL REFERENCES identities(id),    -- was to_lender_id
    purchase_price DECIMAL(15,2) NOT NULL CHECK (purchase_price > 0),
    transfer_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (transfer_status IN ('pending', 'completed', 'failed', 'cancelled')),
    transfer_date TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,  -- When transfer actually completed
    notes TEXT,  -- Optional transfer notes
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT deal_transfers_no_self_transfer CHECK (from_investor_id != to_investor_id)  -- Cannot transfer to self
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Deal indexes
CREATE INDEX idx_deals_originator_id ON deals(originator_id);
CREATE INDEX idx_deals_investor_id ON deals(investor_id);
CREATE INDEX idx_deals_status ON deals(loan_status);
CREATE INDEX idx_deals_originated_at ON deals(originated_at);
CREATE INDEX idx_deals_outstanding_balance ON deals(outstanding_balance) WHERE outstanding_balance > 0;
CREATE INDEX idx_deals_marketplace ON deals(loan_status, originated_at) WHERE loan_status = 'for_sale';
CREATE INDEX idx_deals_lender_status ON deals(investor_id, loan_status);

-- Deal payment indexes
CREATE INDEX idx_deal_payments_deal_id ON deal_payments(deal_id);
CREATE INDEX idx_deal_payments_payer_id ON deal_payments(payer_id);
CREATE INDEX idx_deal_payments_processed_at ON deal_payments(processed_at);
CREATE INDEX idx_deal_payments_transaction_hash ON deal_payments(transaction_hash) WHERE transaction_hash IS NOT NULL;

-- Deal transaction indexes
CREATE INDEX idx_deal_transactions_user_id_hash ON deal_transactions(user_id_hash);
CREATE INDEX idx_deal_transactions_deal_id_hash ON deal_transactions(deal_id_hash) WHERE deal_id_hash IS NOT NULL;
CREATE INDEX idx_deal_transactions_type ON deal_transactions(transaction_type);
CREATE INDEX idx_deal_transactions_batch_id ON deal_transactions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_deal_transactions_not_batched ON deal_transactions(id) WHERE included_in_batch_id IS NULL;

-- Deal listing indexes
CREATE INDEX idx_deal_listings_status_listed ON deal_listings(status, listed_at);
CREATE INDEX idx_deal_listings_seller ON deal_listings(seller_id);
CREATE INDEX idx_deal_listings_price ON deal_listings(asking_price);

-- Deal transfer indexes
CREATE INDEX idx_deal_transfers_deal ON deal_transfers(deal_id);
CREATE INDEX idx_deal_transfers_from_investor ON deal_transfers(from_investor_id);
CREATE INDEX idx_deal_transfers_to_investor ON deal_transfers(to_investor_id);
CREATE INDEX idx_deal_transfers_status ON deal_transfers(transfer_status, transfer_date);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC TIMESTAMP UPDATES
-- ============================================================================

-- Triggers for updated_at
CREATE TRIGGER update_deals_updated_at BEFORE UPDATE ON deals FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_deal_listings_updated_at BEFORE UPDATE ON deal_listings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- HELPER FUNCTIONS FOR DEAL PROCESSING
-- ============================================================================

-- Function to get deal transactions ready for batching
CREATE OR REPLACE FUNCTION get_unbatched_deal_transactions(limit_count INTEGER DEFAULT 10)
RETURNS TABLE(
    transaction_id UUID,
    transaction_id_hash BYTEA,
    user_id_hash BYTEA,
    deal_id_hash BYTEA,
    transaction_type VARCHAR,
    amount DECIMAL(18, 6),
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        dt.id,
        dt.transaction_id_hash,
        dt.user_id_hash,
        dt.deal_id_hash,
        dt.transaction_type,
        dt.amount,
        dt.created_at
    FROM deal_transactions dt
    WHERE dt.included_in_batch_id IS NULL
    ORDER BY dt.created_at ASC -- FIFO: oldest first
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to update deal balance after payment
CREATE OR REPLACE FUNCTION update_deal_balance_on_payment()
RETURNS TRIGGER AS $$
BEGIN
    -- Update outstanding balance on the deal
    UPDATE deals
    SET
        outstanding_balance = outstanding_balance - NEW.amount,
        loan_status = CASE
            WHEN (outstanding_balance - NEW.amount) <= 0 THEN 'paid_off'
            ELSE loan_status
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
-- INTEGRATION WITH EXISTING BATCH SYSTEM
-- ============================================================================

-- Update the existing create_batch function to handle deal transactions
CREATE OR REPLACE FUNCTION create_batch_with_deals(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    transaction_total INTEGER;
    transaction_id_array INTEGER[];
    deal_transaction_ids UUID[];
BEGIN
    -- Get current counter value
    SELECT get_current_counter_value() INTO previous_counter;

    -- Get unbatched arithmetic transactions and calculate total
    SELECT
        ARRAY_AGG(id ORDER BY id),
        SUM(amount)
    INTO transaction_id_array, transaction_total
    FROM (
        SELECT id, amount
        FROM incoming_transactions
        WHERE included_in_batch_id IS NULL
        ORDER BY id ASC
        LIMIT batch_size / 2  -- Reserve half for arithmetic, half for deals
    ) unbatched;

    -- Get unbatched deal transactions
    SELECT ARRAY_AGG(id ORDER BY created_at)
    INTO deal_transaction_ids
    FROM (
        SELECT id
        FROM deal_transactions
        WHERE included_in_batch_id IS NULL
        ORDER BY created_at ASC
        LIMIT batch_size / 2
    ) unbatched_deals;

    -- Return 0 if no transactions to batch
    IF (transaction_id_array IS NULL OR array_length(transaction_id_array, 1) = 0) AND
       (deal_transaction_ids IS NULL OR array_length(deal_transaction_ids, 1) = 0) THEN
        RETURN 0;
    END IF;

    -- Calculate final counter value (only arithmetic transactions affect counter)
    final_counter := previous_counter + COALESCE(transaction_total, 0);

    -- Create new batch
    INSERT INTO proof_batches (
        previous_counter_value,
        final_counter_value,
        transaction_ids
    ) VALUES (
        previous_counter,
        final_counter,
        COALESCE(transaction_id_array, ARRAY[]::INTEGER[])
    ) RETURNING id INTO new_batch_id;

    -- Mark arithmetic transactions as included in this batch
    IF transaction_id_array IS NOT NULL AND array_length(transaction_id_array, 1) > 0 THEN
        UPDATE incoming_transactions
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(transaction_id_array);
    END IF;

    -- Mark deal transactions as included in this batch
    IF deal_transaction_ids IS NOT NULL AND array_length(deal_transaction_ids, 1) > 0 THEN
        UPDATE deal_transactions
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(deal_transaction_ids);
    END IF;

    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- MARKETPLACE VIEW (converted from marketplace_loans)
-- ============================================================================

-- Create new marketplace view for deals
CREATE VIEW marketplace_deals AS
SELECT
    -- Listing information
    dl.listing_id,
    dl.asking_price,
    dl.description as listing_description,
    dl.listed_at,
    dl.expires_at,
    dl.status as listing_status,

    -- Deal information
    d.id as deal_id,
    d.originator_id,  -- was borrower_id
    d.investor_id,    -- was lender_id
    d.principal_amount,
    d.outstanding_balance,
    d.interest_rate_bps,
    d.loan_status,
    d.originated_at,
    d.title,
    d.description as deal_description,
    d.risk_level,
    d.asset_class,
    d.region,
    d.minimum_investment,

    -- Seller information
    i.email as seller_email,
    i.kyc_status as seller_kyc_status
FROM deal_listings dl
JOIN deals d ON dl.deal_id = d.id
JOIN identities i ON dl.seller_id = i.id
WHERE dl.status = 'active'
  AND d.loan_status = 'for_sale'
  AND i.kyc_status IN ('approved', 'pending', 'submitted'); -- Allow for testing flexibility

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE deals IS 'Deal records with originator/investor relationships and lifecycle management (converted from loans)';
COMMENT ON TABLE deal_payments IS 'Payment history and transaction records for deal servicing (converted from payments)';
COMMENT ON TABLE deal_transactions IS 'Deal transaction records for zkVM batch processing (converted from loan_transactions)';
COMMENT ON TABLE deal_listings IS 'Marketplace listings for deals available for sale (converted from loan_listings)';
COMMENT ON TABLE deal_transfers IS 'Record of all deal ownership transfers (converted from loan_transfers)';
COMMENT ON VIEW marketplace_deals IS 'Combined view of deals available in marketplace with seller info (converted from marketplace_loans)';

COMMENT ON COLUMN deals.originator_id IS 'User who originates the deal (was borrower_id)';
COMMENT ON COLUMN deals.investor_id IS 'User who provides capital for the deal (was lender_id)';
COMMENT ON COLUMN deal_transactions.deal_id_hash IS 'SHA256 hash of deal UUID for zkVM privacy (was loan_id_hash)';
COMMENT ON COLUMN deal_transfers.from_investor_id IS 'Original investor transferring the deal (was from_lender_id)';
COMMENT ON COLUMN deal_transfers.to_investor_id IS 'New investor receiving the deal (was to_lender_id)';

-- ============================================================================
-- PERMISSIONS (adjust based on your user setup)
-- ============================================================================

-- GRANT SELECT, INSERT, UPDATE, DELETE ON deals TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON deal_payments TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON deal_transactions TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON deal_listings TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON deal_transfers TO your_app_user;
-- GRANT SELECT ON marketplace_deals TO your_app_user;
