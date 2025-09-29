-- Migration: Create user_balances table
-- Description: Add table for tracking user balances by currency

BEGIN;

-- Add missing columns to user_balances table if it exists
-- (Migration 30 may have created a simpler version)
DO $$
BEGIN
    -- Only proceed if user_balances table exists
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'user_balances'
    ) THEN

        -- Add id column if it doesn't exist (but don't make it primary key if one exists)
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'id'
        ) THEN
            -- Check if there's already a primary key
            IF NOT EXISTS (
                SELECT 1 FROM information_schema.table_constraints
                WHERE table_name = 'user_balances'
                AND constraint_type = 'PRIMARY KEY'
            ) THEN
                ALTER TABLE user_balances ADD COLUMN id SERIAL PRIMARY KEY;
            ELSE
                ALTER TABLE user_balances ADD COLUMN id SERIAL;
            END IF;
        END IF;

        -- Add reserved_balance column if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'reserved_balance'
        ) THEN
            ALTER TABLE user_balances ADD COLUMN reserved_balance NUMERIC(78, 0) NOT NULL DEFAULT 0;
        END IF;

        -- Add total_deposited column if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'total_deposited'
        ) THEN
            ALTER TABLE user_balances ADD COLUMN total_deposited NUMERIC(78, 0) NOT NULL DEFAULT 0;
        END IF;

        -- Add deposit_count column if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'deposit_count'
        ) THEN
            ALTER TABLE user_balances ADD COLUMN deposit_count INTEGER NOT NULL DEFAULT 0;
        END IF;

        -- Add first_deposit_at column if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'first_deposit_at'
        ) THEN
            ALTER TABLE user_balances ADD COLUMN first_deposit_at TIMESTAMP WITH TIME ZONE;
        END IF;

        -- Add last_deposit_at column if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances'
            AND column_name = 'last_deposit_at'
        ) THEN
            ALTER TABLE user_balances ADD COLUMN last_deposit_at TIMESTAMP WITH TIME ZONE;
        END IF;

        -- Add missing constraints if they don't exist
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

    END IF;
END $$;

-- Indexes for efficient querying (SKIP IF EXISTS)
-- Only create indexes if the table exists and has the required columns
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'user_balances'
    ) THEN
        -- Create indexes only if they don't exist
        IF NOT EXISTS (
            SELECT 1 FROM pg_indexes
            WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_user_id'
        ) THEN
            CREATE INDEX idx_user_balances_user_id ON user_balances(user_id);
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_indexes
            WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_currency'
        ) THEN
            CREATE INDEX idx_user_balances_currency ON user_balances(currency);
        END IF;

        -- Only create these indexes if the columns exist
        IF EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances' AND column_name = 'available_balance'
        ) AND NOT EXISTS (
            SELECT 1 FROM pg_indexes
            WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_available_balance'
        ) THEN
            CREATE INDEX idx_user_balances_available_balance ON user_balances(available_balance) WHERE available_balance > 0;
        END IF;

        IF EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'user_balances' AND column_name = 'last_deposit_at'
        ) AND NOT EXISTS (
            SELECT 1 FROM pg_indexes
            WHERE tablename = 'user_balances' AND indexname = 'idx_user_balances_last_deposit'
        ) THEN
            CREATE INDEX idx_user_balances_last_deposit ON user_balances(last_deposit_at) WHERE last_deposit_at IS NOT NULL;
        END IF;
    END IF;
END $$;

-- Trigger to update updated_at timestamp (SKIP IF EXISTS)
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

-- Function to initialize default balance for new users (SKIP IF EXISTS)
-- Only create if the user_balances table exists
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'user_balances'
    ) THEN
        -- Check if function already exists before creating
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.routines
            WHERE routine_name = 'create_initial_user_balances'
            AND routine_schema = 'public'
        ) THEN
            -- Create function using dynamic SQL
            EXECUTE '
                CREATE FUNCTION create_initial_user_balances()
                RETURNS TRIGGER AS $func$
                BEGIN
                    -- Create initial USDC balance record for new users
                    INSERT INTO user_balances (user_id, currency, available_balance)
                    VALUES (NEW.id, ''USDC'', 0);

                    RETURN NEW;
                END;
                $func$ LANGUAGE plpgsql;
            ';
        END IF;
    END IF;
END $$;

-- Trigger to create initial balance when identity is created (SKIP IF EXISTS)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'user_balances'
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.triggers
        WHERE event_object_table = 'identities'
        AND trigger_name = 'create_balances_on_identity_insert_v2'
    ) THEN
        CREATE TRIGGER create_balances_on_identity_insert_v2
            AFTER INSERT ON identities
            FOR EACH ROW
            EXECUTE FUNCTION create_initial_user_balances();
    END IF;
END $$;

-- Table comments
COMMENT ON TABLE user_balances IS 'User balance tracking by currency with deposit history';
COMMENT ON COLUMN user_balances.available_balance IS 'Available balance for transactions (in smallest currency unit)';
COMMENT ON COLUMN user_balances.reserved_balance IS 'Balance reserved for pending transactions';
COMMENT ON COLUMN user_balances.total_deposited IS 'Cumulative total of all deposits ever made';

COMMIT;