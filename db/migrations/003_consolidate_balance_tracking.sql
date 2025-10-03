-- Migration: Consolidate Balance Tracking System
-- Description: Simplify financial tracking by consolidating multiple balance/deposit/withdrawal
--              tables into a clean, address-centric system with generic currency support.
--
-- Changes:
-- 1. Drop deprecated tables: user_balances, user_deposit_balances, user_withdrawal_balances,
--    account_financial_transactions, balances (old), usdc_deposits, usdc_withdrawals,
--    withdrawal_requests, withdrawal_request_history
-- 2. Create new address-centric balances table
-- 3. Create generic deposits table (replacing usdc_deposits)
-- 4. Create unified withdrawals table (merging withdrawal_requests + usdc_withdrawals)
-- 5. Add automatic balance update triggers
-- 6. Add necessary indexes for performance

BEGIN;

-- ============================================================================
-- STEP 1: DROP OLD TABLES AND TRIGGERS
-- ============================================================================

-- Drop old triggers first (must drop before dropping functions they depend on)
DROP TRIGGER IF EXISTS trigger_update_user_deposit_balance ON usdc_deposits CASCADE;
DROP TRIGGER IF EXISTS trigger_usdc_deposits_updated_at ON usdc_deposits CASCADE;
DROP TRIGGER IF EXISTS trigger_user_deposit_balances_updated_at ON user_deposit_balances CASCADE;
DROP TRIGGER IF EXISTS trigger_update_user_withdrawal_balance ON usdc_withdrawals CASCADE;
DROP TRIGGER IF EXISTS trigger_usdc_withdrawals_updated_at ON usdc_withdrawals CASCADE;
DROP TRIGGER IF EXISTS trigger_user_withdrawal_balances_updated_at ON user_withdrawal_balances CASCADE;
DROP TRIGGER IF EXISTS trigger_withdrawal_requests_updated_at ON withdrawal_requests CASCADE;
DROP TRIGGER IF EXISTS trigger_withdrawal_request_history ON withdrawal_requests CASCADE;
DROP TRIGGER IF EXISTS trigger_initial_withdrawal_history ON withdrawal_requests CASCADE;
DROP TRIGGER IF EXISTS update_account_financial_transactions_updated_at ON account_financial_transactions CASCADE;
DROP TRIGGER IF EXISTS update_balances_updated_at ON balances CASCADE;
DROP TRIGGER IF EXISTS update_user_balances_timestamp ON user_balances CASCADE;

-- Drop old functions (after triggers are dropped)
DROP FUNCTION IF EXISTS update_user_deposit_balance() CASCADE;
DROP FUNCTION IF EXISTS update_user_withdrawal_balance() CASCADE;
DROP FUNCTION IF EXISTS update_withdrawal_request_updated_at() CASCADE;
DROP FUNCTION IF EXISTS create_withdrawal_request_history() CASCADE;
DROP FUNCTION IF EXISTS create_initial_withdrawal_history() CASCADE;
DROP FUNCTION IF EXISTS update_user_balances_timestamp() CASCADE;

-- Drop any existing views that might reference old schema
DROP VIEW IF EXISTS balances_readable CASCADE;
DROP VIEW IF EXISTS deposits_readable CASCADE;
DROP VIEW IF EXISTS withdrawals_readable CASCADE;
DROP VIEW IF EXISTS transaction_history CASCADE;
DROP VIEW IF EXISTS user_financial_transactions CASCADE;

-- Drop old tables (cascade to remove dependent objects)
DROP TABLE IF EXISTS account_financial_transactions CASCADE;
DROP TABLE IF EXISTS user_deposit_balances CASCADE;
DROP TABLE IF EXISTS user_withdrawal_balances CASCADE;
DROP TABLE IF EXISTS deposit_processing_log CASCADE;
DROP TABLE IF EXISTS withdrawal_processing_log CASCADE;
DROP TABLE IF EXISTS withdrawal_request_history CASCADE;
DROP TABLE IF EXISTS withdrawal_requests CASCADE;
DROP TABLE IF EXISTS usdc_withdrawals CASCADE;
DROP TABLE IF EXISTS usdc_deposits CASCADE;
DROP TABLE IF EXISTS user_balances CASCADE;
DROP TABLE IF EXISTS balances CASCADE; -- Old identity-based balances table

-- ============================================================================
-- STEP 2: CREATE ACCOUNTS TABLE
-- ============================================================================

CREATE TABLE accounts (
    account_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES companies(id) ON DELETE CASCADE, -- Owner organization
    user_id UUID REFERENCES identities(id) ON DELETE CASCADE, -- For user-owned accounts (backward compatibility)
    
    -- Account metadata
    account_label VARCHAR(100) NOT NULL, -- Human-friendly label (e.g., "Operating USD")
    account_number VARCHAR(42) NOT NULL, -- Ethereum address (one-to-one relationship)
    account_type VARCHAR(50) DEFAULT 'OPERATING', -- OPERATING, RESERVE, ESCROW, etc.
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC', -- Primary currency for this account
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE', -- ACTIVE, INACTIVE, SUSPENDED
    
    -- Timestamps
    opened_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    closed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_status CHECK (status IN ('ACTIVE', 'INACTIVE', 'SUSPENDED')),
    CONSTRAINT valid_ethereum_address CHECK (account_number ~ '^0x[a-fA-F0-9]{40}$'),
    CONSTRAINT must_have_owner CHECK (org_id IS NOT NULL OR user_id IS NOT NULL),
    -- One address can only belong to one account
    UNIQUE(account_number)
);

-- Indexes for accounts table
CREATE INDEX idx_accounts_org_id ON accounts(org_id) WHERE org_id IS NOT NULL;
CREATE INDEX idx_accounts_user_id ON accounts(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX idx_accounts_account_number ON accounts(account_number);
CREATE INDEX idx_accounts_status ON accounts(status);
CREATE INDEX idx_accounts_currency ON accounts(currency);
CREATE INDEX idx_accounts_created_at ON accounts(created_at);

-- ============================================================================
-- STEP 3: CREATE NEW BALANCES TABLE (ACCOUNT-CENTRIC)
-- ============================================================================

CREATE TABLE balances (
    -- Primary key: Account ID + currency
    account_id UUID REFERENCES accounts(account_id) ON DELETE CASCADE,
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    
    -- Balance tracking (all in smallest unit, e.g., wei for USDC with 6 decimals)
    available_balance NUMERIC(78, 0) NOT NULL DEFAULT 0,
    locked_balance NUMERIC(78, 0) NOT NULL DEFAULT 0, -- Funds locked in active deals
    
    -- Lifetime totals for audit/reconciliation
    total_deposited NUMERIC(78, 0) NOT NULL DEFAULT 0,
    total_withdrawn NUMERIC(78, 0) NOT NULL DEFAULT 0,
    
    -- Transaction counts
    deposit_count INTEGER NOT NULL DEFAULT 0,
    withdrawal_count INTEGER NOT NULL DEFAULT 0,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    first_deposit_at TIMESTAMP WITH TIME ZONE,
    last_deposit_at TIMESTAMP WITH TIME ZONE,
    first_withdrawal_at TIMESTAMP WITH TIME ZONE,
    last_withdrawal_at TIMESTAMP WITH TIME ZONE,
    
    -- Constraints
    PRIMARY KEY (account_id, currency),
    CONSTRAINT positive_available_balance CHECK (available_balance >= 0),
    CONSTRAINT positive_locked_balance CHECK (locked_balance >= 0),
    CONSTRAINT positive_total_deposited CHECK (total_deposited >= 0),
    CONSTRAINT positive_total_withdrawn CHECK (total_withdrawn >= 0),
    -- Balance integrity: available + locked <= total_deposited - total_withdrawn
    CONSTRAINT balance_integrity CHECK (available_balance + locked_balance <= total_deposited - total_withdrawn)
);

-- Indexes for balances table
CREATE INDEX idx_balances_account_id ON balances(account_id);
CREATE INDEX idx_balances_currency ON balances(currency);
CREATE INDEX idx_balances_available_balance ON balances(available_balance) WHERE available_balance > 0;
CREATE INDEX idx_balances_updated_at ON balances(updated_at);

-- ============================================================================
-- STEP 4: CREATE NEW DEPOSITS TABLE (GENERIC, EVENT-TRACKED)
-- ============================================================================

CREATE TABLE deposits (
    id SERIAL PRIMARY KEY,
    
    -- Account reference
    account_id UUID REFERENCES accounts(account_id) ON DELETE CASCADE,
    address VARCHAR(42) NOT NULL, -- Specific Ethereum address that received deposit
    amount NUMERIC(78, 0) NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    
    -- Smart contract event tracking fields
    transaction_hash VARCHAR(66) NOT NULL,
    event_transaction_hash VARCHAR(66) NOT NULL,
    block_number BIGINT NOT NULL,
    event_timestamp BIGINT NOT NULL, -- Unix timestamp from event
    block_timestamp TIMESTAMP WITH TIME ZONE, -- Converted to timestamp
    log_index BIGINT NOT NULL,
    event_signature VARCHAR(66) NOT NULL,
    
    -- Status and confirmations
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, confirmed, processed, failed
    confirmation_count INTEGER NOT NULL DEFAULT 0,
    required_confirmations INTEGER NOT NULL DEFAULT 12,
    
    -- Error tracking
    error_message TEXT,
    processing_details JSONB,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    
    -- Constraints
    CONSTRAINT positive_deposit_amount CHECK (amount > 0),
    CONSTRAINT valid_deposit_status CHECK (status IN ('pending', 'confirmed', 'processed', 'failed')),
    CONSTRAINT valid_deposit_confirmations CHECK (confirmation_count >= 0 AND confirmation_count <= required_confirmations + 10),
    CONSTRAINT valid_deposit_address CHECK (address ~ '^0x[a-fA-F0-9]{40}$'),
    
    -- Ensure unique events (no duplicate processing)
    UNIQUE(transaction_hash, log_index),
    UNIQUE(event_transaction_hash, log_index)
);

-- Indexes for deposits table
CREATE INDEX idx_deposits_account_id ON deposits(account_id);
CREATE INDEX idx_deposits_address ON deposits(address);
CREATE INDEX idx_deposits_currency ON deposits(currency);
CREATE INDEX idx_deposits_block_number ON deposits(block_number);
CREATE INDEX idx_deposits_status ON deposits(status);
CREATE INDEX idx_deposits_transaction_hash ON deposits(transaction_hash);
CREATE INDEX idx_deposits_event_transaction_hash ON deposits(event_transaction_hash);
CREATE INDEX idx_deposits_created_at ON deposits(created_at DESC);
CREATE INDEX idx_deposits_pending ON deposits(account_id, status) WHERE status = 'pending';

-- ============================================================================
-- STEP 5: CREATE NEW WITHDRAWALS TABLE (UNIFIED LIFECYCLE)
-- ============================================================================

CREATE TABLE withdrawals (
    id SERIAL PRIMARY KEY,
    
    -- Account reference
    account_id UUID REFERENCES accounts(account_id) ON DELETE CASCADE,
    address VARCHAR(42) NOT NULL, -- Specific address withdrawing from
    recipient_address VARCHAR(42) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    
    -- Request metadata (for UI-initiated withdrawals)
    requested_by_user_id UUID REFERENCES identities(id) ON DELETE SET NULL,
    request_source VARCHAR(20) NOT NULL DEFAULT 'user', -- 'user' (UI) or 'contract' (direct)
    
    -- State tracking (unified lifecycle)
    state VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- States: pending, validated, approved, submitted, confirmed, failed, cancelled
    
    -- Validation results (for request flow)
    kyc_validated BOOLEAN NOT NULL DEFAULT FALSE,
    balance_validated BOOLEAN NOT NULL DEFAULT FALSE,
    kyc_status_at_request VARCHAR(20),
    available_balance_at_request NUMERIC(78, 0),
    validation_errors JSONB,
    
    -- Smart contract event tracking fields
    transaction_hash VARCHAR(66),
    event_transaction_hash VARCHAR(66),
    block_number BIGINT,
    event_timestamp BIGINT, -- Unix timestamp from event
    block_timestamp TIMESTAMP WITH TIME ZONE,
    log_index BIGINT,
    event_signature VARCHAR(66),
    
    -- Status and confirmations
    confirmation_count INTEGER NOT NULL DEFAULT 0,
    required_confirmations INTEGER NOT NULL DEFAULT 12,
    
    -- Error tracking
    error_message TEXT,
    admin_notes TEXT,
    rejection_reason TEXT,
    
    -- Processing history (replaces withdrawal_request_history table)
    processing_history JSONB DEFAULT '[]'::jsonb,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    validated_at TIMESTAMP WITH TIME ZONE,
    approved_at TIMESTAMP WITH TIME ZONE,
    submitted_at TIMESTAMP WITH TIME ZONE,
    confirmed_at TIMESTAMP WITH TIME ZONE,
    processed_at TIMESTAMP WITH TIME ZONE,
    failed_at TIMESTAMP WITH TIME ZONE,
    cancelled_at TIMESTAMP WITH TIME ZONE,
    
    -- Constraints
    CONSTRAINT positive_withdrawal_amount CHECK (amount > 0),
    CONSTRAINT valid_withdrawal_state CHECK (state IN ('pending', 'validated', 'approved', 'submitted', 'confirmed', 'failed', 'cancelled')),
    CONSTRAINT valid_withdrawal_confirmations CHECK (confirmation_count >= 0 AND confirmation_count <= required_confirmations + 10),
    CONSTRAINT valid_withdrawal_address CHECK (address ~ '^0x[a-fA-F0-9]{40}$'),
    CONSTRAINT valid_recipient_address CHECK (recipient_address ~ '^0x[a-fA-F0-9]{40}$'),
    CONSTRAINT valid_request_source CHECK (request_source IN ('user', 'contract')),
    -- Can't have error message when confirmed
    CONSTRAINT no_error_when_confirmed CHECK (NOT (state = 'confirmed' AND error_message IS NOT NULL))
);

-- Indexes for withdrawals table
CREATE INDEX idx_withdrawals_account_id ON withdrawals(account_id);
CREATE INDEX idx_withdrawals_address ON withdrawals(address);
CREATE INDEX idx_withdrawals_recipient_address ON withdrawals(recipient_address);
CREATE INDEX idx_withdrawals_currency ON withdrawals(currency);
CREATE INDEX idx_withdrawals_state ON withdrawals(state);
CREATE INDEX idx_withdrawals_requested_by_user_id ON withdrawals(requested_by_user_id) WHERE requested_by_user_id IS NOT NULL;
CREATE INDEX idx_withdrawals_transaction_hash ON withdrawals(transaction_hash) WHERE transaction_hash IS NOT NULL;
CREATE INDEX idx_withdrawals_event_transaction_hash ON withdrawals(event_transaction_hash) WHERE event_transaction_hash IS NOT NULL;
CREATE INDEX idx_withdrawals_block_number ON withdrawals(block_number) WHERE block_number IS NOT NULL;
CREATE INDEX idx_withdrawals_created_at ON withdrawals(created_at DESC);
CREATE INDEX idx_withdrawals_pending_validation ON withdrawals(account_id, state) WHERE state = 'pending';
CREATE INDEX idx_withdrawals_pending_approval ON withdrawals(account_id, state) WHERE state = 'validated';
CREATE INDEX idx_withdrawals_pending_submission ON withdrawals(account_id, state) WHERE state = 'approved';

-- Partial unique index to ensure unique transaction hashes (when set)
CREATE UNIQUE INDEX idx_withdrawals_unique_tx_hash ON withdrawals(transaction_hash) WHERE transaction_hash IS NOT NULL;

-- ============================================================================
-- STEP 5: CREATE AUTOMATIC BALANCE UPDATE TRIGGERS
-- ============================================================================

-- Function to update balances when deposits are confirmed
CREATE OR REPLACE FUNCTION update_balance_on_deposit_confirmed()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process when status changes to 'confirmed' or 'processed'
    IF NEW.status IN ('confirmed', 'processed') AND (OLD.status IS DISTINCT FROM NEW.status) THEN
        INSERT INTO balances (
            address,
            currency,
            available_balance,
            total_deposited,
            deposit_count,
            first_deposit_at,
            last_deposit_at,
            updated_at
        ) VALUES (
            NEW.address,
            NEW.currency,
            NEW.amount,
            NEW.amount,
            1,
            COALESCE(NEW.block_timestamp, NOW()),
            COALESCE(NEW.block_timestamp, NOW()),
            NOW()
        )
        ON CONFLICT (address, currency) DO UPDATE SET
            available_balance = balances.available_balance + NEW.amount,
            total_deposited = balances.total_deposited + NEW.amount,
            deposit_count = balances.deposit_count + 1,
            last_deposit_at = GREATEST(balances.last_deposit_at, COALESCE(NEW.block_timestamp, NOW())),
            updated_at = NOW();
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to update balances when withdrawals are confirmed
CREATE OR REPLACE FUNCTION update_balance_on_withdrawal_confirmed()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process when state changes to 'confirmed'
    IF NEW.state = 'confirmed' AND (OLD.state IS DISTINCT FROM NEW.state) THEN
        UPDATE balances
        SET
            available_balance = available_balance - NEW.amount,
            total_withdrawn = total_withdrawn + NEW.amount,
            withdrawal_count = withdrawal_count + 1,
            last_withdrawal_at = GREATEST(COALESCE(last_withdrawal_at, NEW.confirmed_at), NEW.confirmed_at),
            updated_at = NOW()
        WHERE account_id = NEW.account_id AND currency = NEW.currency;
        
        -- If balance doesn't exist, this is an error state (withdrawal without deposit)
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Cannot withdraw from account % with no balance record', NEW.account_id;
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to reserve balance when withdrawal is validated/approved
CREATE OR REPLACE FUNCTION lock_balance_on_withdrawal_approved()
RETURNS TRIGGER AS $$
BEGIN
    -- When withdrawal moves to 'approved' state, lock the funds
    IF NEW.state = 'approved' AND OLD.state != 'approved' THEN
        UPDATE balances
        SET
            available_balance = available_balance - NEW.amount,
            locked_balance = locked_balance + NEW.amount,
            updated_at = NOW()
        WHERE account_id = NEW.account_id AND currency = NEW.currency;
        
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Cannot approve withdrawal for account % with no balance', NEW.account_id;
        END IF;
        
        -- Verify sufficient available balance
        IF (SELECT available_balance FROM balances WHERE account_id = NEW.account_id AND currency = NEW.currency) < 0 THEN
            RAISE EXCEPTION 'Insufficient available balance for withdrawal approval';
        END IF;
    END IF;
    
    -- When withdrawal is cancelled or failed, release the locked funds
    IF NEW.state IN ('cancelled', 'failed') AND OLD.state IN ('approved', 'submitted') THEN
        UPDATE balances
        SET
            available_balance = available_balance + NEW.amount,
            locked_balance = locked_balance - NEW.amount,
            updated_at = NOW()
        WHERE account_id = NEW.account_id AND currency = NEW.currency;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to unlock balance when withdrawal is confirmed (already deducted from locked)
CREATE OR REPLACE FUNCTION unlock_balance_on_withdrawal_confirmed()
RETURNS TRIGGER AS $$
BEGIN
    -- When withdrawal is confirmed, remove from locked balance (already handled by update_balance_on_withdrawal_confirmed for total)
    IF NEW.state = 'confirmed' AND OLD.state IN ('approved', 'submitted') THEN
        UPDATE balances
        SET
            locked_balance = locked_balance - NEW.amount,
            updated_at = NOW()
        WHERE account_id = NEW.account_id AND currency = NEW.currency;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to check account status before deposit processing
CREATE OR REPLACE FUNCTION check_account_active_for_deposit()
RETURNS TRIGGER AS $$
DECLARE
    v_status VARCHAR(20);
BEGIN
    -- Check account status
    SELECT status INTO v_status FROM accounts WHERE account_id = NEW.account_id;
    
    IF v_status != 'ACTIVE' THEN
        RAISE WARNING 'Deposit % for account % blocked: account status is %', 
            NEW.id, NEW.account_id, v_status;
        -- Don't block deposit creation, but log it
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to check account status before withdrawal
CREATE OR REPLACE FUNCTION check_account_active_for_withdrawal()
RETURNS TRIGGER AS $$
DECLARE
    v_status VARCHAR(20);
BEGIN
    -- Check account status
    SELECT status INTO v_status FROM accounts WHERE account_id = NEW.account_id;
    
    IF v_status = 'SUSPENDED' THEN
        NEW.state = 'failed';
        NEW.error_message = 'Account is SUSPENDED';
        NEW.failed_at = NOW();
        RAISE WARNING 'Withdrawal % for account % blocked: account is SUSPENDED', 
            NEW.id, NEW.account_id;
    ELSIF v_status = 'INACTIVE' THEN
        NEW.state = 'failed';
        NEW.error_message = 'Account is INACTIVE';
        NEW.failed_at = NOW();
        RAISE WARNING 'Withdrawal % for account % blocked: account is INACTIVE', 
            NEW.id, NEW.account_id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to check account status on deposit creation
CREATE TRIGGER trigger_check_account_for_deposit
    BEFORE INSERT ON deposits
    FOR EACH ROW
    EXECUTE FUNCTION check_account_active_for_deposit();

-- Trigger to check account status on withdrawal creation
CREATE TRIGGER trigger_check_account_for_withdrawal
    BEFORE INSERT ON withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION check_account_active_for_withdrawal();

-- Trigger to update balances when deposits are confirmed
CREATE TRIGGER trigger_balance_update_on_deposit
    AFTER UPDATE ON deposits
    FOR EACH ROW
    EXECUTE FUNCTION update_balance_on_deposit_confirmed();

-- Trigger to lock balance when withdrawal is approved
CREATE TRIGGER trigger_balance_lock_on_withdrawal_approved
    AFTER UPDATE ON withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION lock_balance_on_withdrawal_approved();

-- Trigger to update balances when withdrawals are confirmed
CREATE TRIGGER trigger_balance_unlock_on_withdrawal_confirmed
    AFTER UPDATE ON withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION unlock_balance_on_withdrawal_confirmed();

-- ============================================================================
-- STEP 6: CREATE TIMESTAMP UPDATE TRIGGERS
-- ============================================================================

-- Trigger to update updated_at on deposits
CREATE TRIGGER trigger_deposits_updated_at
    BEFORE UPDATE ON deposits
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Trigger to update updated_at on withdrawals
CREATE TRIGGER trigger_withdrawals_updated_at
    BEFORE UPDATE ON withdrawals
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Trigger to update updated_at on balances
CREATE TRIGGER trigger_balances_updated_at
    BEFORE UPDATE ON balances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- STEP 7: AUTO-CREATE ACCOUNT WHEN WALLET ADDRESS IS ADDED TO IDENTITY
-- ============================================================================

-- Function to auto-create account when wallet_address is set on identity
CREATE OR REPLACE FUNCTION auto_create_account_for_user()
RETURNS TRIGGER AS $$
DECLARE
    v_account_id UUID;
    v_account_label VARCHAR(100);
BEGIN
    -- Only create account if wallet_address was just added (NULL -> value)
    IF NEW.wallet_address IS NOT NULL AND (OLD.wallet_address IS NULL OR OLD.wallet_address IS DISTINCT FROM NEW.wallet_address) THEN
        -- Generate account label from user info
        v_account_label := COALESCE(NEW.first_name || ' ' || NEW.last_name, NEW.email, 'User Account');
        
        -- Create account for this user
        INSERT INTO accounts (
            user_id,
            account_label,
            account_number,
            account_type,
            status
        ) VALUES (
            NEW.id,
            v_account_label,
            NEW.wallet_address,
            'OPERATING',
            'ACTIVE'
        )
        ON CONFLICT (account_number) DO NOTHING -- In case account already exists
        RETURNING account_id INTO v_account_id;
        
        -- Create initial balance record if account was created
        IF v_account_id IS NOT NULL THEN
            INSERT INTO balances (account_id, currency)
            VALUES (v_account_id, 'USDC')
            ON CONFLICT DO NOTHING;
            
            RAISE NOTICE 'Auto-created account % for user % with address %', 
                v_account_id, NEW.id, NEW.wallet_address;
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-create accounts when wallet address is added
CREATE TRIGGER trigger_auto_create_account_for_user
    AFTER INSERT OR UPDATE OF wallet_address ON identities
    FOR EACH ROW
    EXECUTE FUNCTION auto_create_account_for_user();

-- ============================================================================
-- STEP 8: CREATE HELPER FUNCTIONS
-- ============================================================================

-- Function to get available balance for an account (returns 0 if no record exists)
CREATE OR REPLACE FUNCTION get_available_balance(
    p_account_id UUID,
    p_currency VARCHAR(10) DEFAULT 'USDC'
)
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    RETURN COALESCE(
        (SELECT available_balance FROM balances WHERE account_id = p_account_id AND currency = p_currency),
        0
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get total balance (available + locked) for an account
CREATE OR REPLACE FUNCTION get_total_balance(
    p_account_id UUID,
    p_currency VARCHAR(10) DEFAULT 'USDC'
)
RETURNS NUMERIC(78, 0) AS $$
BEGIN
    RETURN COALESCE(
        (SELECT available_balance + locked_balance FROM balances WHERE account_id = p_account_id AND currency = p_currency),
        0
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get account_id by Ethereum address
CREATE OR REPLACE FUNCTION get_account_id_by_address(
    p_address VARCHAR(42)
)
RETURNS UUID AS $$
BEGIN
    RETURN (SELECT account_id FROM accounts WHERE account_number = p_address);
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if account is active and can perform operations
CREATE OR REPLACE FUNCTION is_account_active(
    p_account_id UUID
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN COALESCE(
        (SELECT status = 'ACTIVE' FROM accounts WHERE account_id = p_account_id),
        false
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to validate account can perform operation (throws exception if not)
CREATE OR REPLACE FUNCTION validate_account_can_operate(
    p_account_id UUID,
    p_operation VARCHAR(50)
)
RETURNS VOID AS $$
DECLARE
    v_status VARCHAR(20);
BEGIN
    SELECT status INTO v_status FROM accounts WHERE account_id = p_account_id;
    
    IF v_status IS NULL THEN
        RAISE EXCEPTION 'Account % does not exist', p_account_id;
    END IF;
    
    IF v_status = 'SUSPENDED' THEN
        RAISE EXCEPTION 'Account % is SUSPENDED and cannot perform % operation', p_account_id, p_operation
            USING HINT = 'Contact support to reactivate your account';
    END IF;
    
    IF v_status = 'INACTIVE' THEN
        RAISE EXCEPTION 'Account % is INACTIVE and cannot perform % operation', p_account_id, p_operation
            USING HINT = 'Account must be activated before use';
    END IF;
    
    -- If we get here, account is ACTIVE
END;
$$ LANGUAGE plpgsql;

-- Function to add processing history entry to withdrawal
CREATE OR REPLACE FUNCTION add_withdrawal_history_entry(
    p_withdrawal_id INTEGER,
    p_event_type VARCHAR(50),
    p_old_state VARCHAR(20),
    p_new_state VARCHAR(20),
    p_success BOOLEAN,
    p_message TEXT DEFAULT NULL,
    p_details JSONB DEFAULT NULL
)
RETURNS VOID AS $$
BEGIN
    UPDATE withdrawals
    SET processing_history = COALESCE(processing_history, '[]'::jsonb) || jsonb_build_object(
        'event_type', p_event_type,
        'old_state', p_old_state,
        'new_state', p_new_state,
        'success', p_success,
        'message', p_message,
        'details', p_details,
        'timestamp', NOW()
    )::jsonb
    WHERE id = p_withdrawal_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 8: CREATE USEFUL VIEWS FOR QUERYING
-- ============================================================================

-- View for human-readable balances (converting from wei) with account info
CREATE OR REPLACE VIEW balances_readable AS
SELECT
    b.account_id,
    b.currency,
    b.available_balance,
    b.locked_balance,
    (b.available_balance::numeric / 1000000::numeric) as available_balance_usdc,
    (b.locked_balance::numeric / 1000000::numeric) as locked_balance_usdc,
    ((b.available_balance + b.locked_balance)::numeric / 1000000::numeric) as total_balance_usdc,
    b.total_deposited,
    b.total_withdrawn,
    (b.total_deposited::numeric / 1000000::numeric) as total_deposited_usdc,
    (b.total_withdrawn::numeric / 1000000::numeric) as total_withdrawn_usdc,
    b.deposit_count,
    b.withdrawal_count,
    b.created_at,
    b.updated_at,
    b.first_deposit_at,
    b.last_deposit_at,
    b.first_withdrawal_at,
    b.last_withdrawal_at,
    -- Account info (joined)
    a.account_label,
    a.account_number as address,
    a.org_id,
    a.user_id,
    a.status as account_status
FROM balances b
LEFT JOIN accounts a ON b.account_id = a.account_id;

-- View for human-readable deposits with account info
CREATE OR REPLACE VIEW deposits_readable AS
SELECT
    d.id,
    d.account_id,
    d.address,
    a.account_label,
    a.org_id,
    a.user_id,
    d.amount,
    d.currency,
    (d.amount::numeric / 1000000::numeric) as amount_usdc,
    d.transaction_hash,
    d.event_transaction_hash,
    d.block_number,
    d.event_timestamp,
    d.block_timestamp,
    d.log_index,
    d.event_signature,
    d.status,
    d.confirmation_count,
    d.required_confirmations,
    d.error_message,
    d.created_at,
    d.updated_at,
    d.processed_at
FROM deposits d
LEFT JOIN accounts a ON d.account_id = a.account_id;

-- View for human-readable withdrawals with account info
CREATE OR REPLACE VIEW withdrawals_readable AS
SELECT
    w.id,
    w.account_id,
    w.address,
    a.account_label,
    a.org_id,
    a.user_id,
    w.recipient_address,
    w.amount,
    w.currency,
    (w.amount::numeric / 1000000::numeric) as amount_usdc,
    w.requested_by_user_id,
    w.request_source,
    w.state,
    w.kyc_validated,
    w.balance_validated,
    w.validation_errors,
    w.transaction_hash,
    w.event_transaction_hash,
    w.block_number,
    w.confirmation_count,
    w.required_confirmations,
    w.error_message,
    w.admin_notes,
    w.rejection_reason,
    w.processing_history,
    w.created_at,
    w.updated_at,
    w.validated_at,
    w.approved_at,
    w.submitted_at,
    w.confirmed_at,
    w.processed_at,
    w.failed_at,
    w.cancelled_at
FROM withdrawals w
LEFT JOIN accounts a ON w.account_id = a.account_id;

-- View for combined transaction history (deposits + withdrawals) with account info
CREATE OR REPLACE VIEW transaction_history AS
SELECT
    'deposit' as transaction_type,
    d.id::TEXT as transaction_id,
    d.account_id,
    d.address,
    a.account_label,
    a.org_id,
    NULL::VARCHAR(42) as recipient_address,
    d.amount,
    d.currency,
    (d.amount::numeric / 1000000::numeric) as amount_usdc,
    'inflow' as direction,
    d.transaction_hash,
    d.block_number,
    d.status,
    d.created_at,
    d.processed_at,
    a.user_id
FROM deposits d
LEFT JOIN accounts a ON d.account_id = a.account_id
UNION ALL
SELECT
    'withdrawal' as transaction_type,
    w.id::TEXT as transaction_id,
    w.account_id,
    w.address,
    a.account_label,
    a.org_id,
    w.recipient_address,
    w.amount,
    w.currency,
    (w.amount::numeric / 1000000::numeric) as amount_usdc,
    'outflow' as direction,
    w.transaction_hash,
    w.block_number,
    w.state as status,
    w.created_at,
    w.processed_at,
    a.user_id
FROM withdrawals w
LEFT JOIN accounts a ON w.account_id = a.account_id
ORDER BY created_at DESC;

-- ============================================================================
-- STEP 10: ADD HELPFUL COMMENTS
-- ============================================================================

COMMENT ON TABLE accounts IS 'Accounts are the primary entity for managing funds. Each account has one Ethereum address (account_number) and belongs to either a user or organization.';
COMMENT ON TABLE balances IS 'Account-centric balance tracking supporting multiple currencies. Balances are automatically updated via triggers when deposits/withdrawals are confirmed.';
COMMENT ON TABLE deposits IS 'Generic deposit event tracking from smart contracts. Supports any currency and records full blockchain event details.';
COMMENT ON TABLE withdrawals IS 'Unified withdrawal lifecycle tracking from user request through blockchain confirmation. Handles both UI-initiated and direct contract withdrawals.';

COMMENT ON COLUMN accounts.account_id IS 'Primary key (UUID) for the account';
COMMENT ON COLUMN accounts.account_number IS 'Ethereum address (one-to-one with account). This is the blockchain wallet address.';
COMMENT ON COLUMN accounts.account_label IS 'Human-friendly label like "Operating USD" or "Reserve Account"';
COMMENT ON COLUMN accounts.status IS 'Account status: ACTIVE (can operate), INACTIVE (disabled), SUSPENDED (temporarily blocked)';

COMMENT ON COLUMN balances.available_balance IS 'Balance available for withdrawal or use (in smallest unit, e.g., USDC with 6 decimals = wei)';
COMMENT ON COLUMN balances.locked_balance IS 'Balance locked in active deals or pending withdrawals (in smallest unit)';

COMMENT ON COLUMN deposits.account_id IS 'Account that received the deposit';
COMMENT ON COLUMN deposits.address IS 'Specific Ethereum address that received the deposit (same as account.account_number)';

COMMENT ON COLUMN withdrawals.account_id IS 'Account initiating the withdrawal';
COMMENT ON COLUMN withdrawals.state IS 'Lifecycle state: pending → validated → approved → submitted → confirmed (or failed/cancelled)';
COMMENT ON COLUMN withdrawals.processing_history IS 'JSONB array of processing events, replaces withdrawal_request_history table';
COMMENT ON COLUMN withdrawals.request_source IS 'Origin of withdrawal: user (UI-initiated) or contract (direct smart contract interaction)';

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES (Run these to verify migration)
-- ============================================================================

-- Verify tables exist
-- SELECT table_name FROM information_schema.tables 
-- WHERE table_name IN ('balances', 'deposits', 'withdrawals') 
-- ORDER BY table_name;

-- Verify triggers exist
-- SELECT trigger_name, event_object_table 
-- FROM information_schema.triggers 
-- WHERE trigger_name LIKE '%balance%' OR trigger_name LIKE '%deposit%' OR trigger_name LIKE '%withdrawal%'
-- ORDER BY event_object_table, trigger_name;

-- Verify old tables are dropped
-- SELECT table_name FROM information_schema.tables 
-- WHERE table_name IN ('user_balances', 'user_deposit_balances', 'usdc_deposits', 'usdc_withdrawals', 'withdrawal_requests')
-- ORDER BY table_name;
-- Expected: No results

