-- Create credit_transactions table
CREATE TABLE IF NOT EXISTS credit_transactions (
	id SERIAL PRIMARY KEY,
	a INTEGER NOT NULL,
	b INTEGER NOT NULL,
	result INTEGER NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	UNIQUE(a, b, result)
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_credit_result ON credit_transactions(result);

CREATE INDEX IF NOT EXISTS idx_credit_created_at ON credit_transactions(created_at);