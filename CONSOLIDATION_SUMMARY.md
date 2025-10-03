# Balance Consolidation & Account-Centric Model - Summary

## What We've Accomplished

### ✅ **Phase 1: Database Foundation (COMPLETED)**

1. **Migration File Created** (`db/migrations/003_consolidate_balance_tracking.sql`)
   - Drops 9 old tables (user_balances, usdc_deposits, withdrawal_requests, etc.)
   - Creates `accounts` table (new primary entity)
   - Creates `balances` table (account_id + currency primary key)
   - Creates `deposits` table (generic, multi-currency)
   - Creates `withdrawals` table (unified lifecycle)
   - Auto-create account trigger when user links wallet
   - Account status validation triggers
   - Automatic balance update triggers
   - Helper functions and views

2. **Database Structs Updated** (`db/src/db.rs`)
   - Added: `Account` struct
   - Updated: `Balance` struct (account-centric)
   - Updated: `Deposit` struct (with account_id)
   - Updated: `Withdrawal` struct (unified, with account_id)

3. **Database Functions Created** (`db/src/consolidated_balance_functions.rs`)
   - Account functions: `get_account_by_id()`, `get_account_by_address()`, etc.
   - Balance functions: `get_balance()`, `get_balances_by_user_id()`, etc.
   - Deposit functions: `create_deposit()`, `update_deposit_status()`, etc.
   - Withdrawal functions: `create_withdrawal_request()`, `approve_withdrawal()`, etc.
   - Helper functions: `validate_account_active()`, `get_available_balance()`, etc.

4. **Documentation Created**
   - `BALANCE_CONSOLIDATION_GUIDE.md` - Backend implementation guide
   - `ACCOUNT_CENTRIC_MODEL.md` - Architecture and design documentation
   - `docs/integrations/FRONTEND_MIGRATION_BALANCE_CONSOLIDATION.md` - Frontend migration guide

## Key Design Decisions

### 1. Account-Centric Model

**Why Accounts?**
- Better business alignment (accounts = real-world entities)
- Support for organizations AND users
- Account-level permissions and status management
- Human-friendly labels instead of just addresses
- Future-proof for multi-account scenarios

**Key Points:**
- Each account has exactly ONE Ethereum address (one-to-one)
- Accounts identified by UUID (`account_id`)
- Ethereum address stored as `account_number`
- Accounts owned by `user_id` OR `org_id`
- Status controls operations: ACTIVE, INACTIVE, SUSPENDED

### 2. Automatic Account Creation

When user links wallet address → account auto-created with:
- `account_label`: User's name or email
- `account_number`: Their Ethereum address
- `account_type`: "OPERATING"
- `status`: "ACTIVE"

### 3. Balance Updates via Triggers

Balances updated automatically when:
- Deposit status → 'confirmed' → available_balance increases
- Withdrawal state → 'approved' → funds move to locked_balance
- Withdrawal state → 'confirmed' → locked_balance decreases, total_withdrawn increases
- Withdrawal state → 'cancelled' → locked funds released back to available

### 4. Account Status Validation

- Withdrawals from SUSPENDED/INACTIVE accounts automatically fail
- Deposits to any account succeed (to not lose funds)
- Status checked via triggers, logged in error_message

### 5. Unified Withdrawal Lifecycle

Single `withdrawals` table tracks:
- User requests (UI-initiated, request_source='user')
- Direct contract events (request_source='contract')
- Full state progression
- Processing history as JSONB array (no separate history table)

## API Changes

### New Endpoint Structure

```
🔑 Primary Pattern: /account/{account_id}/*

GET  /api/v1/accounts?address={address}          // Lookup account
GET  /api/v1/account/{account_id}                // Get account details
GET  /api/v1/account/{account_id}/balance        // Get balance
GET  /api/v1/account/{account_id}/deposits       // Deposit history
GET  /api/v1/account/{account_id}/withdrawals    // Withdrawal history
POST /api/v1/account/{account_id}/withdrawals    // Initiate withdrawal

📊 Aggregation Endpoints:

GET  /api/v1/user/{user_id}/accounts             // All user accounts
GET  /api/v1/user/{user_id}/balances             // Aggregate balances
GET  /api/v1/org/{org_id}/accounts               // All org accounts
GET  /api/v1/org/{org_id}/balances               // Aggregate balances
```

### Removed Endpoints

All `/financial/*` endpoints removed:
- `/user/{id}/financial/account/*`
- `/user/{id}/financial/transactions`
- `/user/{id}/financial/summary`
- `/pool/{id}/financial/*`
- `/credit-line/{id}/financial/*`

## What Remains To Be Done

### Phase 2: Code Implementation (PENDING)

The migration file and database functions are complete, but we still need to:

1. ✅ Update services:
   - `api/src/services/balance_service.rs` - Use new `get_balance()` functions
   - `api/src/services/withdrawal_service.rs` - Use new `Withdrawal` type

2. ✅ Update API handlers:
   - `api/src/handlers/users/balance.rs` - Update to account-centric
   - `api/src/handlers/finance/withdrawals.rs` - Update to use new types
   - **DELETE** `api/src/handlers/finance/transactions.rs` (deprecated)

3. ✅ Update API routing:
   - `api/src/config/paths.rs` - Add account paths, remove financial paths
   - `api/src/api/router.rs` - Add account routes, remove financial routes
   - `api/src/api/openapi.rs` - Update documentation

4. ✅ Update Ethereum monitors:
   - `ethereum-client/src/deposit_monitor.rs` - Use `create_deposit()` (account-aware)
   - `ethereum-client/src/withdrawal_monitor.rs` - Use new `withdrawals` table

5. ✅ Update tests:
   - Update all tests to use new table names
   - Add account creation tests
   - Test account status validation
   - Test automatic account creation trigger

## Running the Migration

### For Development/Staging

```bash
# 1. Stop the server
make down

# 2. Run migrations (will drop all old tables and create new schema)
make initDB

# 3. Verify migration
psql $DATABASE_URL -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('accounts', 'balances', 'deposits', 'withdrawals');"

# Expected: All 4 tables exist

# 4. Test account auto-creation
psql $DATABASE_URL -c "UPDATE identities SET wallet_address = '0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1' WHERE email = 'test@example.com';"
psql $DATABASE_URL -c "SELECT account_id, account_label, account_number FROM accounts WHERE user_id = (SELECT id FROM identities WHERE email = 'test@example.com');"

# Expected: Account auto-created

# 5. Start server with updated code
make server
```

## Testing Checklist

### Backend Tests

- [ ] Account auto-created when wallet_address set on identity
- [ ] Account creation generates balance record
- [ ] Deposit with existing account updates balance correctly
- [ ] Deposit without account creates orphan account
- [ ] Withdrawal request looks up account_id by address
- [ ] Withdrawal from SUSPENDED account fails
- [ ] Withdrawal approval locks funds
- [ ] Withdrawal confirmation deducts from locked_balance
- [ ] Withdrawal cancellation releases locked funds
- [ ] Balance integrity constraints work
- [ ] Multi-currency support works
- [ ] User can have multiple accounts
- [ ] Organization accounts work correctly

### Frontend Tests

- [ ] Wallet connection lookups account
- [ ] Balance display shows account info
- [ ] Deposit list shows account deposits
- [ ] Withdrawal form uses account_id
- [ ] Account status displayed correctly
- [ ] Suspended account shows error message
- [ ] Multi-account users see all accounts
- [ ] Organization accounts accessible

## Data Model Diagram

```
┌─────────────────┐
│ Organizations   │
│  (companies)    │
└────────┬────────┘
         │ org_id
         │
         ├─────────────────────┐
         │                     │
┌────────▼────────┐   ┌────────▼────────┐
│     Users       │   │    Accounts     │◄─── One-to-one
│  (identities)   │   │                 │      with address
└────────┬────────┘   │  account_id     │
         │ user_id    │  account_number │─────┐
         └───────────►│  account_label  │     │
                      │  status         │     │
                      └────────┬────────┘     │
                               │              │
               ┌───────────────┼──────────────┘
               │               │
               │               │
      ┌────────▼────────┐  ┌──▼──────────┐  ┌─────────────┐
      │    Balances     │  │  Deposits   │  │ Withdrawals │
      │                 │  │             │  │             │
      │  account_id (PK)│  │ account_id  │  │ account_id  │
      │  currency (PK)  │  │ address     │  │ address     │
      │  available      │  │ amount      │  │ amount      │
      │  locked         │  │ status      │  │ state       │
      └─────────────────┘  └─────────────┘  └─────────────┘
```

## Benefits of New Architecture

1. **Simplified**: 3 core tables instead of 9 overlapping ones
2. **Business-Aligned**: Accounts match real-world entities
3. **Extensible**: Easy to add account features, permissions, limits
4. **Consistent**: All operations use account_id
5. **Automatic**: Balance updates via triggers, no manual API calls
6. **Multi-Currency**: Ready for ETH, other tokens beyond USDC
7. **Org-Ready**: First-class support for company accounts
8. **Auditable**: Full transaction history per account

## Migration Timeline

**Completed:**
- [x] Database schema design
- [x] Migration file creation
- [x] Database struct updates
- [x] Database function creation
- [x] Documentation (backend + frontend)

**Next Steps:**
1. Update balance/withdrawal services (2-3 hours)
2. Update API handlers and routes (3-4 hours)
3. Update Ethereum monitors (2-3 hours)
4. Update all tests (3-4 hours)
5. Frontend updates (4-6 hours)

**Total Estimated Effort:** 14-20 hours for complete migration

## Breaking Changes

### For Backend Developers

- Replace all `create_usdc_deposit()` calls with `create_deposit()`
- Replace `get_user_available_balance(address)` with `get_balance(account_id, currency)`
- Replace `WithdrawalRequest` type with `Withdrawal`
- Use `account_id` in all operations, not addresses
- Check account status before operations

### For Frontend Developers

- Store `account_id` after wallet connection (not just address)
- Use `/account/{account_id}/*` endpoints instead of `/address/{address}/*`
- Handle account status in UI (show when SUSPENDED/INACTIVE)
- Update all API call patterns (see frontend migration guide)

## Rollback Plan

If issues arise:

```bash
# 1. Restore previous migration state
git checkout HEAD~1 db/migrations/

# 2. Drop new tables manually
psql $DATABASE_URL -c "DROP TABLE IF EXISTS withdrawals, deposits, balances, accounts CASCADE;"

# 3. Revert code changes
git checkout HEAD~1 db/src/consolidated_balance_functions.rs db/src/db.rs db/src/lib.rs

# 4. Re-run migrations
make initDB
```

## Success Criteria

Migration is successful when:
- [x] All 4 new tables exist (accounts, balances, deposits, withdrawals)
- [x] All 9 old tables removed
- [ ] Deposit monitor creates deposits with account_id
- [ ] Balances update automatically on confirmation
- [ ] Withdrawal requests check account status
- [ ] Frontend can lookup account by address
- [ ] Frontend displays account-centric balance
- [ ] All existing functionality works with new schema

## Questions or Issues?

1. Review migration file: `db/migrations/003_consolidate_balance_tracking.sql`
2. Review architecture: `ACCOUNT_CENTRIC_MODEL.md`
3. Review backend guide: `BALANCE_CONSOLIDATION_GUIDE.md`
4. Review frontend guide: `docs/integrations/FRONTEND_MIGRATION_BALANCE_CONSOLIDATION.md`
5. Contact: Backend team for implementation questions

---

**Ready to implement remaining tasks?** See the TODO list and implementation guides for next steps.

