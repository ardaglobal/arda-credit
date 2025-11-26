-- Add CASCADE delete behavior to deal foreign keys
-- This migration allows deals to be deleted along with all related records

-- deal_payments: cascade delete all payments when deal is deleted
ALTER TABLE deal_payments
  DROP CONSTRAINT IF EXISTS deal_payments_deal_id_fkey,
  ADD CONSTRAINT deal_payments_deal_id_fkey
    FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE;

-- deal_listings: cascade delete marketplace listings when deal is deleted
ALTER TABLE deal_listings
  DROP CONSTRAINT IF EXISTS deal_listings_deal_id_fkey,
  ADD CONSTRAINT deal_listings_deal_id_fkey
    FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE;

-- deal_transfers: cascade delete transfer records when deal is deleted
ALTER TABLE deal_transfers
  DROP CONSTRAINT IF EXISTS deal_transfers_deal_id_fkey,
  ADD CONSTRAINT deal_transfers_deal_id_fkey
    FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE;

-- deal_transactions: cascade delete transaction records when deal is deleted
ALTER TABLE deal_transactions
  DROP CONSTRAINT IF EXISTS deal_transactions_deal_id_fkey,
  ADD CONSTRAINT deal_transactions_deal_id_fkey
    FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE;
