-- Update all possible loan operation type constraints to include balance_update
-- This migration handles different constraint naming patterns that may exist

DO $$ 
DECLARE
    constraint_record RECORD;
BEGIN
    -- Find and update any constraint on loan_operation_type column
    FOR constraint_record IN 
        SELECT cc.constraint_name, ccu.table_name 
        FROM information_schema.check_constraints cc
        JOIN information_schema.constraint_column_usage ccu ON cc.constraint_name = ccu.constraint_name
        WHERE ccu.column_name = 'loan_operation_type' AND ccu.table_name = 'nullifiers'
    LOOP
        -- Drop the existing constraint
        EXECUTE format('ALTER TABLE nullifiers DROP CONSTRAINT IF EXISTS %I', constraint_record.constraint_name);
        
        -- Add updated constraint with balance_update included
        EXECUTE format('ALTER TABLE nullifiers ADD CONSTRAINT %I 
            CHECK (loan_operation_type IS NULL OR 
                   loan_operation_type IN (''loan_origination'', ''loan_payment'', ''loan_closeout'', 
                                         ''user_registration'', ''kyc_submission'', ''kyc_verification'',
                                         ''balance_update''))', constraint_record.constraint_name);
                                         
        RAISE NOTICE 'Updated constraint % on table %', constraint_record.constraint_name, constraint_record.table_name;
    END LOOP;
    
    -- If no constraint was found, create the standard one
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints cc
        JOIN information_schema.constraint_column_usage ccu ON cc.constraint_name = ccu.constraint_name
        WHERE ccu.column_name = 'loan_operation_type' AND ccu.table_name = 'nullifiers'
    ) THEN
        ALTER TABLE nullifiers ADD CONSTRAINT chk_loan_operation_type 
            CHECK (loan_operation_type IS NULL OR 
                   loan_operation_type IN ('loan_origination', 'loan_payment', 'loan_closeout', 
                                         'user_registration', 'kyc_submission', 'kyc_verification',
                                         'balance_update'));
        RAISE NOTICE 'Created new constraint chk_loan_operation_type';
    END IF;
END $$;