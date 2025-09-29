-- ============================================================================
-- PAYMENTS AND BALANCES
-- User balance tracking, deposits, withdrawals, and liquidity pools
-- ============================================================================

-- ============================================================================
-- USER BALANCES
-- ============================================================================

-- Main user balance tracking table
CREATE TABLE user_balances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    balance_type VARCHAR(20) NOT NULL DEFAULT 'available' CHECK (balance_type IN ('available', 'locked', 'reserved')),
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0 CHECK (amount >= 0),
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(user_id, balance_type, currency)
);

-- User deposit balance tracking (specific to wallet addresses)
CREATE TABLE user_deposit_balances (
    user_address VARCHAR(42) PRIMARY KEY, -- Ethereum address
    available_balance DECIMAL(18, 6) NOT NULL DEFAULT 0,
    pending_withdrawals DECIMAL(18, 6) NOT NULL DEFAULT 0,
    lifetime_deposits DECIMAL(18, 6) NOT NULL DEFAULT 0,
    lifetime_withdrawals DECIMAL(18, 6) NOT NULL DEFAULT 0,
    lifetime_earnings DECIMAL(18, 6) NOT NULL DEFAULT 0,
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT positive_balances CHECK (available_balance >= 0 AND pending_withdrawals >= 0)
);

-- ============================================================================
-- USDC DEPOSITS FROM SMART CONTRACT
-- ============================================================================

-- Table for tracking USDC deposits from blockchain
CREATE TABLE usdc_deposits (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL, -- Ethereum wallet address
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0), -- Amount in USDC (6 decimals)
    transaction_hash VARCHAR(66) NOT NULL UNIQUE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'processed', 'failed')),
    confirmation_count INTEGER DEFAULT 0,
    block_number BIGINT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE
);

-- ============================================================================
-- USDC WITHDRAWALS
-- ============================================================================

-- Withdrawal requests table
CREATE TABLE withdrawal_requests (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL,
    recipient_address VARCHAR(42) NOT NULL,
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'submitted', 'confirmed', 'failed', 'cancelled')),
    transaction_hash VARCHAR(66), -- Set when submitted to blockchain
    failure_reason TEXT,
    requested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT valid_addresses CHECK (user_address != recipient_address)
);

-- Withdrawal state history for audit trail
CREATE TABLE withdrawal_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id INTEGER NOT NULL REFERENCES withdrawal_requests(id),
    previous_status VARCHAR(20),
    new_status VARCHAR(20) NOT NULL,
    changed_by VARCHAR(100), -- System or user identifier
    change_reason TEXT,
    changed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- LIQUIDITY POOLS
-- ============================================================================

-- Main liquidity pools table
CREATE TABLE liquidity_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    creator_id UUID NOT NULL REFERENCES users(id),

    -- Pool configuration and terms
    max_ltv_ratio DECIMAL(5, 4) NOT NULL CHECK (max_ltv_ratio > 0 AND max_ltv_ratio <= 1), -- e.g., 0.8000 for 80%
    max_interest_rate DECIMAL(6, 4) NOT NULL CHECK (max_interest_rate > 0), -- e.g., 0.1200 for 12%
    repayment_period_days INTEGER NOT NULL CHECK (repayment_period_days > 0),

    -- Pool state and limits
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'PAUSED', 'CLOSED')),
    total_deposits DECIMAL(18, 6) NOT NULL DEFAULT 0,
    available_liquidity DECIMAL(18, 6) NOT NULL DEFAULT 0,
    utilized_liquidity DECIMAL(18, 6) NOT NULL DEFAULT 0,

    -- Borough-based lending limits (geographic restrictions)
    borough VARCHAR(100),
    max_borough_exposure DECIMAL(18, 6), -- Maximum exposure per borough

    -- Pool metadata
    description TEXT,
    terms_and_conditions TEXT,

    -- Audit timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Pool funding transactions
CREATE TABLE pool_funding_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id),
    investor_id UUID NOT NULL REFERENCES users(id),
    transaction_type VARCHAR(20) NOT NULL CHECK (transaction_type IN ('deposit', 'withdrawal')),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    transaction_hash VARCHAR(66), -- On-chain transaction reference
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- CREDIT LINE SYSTEM (for merchant financing)
-- ============================================================================

-- Credit lines extended to merchants
CREATE TABLE credit_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id), -- References users where user_type = merchant
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id),

    -- Credit terms
    credit_limit DECIMAL(18, 6) NOT NULL CHECK (credit_limit > 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0),
    ltv_ratio DECIMAL(5, 4) NOT NULL CHECK (ltv_ratio > 0 AND ltv_ratio <= 1),
    repayment_period_days INTEGER NOT NULL CHECK (repayment_period_days > 0),

    -- Current state
    outstanding_balance DECIMAL(18, 6) NOT NULL DEFAULT 0,
    available_credit DECIMAL(18, 6) NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'closed')),

    -- Credit line lifecycle
    approved_at TIMESTAMP WITH TIME ZONE,
    activated_at TIMESTAMP WITH TIME ZONE,
    expires_at TIMESTAMP WITH TIME ZONE,

    -- Audit timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Credit line transactions (draws and repayments)
CREATE TABLE credit_line_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credit_line_id UUID NOT NULL REFERENCES credit_lines(id),
    transaction_type VARCHAR(20) NOT NULL CHECK (transaction_type IN ('draw', 'repayment', 'fee', 'interest')),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    outstanding_balance_after DECIMAL(18, 6) NOT NULL,
    transaction_hash VARCHAR(66), -- On-chain transaction reference
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- FINANCIAL TRANSACTION SYSTEM (consolidated view)
-- ============================================================================

-- All financial transactions consolidated view
CREATE VIEW financial_transactions AS
SELECT
    'deposit'::VARCHAR(20) as transaction_type,
    d.user_address as user_address,
    NULL::UUID as user_id,
    d.amount,
    d.transaction_hash,
    d.status,
    d.created_at,
    d.processed_at,
    'usdc_deposits'::VARCHAR(50) as source_table
FROM usdc_deposits d

UNION ALL

SELECT
    'withdrawal'::VARCHAR(20) as transaction_type,
    wr.user_address,
    NULL::UUID as user_id,
    wr.amount,
    wr.transaction_hash,
    wr.status,
    wr.created_at,
    wr.processed_at,
    'withdrawal_requests'::VARCHAR(50) as source_table
FROM withdrawal_requests wr

UNION ALL

SELECT
    pft.transaction_type,
    NULL::VARCHAR(42) as user_address,
    pft.investor_id as user_id,
    pft.amount,
    pft.transaction_hash,
    'processed'::VARCHAR(20) as status,
    pft.created_at,
    pft.processed_at,
    'pool_funding_transactions'::VARCHAR(50) as source_table
FROM pool_funding_transactions pft

UNION ALL

SELECT
    clt.transaction_type,
    NULL::VARCHAR(42) as user_address,
    cl.merchant_id as user_id,
    clt.amount,
    clt.transaction_hash,
    'processed'::VARCHAR(20) as status,
    clt.created_at,
    clt.processed_at,
    'credit_line_transactions'::VARCHAR(50) as source_table
FROM credit_line_transactions clt
JOIN credit_lines cl ON clt.credit_line_id = cl.id;

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- User balance indexes
CREATE INDEX idx_user_balances_user_id ON user_balances(user_id);
CREATE INDEX idx_user_balances_type_currency ON user_balances(balance_type, currency);
CREATE INDEX idx_user_balances_updated_at ON user_balances(updated_at);

-- Deposit balance indexes
CREATE INDEX idx_user_deposit_balances_updated ON user_deposit_balances(last_updated);

-- USDC deposit indexes
CREATE INDEX idx_usdc_deposits_user_address ON usdc_deposits(user_address);
CREATE INDEX idx_usdc_deposits_status ON usdc_deposits(status);
CREATE INDEX idx_usdc_deposits_block_number ON usdc_deposits(block_number);
CREATE INDEX idx_usdc_deposits_created_at ON usdc_deposits(created_at);

-- Withdrawal indexes
CREATE INDEX idx_withdrawal_requests_user_address ON withdrawal_requests(user_address);
CREATE INDEX idx_withdrawal_requests_status ON withdrawal_requests(status);
CREATE INDEX idx_withdrawal_requests_created_at ON withdrawal_requests(created_at);
CREATE INDEX idx_withdrawal_history_request_id ON withdrawal_history(request_id);

-- Liquidity pool indexes
CREATE INDEX idx_liquidity_pools_creator_id ON liquidity_pools(creator_id);
CREATE INDEX idx_liquidity_pools_status ON liquidity_pools(status);
CREATE INDEX idx_liquidity_pools_borough ON liquidity_pools(borough) WHERE borough IS NOT NULL;

-- Pool funding indexes
CREATE INDEX idx_pool_funding_pool_id ON pool_funding_transactions(pool_id);
CREATE INDEX idx_pool_funding_investor_id ON pool_funding_transactions(investor_id);
CREATE INDEX idx_pool_funding_type ON pool_funding_transactions(transaction_type);

-- Credit line indexes
CREATE INDEX idx_credit_lines_merchant_id ON credit_lines(merchant_id);
CREATE INDEX idx_credit_lines_pool_id ON credit_lines(pool_id);
CREATE INDEX idx_credit_lines_status ON credit_lines(status);
CREATE INDEX idx_credit_line_transactions_credit_line_id ON credit_line_transactions(credit_line_id);

-- ============================================================================
-- TRIGGERS AND FUNCTIONS
-- ============================================================================

-- Trigger to create initial balances when user is created
CREATE OR REPLACE FUNCTION create_initial_user_balances()
RETURNS TRIGGER AS $$
BEGIN
    -- Create available balance entry
    INSERT INTO user_balances (user_id, balance_type, amount, currency)
    VALUES (NEW.id, 'available', 0, 'USDC');

    -- Create locked balance entry
    INSERT INTO user_balances (user_id, balance_type, amount, currency)
    VALUES (NEW.id, 'locked', 0, 'USDC');

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to create balances when user is created
CREATE TRIGGER create_balances_on_user_insert
    AFTER INSERT ON users
    FOR EACH ROW
    EXECUTE FUNCTION create_initial_user_balances();

-- Function to update deposit balances from blockchain deposits
CREATE OR REPLACE FUNCTION update_deposit_balance_from_usdc()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process confirmed deposits
    IF NEW.status = 'confirmed' AND OLD.status != 'confirmed' THEN
        -- Upsert user deposit balance
        INSERT INTO user_deposit_balances (
            user_address,
            available_balance,
            lifetime_deposits,
            last_updated
        ) VALUES (
            NEW.user_address,
            NEW.amount,
            NEW.amount,
            NOW()
        )
        ON CONFLICT (user_address) DO UPDATE SET
            available_balance = user_deposit_balances.available_balance + NEW.amount,
            lifetime_deposits = user_deposit_balances.lifetime_deposits + NEW.amount,
            last_updated = NOW();
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update balances when USDC deposit is confirmed
CREATE TRIGGER update_balance_on_deposit_confirmed
    AFTER UPDATE ON usdc_deposits
    FOR EACH ROW
    EXECUTE FUNCTION update_deposit_balance_from_usdc();

-- Trigger for updated_at timestamps
CREATE TRIGGER update_user_balances_updated_at BEFORE UPDATE ON user_balances FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_liquidity_pools_updated_at BEFORE UPDATE ON liquidity_pools FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_credit_lines_updated_at BEFORE UPDATE ON credit_lines FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE user_balances IS 'User balance tracking for different balance types and currencies';
COMMENT ON TABLE user_deposit_balances IS 'Wallet-specific deposit balance tracking with lifetime statistics';
COMMENT ON TABLE usdc_deposits IS 'USDC deposits from smart contract with confirmation tracking';
COMMENT ON TABLE withdrawal_requests IS 'User withdrawal requests with approval workflow';
COMMENT ON TABLE withdrawal_history IS 'Audit trail for withdrawal status changes';
COMMENT ON TABLE liquidity_pools IS 'Investor-created liquidity pools for credit line funding';
COMMENT ON TABLE pool_funding_transactions IS 'Deposit and withdrawal transactions for liquidity pools';
COMMENT ON TABLE credit_lines IS 'Credit lines extended to merchants from liquidity pools';
COMMENT ON TABLE credit_line_transactions IS 'Credit line draw and repayment transactions';
COMMENT ON VIEW financial_transactions IS 'Consolidated view of all financial transactions across the system';

COMMENT ON COLUMN user_balances.balance_type IS 'Balance type: available (spendable), locked (reserved), reserved (pending)';
COMMENT ON COLUMN usdc_deposits.amount IS 'Amount in USDC with 6 decimal places (matching USDC token standard)';
COMMENT ON COLUMN liquidity_pools.max_ltv_ratio IS 'Maximum loan-to-value ratio (decimal: 0.8000 = 80%)';
COMMENT ON COLUMN liquidity_pools.max_interest_rate IS 'Maximum interest rate (decimal: 0.1200 = 12% APR)';
COMMENT ON COLUMN credit_lines.ltv_ratio IS 'Loan-to-value ratio for this specific credit line';
COMMENT ON COLUMN credit_lines.interest_rate_bps IS 'Interest rate in basis points (1200 = 12% APR)';