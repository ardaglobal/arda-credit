-- Migration: Add Proof Supersession Tracking
--
-- This migration adds support for marking proofs as superseded when re-genesis occurs.
-- When a historical import or bulk upload requires reprocessing from genesis,
-- old proofs are marked as superseded with a link to the re-genesis transaction.

-- Add supersession tracking columns to proof_batches table
ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS superseded_at TIMESTAMP,
ADD COLUMN IF NOT EXISTS superseded_by_tx UUID REFERENCES deal_transactions(id);

-- Add index for efficient querying of non-superseded proofs
CREATE INDEX IF NOT EXISTS idx_proof_batches_not_superseded 
ON proof_batches(id, proof_status) 
WHERE superseded_at IS NULL;

-- Add index for audit trail queries (finding what superseded a proof)
CREATE INDEX IF NOT EXISTS idx_proof_batches_superseded_by 
ON proof_batches(superseded_by_tx) 
WHERE superseded_by_tx IS NOT NULL;

COMMENT ON COLUMN proof_batches.superseded_at IS 
'Timestamp when this proof was superseded due to re-genesis';

COMMENT ON COLUMN proof_batches.superseded_by_tx IS 
'Link to the StateRegenesis transaction that superseded this proof';

