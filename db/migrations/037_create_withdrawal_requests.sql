-- Migration: Create withdrawal requests table
-- Description: Add table for tracking user-initiated withdrawal requests with KYC validation and balance checks

BEGIN;

-- Table for tracking user withdrawal requests (different from blockchain events)
CREATE TABLE withdrawal_requests (
    id SERIAL PRIMARY KEY,
    -- User identification
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    user_address VARCHAR(42) NOT NULL, -- Ethereum address of the user
    recipient_address VARCHAR(42) NOT NULL, -- Where user wants to withdraw to
    
    -- Withdrawal details
    amount NUMERIC(78, 0) NOT NULL, -- Amount in USDC wei (6 decimals)
    amount_usdc NUMERIC(20, 6) GENERATED ALWAYS AS (amount::NUMERIC / 1000000) STORED, -- Human readable USDC amount
    
    -- Request state tracking
    state VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, validated, submitted, confirmed, failed, cancelled
    
    -- Validation results
    kyc_validated BOOLEAN NOT NULL DEFAULT FALSE,
    balance_validated BOOLEAN NOT NULL DEFAULT FALSE,
    kyc_status_at_request VARCHAR(20), -- KYC status when request was made
    available_balance_at_request NUMERIC(78, 0), -- Available balance when request was made
    
    -- Smart contract integration
    transaction_hash VARCHAR(66), -- Transaction hash when submitted to contract (null until submitted)
    block_number BIGINT, -- Block number when transaction was mined
    confirmation_count INTEGER NOT NULL DEFAULT 0,
    required_confirmations INTEGER NOT NULL DEFAULT 12,
    
    -- Error tracking
    error_message TEXT, -- Error details if request fails
    validation_errors JSONB, -- Structured validation error details
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    validated_at TIMESTAMP WITH TIME ZONE, -- When KYC and balance validation completed
    submitted_at TIMESTAMP WITH TIME ZONE, -- When submitted to smart contract
    confirmed_at TIMESTAMP WITH TIME ZONE, -- When confirmed on blockchain
    failed_at TIMESTAMP WITH TIME ZONE, -- When request failed
    cancelled_at TIMESTAMP WITH TIME ZONE, -- When user cancelled request
    
    -- Constraints
    CONSTRAINT positive_withdrawal_amount CHECK (amount > 0),
    CONSTRAINT valid_withdrawal_state CHECK (state IN ('pending', 'validated', 'submitted', 'confirmed', 'failed', 'cancelled')),
    CONSTRAINT valid_confirmation_count CHECK (confirmation_count >= 0 AND confirmation_count <= required_confirmations + 10),
    -- Can't have both error and success states
    CONSTRAINT no_error_when_confirmed CHECK (NOT (state = 'confirmed' AND error_message IS NOT NULL))
);

-- Table for tracking withdrawal request processing steps and history
CREATE TABLE withdrawal_request_history (
    id SERIAL PRIMARY KEY,
    withdrawal_request_id INTEGER NOT NULL REFERENCES withdrawal_requests(id) ON DELETE CASCADE,
    
    -- Event details
    event_type VARCHAR(50) NOT NULL, -- kyc_validation, balance_validation, contract_submission, confirmation_check, error, cancellation
    old_state VARCHAR(20), -- Previous state before this event
    new_state VARCHAR(20), -- New state after this event
    
    -- Event results
    success BOOLEAN NOT NULL DEFAULT TRUE,
    message TEXT, -- Human readable message about what happened
    details JSONB, -- Structured event details (validation results, transaction info, etc.)
    
    -- Retry tracking for failed operations
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMP WITH TIME ZONE,
    
    -- Timing
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processing_duration_ms INTEGER, -- How long this step took to process
    
    -- Constraints
    CONSTRAINT valid_event_type CHECK (event_type IN ('kyc_validation', 'balance_validation', 'contract_submission', 'confirmation_check', 'error', 'cancellation', 'retry'))
);

-- Indexes for efficient querying
CREATE INDEX idx_withdrawal_requests_user_id ON withdrawal_requests(user_id);
CREATE INDEX idx_withdrawal_requests_user_address ON withdrawal_requests(user_address);
CREATE INDEX idx_withdrawal_requests_state ON withdrawal_requests(state);
CREATE INDEX idx_withdrawal_requests_created_at ON withdrawal_requests(created_at);
CREATE INDEX idx_withdrawal_requests_transaction_hash ON withdrawal_requests(transaction_hash);
CREATE INDEX idx_withdrawal_requests_pending_validation ON withdrawal_requests(state) WHERE state = 'pending';
CREATE INDEX idx_withdrawal_requests_pending_submission ON withdrawal_requests(state) WHERE state = 'validated';

-- Indexes for history table
CREATE INDEX idx_withdrawal_request_history_request_id ON withdrawal_request_history(withdrawal_request_id);
CREATE INDEX idx_withdrawal_request_history_event_type ON withdrawal_request_history(event_type);
CREATE INDEX idx_withdrawal_request_history_created_at ON withdrawal_request_history(created_at);
CREATE INDEX idx_withdrawal_request_history_next_retry ON withdrawal_request_history(next_retry_at) WHERE next_retry_at IS NOT NULL;

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_withdrawal_request_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update updated_at on changes
CREATE TRIGGER trigger_withdrawal_requests_updated_at
    BEFORE UPDATE ON withdrawal_requests
    FOR EACH ROW
    EXECUTE FUNCTION update_withdrawal_request_updated_at();

-- Function to automatically create history entries for state changes
CREATE OR REPLACE FUNCTION create_withdrawal_request_history()
RETURNS TRIGGER AS $$
BEGIN
    -- Only create history entry if state actually changed
    IF OLD.state IS DISTINCT FROM NEW.state THEN
        INSERT INTO withdrawal_request_history (
            withdrawal_request_id,
            event_type,
            old_state,
            new_state,
            success,
            message,
            details
        ) VALUES (
            NEW.id,
            CASE 
                WHEN NEW.state = 'validated' THEN 'balance_validation'
                WHEN NEW.state = 'submitted' THEN 'contract_submission'
                WHEN NEW.state = 'confirmed' THEN 'confirmation_check'
                WHEN NEW.state = 'failed' THEN 'error'
                WHEN NEW.state = 'cancelled' THEN 'cancellation'
                ELSE 'state_change'
            END,
            OLD.state,
            NEW.state,
            NEW.state NOT IN ('failed', 'cancelled'),
            CASE 
                WHEN NEW.state = 'validated' THEN 'KYC and balance validation completed'
                WHEN NEW.state = 'submitted' THEN 'Withdrawal submitted to smart contract'
                WHEN NEW.state = 'confirmed' THEN 'Withdrawal confirmed on blockchain'
                WHEN NEW.state = 'failed' THEN COALESCE(NEW.error_message, 'Withdrawal failed')
                WHEN NEW.state = 'cancelled' THEN 'Withdrawal cancelled by user'
                ELSE 'State changed to ' || NEW.state
            END,
            jsonb_build_object(
                'old_state', OLD.state,
                'new_state', NEW.state,
                'transaction_hash', NEW.transaction_hash,
                'confirmation_count', NEW.confirmation_count
            )
        );
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically create history entries
CREATE TRIGGER trigger_withdrawal_request_history
    AFTER UPDATE ON withdrawal_requests
    FOR EACH ROW
    EXECUTE FUNCTION create_withdrawal_request_history();

-- Create initial history entry for new requests
CREATE OR REPLACE FUNCTION create_initial_withdrawal_history()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO withdrawal_request_history (
        withdrawal_request_id,
        event_type,
        old_state,
        new_state,
        success,
        message,
        details
    ) VALUES (
        NEW.id,
        'kyc_validation',
        NULL,
        NEW.state,
        TRUE,
        'Withdrawal request created',
        jsonb_build_object(
            'amount_usdc', NEW.amount_usdc,
            'recipient_address', NEW.recipient_address,
            'user_address', NEW.user_address
        )
    );
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to create initial history entry
CREATE TRIGGER trigger_initial_withdrawal_history
    AFTER INSERT ON withdrawal_requests
    FOR EACH ROW
    EXECUTE FUNCTION create_initial_withdrawal_history();

COMMIT;