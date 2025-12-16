-- Migration: Add state_regenesis transaction type
-- Description: Adds 'state_regenesis' to allowed transaction types for re-genesis operations

-- ============================================================================
-- STEP 1: Drop the existing CHECK constraint
-- ============================================================================

ALTER TABLE deal_transactions
    DROP CONSTRAINT IF EXISTS deal_transactions_transaction_type_check;

-- ============================================================================
-- STEP 2: Add updated CHECK constraint with state_regenesis
-- ============================================================================

ALTER TABLE deal_transactions
    ADD CONSTRAINT deal_transactions_transaction_type_check
    CHECK (transaction_type IN (
        -- User transaction types
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
        -- Collateral transaction types
        'collateral_creation',
        'collateral_update',
        'collateral_deal_association',
        'collateral_deal_disassociation',
        'collateral_detection_request',
        'collateral_detection_update',
        -- State management transaction types
        'state_regenesis'
    ));

-- ============================================================================
-- STEP 3: Add comment documenting the change
-- ============================================================================

COMMENT ON CONSTRAINT deal_transactions_transaction_type_check ON deal_transactions IS
    'Allowed transaction types including collateral operations and state management for zkVM processing';

