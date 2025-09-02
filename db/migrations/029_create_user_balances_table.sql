-- Migration: Create user_balances table
-- Description: Add table for tracking user balances by currency

BEGIN;

-- Table for tracking user balances by currency
CREATE TABLE user_balances (
    id SERIAL PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    available_balance NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Balance in wei/smallest unit
    reserved_balance NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Reserved for pending transactions
    total_deposited NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Total amount ever deposited
    deposit_count INTEGER NOT NULL DEFAULT 0, -- Number of deposits made
    first_deposit_at TIMESTAMP WITH TIME ZONE,
    last_deposit_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT positive_available_balance CHECK (available_balance >= 0),
    CONSTRAINT positive_reserved_balance CHECK (reserved_balance >= 0),
    CONSTRAINT positive_total_deposited CHECK (total_deposited >= 0),
    CONSTRAINT positive_deposit_count CHECK (deposit_count >= 0),
    CONSTRAINT valid_currency CHECK (currency IN ('USDC', 'USD', 'ETH')),
    
    -- Each user can only have one balance record per currency
    CONSTRAINT unique_user_currency UNIQUE (user_id, currency)
);

-- Indexes for efficient querying
CREATE INDEX idx_user_balances_user_id ON user_balances(user_id);
CREATE INDEX idx_user_balances_currency ON user_balances(currency);
CREATE INDEX idx_user_balances_available_balance ON user_balances(available_balance) WHERE available_balance > 0;
CREATE INDEX idx_user_balances_last_deposit ON user_balances(last_deposit_at) WHERE last_deposit_at IS NOT NULL;

-- Trigger to update updated_at timestamp
CREATE TRIGGER trigger_user_balances_updated_at
    BEFORE UPDATE ON user_balances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Function to initialize default balance for new users
CREATE OR REPLACE FUNCTION create_initial_user_balances()
RETURNS TRIGGER AS $$
BEGIN
    -- Create initial USDC balance record for new users
    INSERT INTO user_balances (user_id, currency, available_balance)
    VALUES (NEW.id, 'USDC', 0);
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to create initial balance when identity is created
CREATE TRIGGER create_balances_on_identity_insert_v2
    AFTER INSERT ON identities
    FOR EACH ROW
    EXECUTE FUNCTION create_initial_user_balances();

-- Table comments
COMMENT ON TABLE user_balances IS 'User balance tracking by currency with deposit history';
COMMENT ON COLUMN user_balances.available_balance IS 'Available balance for transactions (in smallest currency unit)';
COMMENT ON COLUMN user_balances.reserved_balance IS 'Balance reserved for pending transactions';
COMMENT ON COLUMN user_balances.total_deposited IS 'Cumulative total of all deposits ever made';

COMMIT;