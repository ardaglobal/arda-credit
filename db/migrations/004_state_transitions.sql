-- ============================================================================
-- STATE TRANSITIONS TABLE
-- ZK circuit audit trail and batch processing integration
-- Consolidates deal_transactions, document_state_transitions, and state_snapshots
-- ============================================================================

-- Transaction types for ZK proof integration
CREATE TYPE transaction_type AS ENUM (
    -- User management
    'user_registration',
    'kyc_submission',
    'kyc_verification',

    -- Deal operations
    'deal_origination',
    'deal_payment',
    'deal_closeout',

    -- Financial operations
    'user_deposit',
    'user_withdrawal',

    -- Pool operations
    'pool_creation',
    'pool_funding',

    -- Document operations
    'document_upload',
    'document_approval',
    'document_deletion',

    -- Marketplace operations
    'deal_listing',
    'deal_transfer',

    -- System operations
    'balance_update'
);

-- ============================================================================
-- TRANSACTION QUEUE FOR ZK PROCESSING
-- ============================================================================

-- Table for transactions waiting to be included in ZK proofs
CREATE TABLE transaction_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id_hash BYTEA NOT NULL UNIQUE, -- SHA256 hash for zkVM compatibility
    user_id_hash BYTEA NOT NULL, -- SHA256 hash of user UUID
    deal_id_hash BYTEA, -- SHA256 hash of deal UUID, NULL for non-deal transactions
    transaction_type transaction_type NOT NULL,
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0,

    -- Foreign key references to actual records
    user_id UUID REFERENCES users(id),
    deal_id UUID REFERENCES deals(id),
    payment_id UUID REFERENCES deal_payments(id),
    company_id UUID REFERENCES companies(id),

    -- Batch processing integration
    included_in_batch_id INTEGER,
    batch_position INTEGER, -- Position within the batch

    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- PROOF BATCHES
-- ============================================================================

-- Table for proof batches (existing structure from current system)
CREATE TABLE proof_batches (
    id SERIAL PRIMARY KEY,
    previous_counter_value BIGINT NOT NULL DEFAULT 0,
    final_counter_value BIGINT NOT NULL DEFAULT 0,
    transaction_ids INTEGER[] DEFAULT '{}', -- For arithmetic transactions compatibility
    sindri_proof_id VARCHAR(255),
    proof_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (proof_status IN ('pending', 'proven', 'failed')),
    transaction_hash VARCHAR(66), -- On-chain transaction hash
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    proven_at TIMESTAMP WITH TIME ZONE
);

-- ============================================================================
-- STATE SNAPSHOTS
-- ============================================================================

-- Table for point-in-time state snapshots
CREATE TABLE state_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_type VARCHAR(20) NOT NULL DEFAULT 'manual' CHECK (snapshot_type IN ('manual', 'batch', 'scheduled')),
    batch_id INTEGER REFERENCES proof_batches(id),
    merkle_root BYTEA,

    -- Aggregate statistics
    total_deals_count INTEGER NOT NULL DEFAULT 0,
    total_outstanding_balance DECIMAL(18, 6) NOT NULL DEFAULT 0,
    total_paid_amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
    active_users_count INTEGER NOT NULL DEFAULT 0,
    kyc_approved_count INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- ADS/MERKLE TREE STATE MANAGEMENT
-- ============================================================================

-- Table for ADS state commits (smart contract integration)
CREATE TABLE ads_state_commits (
    id SERIAL PRIMARY KEY,
    batch_id INTEGER NOT NULL REFERENCES proof_batches(id),
    merkle_root BYTEA NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Global counter state tracking
CREATE TABLE global_state (
    id SERIAL PRIMARY KEY,
    counter_value BIGINT NOT NULL DEFAULT 0,
    merkle_root BYTEA,
    last_batch_id INTEGER REFERENCES proof_batches(id),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert initial global state
INSERT INTO global_state (counter_value, merkle_root, last_batch_id)
VALUES (0, NULL, NULL);

-- ============================================================================
-- INCOMING TRANSACTIONS (for existing compatibility)
-- ============================================================================

-- Table for simple arithmetic transactions (existing system compatibility)
CREATE TABLE incoming_transactions (
    id SERIAL PRIMARY KEY,
    amount INTEGER NOT NULL,
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Transaction queue indexes
CREATE INDEX idx_transaction_queue_user_id_hash ON transaction_queue(user_id_hash);
CREATE INDEX idx_transaction_queue_deal_id_hash ON transaction_queue(deal_id_hash) WHERE deal_id_hash IS NOT NULL;
CREATE INDEX idx_transaction_queue_type ON transaction_queue(transaction_type);
CREATE INDEX idx_transaction_queue_batch_id ON transaction_queue(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_transaction_queue_not_batched ON transaction_queue(id) WHERE included_in_batch_id IS NULL;
CREATE INDEX idx_transaction_queue_created_at ON transaction_queue(created_at);

-- Proof batch indexes
CREATE INDEX idx_proof_batches_status ON proof_batches(proof_status);
CREATE INDEX idx_proof_batches_created_at ON proof_batches(created_at);
CREATE INDEX idx_proof_batches_sindri_id ON proof_batches(sindri_proof_id) WHERE sindri_proof_id IS NOT NULL;

-- State snapshot indexes
CREATE INDEX idx_state_snapshots_type ON state_snapshots(snapshot_type);
CREATE INDEX idx_state_snapshots_batch_id ON state_snapshots(batch_id) WHERE batch_id IS NOT NULL;
CREATE INDEX idx_state_snapshots_created_at ON state_snapshots(created_at);

-- ADS state indexes
CREATE INDEX idx_ads_state_commits_batch_id ON ads_state_commits(batch_id);
CREATE INDEX idx_ads_state_commits_created_at ON ads_state_commits(created_at);

-- Incoming transaction indexes (compatibility)
CREATE INDEX idx_incoming_transactions_batch_id ON incoming_transactions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_incoming_transactions_not_batched ON incoming_transactions(id) WHERE included_in_batch_id IS NULL;

-- ============================================================================
-- BATCH PROCESSING FUNCTIONS
-- ============================================================================

-- Function to get current counter value
CREATE OR REPLACE FUNCTION get_current_counter_value()
RETURNS BIGINT AS $$
DECLARE
    current_counter BIGINT;
BEGIN
    SELECT counter_value INTO current_counter
    FROM global_state
    ORDER BY id DESC
    LIMIT 1;

    RETURN COALESCE(current_counter, 0);
END;
$$ LANGUAGE plpgsql;

-- Function to get unbatched transactions ready for batching
CREATE OR REPLACE FUNCTION get_unbatched_transactions(limit_count INTEGER DEFAULT 10)
RETURNS TABLE(
    transaction_id UUID,
    transaction_id_hash BYTEA,
    user_id_hash BYTEA,
    deal_id_hash BYTEA,
    transaction_type transaction_type,
    amount DECIMAL(18, 6),
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        tq.id,
        tq.transaction_id_hash,
        tq.user_id_hash,
        tq.deal_id_hash,
        tq.transaction_type,
        tq.amount,
        tq.created_at
    FROM transaction_queue tq
    WHERE tq.included_in_batch_id IS NULL
    ORDER BY tq.created_at ASC -- FIFO: oldest first
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to create batch with mixed transaction types
CREATE OR REPLACE FUNCTION create_batch_with_transactions(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    arithmetic_total INTEGER;
    arithmetic_transaction_ids INTEGER[];
    zk_transaction_ids UUID[];
BEGIN
    -- Get current counter value
    SELECT get_current_counter_value() INTO previous_counter;

    -- Get unbatched arithmetic transactions
    SELECT
        ARRAY_AGG(id ORDER BY id),
        SUM(amount)
    INTO arithmetic_transaction_ids, arithmetic_total
    FROM (
        SELECT id, amount
        FROM incoming_transactions
        WHERE included_in_batch_id IS NULL
        ORDER BY id ASC
        LIMIT batch_size / 2  -- Reserve half for arithmetic, half for ZK
    ) unbatched;

    -- Get unbatched ZK transactions
    SELECT ARRAY_AGG(id ORDER BY created_at)
    INTO zk_transaction_ids
    FROM (
        SELECT id
        FROM transaction_queue
        WHERE included_in_batch_id IS NULL
        ORDER BY created_at ASC
        LIMIT batch_size / 2
    ) unbatched_zk;

    -- Return 0 if no transactions to batch
    IF (arithmetic_transaction_ids IS NULL OR array_length(arithmetic_transaction_ids, 1) = 0) AND
       (zk_transaction_ids IS NULL OR array_length(zk_transaction_ids, 1) = 0) THEN
        RETURN 0;
    END IF;

    -- Calculate final counter value (only arithmetic transactions affect counter)
    final_counter := previous_counter + COALESCE(arithmetic_total, 0);

    -- Create new batch
    INSERT INTO proof_batches (
        previous_counter_value,
        final_counter_value,
        transaction_ids
    ) VALUES (
        previous_counter,
        final_counter,
        COALESCE(arithmetic_transaction_ids, ARRAY[]::INTEGER[])
    ) RETURNING id INTO new_batch_id;

    -- Mark arithmetic transactions as included
    IF arithmetic_transaction_ids IS NOT NULL AND array_length(arithmetic_transaction_ids, 1) > 0 THEN
        UPDATE incoming_transactions
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(arithmetic_transaction_ids);
    END IF;

    -- Mark ZK transactions as included
    IF zk_transaction_ids IS NOT NULL AND array_length(zk_transaction_ids, 1) > 0 THEN
        UPDATE transaction_queue
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(zk_transaction_ids);
    END IF;

    -- Update global state
    UPDATE global_state
    SET
        counter_value = final_counter,
        last_batch_id = new_batch_id,
        updated_at = NOW()
    WHERE id = (SELECT MAX(id) FROM global_state);

    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE transaction_queue IS 'Transactions waiting to be included in ZK proofs with batch processing integration';
COMMENT ON TABLE proof_batches IS 'Batches of transactions with ZK proofs for privacy-preserving verification';
COMMENT ON TABLE state_snapshots IS 'Point-in-time state captures for auditing and rollback capabilities';
COMMENT ON TABLE ads_state_commits IS 'ADS/Merkle tree state commitments for smart contract integration';
COMMENT ON TABLE global_state IS 'Global counter and Merkle root state tracking';
COMMENT ON TABLE incoming_transactions IS 'Simple arithmetic transactions for existing system compatibility';

COMMENT ON COLUMN transaction_queue.transaction_id_hash IS 'SHA256 hash of transaction ID for zkVM privacy';
COMMENT ON COLUMN transaction_queue.user_id_hash IS 'SHA256 hash of user UUID for zkVM privacy';
COMMENT ON COLUMN transaction_queue.deal_id_hash IS 'SHA256 hash of deal UUID for zkVM privacy (NULL for non-deal transactions)';
COMMENT ON COLUMN proof_batches.sindri_proof_id IS 'Sindri service proof identifier for ZK proof verification';
COMMENT ON COLUMN proof_batches.transaction_hash IS 'On-chain transaction hash when proof is submitted to smart contract';