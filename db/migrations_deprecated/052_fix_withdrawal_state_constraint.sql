-- Fix withdrawal state constraint to include 'approved' state
-- This addresses the constraint violation when approving withdrawals

-- Drop the existing constraint
ALTER TABLE withdrawal_requests DROP CONSTRAINT valid_withdrawal_state;

-- Add the updated constraint that includes 'approved'
ALTER TABLE withdrawal_requests ADD CONSTRAINT valid_withdrawal_state
CHECK (state IN ('pending', 'approved', 'validated', 'submitted', 'confirmed', 'failed', 'cancelled'));