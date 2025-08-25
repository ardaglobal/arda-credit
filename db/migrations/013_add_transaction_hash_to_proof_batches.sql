-- Add transaction hash tracking to proof_batches table
-- This allows us to store the Ethereum transaction hash for confirmed batch submissions

-- Add the transaction_hash column to store the Ethereum tx hash after confirmation
ALTER TABLE proof_batches 
ADD COLUMN IF NOT EXISTS transaction_hash VARCHAR(66);

-- Add index for efficient lookups by transaction hash
CREATE INDEX IF NOT EXISTS idx_proof_batches_transaction_hash 
ON proof_batches(transaction_hash)
WHERE transaction_hash IS NOT NULL;

-- Add comment explaining the field
COMMENT ON COLUMN proof_batches.transaction_hash IS 'Ethereum transaction hash of the confirmed batch submission to smart contract';