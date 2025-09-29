-- Add more missing columns for credit line operations

-- Add ltv_ratio to credit_line_requests
ALTER TABLE credit_line_requests ADD COLUMN ltv_ratio DECIMAL(5, 4);

-- Add approved_rate_apr to credit_line_requests (matching approved_interest_rate)
ALTER TABLE credit_line_requests ADD COLUMN approved_rate_apr DECIMAL(5, 4);

-- Update the approved_rate_apr to match approved_interest_rate for existing records
UPDATE credit_line_requests SET approved_rate_apr = approved_interest_rate WHERE approved_interest_rate IS NOT NULL;