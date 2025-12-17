-- Migration: Add Query Columns to State Transactions
-- Description: Adds extracted UUID columns for fast indexed queries while keeping
--              transaction_data BYTEA as the source of truth. These columns are
--              denormalized for query performance but are NOT authoritative.
--
-- Changes:
-- 1. Add nullable UUID columns: user_id, deal_id, originator_company_id, investor_company_id, collateral_id
-- 2. Add foreign key constraints where applicable
-- 3. Create partial indexes on all query columns
-- 4. Create composite indexes for common query patterns

BEGIN;

-- ============================================================================
-- STEP 1: Add extracted UUID columns for querying
-- ============================================================================

ALTER TABLE state_transactions
    ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN deal_id UUID REFERENCES deals(id) ON DELETE SET NULL,
    ADD COLUMN originator_company_id UUID REFERENCES companies(id) ON DELETE SET NULL,
    ADD COLUMN investor_company_id UUID REFERENCES companies(id) ON DELETE SET NULL,
    ADD COLUMN collateral_id UUID REFERENCES collateral(id) ON DELETE SET NULL;

-- Add column comments explaining denormalized nature
COMMENT ON COLUMN state_transactions.user_id IS 
    'Extracted user UUID for fast querying (denormalized, not authoritative). Source of truth is in transaction_data.';
COMMENT ON COLUMN state_transactions.deal_id IS 
    'Extracted deal UUID for fast querying (denormalized, not authoritative). Source of truth is in transaction_data.';
COMMENT ON COLUMN state_transactions.originator_company_id IS 
    'Extracted originator company UUID for fast querying (denormalized, not authoritative). Source of truth is in transaction_data.';
COMMENT ON COLUMN state_transactions.investor_company_id IS 
    'Extracted investor company UUID for fast querying (denormalized, not authoritative). Source of truth is in transaction_data.';
COMMENT ON COLUMN state_transactions.collateral_id IS 
    'Extracted collateral UUID for fast querying (denormalized, not authoritative). Source of truth is in transaction_data.';

-- ============================================================================
-- STEP 2: Create partial indexes for fast queries
-- ============================================================================

-- Index for user-based queries
CREATE INDEX idx_state_transactions_user_id 
    ON state_transactions(user_id) 
    WHERE user_id IS NOT NULL;

-- Index for deal-based queries
CREATE INDEX idx_state_transactions_deal_id 
    ON state_transactions(deal_id) 
    WHERE deal_id IS NOT NULL;

-- Index for originator company queries
CREATE INDEX idx_state_transactions_originator_company_id 
    ON state_transactions(originator_company_id) 
    WHERE originator_company_id IS NOT NULL;

-- Index for investor company queries
CREATE INDEX idx_state_transactions_investor_company_id 
    ON state_transactions(investor_company_id) 
    WHERE investor_company_id IS NOT NULL;

-- Index for collateral queries
CREATE INDEX idx_state_transactions_collateral_id 
    ON state_transactions(collateral_id) 
    WHERE collateral_id IS NOT NULL;

-- ============================================================================
-- STEP 3: Create composite indexes for common query patterns
-- ============================================================================

-- Composite index for deal audit trails (deal_id + transaction_type + created_at)
CREATE INDEX idx_state_transactions_deal_type 
    ON state_transactions(deal_id, transaction_type, created_at DESC) 
    WHERE deal_id IS NOT NULL;

-- Composite index for user audit trails (user_id + transaction_type + created_at)
CREATE INDEX idx_state_transactions_user_type 
    ON state_transactions(user_id, transaction_type, created_at DESC) 
    WHERE user_id IS NOT NULL;

-- Composite index for company audit trails (originator OR investor)
-- Note: We create separate indexes for each since OR queries are harder to optimize
-- Applications should query both and union results if needed

-- ============================================================================
-- STEP 4: Update table comment to reflect new columns
-- ============================================================================

COMMENT ON TABLE state_transactions IS 
    'Unified audit trail for all ZK state transition function transactions. '
    'Stores serialized transaction data (transaction_data) as source of truth. '
    'Extracted UUID columns (user_id, deal_id, etc.) are denormalized for fast querying.';

COMMIT;

