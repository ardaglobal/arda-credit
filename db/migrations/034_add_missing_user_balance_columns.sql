-- Migration: Add missing user balance columns
-- Description: Add columns to user_balances table that were expected by later migrations

BEGIN;

-- Add id column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'id'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN id SERIAL PRIMARY KEY;
    END IF;
END $$;

-- Add reserved_balance column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'reserved_balance'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN reserved_balance NUMERIC(78,0) NOT NULL DEFAULT 0;
    END IF;
END $$;

-- Add total_deposited column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'total_deposited'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN total_deposited NUMERIC(78,0) NOT NULL DEFAULT 0;
    END IF;
END $$;

-- Add deposit_count column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'deposit_count'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN deposit_count INTEGER NOT NULL DEFAULT 0;
    END IF;
END $$;

-- Add first_deposit_at column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'first_deposit_at'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN first_deposit_at TIMESTAMP WITH TIME ZONE;
    END IF;
END $$;

-- Add last_deposit_at column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_balances'
        AND column_name = 'last_deposit_at'
    ) THEN
        ALTER TABLE user_balances ADD COLUMN last_deposit_at TIMESTAMP WITH TIME ZONE;
    END IF;
END $$;

-- Add missing indexes if they don't exist
CREATE INDEX IF NOT EXISTS idx_user_balances_available_balance ON user_balances(available_balance) WHERE available_balance > 0;
CREATE INDEX IF NOT EXISTS idx_user_balances_last_deposit ON user_balances(last_deposit_at) WHERE last_deposit_at IS NOT NULL;

-- Add missing constraints if they don't exist
DO $$
BEGIN
    -- Add positive balance constraints if they don't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'positive_available_balance'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT positive_available_balance CHECK (available_balance >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'positive_reserved_balance'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT positive_reserved_balance CHECK (reserved_balance >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'positive_total_deposited'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT positive_total_deposited CHECK (total_deposited >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'positive_deposit_count'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT positive_deposit_count CHECK (deposit_count >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'valid_currency'
    ) THEN
        ALTER TABLE user_balances ADD CONSTRAINT valid_currency CHECK (currency IN ('USDC', 'USD', 'ETH'));
    END IF;
END $$;

COMMIT;

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- This migration ensures compatibility between different user_balances table schemas
-- created by migrations 30 and 31. It adds any missing columns and constraints
-- that later migrations expect to exist.
