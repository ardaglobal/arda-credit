-- ============================================================================
-- COMPANY WALLET ADDRESSES MIGRATION
-- ============================================================================
-- This migration moves wallet addresses from being user-owned to company-owned,
-- with granular permission-based access control.
--
-- Key changes:
--   - Wallet addresses belong to companies/organizations
--   - Users access addresses through company memberships + wallet permissions
--   - Support for multiple addresses per company
--   - Granular permissions: view_balance, initiate_withdrawal, approve_withdrawal, etc.
--   - Balances are tracked per wallet address (not per user)
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: Create company_wallet_addresses table
-- ============================================================================

CREATE TABLE company_wallet_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,

    -- Ethereum address
    wallet_address VARCHAR(42) NOT NULL UNIQUE CHECK (wallet_address ~ '^0x[a-fA-F0-9]{40}$'),

    -- Address metadata
    address_label VARCHAR(100), -- e.g., "Treasury Wallet", "Operating Account"
    address_type VARCHAR(50) DEFAULT 'hot_wallet' CHECK (address_type IN ('hot_wallet', 'cold_storage', 'multisig', 'smart_contract')),

    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'archived')),
    is_primary BOOLEAN DEFAULT FALSE, -- One primary address per company

    -- Balance tracking (in wei for USDC - 6 decimals)
    available_balance NUMERIC(78,0) NOT NULL DEFAULT 0 CHECK (available_balance >= 0),
    pending_balance NUMERIC(78,0) NOT NULL DEFAULT 0 CHECK (pending_balance >= 0),
    lifetime_deposits NUMERIC(78,0) NOT NULL DEFAULT 0 CHECK (lifetime_deposits >= 0),
    lifetime_withdrawals NUMERIC(78,0) NOT NULL DEFAULT 0 CHECK (lifetime_withdrawals >= 0),

    -- Metadata
    notes TEXT,
    metadata JSONB DEFAULT '{}',

    -- Audit fields
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Business constraints
    CONSTRAINT company_wallet_addresses_unique_primary
        CHECK (NOT is_primary OR status = 'active')
);

-- Create indexes
CREATE INDEX idx_company_wallet_addresses_company_id
    ON company_wallet_addresses(company_id);
CREATE INDEX idx_company_wallet_addresses_wallet_address
    ON company_wallet_addresses(wallet_address);
CREATE INDEX idx_company_wallet_addresses_status
    ON company_wallet_addresses(company_id, status);
CREATE INDEX idx_company_wallet_addresses_primary
    ON company_wallet_addresses(company_id, is_primary)
    WHERE is_primary = TRUE;

-- Add trigger for updated_at
CREATE TRIGGER trigger_company_wallet_addresses_updated_at
    BEFORE UPDATE ON company_wallet_addresses
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- STEP 2: Create wallet_address_permissions table
-- ============================================================================

CREATE TYPE wallet_permission_type AS ENUM (
    'view_balance',           -- Can view wallet balance
    'view_transactions',      -- Can view transaction history
    'initiate_withdrawal',    -- Can request withdrawals
    'approve_withdrawal',     -- Can approve withdrawal requests (ADMIN ONLY)
    'manage_address',         -- Can update address settings (ADMIN ONLY)
    'add_address',           -- Can add new wallet addresses (ADMIN ONLY)
    'archive_address'        -- Can archive/suspend addresses (ADMIN ONLY)
);

CREATE TABLE wallet_address_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Permission scope
    wallet_address_id UUID REFERENCES company_wallet_addresses(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    membership_id UUID NOT NULL REFERENCES company_memberships(id) ON DELETE CASCADE,

    -- Permission details
    permission_type wallet_permission_type NOT NULL,

    -- Optional constraints
    daily_limit NUMERIC(78,0), -- Max withdrawal amount per day (in wei)
    requires_approval BOOLEAN DEFAULT FALSE, -- Requires another user's approval

    -- Audit fields
    granted_by UUID REFERENCES users(id),
    granted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Ensure unique permission per membership per address per type
    UNIQUE(wallet_address_id, membership_id, permission_type)
);

-- Create indexes
CREATE INDEX idx_wallet_address_permissions_wallet
    ON wallet_address_permissions(wallet_address_id);
CREATE INDEX idx_wallet_address_permissions_company
    ON wallet_address_permissions(company_id);
CREATE INDEX idx_wallet_address_permissions_membership
    ON wallet_address_permissions(membership_id);
CREATE INDEX idx_wallet_address_permissions_type
    ON wallet_address_permissions(permission_type);
CREATE INDEX idx_wallet_address_permissions_expiry
    ON wallet_address_permissions(expires_at)
    WHERE expires_at IS NOT NULL;

-- Create trigger to enforce admin-only sensitive permissions
CREATE OR REPLACE FUNCTION enforce_admin_only_permissions()
RETURNS TRIGGER AS $$
DECLARE
    v_member_role VARCHAR(20);
    v_is_sensitive BOOLEAN;
BEGIN
    -- Check if this is a sensitive permission
    v_is_sensitive := NEW.permission_type IN (
        'approve_withdrawal',
        'manage_address',
        'add_address',
        'archive_address'
    );

    -- If it's a sensitive permission, verify user is admin
    IF v_is_sensitive THEN
        SELECT role INTO v_member_role
        FROM company_memberships
        WHERE id = NEW.membership_id;

        IF v_member_role != 'admin' THEN
            RAISE EXCEPTION 'Permission % can only be granted to admin users', NEW.permission_type
                USING HINT = 'Only company admins can approve withdrawals, manage addresses, add addresses, or archive addresses';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_enforce_admin_only_permissions
    BEFORE INSERT OR UPDATE ON wallet_address_permissions
    FOR EACH ROW
    EXECUTE FUNCTION enforce_admin_only_permissions();

-- ============================================================================
-- STEP 3: Update usdc_deposits to reference wallet addresses
-- ============================================================================

-- Add wallet_address_id column
ALTER TABLE usdc_deposits
ADD COLUMN wallet_address_id UUID REFERENCES company_wallet_addresses(id);

-- Add index
CREATE INDEX idx_usdc_deposits_wallet_address
    ON usdc_deposits(wallet_address_id);

-- ============================================================================
-- STEP 4: Update usdc_withdrawals to reference wallet addresses
-- ============================================================================

-- Add wallet_address_id column
ALTER TABLE usdc_withdrawals
ADD COLUMN wallet_address_id UUID REFERENCES company_wallet_addresses(id);

-- Add index
CREATE INDEX idx_usdc_withdrawals_wallet_address
    ON usdc_withdrawals(wallet_address_id);

-- ============================================================================
-- STEP 5: Helper functions for wallet address management
-- ============================================================================

-- Function to check if user has specific wallet permission
CREATE OR REPLACE FUNCTION has_wallet_permission(
    p_user_id UUID,
    p_wallet_address_id UUID,
    p_permission_type wallet_permission_type
)
RETURNS BOOLEAN AS $$
DECLARE
    v_has_permission BOOLEAN;
BEGIN
    -- Check if user has active membership with this permission for this wallet
    SELECT EXISTS(
        SELECT 1
        FROM wallet_address_permissions wap
        JOIN company_memberships cm ON wap.membership_id = cm.id
        WHERE cm.user_id = p_user_id
          AND wap.wallet_address_id = p_wallet_address_id
          AND wap.permission_type = p_permission_type
          AND cm.status = 'active'
          AND (wap.expires_at IS NULL OR wap.expires_at > NOW())
    ) INTO v_has_permission;

    RETURN v_has_permission;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if user can authorize transactions for a wallet
-- (must be admin with approve_withdrawal permission)
CREATE OR REPLACE FUNCTION can_authorize_transaction(
    p_user_id UUID,
    p_wallet_address_id UUID
)
RETURNS BOOLEAN AS $$
DECLARE
    v_can_authorize BOOLEAN;
BEGIN
    -- User must be:
    -- 1. An active admin member of the company
    -- 2. Have approve_withdrawal permission for this wallet
    SELECT EXISTS(
        SELECT 1
        FROM wallet_address_permissions wap
        JOIN company_memberships cm ON wap.membership_id = cm.id
        JOIN company_wallet_addresses cwa ON wap.wallet_address_id = cwa.id
        WHERE cm.user_id = p_user_id
          AND wap.wallet_address_id = p_wallet_address_id
          AND wap.permission_type = 'approve_withdrawal'
          AND cm.role = 'admin'
          AND cm.status = 'active'
          AND cwa.status = 'active'
          AND (wap.expires_at IS NULL OR wap.expires_at > NOW())
    ) INTO v_can_authorize;

    RETURN v_can_authorize;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get user's accessible wallet addresses in a company
CREATE OR REPLACE FUNCTION get_user_wallet_addresses(
    p_user_id UUID,
    p_company_id UUID
)
RETURNS TABLE(
    wallet_address_id UUID,
    wallet_address VARCHAR(42),
    address_label VARCHAR(100),
    available_balance NUMERIC(78,0),
    permissions wallet_permission_type[]
) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT
        cwa.id,
        cwa.wallet_address,
        cwa.address_label,
        cwa.available_balance,
        ARRAY_AGG(DISTINCT wap.permission_type) AS permissions
    FROM company_wallet_addresses cwa
    JOIN wallet_address_permissions wap ON cwa.id = wap.wallet_address_id
    JOIN company_memberships cm ON wap.membership_id = cm.id
    WHERE cm.user_id = p_user_id
      AND cwa.company_id = p_company_id
      AND cm.status = 'active'
      AND cwa.status = 'active'
      AND (wap.expires_at IS NULL OR wap.expires_at > NOW())
    GROUP BY cwa.id, cwa.wallet_address, cwa.address_label, cwa.available_balance;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to set primary wallet address for a company
CREATE OR REPLACE FUNCTION set_primary_wallet_address(
    p_wallet_address_id UUID
)
RETURNS VOID AS $$
DECLARE
    v_company_id UUID;
BEGIN
    -- Get company_id for the wallet
    SELECT company_id INTO v_company_id
    FROM company_wallet_addresses
    WHERE id = p_wallet_address_id;

    -- Unset all other primary addresses for this company
    UPDATE company_wallet_addresses
    SET is_primary = FALSE
    WHERE company_id = v_company_id
      AND id != p_wallet_address_id;

    -- Set this address as primary
    UPDATE company_wallet_addresses
    SET is_primary = TRUE, status = 'active'
    WHERE id = p_wallet_address_id;
END;
$$ LANGUAGE plpgsql;

-- Function to grant default permissions to company admin for a wallet
CREATE OR REPLACE FUNCTION grant_admin_wallet_permissions(
    p_wallet_address_id UUID,
    p_membership_id UUID,
    p_granted_by UUID
)
RETURNS VOID AS $$
DECLARE
    v_company_id UUID;
BEGIN
    -- Get company_id for the wallet
    SELECT company_id INTO v_company_id
    FROM company_wallet_addresses
    WHERE id = p_wallet_address_id;

    -- Grant all permissions to admin
    INSERT INTO wallet_address_permissions (
        wallet_address_id,
        company_id,
        membership_id,
        permission_type,
        granted_by
    )
    SELECT
        p_wallet_address_id,
        v_company_id,
        p_membership_id,
        unnest(ARRAY[
            'view_balance'::wallet_permission_type,
            'view_transactions'::wallet_permission_type,
            'initiate_withdrawal'::wallet_permission_type,
            'approve_withdrawal'::wallet_permission_type,
            'manage_address'::wallet_permission_type,
            'add_address'::wallet_permission_type,
            'archive_address'::wallet_permission_type
        ]),
        p_granted_by
    ON CONFLICT (wallet_address_id, membership_id, permission_type) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 6: Update balance trigger to work with wallet addresses
-- ============================================================================

-- Drop old user_balances trigger if it exists
DROP TRIGGER IF EXISTS update_user_balance_on_deposit ON usdc_deposits;
DROP FUNCTION IF EXISTS update_user_balance_on_deposit();

-- Create new trigger function for wallet address balances
CREATE OR REPLACE FUNCTION update_wallet_balance_on_deposit()
RETURNS TRIGGER AS $$
BEGIN
    -- Only update if wallet_address_id is set and status is confirmed
    IF NEW.wallet_address_id IS NOT NULL AND NEW.status = 'confirmed' THEN
        UPDATE company_wallet_addresses
        SET
            available_balance = available_balance + NEW.amount,
            lifetime_deposits = lifetime_deposits + NEW.amount,
            updated_at = NOW()
        WHERE id = NEW.wallet_address_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on usdc_deposits
CREATE TRIGGER trigger_update_wallet_balance_on_deposit
    AFTER INSERT OR UPDATE ON usdc_deposits
    FOR EACH ROW
    WHEN (NEW.status = 'confirmed')
    EXECUTE FUNCTION update_wallet_balance_on_deposit();

-- ============================================================================
-- STEP 7: Add comments
-- ============================================================================

COMMENT ON TABLE company_wallet_addresses IS
'Wallet addresses owned by companies/organizations. Users access them through membership permissions.';

COMMENT ON TABLE wallet_address_permissions IS
'Granular permissions for users to access and manage company wallet addresses.';

COMMENT ON COLUMN company_wallet_addresses.is_primary IS
'Primary wallet address for the company. Only one active address can be primary per company.';

COMMENT ON COLUMN company_wallet_addresses.available_balance IS
'Available balance in wei (6 decimals for USDC). Updated by deposits and withdrawals.';

COMMENT ON FUNCTION has_wallet_permission(UUID, UUID, wallet_permission_type) IS
'Check if a user has a specific permission for a wallet address through their company membership.';

COMMENT ON FUNCTION get_user_wallet_addresses(UUID, UUID) IS
'Get all wallet addresses accessible to a user in a company, along with their permissions.';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
