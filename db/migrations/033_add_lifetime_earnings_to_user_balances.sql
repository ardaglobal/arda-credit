-- Migration: Add lifetime_earnings and last_updated_at to user_balances
-- Description: Add tracking for lifetime earnings and last update timestamp

BEGIN;

-- Add missing columns to user_balances table (SKIP IF EXISTS)
DO $$
BEGIN
    -- Add lifetime_earnings column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'lifetime_earnings'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN lifetime_earnings NUMERIC(78, 0) NOT NULL DEFAULT 0;
    END IF;

    -- Add last_updated_at column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'last_updated_at'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
    END IF;
END $$;

-- Add constraints for new columns (SKIP IF EXISTS)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'positive_lifetime_earnings'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT positive_lifetime_earnings CHECK (lifetime_earnings >= 0);
    END IF;
END $$;

-- Add indexes for new columns (SKIP IF EXISTS)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_lifetime_earnings'
    ) THEN
        CREATE INDEX idx_user_balances_lifetime_earnings ON user_balances(lifetime_earnings) WHERE lifetime_earnings > 0;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_last_updated'
    ) THEN
        CREATE INDEX idx_user_balances_last_updated ON user_balances(last_updated_at);
    END IF;
END $$;

-- Update the trigger to also update last_updated_at
CREATE OR REPLACE FUNCTION update_user_balances_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.last_updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Replace the existing trigger
DROP TRIGGER IF EXISTS trigger_user_balances_updated_at ON user_balances;
CREATE TRIGGER trigger_user_balances_updated_at
    BEFORE UPDATE ON user_balances
    FOR EACH ROW
    EXECUTE FUNCTION update_user_balances_timestamp();

-- Comments for documentation
COMMENT ON COLUMN user_balances.lifetime_earnings IS 'Total earnings accumulated over the lifetime of the account';
COMMENT ON COLUMN user_balances.last_updated_at IS 'Timestamp of the last balance update operation';

COMMIT;