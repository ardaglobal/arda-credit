-- Migration: Add wallet_address column to identities table
-- Description: Add Ethereum wallet address support for user accounts

BEGIN;

-- Add wallet_address column to identities table
ALTER TABLE identities ADD COLUMN wallet_address VARCHAR(42);

-- Add index for wallet address lookups
CREATE INDEX idx_identities_wallet_address ON identities(wallet_address) 
WHERE wallet_address IS NOT NULL;

-- Add unique constraint for wallet addresses (each wallet can only be linked to one identity)
CREATE UNIQUE INDEX idx_identities_wallet_address_unique ON identities(wallet_address) 
WHERE wallet_address IS NOT NULL;

-- Add comment for documentation
COMMENT ON COLUMN identities.wallet_address IS 'Ethereum wallet address (0x + 40 chars) for blockchain integration';

COMMIT;