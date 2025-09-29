-- ============================================================================
-- ADDITIONAL PERFORMANCE INDEXES
-- Safe additional indexes that don't depend on complex table structures
-- ============================================================================

-- ============================================================================
-- UNIQUE CONSTRAINTS FOR DATA INTEGRITY
-- ============================================================================

-- Ensure wallet addresses are unique (if column exists in users table)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'wallet_address') THEN
        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_wallet_address_unique
        ON users(wallet_address)
        WHERE wallet_address IS NOT NULL;
    END IF;
END
$$;

-- ============================================================================
-- PARTIAL INDEXES FOR FILTERED QUERIES
-- ============================================================================

-- Active users only (exclude soft-deleted if deleted_at column exists)
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'deleted_at') THEN
        CREATE INDEX IF NOT EXISTS idx_users_active
        ON users(id, user_type, kyc_status)
        WHERE deleted_at IS NULL;
    END IF;
END
$$;

-- ============================================================================
-- COVERING INDEXES FOR FREQUENT ACCESS (PostgreSQL 11+)
-- ============================================================================

-- Users covering index for common profile queries
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'user_type') THEN
        CREATE INDEX IF NOT EXISTS idx_users_type_covering
        ON users(user_type)
        INCLUDE (email, created_at);
    END IF;
END
$$;

-- Companies covering index for type-based queries
DO $$
BEGIN
    IF EXISTS (SELECT FROM information_schema.columns
               WHERE table_name = 'companies' AND column_name = 'type') THEN
        CREATE INDEX IF NOT EXISTS idx_companies_type_covering
        ON companies(type)
        INCLUDE (name, created_at);
    END IF;
END
$$;

-- ============================================================================
-- TABLE STATISTICS UPDATE
-- ============================================================================

-- Update statistics for optimal query planning
DO $$
DECLARE
    table_name TEXT;
BEGIN
    FOR table_name IN
        SELECT t.table_name
        FROM information_schema.tables t
        WHERE t.table_schema = 'public'
          AND t.table_type = 'BASE TABLE'
          AND t.table_name NOT LIKE '_sqlx_%'
    LOOP
        EXECUTE 'ANALYZE ' || quote_ident(table_name);
    END LOOP;
END
$$;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

SELECT 'Additional performance indexes migration completed successfully!' as status;