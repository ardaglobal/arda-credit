# Balance Tracking Consolidation - Implementation Guide

## Overview

This document outlines the consolidation of Arda Credit's financial tracking system from multiple fragmented tables to a clean, address-centric architecture.

## Changes Summary

### Database Schema Changes

#### **Tables REMOVED** ❌
1. `user_balances` - Replaced by address-centric `balances`
2. `user_deposit_balances` - Aggregation now handled by `balances` table
3. `user_withdrawal_balances` - Aggregation now handled by `balances` table
4. `account_financial_transactions` - Removed (redundant with deposits/withdrawals)
5. `balances` (old) - Identity-based, replaced by address-based
6. `usdc_deposits` - Renamed to generic `deposits`
7. `usdc_withdrawals` - Merged into unified `withdrawals`
8. `withdrawal_requests` - Merged into unified `withdrawals`
9. `withdrawal_request_history` - Now JSONB field in `withdrawals`

#### **Tables CREATED** ✅
1. **`balances`** - Address-centric balance tracking
   - Primary key: `(address, currency)`
   - Tracks: available_balance, locked_balance, totals, counts
   - Optional links: user_id, company_id
   - Auto-updated via triggers

2. **`deposits`** - Generic deposit event tracking
   - Renamed from `usdc_deposits`
   - Now supports any currency
   - Full event tracking: tx_hash, block, log_index, signature

3. **`withdrawals`** - Unified withdrawal lifecycle
   - Combines `withdrawal_requests` + `usdc_withdrawals`
   - Tracks full lifecycle: request → validate → approve → submit → confirm
   - `request_source`: 'user' (UI) or 'contract' (direct)
   - Processing history stored as JSONB array

### API Endpoint Changes

#### **Endpoints REMOVED** ❌
```
DELETE /api/v1/user/{user_id}/financial/account/deposit
DELETE /api/v1/user/{user_id}/financial/account/withdrawal
DELETE /api/v1/user/{user_id}/financial/account
DELETE /api/v1/user/{user_id}/financial/transactions
DELETE /api/v1/user/{user_id}/financial/summary
DELETE /api/v1/user/{user_id}/financial/pools
DELETE /api/v1/pool/{pool_id}/financial/*
DELETE /api/v1/credit-line/{credit_line_id}/financial/*
DELETE /api/v1/merchant/{merchant_id}/financial/credit-lines
```

#### **Endpoints KEPT/UPDATED** ✅
```
GET /api/v1/address/{address}/balance - Get balance for specific address
GET /api/v1/user/{user_id}/balance - Get balances for user's linked addresses
GET /api/v1/address/{address}/deposits - Get deposit history
GET /api/v1/address/{address}/withdrawals - Get withdrawal history
POST /api/v1/withdrawals - Initiate withdrawal request
GET /api/v1/withdrawals/{id} - Get withdrawal status
POST /api/v1/withdrawals/{id}/cancel - Cancel pending withdrawal
GET /api/v1/withdrawals/pending - Get pending withdrawals (admin)
POST /api/v1/withdrawals/{id}/approve - Approve withdrawal (admin)
POST /api/v1/withdrawals/{id}/reject - Reject withdrawal (admin)
```

## Implementation Steps

### 1. Database Migration ✅
- File: `db/migrations/003_consolidate_balance_tracking.sql`
- Drops old tables and creates new consolidated schema
- Adds triggers for automatic balance updates
- Creates helper functions and views

### 2. Database Structs & Functions ✅
- File: `db/src/consolidated_balance_functions.rs` (NEW)
- Updated structs: `Balance`, `Deposit`, `Withdrawal`
- New functions: `get_balance()`, `create_deposit()`, etc.
- Integrated into `db/src/lib.rs`

### 3. Balance Service Updates 🔄
- File: `api/src/services/balance_service.rs`
- Remove dependency on `user_balances`
- Use new `balances` table
- Update verification logic

### 4. Withdrawal Service Updates 🔄
- File: `api/src/services/withdrawal_service.rs`
- Remove `WithdrawalRequest` type, use `Withdrawal`
- Update all function signatures
- Use unified lifecycle states

### 5. API Handler Updates 🔄
- File: `api/src/handlers/users/balance.rs`
  - Already uses consolidated approach
  - Update to use new database functions
  
- File: `api/src/handlers/finance/withdrawals.rs`
  - Update to use `Withdrawal` instead of `WithdrawalRequest`
  - Update all endpoints to use new database functions

- File: `api/src/handlers/finance/transactions.rs` ❌ DELETE
  - Remove entirely (endpoints deprecated)

### 6. Router & Paths Updates 🔄
- File: `api/src/config/paths.rs`
  - Remove all `/financial/*` path constants
  - Keep balance/deposit/withdrawal paths

- File: `api/src/api/router.rs`
  - Remove `/financial/*` routes
  - Update withdrawal routes

### 7. OpenAPI Documentation 🔄
- File: `api/src/api/openapi.rs`
  - Remove financial transaction types
  - Update withdrawal types
  - Remove deprecated endpoints

### 8. Ethereum Monitors 🔄
- File: `ethereum-client/src/deposit_monitor.rs`
  - Update to use `deposits` table instead of `usdc_deposits`
  - Update to use new balance functions

- File: `ethereum-client/src/withdrawal_monitor.rs`
  - Update to use `withdrawals` table instead of `usdc_withdrawals`
  - Handle unified lifecycle states

### 9. Tests Updates 🔄
- Update all tests to use new table names and functions
- Remove financial transaction tests
- Update withdrawal tests for unified lifecycle

### 10. Frontend Migration Guide 📝
- Document old → new endpoint mappings
- Provide migration examples
- Explain new data models

## Technical Details

### Balance Update Flow

**Deposits:**
```
1. Ethereum event detected → create_deposit()
2. Confirmations increase → update_deposit_status()
3. Status → 'confirmed' → TRIGGER updates balances table
   - available_balance += amount
   - total_deposited += amount
   - deposit_count += 1
```

**Withdrawals (User-Initiated):**
```
1. User requests → create_withdrawal_request()
2. Validate KYC/balance → update_withdrawal_validation()
3. Admin approves → approve_withdrawal()
   - TRIGGER: available_balance -= amount, locked_balance += amount
4. Submit to contract → mark_withdrawal_submitted()
5. Confirmations increase → update_withdrawal_confirmations()
6. Status → 'confirmed' → TRIGGER updates balances table
   - locked_balance -= amount
   - total_withdrawn += amount
   - withdrawal_count += 1
```

**Withdrawals (Contract-Direct):**
```
1. Ethereum event detected → create_withdrawal_from_event()
2. Already in 'submitted' state
3. Confirmations increase → status → 'confirmed'
4. TRIGGER updates balances automatically
```

### Migration Path

**For Development/Staging:**
1. Run migration: `make initDB` (will apply 003_consolidate_balance_tracking.sql)
2. All existing data will be lost (fresh start)
3. Update code to use new functions
4. Test thoroughly

**For Production (future):**
1. Would need data migration script to copy existing balances
2. Not needed now since MVP/proof-of-concept

## Testing Checklist

- [ ] Deposit creates balance record automatically
- [ ] Deposit confirmation updates balance correctly
- [ ] Withdrawal request locks funds
- [ ] Withdrawal approval moves to locked_balance
- [ ] Withdrawal confirmation deducts from locked_balance
- [ ] Withdrawal cancellation releases locked funds
- [ ] Balance integrity constraint prevents overdraft
- [ ] Multiple currencies work correctly
- [ ] User-linked addresses aggregate properly
- [ ] Company-linked addresses aggregate properly

## Rollout Plan

1. ✅ Create migration file
2. ✅ Create new database functions
3. 🔄 Update services (balance, withdrawal)
4. 🔄 Update API handlers
5. 🔄 Update monitors (deposit, withdrawal)
6. 🔄 Remove deprecated code
7. 🔄 Update tests
8. 📝 Create frontend migration guide
9. ✅ Test thoroughly
10. 🚀 Deploy

## Breaking Changes

### For Backend

- `create_usdc_deposit()` → `create_deposit()`
- `get_deposits_by_user_address()` → `get_deposits_by_address()`
- `get_user_available_balance()` → `get_available_balance()` or `get_balance()`
- `create_withdrawal_request()` → `create_withdrawal_request()` (signature changed)
- `WithdrawalRequest` type → `Withdrawal` type
- All `/financial/*` endpoints removed

### For Frontend

See `FRONTEND_MIGRATION_GUIDE.md` for detailed migration instructions.

## Notes

- **Automatic Balance Updates**: Balances are updated automatically via database triggers when deposits/withdrawals change status. No manual balance update calls needed.

- **Address-Centric Design**: All operations now key off Ethereum addresses first, with optional user/company relationships for aggregation.

- **Unified Withdrawal Lifecycle**: Single `withdrawals` table tracks the entire lifecycle from user request through blockchain confirmation, eliminating sync issues between multiple tables.

- **Multi-Currency Support**: All tables support generic currency field (default: 'USDC'), enabling future expansion to other tokens.

- **Processing History**: Withdrawal processing history stored as JSONB array in the withdrawal record itself, eliminating need for separate history table.

