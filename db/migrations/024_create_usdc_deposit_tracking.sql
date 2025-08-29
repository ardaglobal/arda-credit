-- Migration: Create USDC deposit tracking tables
-- Description: Add tables for tracking USDC deposits from smart contract events

BEGIN;

-- Table for tracking USDC deposits from smart contract events
CREATE TABLE usdc_deposits (
    id SERIAL PRIMARY KEY,
    -- Deposit details from smart contract event
    user_address VARCHAR(42) NOT NULL, -- Ethereum address (0x + 40 chars)
    amount NUMERIC(78, 0) NOT NULL, -- U256 amount in USDC (6 decimals)
    deposit_amount_usdc NUMERIC(20, 6) GENERATED ALWAYS AS (amount::NUMERIC / 1000000) STORED, -- Human readable USDC amount
    transaction_hash VARCHAR(66) NOT NULL, -- Transaction hash from event
    event_transaction_hash VARCHAR(66) NOT NULL, -- Transaction hash where event was emitted
    
    -- Block and timestamp information
    block_number BIGINT NOT NULL,
    event_timestamp BIGINT NOT NULL, -- Timestamp from contract event
    block_timestamp TIMESTAMP WITH TIME ZONE, -- Actual block timestamp (filled by indexer)
    
    -- Event metadata
    log_index BIGINT NOT NULL,
    event_signature VARCHAR(66) NOT NULL DEFAULT '0x...', -- Event signature for verification
    
    -- Processing status
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, confirmed, processed, failed
    confirmation_count INTEGER NOT NULL DEFAULT 0,
    required_confirmations INTEGER NOT NULL DEFAULT 12,
    
    -- Database tracking
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    
    -- Ensure uniqueness of events
    CONSTRAINT unique_deposit_event UNIQUE (event_transaction_hash, log_index),
    -- Ensure valid status values
    CONSTRAINT valid_deposit_status CHECK (status IN ('pending', 'confirmed', 'processed', 'failed')),
    -- Ensure positive amount
    CONSTRAINT positive_amount CHECK (amount > 0),
    -- Ensure valid confirmation count
    CONSTRAINT valid_confirmations CHECK (confirmation_count >= 0 AND confirmation_count <= required_confirmations + 10)
);

-- Table for tracking user deposit balances (aggregated view)
CREATE TABLE user_deposit_balances (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL UNIQUE,
    total_deposited NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Total USDC deposited (in wei)
    total_deposited_usdc NUMERIC(20, 6) GENERATED ALWAYS AS (total_deposited::NUMERIC / 1000000) STORED, -- Human readable
    deposit_count INTEGER NOT NULL DEFAULT 0,
    first_deposit_at TIMESTAMP WITH TIME ZONE,
    last_deposit_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Ensure positive values
    CONSTRAINT positive_total_deposited CHECK (total_deposited >= 0),
    CONSTRAINT positive_deposit_count CHECK (deposit_count >= 0)
);

-- Table for tracking deposit processing errors and retry attempts
CREATE TABLE deposit_processing_log (
    id SERIAL PRIMARY KEY,
    deposit_id INTEGER REFERENCES usdc_deposits(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL, -- confirmation_check, balance_update, retry, etc.
    status VARCHAR(20) NOT NULL, -- success, error, retry
    message TEXT,
    error_details JSONB,
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Ensure valid values
    CONSTRAINT valid_log_status CHECK (status IN ('success', 'error', 'retry', 'skipped'))
);

-- Indexes for efficient querying
CREATE INDEX idx_usdc_deposits_user_address ON usdc_deposits(user_address);
CREATE INDEX idx_usdc_deposits_block_number ON usdc_deposits(block_number);
CREATE INDEX idx_usdc_deposits_status ON usdc_deposits(status);
CREATE INDEX idx_usdc_deposits_created_at ON usdc_deposits(created_at);
CREATE INDEX idx_usdc_deposits_transaction_hash ON usdc_deposits(transaction_hash);
CREATE INDEX idx_usdc_deposits_event_tx_hash ON usdc_deposits(event_transaction_hash);

-- Indexes for user balances
CREATE INDEX idx_user_deposit_balances_address ON user_deposit_balances(user_address);
CREATE INDEX idx_user_deposit_balances_total ON user_deposit_balances(total_deposited);
CREATE INDEX idx_user_deposit_balances_last_deposit ON user_deposit_balances(last_deposit_at);

-- Indexes for processing log
CREATE INDEX idx_deposit_processing_log_deposit_id ON deposit_processing_log(deposit_id);
CREATE INDEX idx_deposit_processing_log_status ON deposit_processing_log(status);
CREATE INDEX idx_deposit_processing_log_next_retry ON deposit_processing_log(next_retry_at) WHERE next_retry_at IS NOT NULL;
CREATE INDEX idx_deposit_processing_log_created_at ON deposit_processing_log(created_at);

-- Function to update user deposit balances when deposits are confirmed
CREATE OR REPLACE FUNCTION update_user_deposit_balance()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process when status changes to 'confirmed' or 'processed'
    IF NEW.status IN ('confirmed', 'processed') AND (OLD.status IS DISTINCT FROM NEW.status) THEN
        INSERT INTO user_deposit_balances (
            user_address, 
            total_deposited, 
            deposit_count, 
            first_deposit_at, 
            last_deposit_at,
            updated_at
        ) VALUES (
            NEW.user_address,
            NEW.amount,
            1,
            COALESCE(NEW.block_timestamp, NOW()),
            COALESCE(NEW.block_timestamp, NOW()),
            NOW()
        )
        ON CONFLICT (user_address) DO UPDATE SET
            total_deposited = user_deposit_balances.total_deposited + NEW.amount,
            deposit_count = user_deposit_balances.deposit_count + 1,
            last_deposit_at = GREATEST(user_deposit_balances.last_deposit_at, COALESCE(NEW.block_timestamp, NOW())),
            updated_at = NOW();
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update user balances
CREATE TRIGGER trigger_update_user_deposit_balance
    AFTER UPDATE ON usdc_deposits
    FOR EACH ROW
    EXECUTE FUNCTION update_user_deposit_balance();

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers to update updated_at columns
CREATE TRIGGER trigger_usdc_deposits_updated_at
    BEFORE UPDATE ON usdc_deposits
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_user_deposit_balances_updated_at
    BEFORE UPDATE ON user_deposit_balances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

COMMIT;