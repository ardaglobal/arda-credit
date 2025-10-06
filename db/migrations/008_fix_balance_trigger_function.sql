-- ============================================================================
-- FIX BALANCE TRIGGER FUNCTION
-- ============================================================================
-- This migration updates the create_initial_balances() function to use
-- user_id instead of identity_id, and updates the trigger to work with
-- the users table instead of identities table.
-- ============================================================================

-- ============================================================================
-- STEP 1: Drop old trigger (if it exists on users table from initial schema)
-- ============================================================================

DROP TRIGGER IF EXISTS create_balances_on_identity_insert ON users;

-- ============================================================================
-- STEP 2: Update function to use user_id
-- ============================================================================

CREATE OR REPLACE FUNCTION create_initial_balances()
RETURNS TRIGGER AS $$
BEGIN
    -- Create available balance entry
    INSERT INTO balances (user_id, balance_type, amount, currency)
    VALUES (NEW.id, 'available', 0, 'USDC');

    -- Create locked balance entry
    INSERT INTO balances (user_id, balance_type, amount, currency)
    VALUES (NEW.id, 'locked', 0, 'USDC');

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 3: Create trigger on users table
-- ============================================================================

CREATE TRIGGER create_balances_on_user_insert
    AFTER INSERT ON users
    FOR EACH ROW
    EXECUTE FUNCTION create_initial_balances();

-- ============================================================================
-- STEP 4: Add comment
-- ============================================================================

COMMENT ON FUNCTION create_initial_balances() IS 'Automatically creates initial balance entries when a new user is created';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
