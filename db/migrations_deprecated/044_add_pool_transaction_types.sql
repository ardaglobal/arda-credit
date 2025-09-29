-- Add new pool transaction types to loan_transactions table
-- This migration extends the existing CHECK constraint to support pool operations

-- Drop existing constraint
ALTER TABLE loan_transactions DROP CONSTRAINT loan_transactions_transaction_type_check;

-- Add new constraint with pool transaction types
ALTER TABLE loan_transactions ADD CONSTRAINT loan_transactions_transaction_type_check 
    CHECK (transaction_type IN (
        'user_registration', 
        'kyc_submission', 
        'kyc_verification', 
        'loan_origination', 
        'loan_payment', 
        'loan_closeout',
        'user_deposit',
        'user_withdrawal',
        'investor_registration',
        'pool_creation'
    ));

-- Table comment update
COMMENT ON COLUMN loan_transactions.transaction_type IS 'Transaction type: user_registration, kyc_submission, kyc_verification, loan_origination, loan_payment, loan_closeout, user_deposit, user_withdrawal, investor_registration, pool_creation';