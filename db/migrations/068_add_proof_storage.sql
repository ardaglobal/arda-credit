-- Migration: Add Proof Storage to Database
-- Date: 2024-12-12
-- Purpose: Store complete proof data (bytes, public values, verifying key) in database
--          to eliminate dependency on external services for contract submission

-- Add columns to store actual proof data in proof_batches table
ALTER TABLE proof_batches
ADD COLUMN IF NOT EXISTS proof_bytes BYTEA,                    -- The actual ZK proof
ADD COLUMN IF NOT EXISTS public_values BYTEA,                  -- Public inputs/outputs  
ADD COLUMN IF NOT EXISTS verifying_key BYTEA,                  -- Verification key
ADD COLUMN IF NOT EXISTS proof_size_bytes INTEGER,             -- For monitoring/metrics
ADD COLUMN IF NOT EXISTS public_values_size_bytes INTEGER,     -- For monitoring/metrics
ADD COLUMN IF NOT EXISTS stored_at TIMESTAMP WITH TIME ZONE,   -- When proof was persisted
ADD COLUMN IF NOT EXISTS contract_tx_hash VARCHAR(66);         -- Transaction hash of submission

-- Add index for efficient queries of unposted proofs with data
CREATE INDEX IF NOT EXISTS idx_proof_batches_unposted_with_data 
ON proof_batches(proof_status, posted_to_contract, stored_at)
WHERE proof_status = 'proven' AND posted_to_contract = FALSE AND proof_bytes IS NOT NULL;

-- Add index for proof data availability queries
CREATE INDEX IF NOT EXISTS idx_proof_batches_proof_data_available
ON proof_batches(id)
WHERE proof_bytes IS NOT NULL;

-- Add comments for documentation
COMMENT ON COLUMN proof_batches.proof_bytes IS 'Complete ZK proof bytes for contract submission (1-10 MB). Stored locally for self-sufficiency.';
COMMENT ON COLUMN proof_batches.public_values IS 'Public values (inputs/outputs) from proof generation. Required for contract verification.';
COMMENT ON COLUMN proof_batches.verifying_key IS 'Verification key for on-chain proof verification. Must match deployed contract verifier.';
COMMENT ON COLUMN proof_batches.proof_size_bytes IS 'Size of proof_bytes in bytes. Used for monitoring and alerting.';
COMMENT ON COLUMN proof_batches.public_values_size_bytes IS 'Size of public_values in bytes. Used for monitoring.';
COMMENT ON COLUMN proof_batches.stored_at IS 'Timestamp when proof data was persisted to database. NULL if only proof_id stored.';
COMMENT ON COLUMN proof_batches.contract_tx_hash IS 'Ethereum transaction hash of smart contract submission. NULL if not yet posted.';

