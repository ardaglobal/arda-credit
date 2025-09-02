-- Add KYC Investor Type to identities table
-- This migration adds a new column to classify investors by their qualification type

-- ============================================================================
-- ADD KYC_INVESTOR_TYPE COLUMN TO IDENTITIES TABLE
-- ============================================================================

-- Add kyc_investor_type column with constraint
ALTER TABLE identities ADD COLUMN kyc_investor_type VARCHAR(3) 
CHECK (kyc_investor_type IN ('QIB', 'QP', 'AI'));

-- Create index for filtering by investor type
CREATE INDEX idx_identities_kyc_investor_type 
ON identities(kyc_investor_type) 
WHERE kyc_investor_type IS NOT NULL;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON COLUMN identities.kyc_investor_type IS 'KYC investor qualification type: QIB (Qualified Institutional Buyer), QP (Qualified Purchaser), AI (Accredited Investor)';

-- ============================================================================
-- MIGRATION NOTES
-- ============================================================================

-- This column is nullable to allow existing users who haven't completed KYC
-- with the new investor type requirement to continue functioning.
-- The column will be required for new KYC submissions.