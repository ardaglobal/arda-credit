-- Add missing columns to tree_state
ALTER TABLE tree_state ADD COLUMN tree_id VARCHAR(255) DEFAULT 'default';
ALTER TABLE tree_state ADD COLUMN total_nullifiers INTEGER DEFAULT 0;

-- Create deal_transactions table only if it doesn't exist
CREATE TABLE deal_transactions (
    id SERIAL PRIMARY KEY,
    deal_id UUID NOT NULL,
    payment_id UUID,
    transaction_type VARCHAR(50) NOT NULL,
    amount DECIMAL(18, 6) NOT NULL,
    identity_id UUID,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create credit_line_requests table only if it doesn't exist
CREATE TABLE credit_line_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL,
    requested_amount DECIMAL(18, 6) NOT NULL,
    requested_ltv_ratio DECIMAL(5, 4) NOT NULL,
    requested_interest_rate DECIMAL(5, 4) NOT NULL,
    requested_tenor_days INTEGER NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    evaluation_notes TEXT,
    rejection_reason TEXT,
    approved_amount DECIMAL(18, 6),
    approved_ltv_ratio DECIMAL(5, 4),
    approved_interest_rate DECIMAL(5, 4),
    approved_tenor_days INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);