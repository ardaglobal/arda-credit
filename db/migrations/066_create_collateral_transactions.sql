-- Migration: Create collateral_transactions table
-- Description: Creates a separate table to track collateral lifecycle events
--              for comprehensive audit trail support

-- ============================================================================
-- STEP 1: Create collateral_transactions table
-- ============================================================================

CREATE TABLE collateral_transactions (
    transaction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    transaction_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    deal_id UUID REFERENCES deals(id) ON DELETE SET NULL,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- ============================================================================
-- STEP 2: Create indexes for efficient querying
-- ============================================================================

-- Index for audit trail queries (most common use case)
CREATE INDEX idx_collateral_txn_collateral_id ON collateral_transactions(collateral_id);

-- Index for chronological ordering
CREATE INDEX idx_collateral_txn_created_at ON collateral_transactions(created_at);

-- Index for filtering by transaction type
CREATE INDEX idx_collateral_txn_type ON collateral_transactions(transaction_type);

-- Composite index for collateral + chronological queries
CREATE INDEX idx_collateral_txn_collateral_created ON collateral_transactions(collateral_id, created_at DESC);

-- Index for deal association queries
CREATE INDEX idx_collateral_txn_deal_id ON collateral_transactions(deal_id) WHERE deal_id IS NOT NULL;

-- Index for user audit queries
CREATE INDEX idx_collateral_txn_created_by ON collateral_transactions(created_by) WHERE created_by IS NOT NULL;

-- ============================================================================
-- STEP 3: Add comments
-- ============================================================================

COMMENT ON TABLE collateral_transactions IS 'Transaction event log for collateral lifecycle tracking and audit trail';
COMMENT ON COLUMN collateral_transactions.transaction_id IS 'Unique identifier for the transaction event';
COMMENT ON COLUMN collateral_transactions.collateral_id IS 'Reference to the collateral asset';
COMMENT ON COLUMN collateral_transactions.transaction_type IS 'Type of transaction (collateral_creation, collateral_update, etc.)';
COMMENT ON COLUMN collateral_transactions.created_at IS 'Timestamp when the transaction event occurred';
COMMENT ON COLUMN collateral_transactions.created_by IS 'User who initiated the transaction (nullable for system events)';
COMMENT ON COLUMN collateral_transactions.deal_id IS 'Associated deal ID (for deal association/disassociation events)';
COMMENT ON COLUMN collateral_transactions.metadata IS 'Flexible JSONB field for event-specific data (changes, properties, etc.)';

-- ============================================================================
-- STEP 4: Create helper function for audit trail retrieval
-- ============================================================================

-- Function to get collateral audit trail with user details
CREATE OR REPLACE FUNCTION get_collateral_audit_trail(
    p_collateral_id UUID,
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE (
    transaction_id UUID,
    collateral_id UUID,
    transaction_type VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE,
    created_by UUID,
    user_email VARCHAR(255),
    deal_id UUID,
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ct.transaction_id,
        ct.collateral_id,
        ct.transaction_type,
        ct.created_at,
        ct.created_by,
        u.email as user_email,
        ct.deal_id,
        ct.metadata
    FROM collateral_transactions ct
    LEFT JOIN users u ON ct.created_by = u.id
    WHERE ct.collateral_id = p_collateral_id
    ORDER BY ct.created_at ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_collateral_audit_trail(UUID, INTEGER) IS 'Retrieve audit trail for a collateral asset with user details';
