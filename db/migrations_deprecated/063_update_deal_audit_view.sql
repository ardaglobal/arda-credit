-- ============================================================================
-- UPDATE DEAL AUDIT VIEW
-- Update loan_audit_comprehensive view to reference deal tables after conversion
-- ============================================================================

-- Drop the old view
DROP VIEW IF EXISTS loan_audit_comprehensive CASCADE;

-- Recreate the view with deal table references
CREATE OR REPLACE VIEW deal_audit_comprehensive AS
SELECT
    ae.event_id,
    ae.loan_id as deal_id,  -- Keep the field name for backward compatibility
    ae.user_id,
    ae.loan_operation_type,
    ae.operation_amount,
    ae.nullifier_value,
    ae."timestamp",
    ae.batch_id,
    ae.transaction_hash,
    ae.root_before,
    ae.root_after,
    ae.metadata,
    -- Deal context information (converted from loans)
    d.principal_amount,
    d.outstanding_balance,
    d.loan_status,
    d.originated_at,
    -- User context information
    i.email as user_email,
    i.user_type,
    i.kyc_status
FROM audit_events ae
LEFT JOIN deals d ON ae.loan_id = d.id
LEFT JOIN identities i ON ae.user_id = i.id
WHERE ae.loan_operation_type IS NOT NULL
ORDER BY ae."timestamp" ASC;

-- Add comment
COMMENT ON VIEW deal_audit_comprehensive IS 'Comprehensive audit trail view with deal and user context (updated for deal conversion)';

-- ============================================================================
-- UPDATE FUNCTIONS
-- ============================================================================

-- Update function name to match new view
CREATE OR REPLACE FUNCTION get_deal_audit_comprehensive(
    target_deal_id UUID DEFAULT NULL,
    operation_limit INTEGER DEFAULT 100
)
RETURNS TABLE(
    event_id BIGINT,
    deal_id UUID,
    user_id UUID,
    loan_operation_type VARCHAR(30),
    operation_amount DECIMAL(18, 6),
    nullifier_value BIGINT,
    "timestamp" TIMESTAMP WITH TIME ZONE,
    batch_id INTEGER,
    transaction_hash VARCHAR(66),
    root_before VARCHAR(66),
    root_after VARCHAR(66),
    metadata JSONB,
    principal_amount DECIMAL(18, 6),
    outstanding_balance DECIMAL(18, 6),
    loan_status VARCHAR(20),
    originated_at TIMESTAMP WITH TIME ZONE,
    user_email VARCHAR(255),
    user_type VARCHAR(20),
    kyc_status VARCHAR(20)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        dac.event_id,
        dac.deal_id,
        dac.user_id,
        dac.loan_operation_type,
        dac.operation_amount,
        dac.nullifier_value,
        dac."timestamp",
        dac.batch_id,
        dac.transaction_hash,
        dac.root_before,
        dac.root_after,
        dac.metadata,
        dac.principal_amount,
        dac.outstanding_balance,
        dac.loan_status,
        dac.originated_at,
        dac.user_email,
        dac.user_type,
        dac.kyc_status
    FROM deal_audit_comprehensive dac
    WHERE (target_deal_id IS NULL OR dac.deal_id = target_deal_id)
    ORDER BY dac."timestamp" DESC
    LIMIT operation_limit;
END;
$$ LANGUAGE plpgsql;

-- Keep the old function for backward compatibility but make it delegate to the new one
CREATE OR REPLACE FUNCTION get_loan_audit_comprehensive(
    target_loan_id UUID DEFAULT NULL,
    operation_limit INTEGER DEFAULT 100
)
RETURNS TABLE(
    event_id BIGINT,
    deal_id UUID,
    user_id UUID,
    loan_operation_type VARCHAR(30),
    operation_amount DECIMAL(18, 6),
    nullifier_value BIGINT,
    "timestamp" TIMESTAMP WITH TIME ZONE,
    batch_id INTEGER,
    transaction_hash VARCHAR(66),
    root_before VARCHAR(66),
    root_after VARCHAR(66),
    metadata JSONB,
    principal_amount DECIMAL(18, 6),
    outstanding_balance DECIMAL(18, 6),
    loan_status VARCHAR(20),
    originated_at TIMESTAMP WITH TIME ZONE,
    user_email VARCHAR(255),
    user_type VARCHAR(20),
    kyc_status VARCHAR(20)
) AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM get_deal_audit_comprehensive(target_loan_id, operation_limit);
END;
$$ LANGUAGE plpgsql;

-- Add comments
COMMENT ON FUNCTION get_deal_audit_comprehensive(UUID, INTEGER) IS 'Get comprehensive audit trail for deals (updated for deal conversion)';
COMMENT ON FUNCTION get_loan_audit_comprehensive(UUID, INTEGER) IS 'Backward compatibility function - delegates to get_deal_audit_comprehensive';
