-- Add missing deal columns for enhanced deal metadata
-- These columns enable better deal categorization and document management

-- Add status column for deal lifecycle (separate from loan_status)
ALTER TABLE deals ADD COLUMN IF NOT EXISTS status VARCHAR(50);

-- Add industry column for deal categorization
ALTER TABLE deals ADD COLUMN IF NOT EXISTS industry VARCHAR(100);

-- Add document_ids column for document management
ALTER TABLE deals ADD COLUMN IF NOT EXISTS document_ids JSONB;

-- Create indexes for the new columns
CREATE INDEX IF NOT EXISTS idx_deals_status_new ON deals(status) WHERE status IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_deals_industry ON deals(industry) WHERE industry IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_deals_document_ids ON deals USING GIN(document_ids) WHERE document_ids IS NOT NULL;

-- Update existing deals with default values
UPDATE deals SET status = 'draft' WHERE status IS NULL;
UPDATE deals SET industry = NULL WHERE industry IS NULL;
UPDATE deals SET document_ids = NULL WHERE document_ids IS NULL;

-- Add comments to document the purpose of these columns
COMMENT ON COLUMN deals.status IS 'Deal lifecycle status: draft, active, closed, cancelled (separate from loan_status)';
COMMENT ON COLUMN deals.industry IS 'Industry sector classification for the deal';
COMMENT ON COLUMN deals.document_ids IS 'JSON array of document UUIDs associated with the deal';