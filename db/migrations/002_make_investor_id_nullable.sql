-- Make investor_id nullable in deals table to support draft deals
-- This allows deals to be created without an assigned investor

ALTER TABLE deals
  ALTER COLUMN investor_id DROP NOT NULL;

-- Add a check constraint to ensure finalized deals have an investor
ALTER TABLE deals
  ADD CONSTRAINT deals_finalized_investor_check
  CHECK (
    (status IN ('draft', 'under_review', 'negotiation'))
    OR
    (investor_id IS NOT NULL)
  );

-- Add comment explaining the business logic
COMMENT ON COLUMN deals.investor_id IS
  'Investor (was lender) who provides capital. Can be NULL for draft deals.
   Must be set before deal moves to finalized, executed, or active status.';
