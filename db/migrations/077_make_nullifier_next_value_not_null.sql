-- Make nullifier next_value NOT NULL with default 0
-- Migration 077: Fix nullifier next_value to be NOT NULL

-- First, update any existing NULL values to 0
UPDATE nullifiers 
SET next_value = 0 
WHERE next_value IS NULL;

-- Then add the NOT NULL constraint
ALTER TABLE nullifiers 
ALTER COLUMN next_value SET NOT NULL;

-- Set default for future insertions
ALTER TABLE nullifiers 
ALTER COLUMN next_value SET DEFAULT 0;

