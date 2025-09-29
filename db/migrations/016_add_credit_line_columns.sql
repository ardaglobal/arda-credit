-- Add missing columns to credit_lines table

-- Add utilized_amount (tracks how much of the credit limit is being used)
ALTER TABLE credit_lines ADD COLUMN utilized_amount DECIMAL(18, 6) DEFAULT 0;

-- Add rate_apr (annual percentage rate)
ALTER TABLE credit_lines ADD COLUMN rate_apr DECIMAL(5, 4);

-- Add repayment_window_days (repayment period)
ALTER TABLE credit_lines ADD COLUMN repayment_window_days INTEGER;

-- Add opened_at (when credit line was opened/activated)
ALTER TABLE credit_lines ADD COLUMN opened_at TIMESTAMP WITH TIME ZONE;

-- Add closed_at (when credit line was closed)
ALTER TABLE credit_lines ADD COLUMN closed_at TIMESTAMP WITH TIME ZONE;

-- Update existing records with default values
UPDATE credit_lines SET utilized_amount = 0 WHERE utilized_amount IS NULL;
UPDATE credit_lines SET opened_at = created_at WHERE opened_at IS NULL;

-- Set NOT NULL constraints for essential columns
ALTER TABLE credit_lines ALTER COLUMN utilized_amount SET NOT NULL;