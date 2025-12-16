-- ============================================================================
-- STANDARDIZE deal_transactions TO USE user_id COLUMN
-- ============================================================================
-- This migration ensures deal_transactions uses user_id (not identity_id)
-- to match the current naming convention after migration 005.
--
-- Context: Migration 001 created the table with identity_id, and migration 005
-- renamed it to user_id. However, fresh databases from migration 001 still have
-- identity_id, causing column name mismatches in application code.
--
-- This migration is idempotent and can safely run on databases that already
-- have the correct column name.
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: Check if we need to rename the column
-- ============================================================================

DO $$
BEGIN
    -- Check if identity_id column exists (old name)
    IF EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'deal_transactions' 
        AND column_name = 'identity_id'
    ) THEN
        RAISE NOTICE 'Found identity_id column, renaming to user_id...';
        
        -- Drop the old foreign key constraint if it exists
        ALTER TABLE deal_transactions
            DROP CONSTRAINT IF EXISTS deal_transactions_identity_id_fkey;
        
        -- Rename the column
        ALTER TABLE deal_transactions 
            RENAME COLUMN identity_id TO user_id;
        
        RAISE NOTICE '✅ Successfully renamed identity_id to user_id';
    ELSE
        RAISE NOTICE '✅ Column user_id already exists, no changes needed';
    END IF;
    
    -- Ensure foreign key constraint exists (idempotent - works whether we just renamed or it already existed)
    IF NOT EXISTS (
        SELECT 1
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_name = kcu.constraint_name
        WHERE tc.table_name = 'deal_transactions'
        AND tc.constraint_type = 'FOREIGN KEY'
        AND kcu.column_name = 'user_id'
        AND tc.constraint_name = 'deal_transactions_user_id_fkey'
    ) THEN
        RAISE NOTICE 'Creating foreign key constraint on user_id...';
        ALTER TABLE deal_transactions
            ADD CONSTRAINT deal_transactions_user_id_fkey
            FOREIGN KEY (user_id) REFERENCES users(id);
        RAISE NOTICE '✅ Created foreign key constraint';
    ELSE
        RAISE NOTICE '✅ Foreign key constraint already exists';
    END IF;
    
    -- Ensure index exists (idempotent)
    IF NOT EXISTS (
        SELECT 1
        FROM pg_indexes
        WHERE tablename = 'deal_transactions'
        AND indexname = 'idx_deal_transactions_user_id'
    ) THEN
        RAISE NOTICE 'Creating index on user_id...';
        CREATE INDEX idx_deal_transactions_user_id ON deal_transactions(user_id);
        RAISE NOTICE '✅ Created index';
    ELSE
        RAISE NOTICE '✅ Index already exists';
    END IF;
END $$;

-- ============================================================================
-- STEP 2: Update comments for clarity
-- ============================================================================

COMMENT ON COLUMN deal_transactions.user_id IS 
    'Reference to user account - renamed from identity_id to match users table';

-- ============================================================================
-- STEP 3: Verify the final state
-- ============================================================================

DO $$
BEGIN
    -- Verify user_id column exists
    IF NOT EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'deal_transactions' 
        AND column_name = 'user_id'
    ) THEN
        RAISE EXCEPTION 'Migration failed: user_id column does not exist in deal_transactions';
    END IF;
    
    -- Verify foreign key exists
    IF NOT EXISTS (
        SELECT 1
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_name = kcu.constraint_name
        WHERE tc.table_name = 'deal_transactions'
        AND tc.constraint_type = 'FOREIGN KEY'
        AND kcu.column_name = 'user_id'
    ) THEN
        RAISE EXCEPTION 'Migration failed: foreign key constraint on user_id does not exist';
    END IF;
    
    RAISE NOTICE '✅ Verification complete: deal_transactions.user_id is properly configured';
END $$;

COMMIT;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

