-- Add balance_update to allowed loan operation types
-- This migration extends the check constraint to allow 'balance_update' operation type

DO $$ 
BEGIN
    -- Drop existing constraint if it exists
    BEGIN
        ALTER TABLE nullifiers DROP CONSTRAINT IF EXISTS chk_loan_operation_type;
    EXCEPTION
        WHEN undefined_object THEN null;
    END;
    
    -- Add updated constraint with balance_update operation type
    ALTER TABLE nullifiers ADD CONSTRAINT chk_loan_operation_type 
        CHECK (loan_operation_type IS NULL OR 
               loan_operation_type IN ('loan_origination', 'loan_payment', 'loan_closeout', 
                                     'user_registration', 'kyc_submission', 'kyc_verification',
                                     'balance_update'));
END $$;