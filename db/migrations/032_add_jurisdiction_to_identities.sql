-- Add jurisdiction column to identities table
-- This migration adds the missing jurisdiction field that was referenced in the API but not implemented in the database

-- Add jurisdiction column
ALTER TABLE identities ADD COLUMN jurisdiction VARCHAR(100);

-- Create index for jurisdiction lookups
CREATE INDEX idx_identities_jurisdiction ON identities(jurisdiction) WHERE jurisdiction IS NOT NULL;

-- Add constraint to ensure jurisdiction is not empty if provided
ALTER TABLE identities ADD CONSTRAINT identities_jurisdiction_length CHECK (jurisdiction IS NULL OR LENGTH(TRIM(jurisdiction)) > 0);

-- Add comment for documentation
COMMENT ON COLUMN identities.jurisdiction IS 'User jurisdiction/country for regulatory compliance and KYC purposes (e.g., US, CA, GB)';
