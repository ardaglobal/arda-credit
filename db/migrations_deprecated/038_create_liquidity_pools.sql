-- Create liquidity pools for investor auto-deployment terms
-- This migration adds liquidity pool management for automatic merchant credit line matching

-- ============================================================================
-- LIQUIDITY POOLS TABLE
-- ============================================================================

-- Table for investor-created liquidity pools with auto-deployment parameters
CREATE TABLE liquidity_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investor_id UUID NOT NULL REFERENCES identities(id) ON DELETE RESTRICT,
    name VARCHAR(255) NOT NULL,
    total_deposits DECIMAL(20, 6) NOT NULL DEFAULT 0 CHECK (total_deposits >= 0),
    available_liquidity DECIMAL(20, 6) NOT NULL DEFAULT 0 CHECK (available_liquidity >= 0),
    utilized_liquidity DECIMAL(20, 6) NOT NULL DEFAULT 0 CHECK (utilized_liquidity >= 0),
    
    -- Auto-deployment terms
    max_ltv_ratio DECIMAL(5, 4) NOT NULL CHECK (max_ltv_ratio >= 0.1000 AND max_ltv_ratio <= 0.9500), -- 10% to 95%
    max_interest_rate DECIMAL(7, 4) NOT NULL CHECK (max_interest_rate >= 0.0100 AND max_interest_rate <= 0.5000), -- 1% to 50%
    repayment_period_days INTEGER NOT NULL CHECK (repayment_period_days >= 7 AND repayment_period_days <= 365), -- 7 days to 1 year
    
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'PAUSED', 'CLOSED')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- LIQUIDITY POOL AUDIT TABLE
-- ============================================================================

-- Audit trail for all pool operations
CREATE TABLE liquidity_pool_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id) ON DELETE CASCADE,
    operation_type VARCHAR(30) NOT NULL CHECK (operation_type IN ('POOL_CREATED', 'POOL_UPDATED', 'POOL_FUNDED', 'POOL_WITHDRAWN', 'POOL_MATCHED', 'POOL_STATUS_CHANGED')),
    old_values JSONB,
    new_values JSONB,
    operator_id UUID REFERENCES identities(id),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- CONSTRAINTS AND VALIDATIONS
-- ============================================================================

-- Ensure available + utilized = total deposits
ALTER TABLE liquidity_pools ADD CONSTRAINT liquidity_balance_consistency 
    CHECK (available_liquidity + utilized_liquidity = total_deposits);

-- Ensure pool name is unique per investor
CREATE UNIQUE INDEX idx_liquidity_pools_investor_name ON liquidity_pools(investor_id, name) WHERE status != 'CLOSED';

-- Note: Investor validation will be enforced at the application layer
-- PostgreSQL doesn't support subqueries in CHECK constraints

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Primary access patterns
CREATE INDEX idx_liquidity_pools_investor_id ON liquidity_pools(investor_id);
CREATE INDEX idx_liquidity_pools_status ON liquidity_pools(status);
CREATE INDEX idx_liquidity_pools_created_at ON liquidity_pools(created_at);
CREATE INDEX idx_liquidity_pools_available_liquidity ON liquidity_pools(available_liquidity) WHERE available_liquidity > 0;

-- Auto-deployment matching indexes
CREATE INDEX idx_liquidity_pools_ltv_ratio ON liquidity_pools(max_ltv_ratio) WHERE status = 'ACTIVE';
CREATE INDEX idx_liquidity_pools_interest_rate ON liquidity_pools(max_interest_rate) WHERE status = 'ACTIVE';
CREATE INDEX idx_liquidity_pools_repayment_period ON liquidity_pools(repayment_period_days) WHERE status = 'ACTIVE';

-- Composite index for active pool filtering
CREATE INDEX idx_liquidity_pools_active_params ON liquidity_pools(status, available_liquidity, max_ltv_ratio, max_interest_rate, repayment_period_days) 
    WHERE status = 'ACTIVE' AND available_liquidity > 0;

-- Audit table indexes
CREATE INDEX idx_liquidity_pool_audit_pool_id ON liquidity_pool_audit(pool_id);
CREATE INDEX idx_liquidity_pool_audit_operation_type ON liquidity_pool_audit(operation_type);
CREATE INDEX idx_liquidity_pool_audit_created_at ON liquidity_pool_audit(created_at);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Trigger for updating updated_at timestamp
CREATE TRIGGER update_liquidity_pools_updated_at 
    BEFORE UPDATE ON liquidity_pools 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Function to create audit records for pool operations
CREATE OR REPLACE FUNCTION create_pool_audit_record()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO liquidity_pool_audit (pool_id, operation_type, new_values, operator_id)
        VALUES (NEW.id, 'POOL_CREATED', to_jsonb(NEW), NEW.investor_id);
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        -- Check what changed and create appropriate audit record
        IF OLD.status != NEW.status THEN
            INSERT INTO liquidity_pool_audit (pool_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'POOL_STATUS_CHANGED', 
                    jsonb_build_object('status', OLD.status), 
                    jsonb_build_object('status', NEW.status), 
                    NEW.investor_id);
        ELSE
            INSERT INTO liquidity_pool_audit (pool_id, operation_type, old_values, new_values, operator_id)
            VALUES (NEW.id, 'POOL_UPDATED', to_jsonb(OLD), to_jsonb(NEW), NEW.investor_id);
        END IF;
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically create audit records
CREATE TRIGGER pool_audit_trigger
    AFTER INSERT OR UPDATE ON liquidity_pools
    FOR EACH ROW
    EXECUTE FUNCTION create_pool_audit_record();

-- ============================================================================
-- HELPER FUNCTIONS FOR POOL MANAGEMENT
-- ============================================================================

-- Function to get active pools for auto-deployment matching
CREATE OR REPLACE FUNCTION get_matching_pools(
    required_liquidity DECIMAL(20, 6),
    max_ltv DECIMAL(5, 4),
    max_rate DECIMAL(7, 4),
    min_repayment_days INTEGER
)
RETURNS TABLE(
    pool_id UUID,
    investor_id UUID,
    pool_name VARCHAR(255),
    available_liquidity DECIMAL(20, 6),
    max_ltv_ratio DECIMAL(5, 4),
    max_interest_rate DECIMAL(7, 4),
    repayment_period_days INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lp.id,
        lp.investor_id,
        lp.name,
        lp.available_liquidity,
        lp.max_ltv_ratio,
        lp.max_interest_rate,
        lp.repayment_period_days
    FROM liquidity_pools lp
    WHERE lp.status = 'ACTIVE'
        AND lp.available_liquidity >= required_liquidity
        AND lp.max_ltv_ratio >= max_ltv
        AND lp.max_interest_rate >= max_rate
        AND lp.repayment_period_days >= min_repayment_days
    ORDER BY lp.max_interest_rate ASC, lp.available_liquidity DESC; -- Prefer lower rates, higher liquidity
END;
$$ LANGUAGE plpgsql;

-- Function to validate pool parameters
CREATE OR REPLACE FUNCTION validate_pool_parameters(
    ltv_ratio DECIMAL(5, 4),
    interest_rate DECIMAL(7, 4),
    repayment_days INTEGER
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN (
        ltv_ratio >= 0.1000 AND ltv_ratio <= 0.9500 AND
        interest_rate >= 0.0100 AND interest_rate <= 0.5000 AND
        repayment_days >= 7 AND repayment_days <= 365
    );
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INTEGRATION WITH LOAN TRANSACTIONS
-- ============================================================================

-- Add pool_id to loan_transactions for tracking pool-originated loans
ALTER TABLE loan_transactions ADD COLUMN pool_id UUID REFERENCES liquidity_pools(id);
CREATE INDEX idx_loan_transactions_pool_id ON loan_transactions(pool_id) WHERE pool_id IS NOT NULL;

-- Function to link loan transactions with originating pools
CREATE OR REPLACE FUNCTION link_loan_to_pool(
    transaction_id UUID,
    originating_pool_id UUID
)
RETURNS BOOLEAN AS $$
BEGIN
    UPDATE loan_transactions 
    SET pool_id = originating_pool_id
    WHERE id = transaction_id;
    
    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE liquidity_pools IS 'Investor-created liquidity pools with auto-deployment parameters for merchant credit line matching';
COMMENT ON TABLE liquidity_pool_audit IS 'Audit trail for all liquidity pool operations and state changes';

COMMENT ON COLUMN liquidity_pools.max_ltv_ratio IS 'Maximum loan-to-value ratio (decimal: 0.8000 = 80%)';
COMMENT ON COLUMN liquidity_pools.max_interest_rate IS 'Maximum interest rate (decimal: 0.1200 = 12%)';
COMMENT ON COLUMN liquidity_pools.repayment_period_days IS 'Repayment period in days (7-365 days allowed)';
COMMENT ON COLUMN liquidity_pools.total_deposits IS 'Total funds deposited into the pool';
COMMENT ON COLUMN liquidity_pools.available_liquidity IS 'Available funds for new deployments';
COMMENT ON COLUMN liquidity_pools.utilized_liquidity IS 'Currently deployed funds in active loans';