-- ============================================================================
-- POSEIDON HASH MIGRATION - CLEAN BREAK
-- ============================================================================
-- This migration switches the Indexed Merkle Tree from SHA-256 to Poseidon2
-- hashing for ZK-circuit efficiency.
--
-- Migration Strategy: CLEAN BREAK
-- - All existing merkle nodes are cleared
-- - All nullifiers are cleared
-- - Tree state is reset with Poseidon2 zero hash
-- - System will need to reprocess transactions after migration
--
-- Performance Impact:
-- - Before: SHA-256 (~27,000 R1CS constraints per hash)
-- - After: Poseidon2 (~8 Plonky3 constraints per hash)
-- - Speedup: ~3,400x reduction in constraint count
-- ============================================================================

DO $$
DECLARE
    poseidon_zero_hash BYTEA;
BEGIN
    -- Log the migration start
    RAISE NOTICE 'Starting Poseidon2 hash migration (clean break)';
    
    -- Clear all existing merkle tree data
    RAISE NOTICE 'Clearing merkle nodes...';
    TRUNCATE TABLE merkle_nodes;
    
    RAISE NOTICE 'Clearing nullifiers...';
    TRUNCATE TABLE nullifiers CASCADE;
    
    -- Reset tree state with Poseidon2 zero hash
    -- The zero hash will be computed by the application code on first use
    -- We use a sentinel value (all zeros) that will be replaced on first insertion
    poseidon_zero_hash := '\x0000000000000000000000000000000000000000000000000000000000000000'::bytea;
    
    RAISE NOTICE 'Resetting tree state...';
    UPDATE tree_state 
    SET root_hash = poseidon_zero_hash,
        next_available_index = 0,
        total_nullifiers = 0,
        total_loan_originations = 0,
        total_loan_payments = 0,
        total_loan_closeouts = 0,
        total_loan_amount = 0,
        active_loans_count = 0,
        updated_at = NOW(),
        loan_stats_updated_at = NOW()
    WHERE tree_id = 'default';
    
    -- If tree_state doesn't exist, insert it
    INSERT INTO tree_state (
        tree_id, 
        root_hash, 
        next_available_index, 
        tree_height, 
        total_nullifiers,
        total_loan_originations,
        total_loan_payments,
        total_loan_closeouts,
        total_loan_amount,
        active_loans_count
    )
    VALUES (
        'default', 
        poseidon_zero_hash, 
        0, 
        32, 
        0,
        0,
        0,
        0,
        0,
        0
    )
    ON CONFLICT (tree_id) DO NOTHING;
    
    -- Log completion
    RAISE NOTICE '✅ Poseidon2 migration complete - tree state reset';
    RAISE NOTICE '⚠️  All transactions will need to be reprocessed through batch processing';
    RAISE NOTICE '   Run: make server (will auto-trigger batch processing)';
    
END $$;

-- Add migration comment for documentation
COMMENT ON TABLE tree_state IS 'Merkle tree state using Poseidon2 hashing (migrated from SHA-256)';
COMMENT ON TABLE merkle_nodes IS 'Merkle tree nodes with Poseidon2 hashes (32 bytes each)';
COMMENT ON TABLE nullifiers IS 'Indexed nullifier set with Poseidon2 Merkle tree integration';

