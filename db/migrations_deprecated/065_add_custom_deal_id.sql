-- Add custom_deal_id column to deals table
-- Allows users to map deals to their internal system IDs

ALTER TABLE deals
ADD COLUMN custom_deal_id VARCHAR(255);

-- Add unique constraint to prevent duplicate custom IDs within the same user's deals
-- Note: This allows NULL values and uniqueness only among non-NULL values
CREATE UNIQUE INDEX idx_deals_custom_deal_id ON deals(custom_deal_id) WHERE custom_deal_id IS NOT NULL;

-- Add index for efficient lookups by custom_deal_id
CREATE INDEX idx_deals_custom_deal_id_lookup ON deals(custom_deal_id) WHERE custom_deal_id IS NOT NULL;

-- Add comment explaining the column
COMMENT ON COLUMN deals.custom_deal_id IS 'Optional user-defined identifier for mapping deals to external systems (e.g., internal deal numbers)';
