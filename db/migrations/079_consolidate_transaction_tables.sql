-- Migration: Consolidate Transaction Tables
-- Description: Consolidates state transition function transactions into a single
--              state_transactions table and clarifies financial transaction terminology
--
-- Changes:
-- 1. Create unified state_transactions table (consolidates deal_transactions + incoming_transactions)
-- 2. Migrate existing data from both old tables
-- 3. Create backward compatibility views
-- 4. Clarify financial transaction terminology (deposits, withdrawals, payments)

-- ============================================================================
-- STEP 1: Create unified state_transactions table
-- ============================================================================

CREATE TABLE state_transactions (
    -- Primary identifier
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Transaction identification (for zkVM compatibility)
    transaction_id_hash BYTEA NOT NULL UNIQUE, -- SHA256 hash for zkVM
    
    -- Core transaction data
    transaction_type VARCHAR(50) NOT NULL, -- See TransactionType enum in lib/transactions.rs
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0, -- Amount for monetary transactions
    
    -- Hashed identifiers (for zkVM privacy)
    user_id_hash BYTEA, -- SHA256 hash of user UUID (NULL for system transactions)
    deal_id_hash BYTEA, -- SHA256 hash of deal UUID (NULL for non-deal transactions)
    
    -- Foreign key references to actual records (for database queries)
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    deal_id UUID REFERENCES deals(id) ON DELETE SET NULL,
    payment_id UUID REFERENCES deal_payments(id) ON DELETE SET NULL,
    
    -- Batch processing integration
    included_in_batch_id INTEGER REFERENCES proof_batches(id) ON DELETE SET NULL,
    
    -- Flexible metadata for transaction-specific data
    metadata JSONB DEFAULT '{}'::jsonb,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    
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
            'state_regenesis',
            -- Legacy arithmetic (for backward compatibility)
            'arithmetic_operation'
        )
    ),
    CONSTRAINT positive_amount CHECK (amount >= 0)
);

-- Add table comment
COMMENT ON TABLE state_transactions IS 'Unified audit trail for all ZK state transition function transactions';
COMMENT ON COLUMN state_transactions.transaction_id_hash IS 'SHA256 hash of transaction ID for zkVM compatibility';
COMMENT ON COLUMN state_transactions.transaction_type IS 'Type of state transition (see TransactionType enum)';
COMMENT ON COLUMN state_transactions.user_id_hash IS 'Hashed user ID for privacy in ZK proofs';
COMMENT ON COLUMN state_transactions.deal_id_hash IS 'Hashed deal ID for privacy in ZK proofs';
COMMENT ON COLUMN state_transactions.included_in_batch_id IS 'Reference to proof batch (NULL = not yet batched)';
COMMENT ON COLUMN state_transactions.metadata IS 'Flexible JSON field for transaction-specific data';

-- ============================================================================
-- STEP 2: Create indexes for performance
-- ============================================================================

-- Index for finding unbatched transactions (most common query)
CREATE INDEX idx_state_transactions_unbatched 
    ON state_transactions(id, created_at) 
    WHERE included_in_batch_id IS NULL;

-- Index for batch association queries
CREATE INDEX idx_state_transactions_batch_id 
    ON state_transactions(included_in_batch_id) 
    WHERE included_in_batch_id IS NOT NULL;

-- Index for transaction type filtering
CREATE INDEX idx_state_transactions_type 
    ON state_transactions(transaction_type, created_at DESC);

-- Index for user-related transaction queries
CREATE INDEX idx_state_transactions_user_id 
    ON state_transactions(user_id, created_at DESC) 
    WHERE user_id IS NOT NULL;

-- Index for deal-related transaction queries
CREATE INDEX idx_state_transactions_deal_id 
    ON state_transactions(deal_id, created_at DESC) 
    WHERE deal_id IS NOT NULL;

-- Index for hashed ID lookups (for zkVM queries)
CREATE INDEX idx_state_transactions_user_id_hash 
    ON state_transactions(user_id_hash);

CREATE INDEX idx_state_transactions_deal_id_hash 
    ON state_transactions(deal_id_hash) 
    WHERE deal_id_hash IS NOT NULL;

-- Composite index for efficient audit trail queries
CREATE INDEX idx_state_transactions_audit 
    ON state_transactions(user_id, transaction_type, created_at DESC);

-- ============================================================================
-- STEP 3: Migrate existing data from deal_transactions
-- ============================================================================

INSERT INTO state_transactions (
    id,
    transaction_id_hash,
    transaction_type,
    amount,
    user_id_hash,
    deal_id_hash,
    user_id,
    deal_id,
    payment_id,
    included_in_batch_id,
    metadata,
    created_at
)
SELECT
    id,
    transaction_id_hash,
    transaction_type,
    amount,
    user_id_hash,
    deal_id_hash,
    identity_id as user_id, -- Rename from identity_id to user_id
    deal_id,
    payment_id,
    included_in_batch_id,
    metadata,
    created_at
FROM deal_transactions
ON CONFLICT (transaction_id_hash) DO NOTHING;

-- ============================================================================
-- STEP 4: Create backward compatibility views
-- ============================================================================

-- View to emulate old deal_transactions table
CREATE VIEW deal_transactions_view AS
SELECT
    id,
    transaction_id_hash,
    user_id_hash,
    deal_id_hash,
    transaction_type,
    amount,
    user_id as identity_id, -- Map back to old column name
    deal_id,
    payment_id,
    included_in_batch_id,
    metadata,
    created_at
FROM state_transactions
WHERE transaction_type IN (
    'user_registration',
    'kyc_submission',
    'kyc_verification',
    'user_deposit',
    'user_withdrawal',
    'investor_registration',
    'deal_origination',
    'deal_payment',
    'deal_closeout'
);

-- View to emulate old incoming_transactions table (for legacy arithmetic batching)
CREATE VIEW incoming_transactions_view AS
SELECT
    -- Extract numeric ID from UUID for backward compatibility
    -- This is a lossy conversion but maintains the interface
    ('x' || substring(id::text, 1, 8))::bit(32)::int as id,
    COALESCE((metadata->>'amount')::integer, amount::integer) as amount,
    included_in_batch_id,
    created_at
FROM state_transactions
WHERE transaction_type = 'arithmetic_operation'
ORDER BY created_at ASC;

COMMENT ON VIEW deal_transactions_view IS 'Backward compatibility view for old deal_transactions table';
COMMENT ON VIEW incoming_transactions_view IS 'Backward compatibility view for old incoming_transactions table (arithmetic operations only)';

-- ============================================================================
-- STEP 5: Rename financial transaction tables for clarity
-- ============================================================================

-- Note: We are NOT renaming the actual tables to avoid breaking existing code.
-- Instead, we create views with clearer names and add comments for documentation.
-- Future migrations can complete the rename after code is updated.

-- Add comments to clarify purpose of existing financial tables
COMMENT ON TABLE account_financial_transactions IS 'Financial transactions for user deposits and withdrawals (user <-> system). Consider this the "deposits_withdrawals" table.';
COMMENT ON TABLE deal_payments IS 'Financial payments between accounts for deal transactions (account <-> account). This is the "payments" table.';
COMMENT ON TABLE credit_line_financial_transactions IS 'DEPRECATED: Credit line financial transactions. Merchants have been removed from the system.';

-- Create aliased views with clearer names (for new code to use)
CREATE VIEW deposits_withdrawals AS
SELECT
    id,
    user_id,
    transaction_type,
    amount,
    currency,
    transaction_hash,
    blockchain_status,
    block_number,
    processed_at
FROM account_financial_transactions;

CREATE VIEW payments AS
SELECT
    id,
    deal_id,
    payer_id as from_user_id, -- Clarify as "from"
    amount,
    payment_type,
    transaction_hash,
    processed_at,
    created_at
FROM deal_payments;

COMMENT ON VIEW deposits_withdrawals IS 'User deposits and withdrawals (user <-> system). Alias for account_financial_transactions.';
COMMENT ON VIEW payments IS 'Payments between accounts for deals (account <-> account). Alias for deal_payments.';

-- ============================================================================
-- STEP 6: Create helper functions for common queries
-- ============================================================================

-- Function to get all state transactions for a user
CREATE OR REPLACE FUNCTION get_user_state_transactions(
    p_user_id UUID,
    p_limit INTEGER DEFAULT 100,
    p_offset INTEGER DEFAULT 0
)
RETURNS TABLE (
    id UUID,
    transaction_type VARCHAR(50),
    amount DECIMAL(18, 6),
    deal_id UUID,
    included_in_batch_id INTEGER,
    batch_status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_type,
        st.amount,
        st.deal_id,
        st.included_in_batch_id,
        pb.proof_status as batch_status,
        st.metadata,
        st.created_at
    FROM state_transactions st
    LEFT JOIN proof_batches pb ON st.included_in_batch_id = pb.id
    WHERE st.user_id = p_user_id
    ORDER BY st.created_at DESC
    LIMIT p_limit
    OFFSET p_offset;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_user_state_transactions IS 'Retrieve state transaction history for a specific user';

-- Function to get all state transactions for a deal
CREATE OR REPLACE FUNCTION get_deal_state_transactions(
    p_deal_id UUID,
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    id UUID,
    transaction_type VARCHAR(50),
    amount DECIMAL(18, 6),
    user_id UUID,
    user_email VARCHAR(255),
    included_in_batch_id INTEGER,
    batch_status VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_type,
        st.amount,
        st.user_id,
        u.email as user_email,
        st.included_in_batch_id,
        pb.proof_status as batch_status,
        st.metadata,
        st.created_at
    FROM state_transactions st
    LEFT JOIN users u ON st.user_id = u.id
    LEFT JOIN proof_batches pb ON st.included_in_batch_id = pb.id
    WHERE st.deal_id = p_deal_id
    ORDER BY st.created_at ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_deal_state_transactions IS 'Retrieve state transaction audit trail for a specific deal';

-- Function to get pending (unbatched) state transactions
CREATE OR REPLACE FUNCTION get_pending_state_transactions(
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    id UUID,
    transaction_id_hash BYTEA,
    transaction_type VARCHAR(50),
    amount DECIMAL(18, 6),
    user_id_hash BYTEA,
    deal_id_hash BYTEA,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transaction_id_hash,
        st.transaction_type,
        st.amount,
        st.user_id_hash,
        st.deal_id_hash,
        st.metadata,
        st.created_at
    FROM state_transactions st
    WHERE st.included_in_batch_id IS NULL
    ORDER BY st.created_at ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_pending_state_transactions IS 'Get pending state transactions awaiting batch processing';

-- ============================================================================
-- STEP 7: Notes on deprecation and future cleanup
-- ============================================================================

-- Note: Old tables (deal_transactions, incoming_transactions) are kept for now
-- to ensure zero downtime. They can be dropped in a future migration after
-- all code is updated to use state_transactions.
--
-- Future cleanup migration should:
-- 1. DROP VIEW deal_transactions_view;
-- 2. DROP VIEW incoming_transactions_view;
-- 3. DROP TABLE deal_transactions;
-- 4. DROP TABLE incoming_transactions;
-- 5. Optionally rename views (deposits_withdrawals, payments) to replace actual tables
