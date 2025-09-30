-- Rename loan_status to deal_status in deals table
-- This completes the migration from loan terminology to deal terminology

-- ============================================================================
-- RENAME COLUMN IN DEALS TABLE
-- ============================================================================

-- Rename loan_status to deal_status
ALTER TABLE deals RENAME COLUMN loan_status TO deal_status;

-- Update the check constraint to use new column name
ALTER TABLE deals DROP CONSTRAINT IF EXISTS deals_loan_status_check;
ALTER TABLE deals DROP CONSTRAINT IF EXISTS loans_loan_status_check;

-- Add new constraint with deal_status
ALTER TABLE deals ADD CONSTRAINT deals_deal_status_check
    CHECK (deal_status IN ('originated', 'active', 'paid_off', 'closed', 'defaulted'));

-- ============================================================================
-- UPDATE INDEXES
-- ============================================================================

-- Drop old index if it exists
DROP INDEX IF EXISTS idx_loans_status;
DROP INDEX IF EXISTS idx_deals_loan_status;

-- Create new index with updated name
CREATE INDEX IF NOT EXISTS idx_deals_deal_status ON deals(deal_status);

-- ============================================================================
-- UPDATE VIEWS
-- ============================================================================

-- Recreate deal_audit_view if it exists
DROP VIEW IF EXISTS deal_audit_view CASCADE;

CREATE VIEW deal_audit_view AS
SELECT
    d.id as deal_id,
    d.custom_deal_id,
    d.originator_id,
    d.investor_id,
    d.principal_amount,
    d.outstanding_balance,
    d.deal_status,
    d.title,
    d.description,
    d.originated_at,
    d.created_at,
    d.updated_at,
    COUNT(dp.id) as payment_count,
    COALESCE(SUM(dp.amount), 0) as total_payments,
    CASE
        WHEN d.outstanding_balance = 0 THEN 'PAID_OFF'
        WHEN d.outstanding_balance < d.principal_amount THEN 'IN_PROGRESS'
        ELSE 'ORIGINATED'
    END as payment_status
FROM deals d
LEFT JOIN deal_payments dp ON d.id = dp.deal_id
GROUP BY d.id, d.custom_deal_id, d.originator_id, d.investor_id,
         d.principal_amount, d.outstanding_balance, d.deal_status,
         d.title, d.description, d.originated_at, d.created_at, d.updated_at;

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================
--
-- This migration completes the loan -> deal terminology conversion by:
-- 1. Renaming loan_status column to deal_status
-- 2. Updating related constraints and indexes
-- 3. Updating any views that reference the old column name
--
-- This is a breaking change for any code that directly references loan_status
-- All application code must be updated to use deal_status instead