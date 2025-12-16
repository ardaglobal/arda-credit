-- Migration: Add collateral transaction types to deal_transactions
-- Description: Updates the deal_transactions table CHECK constraint to include
--              collateral-related transaction types for zkVM integration

-- ============================================================================
-- STEP 1: Drop the existing CHECK constraint
-- ============================================================================

ALTER TABLE deal_transactions
    DROP CONSTRAINT IF EXISTS deal_transactions_transaction_type_check;

-- ============================================================================
-- STEP 2: Add updated CHECK constraint with collateral transaction types
-- ============================================================================

ALTER TABLE deal_transactions
    ADD CONSTRAINT deal_transactions_transaction_type_check
    CHECK (transaction_type IN (
        -- Existing transaction types (keep for backward compatibility)
        'user_registration',
        'kyc_submission',
        'kyc_verification',
        'user_deposit',
        'user_withdrawal',
        'investor_registration',
        -- Deal transaction types
        'deal_origination',
        'deal_payment',
        'deal_closeout',
        -- NEW: Collateral transaction types (for zkVM coverage)
        'collateral_creation',
        'collateral_update',
        'collateral_deal_association',
        'collateral_deal_disassociation',
        'collateral_detection_request',
        'collateral_detection_update'
    ));

-- ============================================================================
-- STEP 3: Add comment documenting the change
-- ============================================================================

COMMENT ON CONSTRAINT deal_transactions_transaction_type_check ON deal_transactions IS
    'Allowed transaction types including collateral operations for zkVM processing';

