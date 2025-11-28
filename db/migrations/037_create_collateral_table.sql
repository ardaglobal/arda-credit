-- Migration: Create collateral table
-- Description: Creates a collateral table to track collateral assets with metadata
--              and includes double-pledge detection support

-- ============================================================================
-- STEP 1: Create collateral table
-- ============================================================================

CREATE TABLE collateral (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(500) NOT NULL, -- Only required field
    collateral_type VARCHAR(100), -- Type of collateral (e.g., real_estate, equipment, inventory, etc.)
    description TEXT,
    address TEXT, -- Physical or blockchain address
    custom_id VARCHAR(255), -- User's internal ID for this collateral
    double_pledged UUID, -- UUID of another collateral that might be the same (populated by async check)
    
    -- Audit fields
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL, -- User who created this collateral entry
    
    -- Metadata for flexible matching
    metadata JSONB DEFAULT '{}'::jsonb -- Additional metadata for matching algorithms
);

-- ============================================================================
-- STEP 2: Create indexes for efficient querying
-- ============================================================================

-- Index for name searches (for duplicate detection)
CREATE INDEX idx_collateral_name ON collateral(name);

-- Index for type filtering
CREATE INDEX idx_collateral_type ON collateral(collateral_type);

-- Index for custom_id lookups
CREATE INDEX idx_collateral_custom_id ON collateral(custom_id) WHERE custom_id IS NOT NULL;

-- Index for address lookups
CREATE INDEX idx_collateral_address ON collateral(address) WHERE address IS NOT NULL;

-- Index for double_pledged relationships
CREATE INDEX idx_collateral_double_pledged ON collateral(double_pledged) WHERE double_pledged IS NOT NULL;

-- Full text search index on name and description
CREATE INDEX idx_collateral_fulltext ON collateral USING GIN(to_tsvector('english', name || ' ' || COALESCE(description, '')));

-- Index for metadata JSONB queries
CREATE INDEX idx_collateral_metadata ON collateral USING GIN(metadata);

-- ============================================================================
-- STEP 3: Create collateral_checks table for tracking duplicate detection
-- ============================================================================

CREATE TABLE collateral_checks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    checked_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    potential_matches UUID[], -- Array of potential duplicate collateral IDs
    match_score DECIMAL(5, 2), -- Confidence score 0-100
    check_status VARCHAR(20) DEFAULT 'pending' CHECK (check_status IN ('pending', 'in_progress', 'completed', 'failed')),
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Index for collateral_checks queries
CREATE INDEX idx_collateral_checks_collateral_id ON collateral_checks(collateral_id);
CREATE INDEX idx_collateral_checks_status ON collateral_checks(check_status);

-- ============================================================================
-- STEP 4: Add update trigger for collateral
-- ============================================================================

CREATE OR REPLACE FUNCTION update_collateral_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_collateral_updated_at
    BEFORE UPDATE ON collateral
    FOR EACH ROW
    EXECUTE FUNCTION update_collateral_updated_at();

-- ============================================================================
-- STEP 5: Add comments
-- ============================================================================

COMMENT ON TABLE collateral IS 'Collateral assets with metadata for tracking and double-pledge detection';
COMMENT ON COLUMN collateral.id IS 'Auto-generated UUID for the collateral';
COMMENT ON COLUMN collateral.name IS 'Name of the collateral (required field)';
COMMENT ON COLUMN collateral.collateral_type IS 'Type of collateral (e.g., real_estate, equipment, inventory)';
COMMENT ON COLUMN collateral.description IS 'Description of the collateral';
COMMENT ON COLUMN collateral.address IS 'Physical or blockchain address of the collateral';
COMMENT ON COLUMN collateral.custom_id IS 'User internal ID for this collateral';
COMMENT ON COLUMN collateral.double_pledged IS 'UUID of similar collateral detected by async check';
COMMENT ON COLUMN collateral.metadata IS 'Additional metadata for flexible matching';
COMMENT ON COLUMN collateral.created_by IS 'User who created this collateral entry';

COMMENT ON TABLE collateral_checks IS 'Tracks async duplicate detection checks for collateral';
COMMENT ON COLUMN collateral_checks.potential_matches IS 'Array of potential duplicate collateral IDs';
COMMENT ON COLUMN collateral_checks.match_score IS 'Confidence score for duplicate detection (0-100)';
