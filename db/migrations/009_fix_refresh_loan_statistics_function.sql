-- ============================================================================
-- FIX REFRESH_LOAN_STATISTICS FUNCTION
-- ============================================================================
-- This migration updates the refresh_loan_statistics() function to use
-- the users table instead of identities table.
-- ============================================================================

CREATE OR REPLACE FUNCTION refresh_loan_statistics()
RETURNS void
LANGUAGE plpgsql
AS $function$
BEGIN
    -- Force update of loan statistics (handled by triggers automatically)
    -- Triggers will handle updating statistics on table changes

    -- Additional optimizations
    ANALYZE nullifiers;
    ANALYZE audit_events;
    ANALYZE deals;
    ANALYZE users;  -- Changed from identities
    ANALYZE proof_batches;
END;
$function$;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;
