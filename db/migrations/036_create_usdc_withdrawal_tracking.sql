-- Migration: Create USDC withdrawal tracking tables
-- Description: Add tables for tracking USDC withdrawals from smart contract events

BEGIN;

-- Table for tracking USDC withdrawals from smart contract events
CREATE TABLE usdc_withdrawals (
    id SERIAL PRIMARY KEY,
    -- Withdrawal details from smart contract event
    user_address VARCHAR(42) NOT NULL, -- Ethereum address (0x + 40 chars)
    recipient_address VARCHAR(42) NOT NULL, -- Recipient address for withdrawal
    amount NUMERIC(78, 0) NOT NULL, -- U256 amount in USDC (6 decimals)
    withdrawal_amount_usdc NUMERIC(20, 6) GENERATED ALWAYS AS (amount::NUMERIC / 1000000) STORED, -- Human readable USDC amount
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
    CONSTRAINT unique_withdrawal_event UNIQUE (event_transaction_hash, log_index),
    -- Ensure valid status values
    CONSTRAINT valid_withdrawal_status CHECK (status IN ('pending', 'confirmed', 'processed', 'failed')),
    -- Ensure positive amount
    CONSTRAINT positive_amount CHECK (amount > 0),
    -- Ensure valid confirmation count
    CONSTRAINT valid_confirmations CHECK (confirmation_count >= 0 AND confirmation_count <= required_confirmations + 10)
);

-- Table for tracking user withdrawal balances (aggregated view)
CREATE TABLE user_withdrawal_balances (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL UNIQUE,
    total_withdrawn NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Total USDC withdrawn (in wei)
    total_withdrawn_usdc NUMERIC(20, 6) GENERATED ALWAYS AS (total_withdrawn::NUMERIC / 1000000) STORED, -- Human readable
    withdrawal_count INTEGER NOT NULL DEFAULT 0,
    first_withdrawal_at TIMESTAMP WITH TIME ZONE,
    last_withdrawal_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Ensure positive values
    CONSTRAINT positive_total_withdrawn CHECK (total_withdrawn >= 0),
    CONSTRAINT positive_withdrawal_count CHECK (withdrawal_count >= 0)
);

-- Table for tracking withdrawal processing errors and retry attempts
CREATE TABLE withdrawal_processing_log (
    id SERIAL PRIMARY KEY,
    withdrawal_id INTEGER REFERENCES usdc_withdrawals(id) ON DELETE CASCADE,
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
CREATE INDEX idx_usdc_withdrawals_user_address ON usdc_withdrawals(user_address);
CREATE INDEX idx_usdc_withdrawals_recipient_address ON usdc_withdrawals(recipient_address);
CREATE INDEX idx_usdc_withdrawals_block_number ON usdc_withdrawals(block_number);
CREATE INDEX idx_usdc_withdrawals_status ON usdc_withdrawals(status);
CREATE INDEX idx_usdc_withdrawals_created_at ON usdc_withdrawals(created_at);
CREATE INDEX idx_usdc_withdrawals_transaction_hash ON usdc_withdrawals(transaction_hash);
CREATE INDEX idx_usdc_withdrawals_event_tx_hash ON usdc_withdrawals(event_transaction_hash);

-- Indexes for user withdrawal balances
CREATE INDEX idx_user_withdrawal_balances_address ON user_withdrawal_balances(user_address);
CREATE INDEX idx_user_withdrawal_balances_total ON user_withdrawal_balances(total_withdrawn);
CREATE INDEX idx_user_withdrawal_balances_last_withdrawal ON user_withdrawal_balances(last_withdrawal_at);

-- Indexes for processing log
CREATE INDEX idx_withdrawal_processing_log_withdrawal_id ON withdrawal_processing_log(withdrawal_id);
CREATE INDEX idx_withdrawal_processing_log_status ON withdrawal_processing_log(status);
CREATE INDEX idx_withdrawal_processing_log_next_retry ON withdrawal_processing_log(next_retry_at) WHERE next_retry_at IS NOT NULL;
CREATE INDEX idx_withdrawal_processing_log_created_at ON withdrawal_processing_log(created_at);

-- Function to update user withdrawal balances when withdrawals are confirmed
CREATE OR REPLACE FUNCTION update_user_withdrawal_balance()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process when status changes to 'confirmed' or 'processed'
    IF NEW.status IN ('confirmed', 'processed') AND (OLD.status IS DISTINCT FROM NEW.status) THEN
        INSERT INTO user_withdrawal_balances (
            user_address, 
            total_withdrawn, 
            withdrawal_count, 
            first_withdrawal_at, 
            last_withdrawal_at,
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
            total_withdrawn = user_withdrawal_balances.total_withdrawn + NEW.amount,
            withdrawal_count = user_withdrawal_balances.withdrawal_count + 1,
            last_withdrawal_at = GREATEST(user_withdrawal_balances.last_withdrawal_at, COALESCE(NEW.block_timestamp, NOW())),
            updated_at = NOW();
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update user withdrawal balances
CREATE TRIGGER trigger_update_user_withdrawal_balance
    AFTER UPDATE ON usdc_withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION update_user_withdrawal_balance();

-- Triggers to update updated_at columns
CREATE TRIGGER trigger_usdc_withdrawals_updated_at
    BEFORE UPDATE ON usdc_withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_user_withdrawal_balances_updated_at
    BEFORE UPDATE ON user_withdrawal_balances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

COMMIT;