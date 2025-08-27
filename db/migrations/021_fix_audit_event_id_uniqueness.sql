-- Fix audit event ID generation to ensure uniqueness
-- Replace timestamp-based ID with UUID-based approach

DO $$ 
BEGIN
    -- Drop and recreate the audit trigger function with UUID-based event IDs
    DROP FUNCTION IF EXISTS create_loan_operation_audit_trigger() CASCADE;
    
    CREATE OR REPLACE FUNCTION create_loan_operation_audit_trigger()
    RETURNS TRIGGER AS $trigger$
    DECLARE
        current_root BYTEA;
        previous_root BYTEA;
        event_uuid VARCHAR;
    BEGIN
        -- Only process if this is a loan operation
        IF NEW.loan_operation_type IS NOT NULL THEN
            -- Get current tree root
            SELECT root_hash INTO current_root FROM tree_state WHERE tree_id = 'default';
            
            -- For new insertions, use previous root (simplified approach)
            previous_root := COALESCE(current_root, '\x0000000000000000000000000000000000000000000000000000000000000000'::bytea);
            
            -- Generate unique event ID using UUID and nullifier ID
            event_uuid := 'loan_' || NEW.loan_operation_type || '_' || NEW.id::text || '_' || replace(gen_random_uuid()::text, '-', '');
            
            -- Create audit event
            PERFORM create_loan_audit_event(
                event_uuid,
                NEW.value,
                NEW.loan_operation_type,
                previous_root,
                current_root,
                NEW.loan_id,
                NEW.user_id,
                NEW.operation_amount,
                NEW.batch_id,
                NULL, -- transaction_hash (to be filled when posted on-chain)
                0,    -- block_height (to be filled when posted on-chain)
                NULL, -- operator (to be filled when posted on-chain)
                NEW.operation_metadata
            );
        END IF;
        
        RETURN NEW;
    END;
    $trigger$ LANGUAGE plpgsql;

    -- Recreate the trigger
    DROP TRIGGER IF EXISTS nullifiers_loan_audit_trigger ON nullifiers;
    CREATE TRIGGER nullifiers_loan_audit_trigger
        AFTER INSERT OR UPDATE ON nullifiers
        FOR EACH ROW
        EXECUTE FUNCTION create_loan_operation_audit_trigger();
        
    RAISE NOTICE 'Updated audit trigger function to use UUID-based event IDs';
END $$;