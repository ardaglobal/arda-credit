-- Add loan tape specific columns to deals table
-- Migration: 031_add_loan_tape_columns.sql

-- Payment frequency (monthly, quarterly, biweekly, etc.)
ALTER TABLE deals ADD COLUMN IF NOT EXISTS payment_frequency VARCHAR(50);

-- Collateral information
ALTER TABLE deals ADD COLUMN IF NOT EXISTS collateral_type VARCHAR(200);
ALTER TABLE deals ADD COLUMN IF NOT EXISTS collateral_value DECIMAL(18, 2);
ALTER TABLE deals ADD COLUMN IF NOT EXISTS ltv_ratio DECIMAL(5, 4); -- e.g., 0.7143 for 71.43%

-- Credit metrics
ALTER TABLE deals ADD COLUMN IF NOT EXISTS credit_score INTEGER;
ALTER TABLE deals ADD COLUMN IF NOT EXISTS dscr DECIMAL(10, 4); -- Debt Service Coverage Ratio e.g., 1.4500

-- ZK proof tracking
ALTER TABLE deals ADD COLUMN IF NOT EXISTS state_transition_hash BYTEA;
ALTER TABLE deals ADD COLUMN IF NOT EXISTS proof_included BOOLEAN DEFAULT FALSE;

-- Add indexes for frequently queried fields
CREATE INDEX IF NOT EXISTS idx_deals_payment_frequency ON deals(payment_frequency);
CREATE INDEX IF NOT EXISTS idx_deals_collateral_type ON deals(collateral_type);
CREATE INDEX IF NOT EXISTS idx_deals_credit_score ON deals(credit_score);
CREATE INDEX IF NOT EXISTS idx_deals_ltv_ratio ON deals(ltv_ratio);
CREATE INDEX IF NOT EXISTS idx_deals_proof_included ON deals(proof_included);
CREATE INDEX IF NOT EXISTS idx_deals_state_transition_hash ON deals USING hash(state_transition_hash);

-- Add comments for documentation
COMMENT ON COLUMN deals.payment_frequency IS 'Payment schedule: daily, weekly, biweekly, monthly, quarterly, semiannually, annually';
COMMENT ON COLUMN deals.collateral_type IS 'Type of collateral backing the deal: Equipment, Real Estate, Inventory & Receivables, etc.';
COMMENT ON COLUMN deals.collateral_value IS 'Appraised value of the collateral in USD';
COMMENT ON COLUMN deals.ltv_ratio IS 'Loan-to-Value ratio (0-1 range, e.g., 0.7143 = 71.43%)';
COMMENT ON COLUMN deals.credit_score IS 'Numeric credit score (typically 300-850 range)';
COMMENT ON COLUMN deals.dscr IS 'Debt Service Coverage Ratio (e.g., 1.45 means income is 1.45x debt obligations)';
COMMENT ON COLUMN deals.state_transition_hash IS 'Blockchain transaction hash for state transitions';
COMMENT ON COLUMN deals.proof_included IS 'Whether this deal is included in a ZK proof batch';
