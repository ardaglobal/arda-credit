# Database Migrations - Refactored Schema

This directory contains clean, consolidated database migrations for the Arda Credit system after a major refactoring to improve data model consistency and naming conventions.

## Migration Files

The migrations have been reorganized into logical entity groups:

### Current Clean Migrations (Post-Refactor)

- **`001_companies.sql`** - Company/organization management with type field (investor/originator)
- **`002_users.sql`** - User management consolidating identities table, with company relationships and system admin flags
- **`003_deals.sql`** - Core deal management (renamed from loans) with proper naming conventions (no stutter)
- **`004_state_transitions.sql`** - ZK circuit audit trail and batch processing integration
- **`005_payments_balances.sql`** - User payments, balances, deposits, withdrawals, and liquidity pools
- **`006_invites.sql`** - Invite codes, document storage, and AI analysis tables

### Key Refactoring Changes

#### ✅ Schema Improvements Made

1. **Removed identities table** - Consolidated into `users` table with better structure
2. **Fixed stutter naming** - `deals.status` instead of `deals.deal_status`
3. **Renamed loan → deal** - Throughout schema for business clarity
4. **Added company relationships** - `users.company_id` for team-based access control
5. **Proper naming conventions** - Consistent, clear field names
6. **Enhanced type safety** - Better constraints and enum types

#### 📊 Major Entity Changes

- **`identities` → `users`** - Consolidated with added `company_id` and `system_admin` fields
- **`loans` → `deals`** - Renamed with fixed status field structure
- **`borrower_id` → `originator_id`** - More business-appropriate terminology
- **`lender_id` → `investor_id`** - Clearer role definition
- **`loan_status` → `status`** - Removed redundant naming

#### 🔄 Data Model Consolidation

- **Authentication** - Magic links and sessions properly integrated
- **Document Storage** - Company-based access control with S3 integration
- **Financial Tracking** - Unified balance and transaction management
- **ZK Integration** - Clean transaction queue and proof batch structure

## Deprecated Migrations

Old migrations (001-068) have been archived in `../migrations_deprecated/` folder. These contain the incremental changes that led to schema inconsistencies and are preserved for historical reference only.

**⚠️ Do not use deprecated migrations for new deployments.**

## Running Migrations

For fresh deployments, use only the new consolidated migrations:

```bash
# Initialize database with clean schema
make initDB

# Or manually with SQLx
DATABASE_URL="your-connection-string" sqlx migrate run --source ./db/migrations
```

## Database Schema Overview

The refactored schema provides:

- **Clean entity relationships** with proper foreign keys
- **Consistent naming conventions** across all tables
- **Enhanced security model** with company-based access control
- **ZK proof integration** with transaction queue and batch processing
- **Document management** with audit trails and sharing capabilities
- **Financial tracking** with USDC deposits/withdrawals and liquidity pools

## Migration Best Practices

- Always use the new consolidated migrations for fresh deployments
- Test migrations thoroughly on development data
- Use transactions for complex schema changes
- Include proper indexes for performance
- Add meaningful comments and constraints
- Maintain referential integrity with proper foreign keys

## For Developers

When adding new migrations:

1. Create new migration with next sequential number (007_, 008_, etc.)
2. Follow established naming conventions (no stutter, clear purpose)
3. Include proper constraints, indexes, and comments
4. Test with representative data
5. Update this README with migration description