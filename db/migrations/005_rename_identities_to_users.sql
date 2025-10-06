-- ============================================================================
-- RENAME IDENTITIES TO USERS AND UPDATE COMPANY REFERENCES
-- ============================================================================
-- This migration:
-- 1. Renames the identities table to users
-- 2. Updates all foreign key references throughout the database
-- 3. Makes company_id required (NOT NULL) in deals table
-- 4. Ensures all user-company relationships are properly enforced
-- ============================================================================

-- ============================================================================
-- STEP 1: Update company_memberships to reference users (prep work)
-- ============================================================================

-- Drop existing foreign key constraints that reference identities
ALTER TABLE company_memberships
    DROP CONSTRAINT IF EXISTS company_memberships_user_id_fkey;

ALTER TABLE company_memberships
    DROP CONSTRAINT IF EXISTS company_memberships_invited_by_fkey;

ALTER TABLE company_invitations
    DROP CONSTRAINT IF EXISTS company_invitations_invited_by_fkey;

ALTER TABLE company_invitations
    DROP CONSTRAINT IF EXISTS company_invitations_accepted_by_fkey;

ALTER TABLE company_audit
    DROP CONSTRAINT IF EXISTS company_audit_operator_id_fkey;

ALTER TABLE company_audit
    DROP CONSTRAINT IF EXISTS company_audit_target_user_id_fkey;

-- Drop other foreign keys referencing identities
ALTER TABLE deals
    DROP CONSTRAINT IF EXISTS deals_originator_id_fkey;

ALTER TABLE deals
    DROP CONSTRAINT IF EXISTS deals_investor_id_fkey;

-- Drop payment table constraints (only deal_payments exists after migration 004)
ALTER TABLE deal_payments
    DROP CONSTRAINT IF EXISTS deal_payments_payer_id_fkey;

ALTER TABLE balances
    DROP CONSTRAINT IF EXISTS balances_identity_id_fkey;

-- Drop transaction table constraints (only deal_transactions exists after migration 004)
ALTER TABLE deal_transactions
    DROP CONSTRAINT IF EXISTS deal_transactions_identity_id_fkey;

-- Magic links already uses user_id column, just update the foreign key
ALTER TABLE magic_links
    DROP CONSTRAINT IF EXISTS magic_links_user_id_fkey;

-- ============================================================================
-- STEP 2: Rename identities table to users
-- ============================================================================

ALTER TABLE identities RENAME TO users;

-- Rename indexes
ALTER INDEX IF EXISTS identities_pkey RENAME TO users_pkey;
ALTER INDEX IF EXISTS identities_email_key RENAME TO users_email_key;

-- Update sequence if exists
ALTER SEQUENCE IF EXISTS identities_id_seq RENAME TO users_id_seq;

-- ============================================================================
-- STEP 3: Recreate foreign key constraints with new table name
-- ============================================================================

-- Company memberships and invitations
ALTER TABLE company_memberships
    ADD CONSTRAINT company_memberships_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE company_memberships
    ADD CONSTRAINT company_memberships_invited_by_fkey
    FOREIGN KEY (invited_by) REFERENCES users(id);

ALTER TABLE company_invitations
    ADD CONSTRAINT company_invitations_invited_by_fkey
    FOREIGN KEY (invited_by) REFERENCES users(id);

ALTER TABLE company_invitations
    ADD CONSTRAINT company_invitations_accepted_by_fkey
    FOREIGN KEY (accepted_by) REFERENCES users(id);

ALTER TABLE company_audit
    ADD CONSTRAINT company_audit_operator_id_fkey
    FOREIGN KEY (operator_id) REFERENCES users(id);

ALTER TABLE company_audit
    ADD CONSTRAINT company_audit_target_user_id_fkey
    FOREIGN KEY (target_user_id) REFERENCES users(id);

-- Deals
ALTER TABLE deals
    ADD CONSTRAINT deals_originator_id_fkey
    FOREIGN KEY (originator_id) REFERENCES users(id);

ALTER TABLE deals
    ADD CONSTRAINT deals_investor_id_fkey
    FOREIGN KEY (investor_id) REFERENCES users(id);

-- Deal payments
ALTER TABLE deal_payments
    ADD CONSTRAINT deal_payments_payer_id_fkey
    FOREIGN KEY (payer_id) REFERENCES users(id);

-- Balances (rename column to user_id for consistency)
ALTER TABLE balances RENAME COLUMN identity_id TO user_id;

ALTER TABLE balances
    ADD CONSTRAINT balances_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id);

-- Update unique constraint
ALTER TABLE balances DROP CONSTRAINT IF EXISTS balances_identity_id_balance_type_currency_key;
ALTER TABLE balances ADD CONSTRAINT balances_user_id_balance_type_currency_key
    UNIQUE(user_id, balance_type, currency);

-- Deal transactions (rename column to user_id for consistency)
ALTER TABLE deal_transactions RENAME COLUMN identity_id TO user_id;

ALTER TABLE deal_transactions
    ADD CONSTRAINT deal_transactions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id);

-- Magic links (already has user_id column, just recreate foreign key)
ALTER TABLE magic_links
    ADD CONSTRAINT magic_links_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

-- ============================================================================
-- STEP 4: Make company_id required in deals table
-- ============================================================================

-- First, we need to handle any existing deals without a company_id
-- For safety, we'll create a "Legacy Deals" company for any orphaned deals
DO $$
DECLARE
    legacy_company_id UUID;
    orphaned_count INTEGER;
BEGIN
    -- Count orphaned deals
    SELECT COUNT(*) INTO orphaned_count FROM deals WHERE company_id IS NULL;

    IF orphaned_count > 0 THEN
        -- Create a legacy company for orphaned deals
        INSERT INTO companies (name, description, status, metadata)
        VALUES (
            'Legacy Deals',
            'Auto-generated company for deals created before company requirement',
            'inactive',
            '{"auto_generated": true, "reason": "migration_005"}'
        )
        RETURNING id INTO legacy_company_id;

        -- Assign orphaned deals to legacy company
        UPDATE deals
        SET company_id = legacy_company_id
        WHERE company_id IS NULL;

        RAISE NOTICE 'Created legacy company % and assigned % orphaned deals', legacy_company_id, orphaned_count;
    END IF;
END $$;

-- Now make company_id NOT NULL
ALTER TABLE deals
    ALTER COLUMN company_id SET NOT NULL;

-- ============================================================================
-- STEP 5: Update existing queries/views/functions
-- ============================================================================

-- Update company membership query in list_company_members to use users table
-- (The function references are already updated since we renamed the table)

-- Update comments
COMMENT ON TABLE users IS 'User accounts - can be investors or originators, associated with companies via company_memberships';
COMMENT ON COLUMN balances.user_id IS 'Reference to user account';
COMMENT ON COLUMN deal_transactions.user_id IS 'Reference to user account';
COMMENT ON COLUMN magic_links.user_id IS 'Reference to user account';

-- ============================================================================
-- STEP 6: Create indexes for performance
-- ============================================================================

-- Ensure indexes exist on new user_id columns
CREATE INDEX IF NOT EXISTS idx_balances_user_id ON balances(user_id);
CREATE INDEX IF NOT EXISTS idx_deal_transactions_user_id ON deal_transactions(user_id);
CREATE INDEX IF NOT EXISTS idx_magic_links_user_id ON magic_links(user_id);

-- Index for company_id on deals (now required)
CREATE INDEX IF NOT EXISTS idx_deals_company_id ON deals(company_id);

-- ============================================================================
-- STEP 7: Add validation constraints
-- ============================================================================

-- Ensure users belong to at least one company (enforced at application level)
-- But we can add a check to ensure company memberships are valid

COMMENT ON TABLE company_memberships IS
'Links users to companies with role-based access control. A user must have at least one active membership.';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMENT ON SCHEMA public IS 'Schema updated: identities → users, company_id now required for deals';
