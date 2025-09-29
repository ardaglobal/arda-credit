-- Migration: Add wallet address and user balances schema
-- Description: Add missing schema elements required by the application code

BEGIN;

-- ============================================================================
-- ADD WALLET ADDRESS COLUMN TO IDENTITIES TABLE (SKIP IF EXISTS)
-- ============================================================================

-- Only add wallet_address if it doesn't already exist (migration 28 may have added it)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'identities'
        AND column_name = 'wallet_address'
    ) THEN
        ALTER TABLE identities ADD COLUMN wallet_address VARCHAR(42);
    END IF;
END $$;

-- ============================================================================
-- CREATE USER BALANCES TABLE (SKIP IF EXISTS)
-- ============================================================================

-- Only create user_balances if it doesn't already exist (migration 31 may have created it)
CREATE TABLE IF NOT EXISTS user_balances (
    user_id UUID PRIMARY KEY REFERENCES identities(id),
    currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    available_balance NUMERIC(78,0) NOT NULL DEFAULT 0,
    lifetime_earnings NUMERIC(78,0) NOT NULL DEFAULT 0,
    last_updated_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(user_id, currency)
);

-- ============================================================================
-- ADD INDEXES FOR USER BALANCES (SKIP IF EXISTS)
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_user_balances_user_id ON user_balances(user_id);
CREATE INDEX IF NOT EXISTS idx_user_balances_currency ON user_balances(currency);
CREATE INDEX IF NOT EXISTS idx_user_balances_updated_at ON user_balances(updated_at);

-- ============================================================================
-- ADD TRIGGER FOR UPDATED_AT ON USER_BALANCES (SKIP IF EXISTS)
-- ============================================================================

-- Only create trigger if it doesn't already exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.triggers
        WHERE event_object_table = 'user_balances'
        AND trigger_name = 'trigger_user_balances_updated_at'
    ) THEN
        CREATE TRIGGER trigger_user_balances_updated_at
            BEFORE UPDATE ON user_balances
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;

-- ============================================================================
-- ADD INDEX FOR WALLET ADDRESS LOOKUP (SKIP IF EXISTS)
-- ============================================================================

-- Only create index if it doesn't already exist (migration 28 may have created it)
CREATE INDEX IF NOT EXISTS idx_identities_wallet_address ON identities(wallet_address)
WHERE wallet_address IS NOT NULL;

COMMIT;

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- This migration adds the following schema elements required by the application:
-- 1. wallet_address column in identities table for blockchain wallet addresses
-- 2. user_balances table for tracking user account balances
-- 3. Appropriate indexes for performance
-- 4. Triggers for automatic timestamp updates
