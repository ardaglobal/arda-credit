-- ============================================================================
-- BASIC PERFORMANCE INDEXES FOR CORE TABLES
-- Simple performance optimizations for tables created in migrations 001-008
-- ============================================================================

-- ============================================================================
-- COMPOSITE INDEXES FOR COMMON QUERY PATTERNS
-- ============================================================================

-- Company queries: Find companies by type and status (if kyb_status column exists)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'companies' AND column_name = 'kyb_status') THEN
        CREATE INDEX IF NOT EXISTS idx_companies_type_kyb_status
        ON companies(type, kyb_status);
    END IF;
END
$$;

-- User queries: Find users by type and status (if these columns exist)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'user_type') AND
       EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'kyc_status') THEN
        CREATE INDEX IF NOT EXISTS idx_users_type_kyc_status
        ON users(user_type, kyc_status);
    END IF;
END
$$;

-- Deal queries (if deals table exists from earlier migrations)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'deals') THEN
        -- Deal queries by investor and status
        IF EXISTS (SELECT FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'investor_id') AND
           EXISTS (SELECT FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'status') THEN
            CREATE INDEX IF NOT EXISTS idx_deals_investor_status_originated
            ON deals(investor_id, status, originated_at);
        END IF;

        -- Deal queries by originator and status
        IF EXISTS (SELECT FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'originator_id') THEN
            CREATE INDEX IF NOT EXISTS idx_deals_originator_status_originated
            ON deals(originator_id, status, originated_at);
        END IF;

        -- Active deals with outstanding balance
        IF EXISTS (SELECT FROM information_schema.columns
                   WHERE table_name = 'deals' AND column_name = 'outstanding_balance') THEN
            CREATE INDEX IF NOT EXISTS idx_deals_status_balance
            ON deals(status, outstanding_balance)
            WHERE outstanding_balance > 0;
        END IF;
    END IF;
END
$$;

-- Team member queries (if team_members table exists)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'team_members') THEN
        CREATE INDEX IF NOT EXISTS idx_team_members_company_role_status
        ON team_members(company_id, role, status);
    END IF;
END
$$;

-- Company invitation queries (from migration 008)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'company_invitations') THEN
        CREATE INDEX IF NOT EXISTS idx_company_invitations_status_expires
        ON company_invitations(status, expires_at);

        CREATE INDEX IF NOT EXISTS idx_company_invitations_company_status
        ON company_invitations(company_id, status);
    END IF;
END
$$;

-- ============================================================================
-- PERFORMANCE MONITORING VIEW
-- ============================================================================

-- Create a simple view for index monitoring
CREATE OR REPLACE VIEW basic_index_usage AS
SELECT
    schemaname,
    relname as tablename,
    indexrelname as indexname,
    idx_scan as scans,
    idx_tup_read as tuples_read,
    CASE
        WHEN idx_scan = 0 THEN 'UNUSED'
        WHEN idx_scan < 10 THEN 'LOW USAGE'
        ELSE 'ACTIVE'
    END as usage_status
FROM pg_stat_user_indexes
WHERE schemaname = 'public'
ORDER BY idx_scan DESC;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

SELECT 'Basic performance indexes migration completed successfully!' as status;