-- Migration: Add status, industry, and document_ids fields to deals table (corrected)
-- Created: 2025-01-25
-- Description: Adds deal status (draft, closed, etc.), industry field, and document associations

-- Add new columns to deals table (with IF NOT EXISTS to avoid conflicts)
DO $$
BEGIN
    -- Add status column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'status') THEN
        ALTER TABLE deals ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'closed', 'cancelled'));
    END IF;

    -- Add industry column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'industry') THEN
        ALTER TABLE deals ADD COLUMN industry VARCHAR(100);
    END IF;

    -- Add document_ids column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'document_ids') THEN
        ALTER TABLE deals ADD COLUMN document_ids JSONB DEFAULT '[]'::jsonb;
    END IF;
END $$;

-- Add comments for new columns
COMMENT ON COLUMN deals.status IS 'Deal status: draft (initial), active (in progress), closed (completed), cancelled (terminated)';
COMMENT ON COLUMN deals.industry IS 'Industry sector for the deal (e.g., technology, healthcare, finance)';
COMMENT ON COLUMN deals.document_ids IS 'JSON array of document UUIDs associated with this deal';

-- Create indexes (with IF NOT EXISTS to avoid conflicts)
DO $$
BEGIN
    -- Create index on status for filtering
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'deals' AND indexname = 'idx_deals_new_status') THEN
        CREATE INDEX idx_deals_new_status ON deals(status);
    END IF;

    -- Create index on industry for filtering
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'deals' AND indexname = 'idx_deals_industry') THEN
        CREATE INDEX idx_deals_industry ON deals(industry) WHERE industry IS NOT NULL;
    END IF;

    -- Create GIN index on document_ids for efficient JSON array queries
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'deals' AND indexname = 'idx_deals_document_ids') THEN
        CREATE INDEX idx_deals_document_ids ON deals USING gin(document_ids);
    END IF;
END $$;

-- Update existing deals to have 'active' status instead of 'draft' if they have payments
UPDATE deals
SET status = 'active'
WHERE id IN (
    SELECT DISTINCT d.id
    FROM deals d
    JOIN deal_payments dp ON d.id = dp.deal_id
) AND status = 'draft';

-- Update deals that are paid_off or closed to have 'closed' status
UPDATE deals
SET status = 'closed'
WHERE loan_status IN ('paid_off', 'closed') AND status = 'draft';