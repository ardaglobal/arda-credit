-- Add missing columns referenced in Rust SQLx queries

-- Add collateral_basis to credit_line_requests
ALTER TABLE credit_line_requests ADD COLUMN collateral_basis TEXT;

-- Add request_id to credit_lines (reference to credit_line_requests)
ALTER TABLE credit_lines ADD COLUMN request_id UUID;

-- Add foreign key constraint for request_id
ALTER TABLE credit_lines ADD CONSTRAINT fk_credit_lines_request_id
    FOREIGN KEY (request_id) REFERENCES credit_line_requests(id);

-- Add matched_pool_id to credit_line_requests if missing
ALTER TABLE credit_line_requests ADD COLUMN matched_pool_id UUID;

-- Add evaluation_json to credit_line_requests if missing
ALTER TABLE credit_line_requests ADD COLUMN evaluation_json JSONB;

-- Make sure users table has all required columns with correct constraints
ALTER TABLE users ALTER COLUMN system_admin SET NOT NULL;
ALTER TABLE users ALTER COLUMN email_verified SET NOT NULL;
ALTER TABLE users ALTER COLUMN user_type SET NOT NULL;

-- Set NOT NULL constraints after ensuring all rows have values
UPDATE users SET system_admin = FALSE WHERE system_admin IS NULL;
UPDATE users SET email_verified = FALSE WHERE email_verified IS NULL;
UPDATE users SET user_type = 'investor' WHERE user_type IS NULL;