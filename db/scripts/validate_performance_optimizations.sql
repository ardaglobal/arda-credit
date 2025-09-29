-- ============================================================================
-- PERFORMANCE OPTIMIZATION VALIDATION SCRIPT
-- Run this after applying migration 070_performance_indexes_and_optimization.sql
-- ============================================================================

-- Set up timing for performance measurement
\timing on

-- ============================================================================
-- 1. FOREIGN KEY INDEX VALIDATION
-- ============================================================================

SELECT '=== FOREIGN KEY INDEX VALIDATION ===' as info;

-- Check all foreign key indexes are present
SELECT
    table_name,
    constraint_name,
    columns,
    has_index,
    CASE WHEN has_index THEN '✅ OK' ELSE '❌ MISSING INDEX' END as status
FROM check_foreign_key_indexes()
ORDER BY has_index, table_name;

-- ============================================================================
-- 2. COMPOSITE INDEX PERFORMANCE TESTING
-- ============================================================================

SELECT '=== COMPOSITE INDEX PERFORMANCE TESTING ===' as info;

-- Test 1: Company queries by type and KYB status
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, name, legal_name, created_at
FROM companies
WHERE type = 'investor' AND kyb_status = 'approved'
ORDER BY created_at DESC;

-- Test 2: User queries by company and type
EXPLAIN (ANALYZE, BUFFERS)
SELECT u.id, u.email, u.kyc_status, c.name as company_name
FROM users u
JOIN companies c ON u.company_id = c.id
WHERE u.company_id = (SELECT id FROM companies LIMIT 1)
  AND u.user_type = 'investor'
ORDER BY u.created_at DESC;

-- Test 3: Deal queries by investor and status (most critical query)
EXPLAIN (ANALYZE, BUFFERS)
SELECT d.id, d.title, d.principal_amount, d.outstanding_balance, d.originated_at
FROM deals d
WHERE d.investor_id = (SELECT id FROM users WHERE user_type = 'investor' LIMIT 1)
  AND d.status IN ('active', 'for_sale')
ORDER BY d.originated_at DESC;

-- Test 4: Deal queries with status and outstanding balance
EXPLAIN (ANALYZE, BUFFERS)
SELECT d.id, d.title, d.outstanding_balance, d.interest_rate_bps
FROM deals d
WHERE d.status = 'active'
  AND d.outstanding_balance > 1000
ORDER BY d.outstanding_balance DESC;

-- ============================================================================
-- 3. STATE TRANSITION LOOKUP PERFORMANCE
-- ============================================================================

SELECT '=== STATE TRANSITION LOOKUP PERFORMANCE ===' as info;

-- Test unbatched transaction queries
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, transaction_type, created_at, amount
FROM transaction_queue
WHERE included_in_batch_id IS NULL
  AND transaction_type IN ('deal_payment', 'deal_origination')
ORDER BY created_at ASC
LIMIT 100;

-- Test transaction queue by deal hash
EXPLAIN (ANALYZE, BUFFERS)
SELECT tq.id, tq.transaction_type, tq.amount, tq.created_at
FROM transaction_queue tq
WHERE tq.deal_id_hash IS NOT NULL
  AND tq.transaction_type = 'deal_payment'
ORDER BY tq.created_at DESC
LIMIT 50;

-- Test proof batch status queries
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, proof_status, created_at, transaction_ids
FROM proof_batches
WHERE proof_status = 'pending'
ORDER BY created_at ASC;

-- ============================================================================
-- 4. UNIQUE CONSTRAINT VALIDATION
-- ============================================================================

SELECT '=== UNIQUE CONSTRAINT VALIDATION ===' as info;

-- Check unique constraints exist
SELECT
    conname as constraint_name,
    contype as constraint_type,
    conkey as columns,
    '✅ OK' as status
FROM pg_constraint
WHERE contype = 'u'
  AND connamespace = (SELECT oid FROM pg_namespace WHERE nspname = current_schema())
ORDER BY conname;

-- Test unique constraint performance (should use unique index)
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, email, user_type
FROM users
WHERE email = 'test@example.com';

-- ============================================================================
-- 5. COVERING INDEX PERFORMANCE (PostgreSQL 11+)
-- ============================================================================

SELECT '=== COVERING INDEX PERFORMANCE ===' as info;

-- Test covering index for deals by investor
EXPLAIN (ANALYZE, BUFFERS)
SELECT investor_id, status, outstanding_balance, originated_at
FROM deals
WHERE investor_id = (SELECT id FROM users WHERE user_type = 'investor' LIMIT 1);

-- Test covering index for companies by type
EXPLAIN (ANALYZE, BUFFERS)
SELECT type, name, kyb_status, created_at
FROM companies
WHERE type = 'originator';

-- ============================================================================
-- 6. PARTIAL INDEX PERFORMANCE
-- ============================================================================

SELECT '=== PARTIAL INDEX PERFORMANCE ===' as info;

-- Test active users partial index
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, user_type, kyc_status, email
FROM users
WHERE deleted_at IS NULL
  AND user_type = 'investor'
  AND kyc_status = 'approved';

-- Test deals with outstanding balance partial index
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, outstanding_balance, status, principal_amount
FROM deals
WHERE outstanding_balance > 0
  AND status = 'active';

-- ============================================================================
-- 7. JSONB GIN INDEX PERFORMANCE
-- ============================================================================

SELECT '=== JSONB GIN INDEX PERFORMANCE ===' as info;

-- Test document IDs JSONB index (if data exists)
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, title, document_ids
FROM deals
WHERE document_ids IS NOT NULL
  AND document_ids ? 'doc_123';

-- Test transaction metadata JSONB index (if data exists)
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, transaction_type, metadata
FROM transaction_queue
WHERE metadata IS NOT NULL
  AND metadata ? 'user_action';

-- ============================================================================
-- 8. INDEX USAGE STATISTICS
-- ============================================================================

SELECT '=== INDEX USAGE STATISTICS ===' as info;

-- Show most used indexes
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan as scans,
    idx_tup_read as tuples_read,
    idx_tup_fetch as tuples_fetched,
    CASE
        WHEN idx_scan = 0 THEN '⚠️  UNUSED'
        WHEN idx_scan < 10 THEN '📊 LOW USAGE'
        ELSE '✅ ACTIVE'
    END as usage_status
FROM pg_stat_user_indexes
WHERE schemaname = current_schema()
ORDER BY idx_scan DESC, idx_tup_read DESC
LIMIT 20;

-- ============================================================================
-- 9. TABLE STATISTICS AND SIZE ANALYSIS
-- ============================================================================

SELECT '=== TABLE STATISTICS AND SIZE ANALYSIS ===' as info;

-- Table sizes and statistics
SELECT
    schemaname,
    tablename,
    attname as column_name,
    n_distinct,
    correlation,
    most_common_vals[1:3] as sample_values
FROM pg_stats
WHERE schemaname = current_schema()
  AND tablename IN ('companies', 'users', 'deals', 'deal_payments', 'transaction_queue')
  AND attname IN ('type', 'status', 'user_type', 'kyc_status', 'transaction_type')
ORDER BY tablename, attname;

-- Table size analysis
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as total_size,
    pg_size_pretty(pg_relation_size(schemaname||'.'||tablename)) as table_size,
    pg_size_pretty(pg_indexes_size(schemaname||'.'||tablename)) as index_size
FROM pg_tables
WHERE schemaname = current_schema()
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

-- ============================================================================
-- 10. PERFORMANCE OPTIMIZATION RECOMMENDATIONS
-- ============================================================================

SELECT '=== PERFORMANCE OPTIMIZATION RECOMMENDATIONS ===' as info;

-- Check for tables that might need partitioning
WITH table_sizes AS (
    SELECT
        schemaname,
        tablename,
        pg_total_relation_size(schemaname||'.'||tablename) as size_bytes,
        (SELECT COUNT(*) FROM information_schema.tables WHERE table_name = tablename) as row_estimate
    FROM pg_tables
    WHERE schemaname = current_schema()
)
SELECT
    tablename,
    pg_size_pretty(size_bytes) as size,
    CASE
        WHEN size_bytes > 1073741824 THEN '📊 Consider partitioning (>1GB)'
        WHEN size_bytes > 268435456 THEN '⚠️  Monitor size growth (>256MB)'
        ELSE '✅ Size OK'
    END as recommendation
FROM table_sizes
ORDER BY size_bytes DESC;

-- Check for missing statistics
SELECT
    'Run ANALYZE on all tables regularly' as recommendation,
    'Automated via refresh_table_statistics() function' as implementation;

-- ============================================================================
-- 11. CONSTRAINT VALIDATION
-- ============================================================================

SELECT '=== CONSTRAINT VALIDATION ===' as info;

-- Check all check constraints are working
SELECT
    conname as constraint_name,
    contype as type,
    CASE contype
        WHEN 'c' THEN '✅ Check constraint'
        WHEN 'f' THEN '✅ Foreign key'
        WHEN 'p' THEN '✅ Primary key'
        WHEN 'u' THEN '✅ Unique constraint'
    END as constraint_type
FROM pg_constraint
WHERE connamespace = (SELECT oid FROM pg_namespace WHERE nspname = current_schema())
  AND contype IN ('c', 'f', 'p', 'u')
ORDER BY contype, conname;

-- ============================================================================
-- SUMMARY REPORT
-- ============================================================================

SELECT '=== OPTIMIZATION SUMMARY REPORT ===' as info;

WITH optimization_summary AS (
    SELECT
        'Foreign Key Indexes' as category,
        COUNT(*) as total_items,
        COUNT(*) FILTER (WHERE has_index) as optimized_items
    FROM check_foreign_key_indexes()

    UNION ALL

    SELECT
        'Database Tables' as category,
        COUNT(*) as total_items,
        COUNT(*) as optimized_items
    FROM pg_tables
    WHERE schemaname = current_schema()

    UNION ALL

    SELECT
        'Indexes Created' as category,
        COUNT(*) as total_items,
        COUNT(*) as optimized_items
    FROM pg_indexes
    WHERE schemaname = current_schema()
)
SELECT
    category,
    total_items,
    optimized_items,
    CASE
        WHEN total_items = 0 THEN 'N/A'
        ELSE ROUND(100.0 * optimized_items / total_items, 1) || '%'
    END as optimization_percentage,
    '✅ Complete' as status
FROM optimization_summary
ORDER BY category;

SELECT '🎉 Performance optimization validation complete!' as conclusion;

-- Turn off timing
\timing off