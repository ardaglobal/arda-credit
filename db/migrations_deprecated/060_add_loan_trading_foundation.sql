-- ============================================================================
-- LOAN TRADING FOUNDATION MIGRATION
-- Creates core tables and schema extensions for loan marketplace and transfers
--
-- This migration provides the foundation for:
-- - Worker A: Marketplace listings and browsing
-- - Worker B: Loan transfers and payment settlement
-- ============================================================================

-- 1. Create loan listings table for marketplace
CREATE TABLE loan_listings (
    listing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(id),
    seller_id UUID NOT NULL REFERENCES identities(id),  -- Note: using identities table
    asking_price DECIMAL(15,2) NOT NULL CHECK (asking_price > 0),
    description TEXT,
    listed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,  -- Optional expiration for listings
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'sold', 'cancelled', 'expired')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT loan_listings_unique_loan UNIQUE(loan_id)  -- Only one active listing per loan
);

-- 2. Create loan transfers table for ownership changes
CREATE TABLE loan_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(id),
    from_lender_id UUID NOT NULL REFERENCES identities(id),
    to_lender_id UUID NOT NULL REFERENCES identities(id),
    purchase_price DECIMAL(15,2) NOT NULL CHECK (purchase_price > 0),
    transfer_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (transfer_status IN ('pending', 'completed', 'failed', 'cancelled')),
    transfer_date TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,  -- When transfer actually completed
    notes TEXT,  -- Optional transfer notes
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT loan_transfers_no_self_transfer CHECK (from_lender_id != to_lender_id)  -- Cannot transfer to self
);

-- Create indexes for loan_listings table
CREATE INDEX idx_loan_listings_status_listed ON loan_listings(status, listed_at);
CREATE INDEX idx_loan_listings_seller ON loan_listings(seller_id);
CREATE INDEX idx_loan_listings_price ON loan_listings(asking_price);

-- Create indexes for loan_transfers table
CREATE INDEX idx_loan_transfers_loan ON loan_transfers(loan_id);
CREATE INDEX idx_loan_transfers_from_lender ON loan_transfers(from_lender_id);
CREATE INDEX idx_loan_transfers_to_lender ON loan_transfers(to_lender_id);
CREATE INDEX idx_loan_transfers_status ON loan_transfers(transfer_status, transfer_date);

-- 3. Extend loan status enum (CRITICAL: Additive only - keep existing values)
-- Current values: originated, active, paid_off, closed, defaulted
-- Adding: for_sale, transfer_pending
ALTER TABLE loans DROP CONSTRAINT IF EXISTS loans_loan_status_check;
ALTER TABLE loans ADD CONSTRAINT loans_loan_status_check CHECK (loan_status IN (
    -- Existing statuses (DO NOT CHANGE)
    'originated', 'active', 'paid_off', 'closed', 'defaulted',
    -- New marketplace statuses
    'for_sale', 'transfer_pending'
));

-- 4. Add marketplace-friendly fields to loans table (all optional to avoid breaking existing data)
ALTER TABLE loans ADD COLUMN IF NOT EXISTS title VARCHAR(255);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS description TEXT;
ALTER TABLE loans ADD COLUMN IF NOT EXISTS risk_level VARCHAR(10) CHECK (risk_level IN ('low', 'medium', 'high'));
ALTER TABLE loans ADD COLUMN IF NOT EXISTS asset_class VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS region VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS minimum_investment DECIMAL(15,2);

-- 5. Add performance indexes for marketplace queries
CREATE INDEX IF NOT EXISTS idx_loans_marketplace ON loans(loan_status, originated_at) WHERE loan_status = 'for_sale';
CREATE INDEX IF NOT EXISTS idx_loans_lender_status ON loans(lender_id, loan_status);

-- 6. Add trigger to update updated_at timestamp on loan_listings
CREATE OR REPLACE FUNCTION update_loan_listings_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_loan_listings_updated_at
    BEFORE UPDATE ON loan_listings
    FOR EACH ROW EXECUTE PROCEDURE update_loan_listings_updated_at();

-- 7. Create helpful view for combined marketplace data (used by both workers)
CREATE VIEW marketplace_loans AS
SELECT
    -- Listing information
    ll.listing_id,
    ll.asking_price,
    ll.description as listing_description,
    ll.listed_at,
    ll.expires_at,
    ll.status as listing_status,

    -- Loan information
    l.id as loan_id,
    l.borrower_id,
    l.lender_id,
    l.principal_amount,
    l.outstanding_balance,
    l.interest_rate_bps,
    l.loan_status,
    l.originated_at,
    l.title,
    l.description as loan_description,
    l.risk_level,
    l.asset_class,
    l.region,
    l.minimum_investment,

    -- Seller information
    i.email as seller_email,
    i.kyc_status as seller_kyc_status
FROM loan_listings ll
JOIN loans l ON ll.loan_id = l.id
JOIN identities i ON ll.seller_id = i.id
WHERE ll.status = 'active'
  AND l.loan_status = 'for_sale'
  AND i.kyc_status IN ('approved', 'pending', 'submitted'); -- Allow for testing flexibility

-- 8. Add comments for documentation
COMMENT ON TABLE loan_listings IS 'Marketplace listings for loans available for sale';
COMMENT ON TABLE loan_transfers IS 'Record of all loan ownership transfers';
COMMENT ON VIEW marketplace_loans IS 'Combined view of loans available in marketplace with seller info';

-- 9. Grant permissions (adjust based on your user setup)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON loan_listings TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON loan_transfers TO your_app_user;
-- GRANT SELECT ON marketplace_loans TO your_app_user;
