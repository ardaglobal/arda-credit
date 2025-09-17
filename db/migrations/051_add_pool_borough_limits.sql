-- Add pool borough percentage limits for geographic lending restrictions
-- This migration adds borough-based lending limits tied to total pool deposits
-- rather than just available liquidity

-- ============================================================================
-- POOL BOROUGH LIMITS TABLE
-- ============================================================================

-- Table for defining maximum percentage limits per borough/region for each pool
CREATE TABLE pool_borough_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id) ON DELETE CASCADE,
    borough VARCHAR(100) NOT NULL,  -- Geographic region (e.g., "Manhattan", "Brooklyn", "Queens", etc.)
    max_percentage DECIMAL(5,4) NOT NULL CHECK (max_percentage > 0 AND max_percentage <= 1.0000), -- 0.1000 = 10%
    current_allocated DECIMAL(20,6) NOT NULL DEFAULT 0 CHECK (current_allocated >= 0), -- Currently allocated amount
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Ensure one limit per borough per pool
    UNIQUE(pool_id, borough)
);

-- ============================================================================
-- ADD BOROUGH FIELD TO MERCHANTS
-- ============================================================================

-- Add borough field to merchants table to track borrower location
ALTER TABLE merchants ADD COLUMN IF NOT EXISTS borough VARCHAR(100);

-- Create index for borough-based queries
CREATE INDEX IF NOT EXISTS idx_merchants_borough ON merchants(borough) WHERE borough IS NOT NULL;

-- ============================================================================
-- BOROUGH VALIDATION FUNCTION
-- ============================================================================

-- Function to validate if a loan request would exceed borough percentage limits
CREATE OR REPLACE FUNCTION validate_borough_borrowing_limit(
    p_pool_id UUID,
    p_borough VARCHAR(100),
    p_requested_amount DECIMAL(20,6)
)
RETURNS TABLE(
    is_valid BOOLEAN,
    current_allocated DECIMAL(20,6),
    max_allowed DECIMAL(20,6),
    total_pool_deposits DECIMAL(20,6),
    percentage_after_loan DECIMAL(5,4),
    max_percentage DECIMAL(5,4)
) AS $$
DECLARE
    v_pool_total_deposits DECIMAL(20,6);
    v_current_borough_allocated DECIMAL(20,6);
    v_max_percentage DECIMAL(5,4);
    v_max_allowed_amount DECIMAL(20,6);
    v_percentage_after DECIMAL(5,4);
BEGIN
    -- Get pool's total deposits
    SELECT lp.total_deposits INTO v_pool_total_deposits
    FROM liquidity_pools lp
    WHERE lp.id = p_pool_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Pool with ID % not found', p_pool_id;
    END IF;

    -- Get current allocation for this borough (default to 0 if no limit set)
    SELECT
        COALESCE(pbl.current_allocated, 0),
        COALESCE(pbl.max_percentage, 1.0000)  -- Default to 100% if no limit
    INTO v_current_borough_allocated, v_max_percentage
    FROM pool_borough_limits pbl
    WHERE pbl.pool_id = p_pool_id AND pbl.borough = p_borough;

    -- Handle case where no borough limit exists (allow all)
    IF NOT FOUND THEN
        v_current_borough_allocated := 0;
        v_max_percentage := 1.0000;  -- 100% - no limit
    END IF;

    -- Calculate maximum allowed amount for this borough
    v_max_allowed_amount := v_pool_total_deposits * v_max_percentage;

    -- Calculate what percentage this would be after the loan
    IF v_pool_total_deposits > 0 THEN
        v_percentage_after := (v_current_borough_allocated + p_requested_amount) / v_pool_total_deposits;
    ELSE
        v_percentage_after := 0;
    END IF;

    RETURN QUERY SELECT
        (v_current_borough_allocated + p_requested_amount) <= v_max_allowed_amount,
        v_current_borough_allocated,
        v_max_allowed_amount,
        v_pool_total_deposits,
        v_percentage_after,
        v_max_percentage;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- BOROUGH ALLOCATION UPDATE FUNCTION
-- ============================================================================

-- Function to update borough allocation when loans are originated or repaid
CREATE OR REPLACE FUNCTION update_borough_allocation(
    p_pool_id UUID,
    p_borough VARCHAR(100),
    p_amount_change DECIMAL(20,6)  -- Positive for new loans, negative for repayments
)
RETURNS VOID AS $$
BEGIN
    -- Insert or update the borough allocation
    INSERT INTO pool_borough_limits (pool_id, borough, current_allocated, max_percentage)
    VALUES (p_pool_id, p_borough, GREATEST(0, p_amount_change), 1.0000)  -- Default to no limit
    ON CONFLICT (pool_id, borough)
    DO UPDATE SET
        current_allocated = GREATEST(0, pool_borough_limits.current_allocated + p_amount_change),
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- GET BOROUGH SUMMARY FUNCTION
-- ============================================================================

-- Function to get borough allocation summary for a pool
CREATE OR REPLACE FUNCTION get_pool_borough_summary(p_pool_id UUID)
RETURNS TABLE(
    borough VARCHAR(100),
    current_allocated DECIMAL(20,6),
    max_percentage DECIMAL(5,4),
    max_allowed DECIMAL(20,6),
    remaining_capacity DECIMAL(20,6),
    utilization_percentage DECIMAL(5,4)
) AS $$
DECLARE
    v_total_deposits DECIMAL(20,6);
BEGIN
    -- Get pool's total deposits
    SELECT lp.total_deposits INTO v_total_deposits
    FROM liquidity_pools lp
    WHERE lp.id = p_pool_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Pool with ID % not found', p_pool_id;
    END IF;

    RETURN QUERY
    SELECT
        pbl.borough,
        pbl.current_allocated,
        pbl.max_percentage,
        (v_total_deposits * pbl.max_percentage) as max_allowed,
        GREATEST(0, (v_total_deposits * pbl.max_percentage) - pbl.current_allocated) as remaining_capacity,
        CASE
            WHEN v_total_deposits > 0 THEN pbl.current_allocated / v_total_deposits
            ELSE 0
        END as utilization_percentage
    FROM pool_borough_limits pbl
    WHERE pbl.pool_id = p_pool_id
    ORDER BY pbl.current_allocated DESC;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Primary access patterns for borough limits
CREATE INDEX idx_pool_borough_limits_pool_id ON pool_borough_limits(pool_id);
CREATE INDEX idx_pool_borough_limits_borough ON pool_borough_limits(borough);
CREATE INDEX idx_pool_borough_limits_pool_borough ON pool_borough_limits(pool_id, borough);

-- Index for current allocation queries
CREATE INDEX idx_pool_borough_limits_allocation ON pool_borough_limits(current_allocated) WHERE current_allocated > 0;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

-- Trigger for updating updated_at timestamp
CREATE TRIGGER update_pool_borough_limits_updated_at
    BEFORE UPDATE ON pool_borough_limits
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE pool_borough_limits IS 'Geographic lending limits for liquidity pools based on total deposits';
COMMENT ON COLUMN pool_borough_limits.max_percentage IS 'Maximum percentage of total_deposits that can be allocated to this borough (0.1000 = 10%)';
COMMENT ON COLUMN pool_borough_limits.current_allocated IS 'Currently allocated amount to borrowers in this borough';
COMMENT ON COLUMN pool_borough_limits.borough IS 'Geographic region identifier (e.g., Manhattan, Brooklyn, etc.)';

-- Add comment to merchants table for the new borough column
COMMENT ON COLUMN merchants.borough IS 'Geographic region where merchant operates (for borough percentage limits)';