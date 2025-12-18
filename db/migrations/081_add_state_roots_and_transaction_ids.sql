-- Add state roots and UUID transaction membership to proof_batches
-- 
-- This migration:
-- 1. Adds state_transaction_ids UUID[] for batch membership
-- 2. Adds previous_state_root and final_state_root BYTEA columns
-- 3. Standardizes contract_tx_hash (ensures it exists)
-- 4. Backfills data from existing state_transactions and ads_state_commits

-- ============================================================================
-- STEP 1: Add new columns
-- ============================================================================

-- Add state_transaction_ids UUID[] for tracking which state transactions are in this batch
ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS state_transaction_ids UUID[] NOT NULL DEFAULT '{}';

-- Add state root columns (will be 32-byte merkle roots)
ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS previous_state_root BYTEA;

ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS final_state_root BYTEA;

-- Ensure contract_tx_hash exists (may have been added in earlier migration as transaction_hash)
ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS contract_tx_hash VARCHAR(66);

-- ============================================================================
-- STEP 2: Backfill contract_tx_hash from transaction_hash if needed
-- ============================================================================

-- If transaction_hash column exists and contract_tx_hash is NULL, copy the value
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'proof_batches' AND column_name = 'transaction_hash'
    ) THEN
        UPDATE proof_batches
        SET contract_tx_hash = transaction_hash
        WHERE contract_tx_hash IS NULL AND transaction_hash IS NOT NULL;
        
        RAISE NOTICE 'Backfilled contract_tx_hash from transaction_hash';
    END IF;
END $$;

-- ============================================================================
-- STEP 3: Backfill state_transaction_ids from state_transactions
-- ============================================================================

-- For each batch, populate state_transaction_ids with the UUIDs of state transactions
-- that reference this batch
UPDATE proof_batches pb
SET state_transaction_ids = (
    SELECT COALESCE(ARRAY_AGG(st.id ORDER BY st.created_at, st.id), '{}')
    FROM state_transactions st
    WHERE st.included_in_batch_id = pb.id
)
WHERE state_transaction_ids = '{}';

-- Log the backfill
DO $$
DECLARE
    updated_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO updated_count
    FROM proof_batches
    WHERE array_length(state_transaction_ids, 1) > 0;
    
    RAISE NOTICE 'Backfilled state_transaction_ids for % batches', updated_count;
END $$;

-- ============================================================================
-- STEP 4: Backfill final_state_root from ads_state_commits
-- ============================================================================

-- Set final_state_root from the merkle_root in ads_state_commits
UPDATE proof_batches pb
SET final_state_root = ads.merkle_root
FROM ads_state_commits ads
WHERE ads.batch_id = pb.id
  AND pb.final_state_root IS NULL;

-- Log the backfill
DO $$
DECLARE
    updated_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO updated_count
    FROM proof_batches
    WHERE final_state_root IS NOT NULL;
    
    RAISE NOTICE 'Backfilled final_state_root for % batches', updated_count;
END $$;

-- ============================================================================
-- STEP 5: Backfill previous_state_root from prior batch's final_state_root
-- ============================================================================

-- For each batch, set previous_state_root to the final_state_root of the previous batch
-- (ordered by id). Genesis batch (first batch or batch with no prior proven batch) gets zeros.

WITH batch_sequence AS (
    SELECT 
        pb.id,
        LAG(pb.final_state_root) OVER (ORDER BY pb.id) AS prev_batch_final_root
    FROM proof_batches pb
)
UPDATE proof_batches pb
SET previous_state_root = COALESCE(
    bs.prev_batch_final_root,
    decode('0000000000000000000000000000000000000000000000000000000000000000', 'hex')
)
FROM batch_sequence bs
WHERE bs.id = pb.id
  AND pb.previous_state_root IS NULL;

-- Log the backfill
DO $$
DECLARE
    updated_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO updated_count
    FROM proof_batches
    WHERE previous_state_root IS NOT NULL;
    
    RAISE NOTICE 'Backfilled previous_state_root for % batches', updated_count;
END $$;

-- ============================================================================
-- STEP 6: Add indexes for performance
-- ============================================================================

-- Index for querying batches by state transaction membership
CREATE INDEX IF NOT EXISTS idx_proof_batches_state_transaction_ids 
ON proof_batches USING GIN (state_transaction_ids);

-- Index for contract tx hash lookups
CREATE INDEX IF NOT EXISTS idx_proof_batches_contract_tx_hash 
ON proof_batches(contract_tx_hash)
WHERE contract_tx_hash IS NOT NULL;

-- Index on state_transactions for the backfill query (if not already exists)
CREATE INDEX IF NOT EXISTS idx_state_transactions_batch_id_created 
ON state_transactions(included_in_batch_id, created_at, id)
WHERE included_in_batch_id IS NOT NULL;

-- ============================================================================
-- STEP 7: Add length constraints for state roots (once backfilled)
-- ============================================================================

-- Add check constraints to ensure state roots are exactly 32 bytes when set
ALTER TABLE proof_batches 
ADD CONSTRAINT chk_previous_state_root_length 
CHECK (previous_state_root IS NULL OR length(previous_state_root) = 32);

ALTER TABLE proof_batches 
ADD CONSTRAINT chk_final_state_root_length 
CHECK (final_state_root IS NULL OR length(final_state_root) = 32);

-- ============================================================================
-- STEP 8: Add comments
-- ============================================================================

COMMENT ON COLUMN proof_batches.state_transaction_ids IS 
'UUIDs of state_transactions included in this batch (source of truth for membership)';

COMMENT ON COLUMN proof_batches.previous_state_root IS 
'Merkle root state before this batch was applied (32 bytes, genesis batch uses 0x00..00)';

COMMENT ON COLUMN proof_batches.final_state_root IS 
'Merkle root state after this batch was applied (32 bytes, from ads_state_commits)';

COMMENT ON COLUMN proof_batches.contract_tx_hash IS 
'Ethereum transaction hash when batch proof was posted to smart contract (standardized field)';

