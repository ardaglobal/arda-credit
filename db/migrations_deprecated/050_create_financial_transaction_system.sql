-- Migration: Create financial transaction system
-- Description: Add tables for tracking financial transactions separate from state transactions

BEGIN;

-- ============================================================================
-- ACCOUNT FINANCIAL TRANSACTIONS
-- ============================================================================

-- Track all financial deposits/withdrawals to user accounts from smart contract
CREATE TABLE account_financial_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    transaction_type VARCHAR(20) NOT NULL, -- DEPOSIT, WITHDRAWAL
    amount DECIMAL(20,6) NOT NULL,
    currency VARCHAR(10) DEFAULT 'USDC',
    transaction_hash VARCHAR(66) NOT NULL, -- Smart contract transaction hash
    blockchain_status VARCHAR(20) DEFAULT 'CONFIRMED', -- PENDING, CONFIRMED, FAILED
    block_number BIGINT,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT valid_account_transaction_type CHECK (
        transaction_type IN ('DEPOSIT', 'WITHDRAWAL')
    ),
    CONSTRAINT positive_amount CHECK (amount > 0),
    UNIQUE(transaction_hash)
);

-- ============================================================================
-- POOL FINANCIAL TRANSACTIONS
-- ============================================================================

-- Track financial movements in/out of liquidity pools
CREATE TABLE pool_financial_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    transaction_type VARCHAR(20) NOT NULL, -- FUND, WITHDRAW, INTEREST, RETURN
    amount DECIMAL(20,6) NOT NULL,
    currency VARCHAR(10) DEFAULT 'USDC',
    -- Link to state transaction if applicable
    state_transaction_id UUID, -- Can reference state_transactions(id) if this was triggered by a state change
    transaction_hash VARCHAR(66), -- Blockchain transaction hash if applicable
    description TEXT,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT valid_pool_transaction_type CHECK (
        transaction_type IN ('FUND', 'WITHDRAW', 'INTEREST', 'RETURN')
    ),
    CONSTRAINT positive_amount CHECK (amount > 0)
);

-- ============================================================================
-- CREDIT LINE FINANCIAL TRANSACTIONS
-- ============================================================================

-- Track financial movements in/out of credit lines
CREATE TABLE credit_line_financial_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credit_line_id UUID NOT NULL REFERENCES credit_lines(id) ON DELETE CASCADE,
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    transaction_type VARCHAR(20) NOT NULL, -- DRAW, PAYMENT, INTEREST, FEE
    amount DECIMAL(20,6) NOT NULL,
    currency VARCHAR(10) DEFAULT 'USDC',
    -- Link to state transaction if applicable
    state_transaction_id UUID, -- Can reference state_transactions(id) if this was triggered by a state change
    transaction_hash VARCHAR(66), -- Blockchain transaction hash if applicable
    description TEXT,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT valid_credit_line_transaction_type CHECK (
        transaction_type IN ('DRAW', 'PAYMENT', 'INTEREST', 'FEE', 'PENALTY')
    ),
    CONSTRAINT positive_amount CHECK (amount > 0)
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Account financial transactions indexes
CREATE INDEX idx_account_financial_transactions_user_id ON account_financial_transactions(user_id);
CREATE INDEX idx_account_financial_transactions_type ON account_financial_transactions(transaction_type);
CREATE INDEX idx_account_financial_transactions_hash ON account_financial_transactions(transaction_hash);
CREATE INDEX idx_account_financial_transactions_processed_at ON account_financial_transactions(processed_at DESC);

-- Pool financial transactions indexes
CREATE INDEX idx_pool_financial_transactions_pool_id ON pool_financial_transactions(pool_id);
CREATE INDEX idx_pool_financial_transactions_user_id ON pool_financial_transactions(user_id);
CREATE INDEX idx_pool_financial_transactions_type ON pool_financial_transactions(transaction_type);
CREATE INDEX idx_pool_financial_transactions_processed_at ON pool_financial_transactions(processed_at DESC);

-- Credit line financial transactions indexes
CREATE INDEX idx_credit_line_financial_transactions_credit_line_id ON credit_line_financial_transactions(credit_line_id);
CREATE INDEX idx_credit_line_financial_transactions_merchant_id ON credit_line_financial_transactions(merchant_id);
CREATE INDEX idx_credit_line_financial_transactions_type ON credit_line_financial_transactions(transaction_type);
CREATE INDEX idx_credit_line_financial_transactions_processed_at ON credit_line_financial_transactions(processed_at DESC);

-- ============================================================================
-- UPDATE EXISTING TABLES
-- ============================================================================

-- Add financial transaction tracking to user_balances
ALTER TABLE user_balances
ADD COLUMN IF NOT EXISTS last_financial_transaction_at TIMESTAMP WITH TIME ZONE;

-- Add financial transaction tracking to liquidity_pools
ALTER TABLE liquidity_pools
ADD COLUMN IF NOT EXISTS total_financial_volume DECIMAL(20,6) DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_financial_transaction_at TIMESTAMP WITH TIME ZONE;

-- Add financial transaction tracking to credit_lines
ALTER TABLE credit_lines
ADD COLUMN IF NOT EXISTS total_financial_volume DECIMAL(20,6) DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_financial_transaction_at TIMESTAMP WITH TIME ZONE;

-- ============================================================================
-- UNIFIED FINANCIAL TRANSACTION VIEW
-- ============================================================================

-- Combined view of all financial transactions for a user
CREATE VIEW user_financial_transactions AS
SELECT
    'ACCOUNT'::text as transaction_category,
    aft.id,
    aft.user_id,
    aft.transaction_type,
    aft.amount,
    aft.currency,
    aft.transaction_hash,
    aft.processed_at,
    NULL::uuid as pool_id,
    NULL::uuid as credit_line_id,
    ('Smart contract ' || LOWER(aft.transaction_type))::text as description
FROM account_financial_transactions aft

UNION ALL

SELECT
    'POOL'::text as transaction_category,
    pft.id,
    pft.user_id,
    pft.transaction_type,
    pft.amount,
    pft.currency,
    pft.transaction_hash,
    pft.processed_at,
    pft.pool_id,
    NULL::uuid as credit_line_id,
    COALESCE(pft.description, 'Pool ' || LOWER(pft.transaction_type))::text as description
FROM pool_financial_transactions pft

UNION ALL

SELECT
    'CREDIT_LINE'::text as transaction_category,
    clft.id,
    clft.merchant_id as user_id,
    clft.transaction_type,
    clft.amount,
    clft.currency,
    clft.transaction_hash,
    clft.processed_at,
    NULL::uuid as pool_id,
    clft.credit_line_id,
    COALESCE(clft.description, 'Credit line ' || LOWER(clft.transaction_type))::text as description
FROM credit_line_financial_transactions clft;

-- ============================================================================
-- TRIGGERS FOR UPDATED_AT TIMESTAMPS
-- ============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers for financial transaction tables
CREATE TRIGGER update_account_financial_transactions_updated_at
    BEFORE UPDATE ON account_financial_transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_pool_financial_transactions_updated_at
    BEFORE UPDATE ON pool_financial_transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_credit_line_financial_transactions_updated_at
    BEFORE UPDATE ON credit_line_financial_transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to get user's financial transaction summary
CREATE OR REPLACE FUNCTION get_user_financial_summary(p_user_id UUID)
RETURNS TABLE(
    total_deposits DECIMAL(20,6),
    total_withdrawals DECIMAL(20,6),
    net_account_flow DECIMAL(20,6),
    total_pool_funding DECIMAL(20,6),
    total_pool_withdrawals DECIMAL(20,6),
    total_credit_line_payments DECIMAL(20,6),
    total_credit_line_draws DECIMAL(20,6)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COALESCE(SUM(CASE WHEN aft.transaction_type = 'DEPOSIT' THEN aft.amount ELSE 0 END), 0) as total_deposits,
        COALESCE(SUM(CASE WHEN aft.transaction_type = 'WITHDRAWAL' THEN aft.amount ELSE 0 END), 0) as total_withdrawals,
        COALESCE(SUM(CASE WHEN aft.transaction_type = 'DEPOSIT' THEN aft.amount
                         WHEN aft.transaction_type = 'WITHDRAWAL' THEN -aft.amount ELSE 0 END), 0) as net_account_flow,
        COALESCE(SUM(CASE WHEN pft.transaction_type = 'FUND' THEN pft.amount ELSE 0 END), 0) as total_pool_funding,
        COALESCE(SUM(CASE WHEN pft.transaction_type = 'WITHDRAW' THEN pft.amount ELSE 0 END), 0) as total_pool_withdrawals,
        COALESCE(SUM(CASE WHEN clft.transaction_type = 'PAYMENT' THEN clft.amount ELSE 0 END), 0) as total_credit_line_payments,
        COALESCE(SUM(CASE WHEN clft.transaction_type = 'DRAW' THEN clft.amount ELSE 0 END), 0) as total_credit_line_draws
    FROM identities i
    LEFT JOIN account_financial_transactions aft ON aft.user_id = i.id
    LEFT JOIN pool_financial_transactions pft ON pft.user_id = i.id
    LEFT JOIN merchants m ON m.user_id = i.id
    LEFT JOIN credit_line_financial_transactions clft ON clft.merchant_id = m.id
    WHERE i.id = p_user_id;
END;
$$ LANGUAGE plpgsql;

COMMIT;

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- This migration creates the financial transaction system with:
-- 1. account_financial_transactions - Track smart contract deposits/withdrawals
-- 2. pool_financial_transactions - Track pool funding/withdrawals
-- 3. credit_line_financial_transactions - Track credit line draws/payments
-- 4. user_financial_transactions view - Unified view of all financial activity
-- 5. Helper functions for financial summaries
-- 6. Indexes for performance
-- 7. Triggers for automatic timestamp updates
