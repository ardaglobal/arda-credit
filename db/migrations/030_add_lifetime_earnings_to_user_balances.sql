-- Migration: Add lifetime_earnings and last_updated_at to user_balances
-- Description: Add tracking for lifetime earnings and last update timestamp

BEGIN;

-- Add missing columns to user_balances table
ALTER TABLE user_balances ADD COLUMN lifetime_earnings NUMERIC(78, 0) NOT NULL DEFAULT 0;
ALTER TABLE user_balances ADD COLUMN last_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- Add constraints for new columns
ALTER TABLE user_balances ADD CONSTRAINT positive_lifetime_earnings CHECK (lifetime_earnings >= 0);

-- Add index for lifetime earnings queries
CREATE INDEX idx_user_balances_lifetime_earnings ON user_balances(lifetime_earnings) WHERE lifetime_earnings > 0;
CREATE INDEX idx_user_balances_last_updated ON user_balances(last_updated_at);

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