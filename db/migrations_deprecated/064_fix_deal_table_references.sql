-- Fix deal table references in functions and views created by migration 016
-- This migration runs after 061_convert_loans_to_deals.sql

-- Note: Foreign key constraint to deals table is not enforced in tests
-- to allow for test data that may not have corresponding deal records

-- Recreate loan_operation_history view with correct table reference
DROP VIEW IF EXISTS loan_operation_history CASCADE;
CREATE VIEW loan_operation_history AS
SELECT
    n.id as nullifier_id,
    n.value as nullifier_value,
    n.loan_operation_type,
    n.loan_id,
    n.user_id,
    n.operation_amount,
    n.operation_metadata,
    n.batch_id,
    n.tree_index,
    n.created_at,
    n.is_active,
    -- Loan context
    l.principal_amount,
    l.outstanding_balance,
    l.loan_status,
    l.originated_at as loan_originated_at,
    l.maturity_date,
    -- User context
    i.email as user_email,
    i.user_type,
    i.kyc_status,
    -- Batch context
    pb.proof_status as batch_proof_status,
    pb.created_at as batch_created_at
FROM nullifiers n
LEFT JOIN deals l ON n.loan_id = l.id
LEFT JOIN identities i ON n.user_id = i.id
LEFT JOIN proof_batches pb ON n.batch_id = pb.id
WHERE n.loan_operation_type IS NOT NULL
ORDER BY n.created_at DESC;

-- Recreate loan_lifecycle_summary view with correct table reference
DROP VIEW IF EXISTS loan_lifecycle_summary CASCADE;
CREATE OR REPLACE VIEW loan_lifecycle_summary AS
SELECT
    n.loan_id,
    COUNT(*) as total_operations,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_origination') as origination_count,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_payment') as payment_count,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_closeout') as closeout_count,
    SUM(n.operation_amount) FILTER (WHERE n.loan_operation_type = 'deal_payment') as total_payment_amount,
    MIN(n.created_at) as first_operation_date,
    MAX(n.created_at) as last_operation_date,
    COUNT(DISTINCT n.batch_id) FILTER (WHERE n.batch_id IS NOT NULL) as operations_in_batches,
    EXTRACT(EPOCH FROM (MAX(n.created_at) - MIN(n.created_at))) /
        GREATEST(COUNT(*) - 1, 1) as avg_time_between_operations,
    -- Current loan status
    l.loan_status,
    l.principal_amount,
    l.outstanding_balance,
    l.interest_rate_bps
FROM nullifiers n
JOIN deals l ON n.loan_id = l.id
WHERE n.loan_operation_type IS NOT NULL
    AND n.is_active = true
GROUP BY n.loan_id, l.loan_status, l.principal_amount, l.outstanding_balance, l.interest_rate_bps;

-- Recreate loan_audit_comprehensive view with correct table reference
DROP VIEW IF EXISTS loan_audit_comprehensive CASCADE;
CREATE OR REPLACE VIEW loan_audit_comprehensive AS
SELECT
    ae.event_id,
    ae.loan_id,
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
    -- Loan context information
    l.principal_amount,
    l.outstanding_balance,
    l.loan_status,
    l.originated_at,
    -- User context information
    i.email as user_email,
    i.user_type,
    i.kyc_status
FROM audit_events ae
LEFT JOIN deals l ON ae.loan_id = l.id
LEFT JOIN identities i ON ae.user_id = i.id
WHERE ae.loan_operation_type IS NOT NULL
ORDER BY ae."timestamp" ASC;

-- Update the refresh_loan_statistics function to reference deals table
CREATE OR REPLACE FUNCTION refresh_loan_statistics()
RETURNS VOID AS $$
BEGIN
    -- Force update of loan statistics (handled by triggers automatically)
    -- Triggers will handle updating statistics on table changes

    -- Additional optimizations
    ANALYZE nullifiers;
    ANALYZE audit_events;
    ANALYZE deals;
    ANALYZE identities;
    ANALYZE proof_batches;
END;
$$ LANGUAGE plpgsql;

-- Update the update_tree_loan_stats function to handle deal operation types
CREATE OR REPLACE FUNCTION update_tree_loan_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update loan statistics in tree_state
    UPDATE tree_state
    SET
        total_loan_originations = (
            SELECT COUNT(*) FROM nullifiers
            WHERE loan_operation_type = 'deal_origination' AND is_active = true
        ),
        total_loan_payments = (
            SELECT COUNT(*) FROM nullifiers
            WHERE loan_operation_type = 'deal_payment' AND is_active = true
        ),
        total_loan_closeouts = (
            SELECT COUNT(*) FROM nullifiers
            WHERE loan_operation_type = 'deal_closeout' AND is_active = true
        ),
        total_loan_amount = (
            SELECT COALESCE(SUM(operation_amount), 0) FROM nullifiers
            WHERE loan_operation_type IS NOT NULL AND is_active = true
        ),
        active_loans_count = (
            SELECT COUNT(DISTINCT loan_id) FROM nullifiers
            WHERE loan_operation_type IN ('deal_origination', 'deal_payment')
            AND is_active = true
            AND loan_id NOT IN (
                SELECT DISTINCT loan_id FROM nullifiers
                WHERE loan_operation_type = 'deal_closeout'
                AND is_active = true AND loan_id IS NOT NULL
            )
        ),
        loan_stats_updated_at = NOW()
    WHERE tree_id = 'default';

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Update the imt_loan_stats view to use deal operation types
DROP VIEW IF EXISTS imt_loan_stats CASCADE;
CREATE OR REPLACE VIEW imt_loan_stats AS
SELECT
    COUNT(*) as total_loan_nullifiers,
    COUNT(*) FILTER (WHERE loan_operation_type = 'deal_origination') as total_loan_originations,
    COUNT(*) FILTER (WHERE loan_operation_type = 'deal_payment') as total_loan_payments,
    COUNT(*) FILTER (WHERE loan_operation_type = 'deal_closeout') as total_loan_closeouts,
    SUM(operation_amount) as total_loan_amount,
    COUNT(DISTINCT loan_id) as unique_loans,
    COUNT(DISTINCT user_id) as unique_users,
    COUNT(DISTINCT batch_id) FILTER (WHERE batch_id IS NOT NULL) as batches_with_loans,
    ROUND(COUNT(*)::numeric / GREATEST(COUNT(DISTINCT batch_id) FILTER (WHERE batch_id IS NOT NULL), 1), 2) as avg_operations_per_batch,
    MIN(created_at) as first_loan_operation,
    MAX(created_at) as last_loan_operation,
    COUNT(DISTINCT user_id) FILTER (WHERE loan_operation_type IN ('deal_origination', 'deal_payment')
        AND loan_id NOT IN (
            SELECT DISTINCT loan_id FROM nullifiers
            WHERE loan_operation_type = 'deal_closeout'
            AND is_active = true AND loan_id IS NOT NULL
        )) as total_loan_users,
    EXTRACT(EPOCH FROM (MAX(created_at) - MIN(created_at))) /
        GREATEST(COUNT(*) - 1, 1) as avg_operation_interval_seconds,
    NOW() as loan_stats_updated_at
FROM nullifiers
WHERE loan_operation_type IS NOT NULL
    AND is_active = true;

-- Update user_loan_activity view to use deal operation types
DROP VIEW IF EXISTS user_loan_activity CASCADE;
CREATE OR REPLACE VIEW user_loan_activity AS
SELECT
    n.user_id,
    COUNT(*) as total_operations,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_origination') as total_originations,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_payment') as total_payments,
    COUNT(*) FILTER (WHERE n.loan_operation_type = 'deal_closeout') as total_closeouts,
    SUM(n.operation_amount) as total_amount_involved,
    COUNT(DISTINCT n.loan_id) as unique_loans_involved,
    MIN(n.created_at) as first_operation_date,
    MAX(n.created_at) as last_operation_date,
    COUNT(DISTINCT n.batch_id) FILTER (WHERE n.batch_id IS NOT NULL) as batches_participated,
    COUNT(DISTINCT n.loan_id) FILTER (WHERE n.loan_operation_type IN ('deal_origination', 'deal_payment')
        AND n.loan_id NOT IN (
            SELECT DISTINCT loan_id FROM nullifiers
            WHERE loan_operation_type = 'deal_closeout'
            AND is_active = true AND loan_id IS NOT NULL
        )) as active_loans_count,
    -- User context
    i.email,
    i.user_type,
    i.kyc_status
FROM nullifiers n
JOIN identities i ON n.user_id = i.id
WHERE n.loan_operation_type IS NOT NULL
    AND n.is_active = true
GROUP BY n.user_id, i.email, i.user_type, i.kyc_status;
