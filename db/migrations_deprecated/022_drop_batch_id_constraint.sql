-- Drop batch_id foreign key constraint for testing
-- This allows nullifiers to reference non-existent batch IDs in tests

DO $$ 
BEGIN
    BEGIN
        ALTER TABLE nullifiers DROP CONSTRAINT IF EXISTS fk_nullifiers_batch_id;
        RAISE NOTICE 'Dropped fk_nullifiers_batch_id constraint';
    EXCEPTION
        WHEN undefined_object THEN null;
    END;
    
    RAISE NOTICE 'Disabled batch_id foreign key constraint for testing';
END $$;