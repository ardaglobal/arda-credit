-- Create credit line request and approval system
-- Enables merchants to request credit lines with automated evaluation and pool matching

-- ============================================================================
-- CREATE CREDIT LINE REQUESTS TABLE
-- ============================================================================

CREATE TABLE credit_line_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    requested_amount DECIMAL(20,6) NOT NULL,
    requested_rate_apr DECIMAL(5,4), -- Optional rate preference (e.g., 0.1200 = 12%)
    requested_tenor_days INTEGER DEFAULT 30,
    collateral_basis DECIMAL(20,6), -- Calculated from turnover
    ltv_ratio DECIMAL(5,4), -- Calculated LTV (e.g., 0.8000 = 80%)
    status VARCHAR(20) DEFAULT 'SUBMITTED',
    matched_pool_id UUID REFERENCES liquidity_pools(id),
    approved_amount DECIMAL(20,6), -- May be less than requested
    approved_rate_apr DECIMAL(5,4),
    evaluation_notes JSONB,
    rejection_reason VARCHAR(500),
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance
CREATE INDEX idx_credit_line_requests_merchant_id ON credit_line_requests(merchant_id);
CREATE INDEX idx_credit_line_requests_status ON credit_line_requests(status);
CREATE INDEX idx_credit_line_requests_pool_id ON credit_line_requests(matched_pool_id);
CREATE INDEX idx_credit_line_requests_created_at ON credit_line_requests(created_at DESC);

-- ============================================================================
-- CREATE ACTIVE CREDIT LINES TABLE
-- ============================================================================

CREATE TABLE credit_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES credit_line_requests(id),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id),
    credit_limit DECIMAL(20,6) NOT NULL,
    available_credit DECIMAL(20,6) NOT NULL,
    utilized_amount DECIMAL(20,6) DEFAULT 0,
    rate_apr DECIMAL(5,4) NOT NULL,
    repayment_window_days INTEGER NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    opened_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP,
    closed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance
CREATE INDEX idx_credit_lines_merchant_id ON credit_lines(merchant_id);
CREATE INDEX idx_credit_lines_pool_id ON credit_lines(pool_id);
CREATE INDEX idx_credit_lines_status ON credit_lines(status);
CREATE INDEX idx_credit_lines_request_id ON credit_lines(request_id);
CREATE INDEX idx_credit_lines_expires_at ON credit_lines(expires_at);

-- ============================================================================
-- CREATE CREDIT LINE UTILIZATION HISTORY TABLE
-- ============================================================================

CREATE TABLE credit_line_utilization_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credit_line_id UUID NOT NULL REFERENCES credit_lines(id) ON DELETE CASCADE,
    transaction_type VARCHAR(20) NOT NULL, -- DRAWDOWN, REPAYMENT, INTEREST_CHARGE
    amount DECIMAL(20,6) NOT NULL,
    previous_balance DECIMAL(20,6) NOT NULL,
    new_balance DECIMAL(20,6) NOT NULL,
    description TEXT,
    transaction_reference VARCHAR(100), -- External reference if any
    created_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance
CREATE INDEX idx_credit_utilization_credit_line_id ON credit_line_utilization_history(credit_line_id);
CREATE INDEX idx_credit_utilization_type ON credit_line_utilization_history(transaction_type);
CREATE INDEX idx_credit_utilization_created_at ON credit_line_utilization_history(created_at DESC);

-- ============================================================================
-- ADD CONSTRAINTS
-- ============================================================================

-- Credit line request constraints
ALTER TABLE credit_line_requests ADD CONSTRAINT credit_requests_status_check 
    CHECK (status IN ('SUBMITTED', 'EVALUATING', 'APPROVED', 'REJECTED', 'EXPIRED'));

ALTER TABLE credit_line_requests ADD CONSTRAINT credit_requests_amounts_positive_check
    CHECK (requested_amount > 0 AND (approved_amount IS NULL OR approved_amount > 0));

ALTER TABLE credit_line_requests ADD CONSTRAINT credit_requests_rates_valid_check
    CHECK ((requested_rate_apr IS NULL OR requested_rate_apr > 0) AND 
           (approved_rate_apr IS NULL OR approved_rate_apr > 0));

ALTER TABLE credit_line_requests ADD CONSTRAINT credit_requests_ltv_valid_check
    CHECK (ltv_ratio IS NULL OR (ltv_ratio > 0 AND ltv_ratio <= 2.0)); -- Max 200% LTV

-- Credit line constraints
ALTER TABLE credit_lines ADD CONSTRAINT credit_lines_status_check 
    CHECK (status IN ('ACTIVE', 'SUSPENDED', 'CLOSED', 'EXPIRED'));

ALTER TABLE credit_lines ADD CONSTRAINT credit_lines_amounts_valid_check
    CHECK (credit_limit > 0 AND available_credit >= 0 AND utilized_amount >= 0 AND
           utilized_amount <= credit_limit AND available_credit = credit_limit - utilized_amount);

ALTER TABLE credit_lines ADD CONSTRAINT credit_lines_rate_positive_check
    CHECK (rate_apr > 0);

ALTER TABLE credit_lines ADD CONSTRAINT credit_lines_dates_valid_check
    CHECK (expires_at IS NULL OR expires_at > opened_at);

-- Utilization history constraints
ALTER TABLE credit_line_utilization_history ADD CONSTRAINT utilization_transaction_type_check
    CHECK (transaction_type IN ('DRAWDOWN', 'REPAYMENT', 'INTEREST_CHARGE', 'FEE', 'REVERSAL'));

ALTER TABLE credit_line_utilization_history ADD CONSTRAINT utilization_amounts_valid_check
    CHECK (amount != 0 AND previous_balance >= 0 AND new_balance >= 0);

-- ============================================================================
-- CREATE TRIGGERS FOR UPDATED_AT TIMESTAMPS
-- ============================================================================

-- Triggers for credit line requests
CREATE TRIGGER update_credit_line_requests_updated_at 
    BEFORE UPDATE ON credit_line_requests 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Triggers for credit lines
CREATE TRIGGER update_credit_lines_updated_at 
    BEFORE UPDATE ON credit_lines 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- CREATE VIEWS FOR ANALYTICS AND REPORTING
-- ============================================================================

-- View for credit line request analytics
CREATE VIEW credit_line_request_analytics AS
SELECT 
    clr.id as request_id,
    clr.merchant_id,
    m.legal_name as merchant_name,
    clr.requested_amount,
    clr.approved_amount,
    clr.ltv_ratio,
    clr.status,
    clr.rejection_reason,
    lp.name as pool_name,
    lp.max_ltv_ratio as pool_max_ltv,
    lp.max_interest_rate as pool_max_rate,
    clr.created_at as requested_at,
    -- Calculate approval rate
    CASE WHEN clr.status = 'APPROVED' THEN 1.0 ELSE 0.0 END as approval_indicator,
    -- Calculate processing time
    EXTRACT(EPOCH FROM (clr.updated_at - clr.created_at)) / 3600 as processing_hours,
    -- Merchant profile completion score
    mpc.completion_score,
    -- Turnover analytics
    mta.avg_daily_receipts,
    mta.data_quality_score
FROM credit_line_requests clr
JOIN merchants m ON clr.merchant_id = m.id
LEFT JOIN liquidity_pools lp ON clr.matched_pool_id = lp.id
LEFT JOIN merchant_profile_completion mpc ON m.id = mpc.merchant_id
LEFT JOIN merchant_turnover_analytics mta ON m.id = mta.merchant_id;

-- View for active credit line portfolio
CREATE VIEW active_credit_lines_portfolio AS
SELECT 
    cl.id as credit_line_id,
    cl.merchant_id,
    m.legal_name as merchant_name,
    cl.pool_id,
    lp.name as pool_name,
    cl.credit_limit,
    cl.available_credit,
    cl.utilized_amount,
    cl.rate_apr,
    cl.status,
    cl.opened_at,
    cl.expires_at,
    -- Calculate utilization percentage
    CASE WHEN cl.credit_limit > 0 
         THEN (cl.utilized_amount / cl.credit_limit * 100)::DECIMAL(5,2) 
         ELSE 0 END as utilization_percentage,
    -- Days until expiration
    CASE WHEN cl.expires_at IS NOT NULL 
         THEN EXTRACT(DAY FROM (cl.expires_at - NOW()))::INTEGER
         ELSE NULL END as days_until_expiration,
    -- Risk indicators
    CASE WHEN cl.utilized_amount / cl.credit_limit > 0.9 THEN 'HIGH'
         WHEN cl.utilized_amount / cl.credit_limit > 0.7 THEN 'MEDIUM'
         ELSE 'LOW' END as utilization_risk,
    -- Pool impact
    cl.credit_limit as pool_commitment,
    lp.total_deposits as pool_total_capacity
FROM credit_lines cl
JOIN merchants m ON cl.merchant_id = m.id
JOIN liquidity_pools lp ON cl.pool_id = lp.id
WHERE cl.status IN ('ACTIVE', 'SUSPENDED');

-- ============================================================================
-- CREATE FUNCTIONS FOR CREDIT LINE OPERATIONS
-- ============================================================================

-- Function to calculate merchant collateral basis from turnover
CREATE OR REPLACE FUNCTION calculate_merchant_collateral_basis(
    p_merchant_id UUID,
    p_lookback_days INTEGER DEFAULT 30,
    p_multiplier DECIMAL DEFAULT 20.0
) 
RETURNS DECIMAL AS $$
DECLARE
    avg_daily_receipts DECIMAL;
    collateral_basis DECIMAL;
BEGIN
    -- Calculate average daily receipts over lookback period
    SELECT COALESCE(AVG(net_receipts), 0)
    INTO avg_daily_receipts
    FROM merchant_turnover_daily
    WHERE merchant_id = p_merchant_id
      AND date >= CURRENT_DATE - INTERVAL '1 day' * p_lookback_days
      AND verified = true;
    
    -- Apply multiplier to get collateral basis
    collateral_basis := avg_daily_receipts * p_multiplier;
    
    RETURN GREATEST(collateral_basis, 0);
END;
$$ LANGUAGE plpgsql;

-- Function to find matching pools for a credit request
CREATE OR REPLACE FUNCTION find_matching_pools_for_request(
    p_requested_amount DECIMAL,
    p_ltv_ratio DECIMAL,
    p_max_rate DECIMAL DEFAULT NULL
)
RETURNS TABLE(
    pool_id UUID,
    pool_name VARCHAR,
    max_ltv_ratio DECIMAL,
    max_interest_rate DECIMAL,
    available_liquidity DECIMAL,
    match_score INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lp.id,
        lp.name,
        lp.max_ltv_ratio,
        lp.max_interest_rate,
        lp.available_liquidity,
        -- Calculate match score (higher is better)
        (
            (CASE WHEN lp.available_liquidity >= p_requested_amount THEN 40 ELSE 0 END) +
            (CASE WHEN lp.max_ltv_ratio >= p_ltv_ratio THEN 30 ELSE 0 END) +
            (CASE WHEN p_max_rate IS NULL OR lp.max_interest_rate <= p_max_rate THEN 30 ELSE 0 END)
        )::INTEGER as match_score
    FROM liquidity_pools lp
    WHERE lp.status = 'ACTIVE'
      AND lp.available_liquidity >= p_requested_amount
      AND lp.max_ltv_ratio >= p_ltv_ratio
      AND (p_max_rate IS NULL OR lp.max_interest_rate <= p_max_rate)
    ORDER BY match_score DESC, lp.max_interest_rate ASC, lp.available_liquidity DESC;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- SAMPLE DATA FOR TESTING (COMMENTED OUT)
-- ============================================================================

/*
-- Test credit line request (requires existing merchant)
INSERT INTO credit_line_requests (merchant_id, requested_amount, requested_rate_apr, requested_tenor_days)
VALUES (
    (SELECT id FROM merchants LIMIT 1),
    50000.00,
    0.1200,
    90
);

-- Test collateral basis calculation
SELECT calculate_merchant_collateral_basis(
    (SELECT id FROM merchants LIMIT 1),
    30,
    20.0
) as calculated_collateral_basis;

-- Test pool matching
SELECT * FROM find_matching_pools_for_request(50000.00, 0.75, 0.15);
*/