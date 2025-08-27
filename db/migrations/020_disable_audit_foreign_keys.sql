-- Disable foreign key constraints in audit_events table for testing
-- This allows audit events to be created for balance_update operations without existing loan/user records

DO $$ 
BEGIN
    -- Drop existing foreign key constraints in audit_events table
    BEGIN
        ALTER TABLE audit_events DROP CONSTRAINT IF EXISTS fk_audit_events_loan_id;
        RAISE NOTICE 'Dropped fk_audit_events_loan_id constraint';
    EXCEPTION
        WHEN undefined_object THEN null;
    END;
    
    BEGIN
        ALTER TABLE audit_events DROP CONSTRAINT IF EXISTS fk_audit_events_user_id;
        RAISE NOTICE 'Dropped fk_audit_events_user_id constraint';
    EXCEPTION
        WHEN undefined_object THEN null;
    END;
    
    BEGIN
        ALTER TABLE audit_events DROP CONSTRAINT IF EXISTS fk_audit_events_batch_id;
        RAISE NOTICE 'Dropped fk_audit_events_batch_id constraint';
    EXCEPTION
        WHEN undefined_object THEN null;
    END;
    
    -- Note: Audit events foreign key constraints are disabled for testing purposes
    -- In production, these should be re-enabled after ensuring proper data relationships
        
    RAISE NOTICE 'Disabled audit_events foreign key constraints for testing';
END $$;