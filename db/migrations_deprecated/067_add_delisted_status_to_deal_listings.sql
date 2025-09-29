-- Add 'delisted' status and delisted_at timestamp to deal_listings table
-- This allows distinguishing between manually removed listings ('delisted')
-- and automatically expired listings ('expired')

-- Add delisted_at column
ALTER TABLE deal_listings ADD COLUMN delisted_at TIMESTAMP;

-- Update the status CHECK constraint to include 'delisted'
ALTER TABLE deal_listings DROP CONSTRAINT IF EXISTS deal_listings_status_check;
ALTER TABLE deal_listings ADD CONSTRAINT deal_listings_status_check
    CHECK (status IN ('active', 'sold', 'cancelled', 'expired', 'delisted'));

-- Update the comment to reflect the new status
COMMENT ON COLUMN deal_listings.status IS 'Listing status: active, sold, cancelled, expired, delisted';

-- Add index for delisted listings
CREATE INDEX idx_deal_listings_delisted_at ON deal_listings(delisted_at) WHERE delisted_at IS NOT NULL;

-- Update trigger to set delisted_at when status changes to 'delisted'
CREATE OR REPLACE FUNCTION update_deal_listing_delisted_at()
RETURNS TRIGGER AS $$
BEGIN
    -- Set delisted_at when status changes to 'delisted'
    IF NEW.status = 'delisted' AND (OLD.status IS NULL OR OLD.status != 'delisted') THEN
        NEW.delisted_at = NOW();
    -- Clear delisted_at if status changes away from 'delisted'
    ELSIF NEW.status != 'delisted' AND OLD.status = 'delisted' THEN
        NEW.delisted_at = NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for delisted_at updates
CREATE TRIGGER update_deal_listing_delisted_at_trigger
    BEFORE UPDATE ON deal_listings
    FOR EACH ROW
    EXECUTE FUNCTION update_deal_listing_delisted_at();
