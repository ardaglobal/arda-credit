-- Migration: Add Company Transaction Types to deal_transactions
--
-- This migration adds company_registration and company_update transaction types
-- to support the company-centric architecture where companies (not users) participate in deals.

-- ============================================================================
-- STEP 1: Drop existing CHECK constraint
-- ============================================================================

ALTER TABLE deal_transactions
    DROP CONSTRAINT IF EXISTS deal_transactions_transaction_type_check;

-- ============================================================================
-- STEP 2: Add updated CHECK constraint with company transaction types
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
        'admin_registration',
        -- Deal transaction types
        'deal_origination',
        'deal_payment',
        'deal_closeout',
        -- Legacy transaction types (for backward compatibility)
        'loan_origination',
        'loan_payment',
        'loan_closeout',
        -- Document verification
        'verify_document',
        -- Collateral transaction types
        'collateral_creation',
        'collateral_update',
        'collateral_deal_association',
        'collateral_deal_disassociation',
        'collateral_detection_request',
        'collateral_detection_update',
        -- State management
        'state_regenesis',
        -- Company transaction types (NEW)
        'company_registration',
        'company_update'
    ));

COMMENT ON CONSTRAINT deal_transactions_transaction_type_check ON deal_transactions IS
    'Allowed transaction types including company operations for company-centric architecture';

