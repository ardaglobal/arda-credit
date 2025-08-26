-- CCR (Credit and Compliance Requirements) Domain Tables
-- This migration adds loan processing capabilities to the existing batch processing system

-- ============================================================================
-- IDENTITY AND COMPLIANCE TABLES
-- ============================================================================

-- Table for user/entity identity management with KYC
CREATE TABLE identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_type VARCHAR(20) NOT NULL CHECK (user_type IN ('investor', 'lender')),
    email VARCHAR(255) NOT NULL UNIQUE,
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted' CHECK (kyc_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    kyc_submitted_at TIMESTAMP WITH TIME ZONE,
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- LOAN MANAGEMENT TABLES
-- ============================================================================

-- Table for loan records and lifecycle management
CREATE TABLE loans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    borrower_id UUID NOT NULL REFERENCES identities(id),
    lender_id UUID NOT NULL REFERENCES identities(id),
    principal_amount DECIMAL(18, 6) NOT NULL CHECK (principal_amount > 0),
    outstanding_balance DECIMAL(18, 6) NOT NULL CHECK (outstanding_balance >= 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0), -- Basis points (1/100th of 1%)
    loan_status VARCHAR(20) NOT NULL DEFAULT 'originated' CHECK (loan_status IN ('originated', 'active', 'paid_off', 'closed', 'defaulted')),
    originated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    maturity_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT borrower_lender_different CHECK (borrower_id != lender_id)
);

-- ============================================================================
-- PAYMENT AND BALANCE TABLES
-- ============================================================================

-- Table for tracking payments made against loans
CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(id),
    payer_id UUID NOT NULL REFERENCES identities(id),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    payment_type VARCHAR(20) NOT NULL DEFAULT 'regular' CHECK (payment_type IN ('regular', 'principal', 'interest', 'penalty')),
    transaction_hash VARCHAR(66), -- Ethereum tx hash if on-chain
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for account balances (USDC-based)
CREATE TABLE balances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(id),
    balance_type VARCHAR(20) NOT NULL DEFAULT 'available' CHECK (balance_type IN ('available', 'locked', 'reserved')),
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0 CHECK (amount >= 0),
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    UNIQUE(identity_id, balance_type, currency)
);

-- ============================================================================
-- STATE AND BATCH INTEGRATION TABLES  
-- ============================================================================

-- Table for loan transaction records (integrates with existing batch system)
CREATE TABLE loan_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id_hash BYTEA NOT NULL UNIQUE, -- SHA256 hash for zkVM compatibility
    user_id_hash BYTEA NOT NULL, -- SHA256 hash of user UUID
    loan_id_hash BYTEA, -- SHA256 hash of loan UUID, NULL for non-loan transactions
    transaction_type VARCHAR(30) NOT NULL CHECK (transaction_type IN ('user_registration', 'kyc_submission', 'kyc_verification', 'loan_origination', 'loan_payment', 'loan_closeout')),
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
    
    -- Foreign key references to actual records
    identity_id UUID REFERENCES identities(id),
    loan_id UUID REFERENCES loans(id),
    payment_id UUID REFERENCES payments(id),
    
    -- Batch processing integration
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for point-in-time state snapshots
CREATE TABLE state_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_type VARCHAR(20) NOT NULL DEFAULT 'manual' CHECK (snapshot_type IN ('manual', 'batch', 'scheduled')),
    batch_id INTEGER REFERENCES proof_batches(id),
    merkle_root BYTEA,
    total_loans_count INTEGER NOT NULL DEFAULT 0,
    total_outstanding_balance DECIMAL(18, 6) NOT NULL DEFAULT 0,
    total_paid_amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
    active_users_count INTEGER NOT NULL DEFAULT 0,
    kyc_approved_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Identity indexes
CREATE INDEX idx_identities_email ON identities(email);
CREATE INDEX idx_identities_kyc_status ON identities(kyc_status);
CREATE INDEX idx_identities_user_type ON identities(user_type);
CREATE INDEX idx_identities_created_at ON identities(created_at);

-- Loan indexes  
CREATE INDEX idx_loans_borrower_id ON loans(borrower_id);
CREATE INDEX idx_loans_lender_id ON loans(lender_id);
CREATE INDEX idx_loans_status ON loans(loan_status);
CREATE INDEX idx_loans_originated_at ON loans(originated_at);
CREATE INDEX idx_loans_outstanding_balance ON loans(outstanding_balance) WHERE outstanding_balance > 0;

-- Payment indexes
CREATE INDEX idx_payments_loan_id ON payments(loan_id);
CREATE INDEX idx_payments_payer_id ON payments(payer_id);
CREATE INDEX idx_payments_processed_at ON payments(processed_at);
CREATE INDEX idx_payments_transaction_hash ON payments(transaction_hash) WHERE transaction_hash IS NOT NULL;

-- Balance indexes
CREATE INDEX idx_balances_identity_id ON balances(identity_id);
CREATE INDEX idx_balances_type_currency ON balances(balance_type, currency);
CREATE INDEX idx_balances_updated_at ON balances(updated_at);

-- Loan transaction indexes
CREATE INDEX idx_loan_transactions_user_id_hash ON loan_transactions(user_id_hash);
CREATE INDEX idx_loan_transactions_loan_id_hash ON loan_transactions(loan_id_hash) WHERE loan_id_hash IS NOT NULL;
CREATE INDEX idx_loan_transactions_type ON loan_transactions(transaction_type);
CREATE INDEX idx_loan_transactions_batch_id ON loan_transactions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_loan_transactions_not_batched ON loan_transactions(id) WHERE included_in_batch_id IS NULL;

-- State snapshot indexes
CREATE INDEX idx_state_snapshots_type ON state_snapshots(snapshot_type);
CREATE INDEX idx_state_snapshots_batch_id ON state_snapshots(batch_id) WHERE batch_id IS NOT NULL;
CREATE INDEX idx_state_snapshots_created_at ON state_snapshots(created_at);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC TIMESTAMP UPDATES
-- ============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at
CREATE TRIGGER update_identities_updated_at BEFORE UPDATE ON identities FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_loans_updated_at BEFORE UPDATE ON loans FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_balances_updated_at BEFORE UPDATE ON balances FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- HELPER FUNCTIONS FOR LOAN PROCESSING
-- ============================================================================

-- Function to create initial balance entries for new identities
CREATE OR REPLACE FUNCTION create_initial_balances()
RETURNS TRIGGER AS $$
BEGIN
    -- Create available balance entry
    INSERT INTO balances (identity_id, balance_type, amount, currency)
    VALUES (NEW.id, 'available', 0, 'USDC');
    
    -- Create locked balance entry  
    INSERT INTO balances (identity_id, balance_type, amount, currency)
    VALUES (NEW.id, 'locked', 0, 'USDC');
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to create balances when identity is created
CREATE TRIGGER create_balances_on_identity_insert
    AFTER INSERT ON identities
    FOR EACH ROW
    EXECUTE FUNCTION create_initial_balances();

-- Function to get loan transactions ready for batching
CREATE OR REPLACE FUNCTION get_unbatched_loan_transactions(limit_count INTEGER DEFAULT 10)
RETURNS TABLE(
    transaction_id UUID,
    transaction_id_hash BYTEA,
    user_id_hash BYTEA,
    loan_id_hash BYTEA,
    transaction_type VARCHAR,
    amount DECIMAL(18, 6),
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lt.id,
        lt.transaction_id_hash,
        lt.user_id_hash,
        lt.loan_id_hash,
        lt.transaction_type,
        lt.amount,
        lt.created_at
    FROM loan_transactions lt
    WHERE lt.included_in_batch_id IS NULL
    ORDER BY lt.created_at ASC -- FIFO: oldest first
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to update loan balance after payment
CREATE OR REPLACE FUNCTION update_loan_balance_on_payment()
RETURNS TRIGGER AS $$
BEGIN
    -- Update outstanding balance on the loan
    UPDATE loans 
    SET 
        outstanding_balance = outstanding_balance - NEW.amount,
        loan_status = CASE 
            WHEN (outstanding_balance - NEW.amount) <= 0 THEN 'paid_off'
            ELSE loan_status
        END,
        updated_at = NOW()
    WHERE id = NEW.loan_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update loan balances when payments are made
CREATE TRIGGER update_loan_on_payment
    AFTER INSERT ON payments
    FOR EACH ROW
    EXECUTE FUNCTION update_loan_balance_on_payment();

-- ============================================================================
-- INTEGRATION WITH EXISTING BATCH SYSTEM
-- ============================================================================

-- Update the existing create_batch function to handle loan transactions
-- This replaces the existing function to support both arithmetic and loan transactions
CREATE OR REPLACE FUNCTION create_batch_with_loans(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    transaction_total INTEGER;
    transaction_id_array INTEGER[];
    loan_transaction_ids UUID[];
BEGIN
    -- Get current counter value
    SELECT get_current_counter_value() INTO previous_counter;
    
    -- Get unbatched arithmetic transactions and calculate total
    SELECT 
        ARRAY_AGG(id ORDER BY id),
        SUM(amount)
    INTO transaction_id_array, transaction_total
    FROM (
        SELECT id, amount 
        FROM incoming_transactions 
        WHERE included_in_batch_id IS NULL 
        ORDER BY id ASC 
        LIMIT batch_size / 2  -- Reserve half for arithmetic, half for loans
    ) unbatched;
    
    -- Get unbatched loan transactions
    SELECT ARRAY_AGG(id ORDER BY created_at)
    INTO loan_transaction_ids
    FROM (
        SELECT id
        FROM loan_transactions
        WHERE included_in_batch_id IS NULL
        ORDER BY created_at ASC
        LIMIT batch_size / 2
    ) unbatched_loans;
    
    -- Return 0 if no transactions to batch
    IF (transaction_id_array IS NULL OR array_length(transaction_id_array, 1) = 0) AND 
       (loan_transaction_ids IS NULL OR array_length(loan_transaction_ids, 1) = 0) THEN
        RETURN 0;
    END IF;
    
    -- Calculate final counter value (only arithmetic transactions affect counter)
    final_counter := previous_counter + COALESCE(transaction_total, 0);
    
    -- Create new batch
    INSERT INTO proof_batches (
        previous_counter_value,
        final_counter_value, 
        transaction_ids
    ) VALUES (
        previous_counter,
        final_counter,
        COALESCE(transaction_id_array, ARRAY[]::INTEGER[])
    ) RETURNING id INTO new_batch_id;
    
    -- Mark arithmetic transactions as included in this batch
    IF transaction_id_array IS NOT NULL AND array_length(transaction_id_array, 1) > 0 THEN
        UPDATE incoming_transactions 
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(transaction_id_array);
    END IF;
    
    -- Mark loan transactions as included in this batch
    IF loan_transaction_ids IS NOT NULL AND array_length(loan_transaction_ids, 1) > 0 THEN
        UPDATE loan_transactions
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(loan_transaction_ids);
    END IF;
    
    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE identities IS 'User identity management with KYC compliance tracking';
COMMENT ON TABLE loans IS 'Loan records with borrower/lender relationships and lifecycle management';
COMMENT ON TABLE payments IS 'Payment history and transaction records for loan servicing';
COMMENT ON TABLE balances IS 'Account balance tracking for USDC and other currencies';
COMMENT ON TABLE loan_transactions IS 'Loan transaction records for zkVM batch processing';
COMMENT ON TABLE state_snapshots IS 'Point-in-time state captures for auditing and rollback';

COMMENT ON COLUMN identities.kyc_status IS 'KYC compliance status: not_submitted, pending, approved, rejected';
COMMENT ON COLUMN loans.interest_rate_bps IS 'Interest rate in basis points (1/100th of 1%)';
COMMENT ON COLUMN loans.outstanding_balance IS 'Current remaining balance to be paid';
COMMENT ON COLUMN loan_transactions.transaction_id_hash IS 'SHA256 hash of transaction ID for zkVM privacy';
COMMENT ON COLUMN loan_transactions.user_id_hash IS 'SHA256 hash of user UUID for zkVM privacy';
COMMENT ON COLUMN loan_transactions.loan_id_hash IS 'SHA256 hash of loan UUID for zkVM privacy (NULL for non-loan transactions)';