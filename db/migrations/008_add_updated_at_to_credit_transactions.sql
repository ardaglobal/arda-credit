-- Add updated_at column to credit_transactions table
ALTER TABLE credit_transactions 
ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

-- Create index for better query performance on updated_at
CREATE INDEX IF NOT EXISTS idx_credit_updated_at ON credit_transactions(updated_at);