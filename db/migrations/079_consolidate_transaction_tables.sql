-- Migration: Consolidate Transaction Tables (Clean Cutover)
-- Description: Consolidates state transition function transactions into a single
--              flexible state_transactions table with serialized transaction data.
--              Removes all arithmetic and legacy code. No backward compatibility.
--
-- Changes:
-- 1. DROP old tables: deal_transactions, incoming_transactions, arithmetic_transactions
-- 2. Create new flexible state_transactions table with serialized bytes
-- 3. Clarify financial transaction terminology (deposits, withdrawals, payments)

BEGIN;

-- ============================================================================
-- STEP 1: Drop old transaction tables (clean cutover)
-- ============================================================================

-- Drop old state transaction tables
DROP TABLE IF EXISTS deal_transactions CASCADE;
DROP TABLE IF EXISTS incoming_transactions CASCADE;
DROP TABLE IF EXISTS arithmetic_transactions CASCADE;

-- ============================================================================
-- STEP 2: Create new flexible state_transactions table
-- ============================================================================

CREATE TABLE state_transactions (
    -- Primary identifier
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Transaction type (enum value as string)
    transaction_type VARCHAR(50) NOT NULL,
    
    -- Serialized transaction data (bincode-encoded DealTransaction from lib)
    transaction_data BYTEA NOT NULL,
    
    -- Batch processing integration
    included_in_batch_id INTEGER REFERENCES proof_batches(id) ON DELETE SET NULL,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    
    -- Constraints
    CONSTRAINT valid_transaction_type CHECK (
        transaction_type IN (
            -- User lifecycle
            'user_registration',
            'kyc_submission',
            'kyc_verification',
            'investor_registration',
            'admin_registration',
            -- Company lifecycle
            'company_registration',
            'company_update',
            -- Deal lifecycle
            'deal_origination',
            'deal_payment',
            'deal_closeout',
            -- Financial operations
            'user_deposit',
            'user_withdrawal',
            -- Document operations
            'verify_document',
            -- Collateral operations
            'collateral_creation',
            'collateral_update',
            'collateral_deal_association',
            'collateral_deal_disassociation',
            'collateral_detection_request',
            'collateral_detection_update',
            -- State management
            'state_regenesis'
        )
    )
);

-- Add table and column comments
COMMENT ON TABLE state_transactions IS 'Unified audit trail for all ZK state transition function transactions. Stores serialized transaction data for flexibility.';
COMMENT ON COLUMN state_transactions.transaction_type IS 'Type of state transition (see TransactionType enum in lib/transactions.rs)';
COMMENT ON COLUMN state_transactions.transaction_data IS 'Bincode-serialized DealTransaction struct from arda-credit-lib';
COMMENT ON COLUMN state_transactions.included_in_batch_id IS 'Reference to proof batch (NULL = not yet batched)';

-- ============================================================================
-- STEP 3: Create indexes for performance
-- ============================================================================

-- Index for finding unbatched transactions (most common query)
CREATE INDEX idx_state_transactions_unbatched 
    ON state_transactions(created_at, id) 
    WHERE included_in_batch_id IS NULL;

-- Index for batch association queries
CREATE INDEX idx_state_transactions_batch_id 
    ON state_transactions(included_in_batch_id) 
    WHERE included_in_batch_id IS NOT NULL;

-- Index for transaction type filtering
CREATE INDEX idx_state_transactions_type 
    ON state_transactions(transaction_type, created_at DESC);

-- Index for timestamp ordering
CREATE INDEX idx_state_transactions_created_at 
    ON state_transactions(created_at DESC);

-- Composite index for efficient batch queries
CREATE INDEX idx_state_transactions_batch_created 
    ON state_transactions(included_in_batch_id, created_at) 
    WHERE included_in_batch_id IS NOT NULL;

-- ============================================================================
-- STEP 4: Update financial transaction table comments for clarity
-- ============================================================================

-- Clarify purpose of existing financial tables
COMMENT ON TABLE account_financial_transactions IS 
    'Financial transactions for user deposits and withdrawals (user <-> system). '
    'DEPOSIT = user deposits into system, WITHDRAWAL = user withdraws from system.';

COMMENT ON TABLE deal_payments IS 
    'Financial payments for deal transactions (account <-> account). '
    'Money movement between accounts for deals.';

-- Drop deprecated credit line table if it exists
DROP TABLE IF EXISTS credit_line_financial_transactions CASCADE;

-- ============================================================================
-- STEP 5: Create helper functions for common queries
-- ============================================================================

-- Function to get pending (unbatched) state transactions
CREATE OR REPLACE FUNCTION get_pending_state_transactions(
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    id UUID,
    transaction_type VARCHAR(50),
    transaction_data BYTEA,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_type,
        st.transaction_data,
        st.created_at
    FROM state_transactions st
    WHERE st.included_in_batch_id IS NULL
    ORDER BY st.created_at ASC, st.id ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_pending_state_transactions IS 
    'Get pending state transactions awaiting batch processing, ordered chronologically';

-- Function to get state transactions for a batch
CREATE OR REPLACE FUNCTION get_batch_state_transactions(
    p_batch_id INTEGER,
    p_limit INTEGER DEFAULT 1000
)
RETURNS TABLE (
    id UUID,
    transaction_type VARCHAR(50),
    transaction_data BYTEA,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_type,
        st.transaction_data,
        st.created_at
    FROM state_transactions st
    WHERE st.included_in_batch_id = p_batch_id
    ORDER BY st.created_at ASC, st.id ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_batch_state_transactions IS 
    'Get all state transactions included in a specific batch';

-- Function to get recent state transactions by type
CREATE OR REPLACE FUNCTION get_recent_state_transactions_by_type(
    p_transaction_type VARCHAR(50),
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    id UUID,
    transaction_type VARCHAR(50),
    transaction_data BYTEA,
    included_in_batch_id INTEGER,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_type,
        st.transaction_data,
        st.included_in_batch_id,
        st.created_at
    FROM state_transactions st
    WHERE st.transaction_type = p_transaction_type
    ORDER BY st.created_at DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_recent_state_transactions_by_type IS 
    'Get recent state transactions of a specific type';

-- ============================================================================
-- STEP 6: Update trigger for updated_at
-- ============================================================================

-- Create or replace function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_state_transaction_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for updated_at
CREATE TRIGGER trigger_state_transactions_updated_at
    BEFORE UPDATE ON state_transactions
    FOR EACH ROW
    EXECUTE FUNCTION update_state_transaction_updated_at();

-- ============================================================================
-- STEP 7: Remove old database functions that referenced old tables
-- ============================================================================

-- Drop functions that referenced deal_transactions or incoming_transactions
DROP FUNCTION IF EXISTS get_unbatched_transactions(INTEGER);
DROP FUNCTION IF EXISTS create_batch(INTEGER);

-- ============================================================================
-- STEP 8: Notes on financial transaction terminology
-- ============================================================================

-- Financial transactions are separate from state transactions:
--
-- State Transactions (this table):
--   - Zero-knowledge state transition functions
--   - Batched and proven via zkVM
--   - Posted to blockchain for verification
--
-- Financial Transactions (separate tables):
--   - account_financial_transactions:
--     * DEPOSIT: User deposits USDC into the system
--     * WITHDRAWAL: User withdraws USDC from the system
--   - deal_payments:
--     * Money movement between accounts for deals
--     * Types: regular, principal, interest, penalty

COMMIT;
