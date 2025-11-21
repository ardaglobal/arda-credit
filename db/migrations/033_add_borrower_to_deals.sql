-- Migration: Add borrower_id to deals table
-- Description: Adds an optional borrower_id field to represent a third-party company
--              involved in the deal (separate from originator and investor)

-- Add borrower_id column (optional company reference)
ALTER TABLE deals
    ADD COLUMN borrower_id UUID REFERENCES companies(id) ON DELETE SET NULL;

-- Add index for borrower_id queries
CREATE INDEX idx_deals_borrower_id ON deals(borrower_id);

-- Add comment
COMMENT ON COLUMN deals.borrower_id IS 'Company ID of the borrower (optional third party). NULL means no borrower exists (originator is NOT assumed to be the borrower).';

-- Update constraint to ensure borrower is different from originator and investor
-- First, drop the old constraint
ALTER TABLE deals DROP CONSTRAINT IF EXISTS originator_investor_different;

-- Add new constraint that checks all three parties are different
ALTER TABLE deals
    ADD CONSTRAINT deal_parties_different
    CHECK (
        -- Originator and investor must be different (if investor exists)
        (investor_id IS NULL OR originator_id != investor_id) AND
        -- If borrower exists, it must be different from originator and investor
        (borrower_id IS NULL OR
         (borrower_id != originator_id AND (investor_id IS NULL OR borrower_id != investor_id)))
    );
