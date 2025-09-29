-- Final migration to add all remaining missing columns

-- Add missing columns to various tables

-- Add available_balance to user-related tables
ALTER TABLE user_balances ADD COLUMN IF NOT EXISTS available_balance DECIMAL(18, 6) DEFAULT 0;

-- Add batch_id to nullifiers
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS batch_id INTEGER;

-- Add deposit_amount_usdc to relevant tables
ALTER TABLE usdc_deposits ADD COLUMN IF NOT EXISTS deposit_amount_usdc DECIMAL(18, 6);

-- Add event_id columns where needed
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS event_id VARCHAR(255);
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS event_id VARCHAR(255);

-- Add event_transaction_hash to usdc_deposits
ALTER TABLE usdc_deposits ADD COLUMN IF NOT EXISTS event_transaction_hash VARCHAR(255);

-- Add missing id columns where needed (if any table is missing primary key)
-- This might be for specific query contexts

-- Add listing_id references where needed
ALTER TABLE deal_listings ADD COLUMN IF NOT EXISTS listing_id UUID DEFAULT gen_random_uuid();

-- Add loan_id columns for compatibility (as aliases to deal_id)
ALTER TABLE deal_transfers ADD COLUMN IF NOT EXISTS loan_id UUID;
ALTER TABLE deal_payments ADD COLUMN IF NOT EXISTS loan_id UUID;

-- Update loan_id columns to match deal_id for existing records
UPDATE deal_transfers SET loan_id = deal_id WHERE loan_id IS NULL;
UPDATE deal_payments SET loan_id = deal_id WHERE loan_id IS NULL;

-- Update deposit_amount_usdc to match amount for existing records
UPDATE usdc_deposits SET deposit_amount_usdc = amount WHERE deposit_amount_usdc IS NULL;

-- Add foreign key constraints for new references
ALTER TABLE nullifiers ADD CONSTRAINT fk_nullifiers_batch_id
    FOREIGN KEY (batch_id) REFERENCES proof_batches(id) ON DELETE SET NULL;

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_user_balances_available_balance ON user_balances(available_balance);
CREATE INDEX IF NOT EXISTS idx_nullifiers_batch_id ON nullifiers(batch_id);
CREATE INDEX IF NOT EXISTS idx_usdc_deposits_deposit_amount ON usdc_deposits(deposit_amount_usdc);
CREATE INDEX IF NOT EXISTS idx_deal_transfers_loan_id ON deal_transfers(loan_id);
CREATE INDEX IF NOT EXISTS idx_deal_payments_loan_id ON deal_payments(loan_id);

-- Add comments for new columns
COMMENT ON COLUMN user_balances.available_balance IS 'Available balance for withdrawals and transactions';
COMMENT ON COLUMN nullifiers.batch_id IS 'Associated proof batch ID';
COMMENT ON COLUMN usdc_deposits.deposit_amount_usdc IS 'Deposit amount in USDC';
COMMENT ON COLUMN deal_transfers.loan_id IS 'Compatibility alias for deal_id';
COMMENT ON COLUMN deal_payments.loan_id IS 'Compatibility alias for deal_id';