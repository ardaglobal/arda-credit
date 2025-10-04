-- Migration: Remove merchant and merchant profile tables
-- Description: Removes all merchant-related tables as the system is moving away from the merchant concept
-- Created: 2025-10-03

-- ============================================================================
-- DROP MERCHANT-RELATED TABLES
-- ============================================================================

-- Drop dependent tables first (to avoid foreign key constraint errors)
DROP TABLE IF EXISTS merchant_documents CASCADE;
DROP TABLE IF EXISTS merchant_turnover_daily CASCADE;
DROP TABLE IF EXISTS merchant_profiles CASCADE;
DROP TABLE IF EXISTS merchants CASCADE;

-- ============================================================================
-- VERIFICATION
-- ============================================================================

-- Verify all merchant tables are removed
-- This query should return 0 rows
-- SELECT table_name FROM information_schema.tables
-- WHERE table_schema = 'public'
-- AND table_name LIKE 'merchant%';
