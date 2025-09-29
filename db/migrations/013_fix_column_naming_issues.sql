-- Fix column naming mismatches for SQLx queries

-- Add loan_status column as alias for status in deals table (for backward compatibility)
ALTER TABLE deals ADD COLUMN loan_status VARCHAR(50);
UPDATE deals SET loan_status = status;

-- Add requested_rate_apr column as alias for requested_interest_rate (for backward compatibility)
ALTER TABLE credit_line_requests ADD COLUMN requested_rate_apr DECIMAL(5, 4);
UPDATE credit_line_requests SET requested_rate_apr = requested_interest_rate;

-- Add missing columns that are referenced in Rust code but don't exist in DB
ALTER TABLE users ADD COLUMN IF NOT EXISTS user_type VARCHAR(20);
ALTER TABLE users ADD COLUMN IF NOT EXISTS system_admin BOOLEAN DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified BOOLEAN DEFAULT FALSE;

-- Update existing users with default values if needed
UPDATE users SET user_type = 'investor' WHERE user_type IS NULL;
UPDATE users SET system_admin = FALSE WHERE system_admin IS NULL;
UPDATE users SET email_verified = FALSE WHERE email_verified IS NULL;

-- Add missing kyc_investor_type column if it doesn't exist
ALTER TABLE users ADD COLUMN IF NOT EXISTS kyc_investor_type VARCHAR(50);