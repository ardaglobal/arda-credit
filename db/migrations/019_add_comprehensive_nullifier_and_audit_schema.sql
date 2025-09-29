-- Add comprehensive nullifier and audit schema

-- Add remaining missing columns to nullifiers table
ALTER TABLE nullifiers ADD COLUMN next_index BIGINT;
ALTER TABLE nullifiers ADD COLUMN loan_operation_type VARCHAR(50);

-- Update tree_state with missing columns
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS low_value BYTEA;

-- Create deal_audit_comprehensive view
CREATE OR REPLACE VIEW deal_audit_comprehensive AS
SELECT
    d.id,
    d.originator_id,
    d.investor_id,
    d.principal_amount,
    d.outstanding_balance,
    d.status,
    d.created_at,
    d.updated_at
FROM deals d;

-- Create audit trail functions
CREATE OR REPLACE FUNCTION get_loan_audit_trail(loan_id UUID, user_id UUID)
RETURNS TABLE (
    event_id VARCHAR(255),
    event_type TEXT,
    event_timestamp TIMESTAMP WITH TIME ZONE,
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ae.event_id,
        ae.event_type,
        ae.timestamp,
        ae.metadata
    FROM audit_events ae
    WHERE ae.metadata->>'loan_id' = loan_id::text
       OR ae.metadata->>'user_id' = user_id::text
    ORDER BY ae.timestamp ASC;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_user_loan_audit_trail(p_user_id UUID, p_loan_id UUID, p_limit INTEGER)
RETURNS TABLE (
    event_id VARCHAR(255),
    event_type TEXT,
    event_timestamp TIMESTAMP WITH TIME ZONE,
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ae.event_id,
        ae.event_type,
        ae.timestamp,
        ae.metadata
    FROM audit_events ae
    WHERE ae.metadata->>'user_id' = p_user_id::text
      AND ae.metadata->>'loan_id' = p_loan_id::text
    ORDER BY ae.timestamp DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_batch_audit_trail(batch_id INTEGER)
RETURNS TABLE (
    event_id VARCHAR(255),
    event_type TEXT,
    event_timestamp TIMESTAMP WITH TIME ZONE,
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ae.event_id,
        ae.event_type,
        ae.timestamp,
        ae.metadata
    FROM audit_events ae
    WHERE ae.metadata->>'batch_id' = batch_id::text
    ORDER BY ae.timestamp ASC;
END;
$$ LANGUAGE plpgsql;

-- Update nullifiers with default values for new columns
UPDATE nullifiers SET next_index = leaf_index + 1 WHERE next_index IS NULL;
UPDATE nullifiers SET loan_operation_type = 'unknown' WHERE loan_operation_type IS NULL;

-- Create additional indexes for performance
CREATE INDEX IF NOT EXISTS idx_nullifiers_next_index ON nullifiers(next_index);
CREATE INDEX IF NOT EXISTS idx_nullifiers_loan_operation_type ON nullifiers(loan_operation_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_metadata_loan_id ON audit_events((metadata->>'loan_id'));
CREATE INDEX IF NOT EXISTS idx_audit_events_metadata_user_id ON audit_events((metadata->>'user_id'));
CREATE INDEX IF NOT EXISTS idx_audit_events_metadata_batch_id ON audit_events((metadata->>'batch_id'));

-- Add comments for new columns and functions
COMMENT ON COLUMN nullifiers.next_index IS 'Index of the next nullifier in the sequence';
COMMENT ON COLUMN nullifiers.loan_operation_type IS 'Type of loan operation associated with this nullifier';
COMMENT ON FUNCTION get_loan_audit_trail(UUID, UUID) IS 'Get audit trail for a specific loan and user';
COMMENT ON FUNCTION get_user_loan_audit_trail(UUID, UUID, INTEGER) IS 'Get user-specific loan audit trail with limit';
COMMENT ON FUNCTION get_batch_audit_trail(INTEGER) IS 'Get audit trail for a specific batch';