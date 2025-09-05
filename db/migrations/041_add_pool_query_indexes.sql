-- Add database indexes for liquidity pool query optimization
-- This migration adds performance indexes for frequent pool query patterns

-- ============================================================================
-- LIQUIDITY POOL QUERY INDEXES
-- ============================================================================

-- Index for filtering pools by investor and status (most common query pattern)
CREATE INDEX IF NOT EXISTS idx_pools_investor_status ON liquidity_pools(investor_id, status);

-- Index for ordering pools by creation date
CREATE INDEX IF NOT EXISTS idx_pools_created_at ON liquidity_pools(created_at DESC);

-- Index for ordering pools by updated date
CREATE INDEX IF NOT EXISTS idx_pools_updated_at ON liquidity_pools(updated_at DESC);

-- Index for ordering pools by total deposits (dashboard analytics)
CREATE INDEX IF NOT EXISTS idx_pools_total_deposits ON liquidity_pools(total_deposits DESC);

-- Index for calculating utilization rates (analytics queries)
CREATE INDEX IF NOT EXISTS idx_pools_utilization ON liquidity_pools(
    (utilized_liquidity::float / NULLIF(total_deposits::float, 0)) DESC
);

-- Index for active pools market overview
CREATE INDEX IF NOT EXISTS idx_pools_active_status ON liquidity_pools(status) 
WHERE status = 'active';

-- ============================================================================
-- LIQUIDITY POOL AUDIT INDEXES
-- ============================================================================

-- Index for pool transaction history queries
CREATE INDEX IF NOT EXISTS idx_pool_audit_pool_id_created ON liquidity_pool_audit(pool_id, created_at DESC);

-- Index for operation type filtering
CREATE INDEX IF NOT EXISTS idx_pool_audit_operation_type ON liquidity_pool_audit(operation_type);

-- ============================================================================
-- COMPOSITE INDEXES FOR COMPLEX QUERIES
-- ============================================================================

-- Index for investor dashboard aggregations
CREATE INDEX IF NOT EXISTS idx_pools_investor_aggregation ON liquidity_pools(
    investor_id, 
    status, 
    total_deposits, 
    available_liquidity, 
    utilized_liquidity
);

-- Index for market overview calculations
CREATE INDEX IF NOT EXISTS idx_pools_market_overview ON liquidity_pools(
    status, 
    total_deposits, 
    available_liquidity, 
    utilized_liquidity,
    max_ltv_ratio,
    max_interest_rate
) WHERE status = 'active';

-- ============================================================================
-- PARTIAL INDEXES FOR SPECIFIC USE CASES
-- ============================================================================

-- Index for finding available pools (non-zero liquidity)
CREATE INDEX IF NOT EXISTS idx_pools_available_liquidity ON liquidity_pools(
    available_liquidity DESC, 
    max_ltv_ratio,
    max_interest_rate
) WHERE available_liquidity > 0 AND status = 'active';

-- Index for high-utilization pools monitoring
CREATE INDEX IF NOT EXISTS idx_pools_high_utilization ON liquidity_pools(
    (utilized_liquidity::float / NULLIF(total_deposits::float, 0)) DESC,
    updated_at
) WHERE total_deposits > 0;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON INDEX idx_pools_investor_status IS 'Optimizes pool queries filtered by investor and status';
COMMENT ON INDEX idx_pools_created_at IS 'Optimizes pool listing ordered by creation date';
COMMENT ON INDEX idx_pools_total_deposits IS 'Optimizes dashboard queries showing pools by size';
COMMENT ON INDEX idx_pools_utilization IS 'Optimizes utilization rate calculations and ordering';
COMMENT ON INDEX idx_pool_audit_pool_id_created IS 'Optimizes pool transaction history queries';
COMMENT ON INDEX idx_pools_investor_aggregation IS 'Optimizes investor dashboard metric calculations';
COMMENT ON INDEX idx_pools_market_overview IS 'Optimizes platform-wide analytics queries';