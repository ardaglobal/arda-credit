# Remaining Implementation Guide - Account-Centric Balance Consolidation

## Status: Services Updated ✅ | Monitors & API Endpoints Pending 🔄

**What's Complete:**
- ✅ Database migration file
- ✅ Database functions (consolidated_balance_functions.rs)
- ✅ Balance service updated
- ✅ Withdrawal service updated
- ✅ All documentation complete

**What Remains:**
- 🔄 Ethereum monitors (deposit/withdrawal)
- 🔄 Account API endpoints
- 🔄 Update existing balance/withdrawal handlers
- 🔄 Remove deprecated code
- 🔄 Update tests

---

## CRITICAL: Update Ethereum Monitors (Required for deposits/withdrawals to work)

### 1. Deposit Monitor (`ethereum-client/src/deposit_monitor.rs`)

**Line 710-738:** Change INSERT statement from `usdc_deposits` to `deposits`

**FIND:**
```rust
INSERT INTO usdc_deposits (
    user_address,
    amount,
    transaction_hash,
    event_transaction_hash,
    block_number,
    event_timestamp,
    log_index,
    status,
    confirmation_count,
    required_confirmations,
    created_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0, $8, NOW())
```

**REPLACE WITH:**
```rust
WITH account_lookup AS (
    SELECT account_id FROM accounts WHERE account_number = $1
),
account_insert AS (
    INSERT INTO accounts (account_label, account_number, account_type, currency, status)
    SELECT 'Unlinked Account (' || substring($1, 1, 10) || '...)', $1, 'OPERATING', 'USDC', 'ACTIVE'
    WHERE NOT EXISTS (SELECT 1 FROM account_lookup)
    RETURNING account_id
),
final_account AS (
    SELECT account_id FROM account_lookup
    UNION ALL
    SELECT account_id FROM account_insert
)
INSERT INTO deposits (
    account_id,
    address,
    amount,
    currency,
    transaction_hash,
    event_transaction_hash,
    block_number,
    event_timestamp,
    log_index,
    event_signature,
    status,
    confirmation_count,
    required_confirmations,
    created_at
) SELECT
    (SELECT account_id FROM final_account),
    $1, $2, 'USDC', $3, $4, $5, $6, $7, '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef',
    'pending', 0, $8, NOW()
ON CONFLICT (transaction_hash, log_index)
DO UPDATE SET updated_at = NOW()
RETURNING id
```

**Lines 282, 322, 509, 568, 621:** Replace table name

**FIND:** `usdc_deposits`  
**REPLACE:** `deposits`

**Lines 322:** Update confirmation check query

**FIND:**
```rust
"UPDATE usdc_deposits SET confirmation_count = $1, updated_at = NOW() WHERE id = $2"
```

**REPLACE:**
```rust
"UPDATE deposits SET confirmation_count = $1, updated_at = NOW() WHERE id = $2"
```

**Lines 443-451:** Remove manual balance update (now automatic via trigger)

**FIND:**
```rust
let _updated_deposit = update_deposit_status_tx(
    &mut tx,
    deposit.id,
    "confirmed",
    deposit.confirmation_count,
    Some(chrono::Utc::now()),
)
.await
.map_err(|e| EthereumError::Database(format!("Failed to update deposit status: {e}")))?;

// 2. Find user ID by wallet address
if let Some(user_id) = get_user_id_by_wallet_address(db_pool, &deposit.user_address)
    .await
    .map_err(|e| EthereumError::Database(e.to_string()))?
{
    // 3. Update user balance atomically (convert wei to USDC: divide by 1,000,000)
    let deposit_amount_decimal = deposit
        .amount
        .parse::<rust_decimal::Decimal>()
        .map_err(|e| EthereumError::Database(format!("Invalid deposit amount: {e}")))?;
    let deposit_amount_usdc =
        deposit_amount_decimal / rust_decimal::Decimal::new(1_000_000, 0);

    update_user_balance_from_deposit_tx(&mut tx, user_id, deposit_amount_usdc)
        .await
        .map_err(|e| EthereumError::Database(format!("Failed to update balance: {e}")))?;

    info!(
        "Updated user balance: user_id={}, wallet={}, amount_usdc={}",
        user_id, deposit.user_address, deposit_amount_usdc
    );
} else {
    warn!("No user found for address {} - deposit stored but balance not updated", deposit.user_address);
}
```

**REPLACE WITH:**
```rust
// Update deposit status to confirmed
// Balance will be updated automatically via trigger
let updated_deposit = arda_credit_db::update_deposit_status(
    &self.db_pool,  // Use db_pool directly, not transaction
    deposit.id,
    "confirmed",
    deposit.confirmation_count,
    Some(chrono::Utc::now()),
)
.await
.map_err(|e| EthereumError::Database(format!("Failed to update deposit status: {e}")))?;

info!(
    "Deposit confirmed: id={}, account_id={}, amount={} (balance updated via trigger)",
    updated_deposit.id,
    updated_deposit.account_id,
    updated_deposit.amount
);
```

**Remove transaction usage:** Change from `&mut tx` to direct pool usage since trigger handles balance update.

### 2. Withdrawal Monitor (`ethereum-client/src/withdrawal_monitor.rs`)

**Find all references to:** `usdc_withdrawals`  
**Replace with:** `withdrawals`

**Find all references to:** `UsdcWithdrawal` (struct)  
**Replace with:** `Withdrawal`

**Add account_id lookup:** When creating withdrawal from event, need to lookup account first.

**Example change:**
```rust
// OLD
let withdrawal = create_withdrawal(...);

// NEW  
let account = get_account_by_address(&db_pool, &user_address).await?;
let withdrawal = create_withdrawal_from_event(
    &db_pool,
    &user_address,
    &recipient_address,
    amount,
    "USDC",
    &tx_hash,
    &event_tx_hash,
    block_number,
    event_timestamp,
    log_index,
    &event_signature,
).await?;
```

---

## Add Account API Endpoints

### Create Account Handler (`api/src/handlers/accounts/mod.rs`)

```rust
//! Account management handlers

pub mod accounts;

pub use accounts::*;
```

### Create Account Handlers (`api/src/handlers/accounts/accounts.rs`)

```rust
//! Account management API handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use arda_credit_db::{
    get_account_by_address, get_account_by_id, get_accounts_by_user_id,
    get_accounts_by_org_id, get_balance, get_balances_for_account,
    get_deposits_by_account_id, get_withdrawals_by_account_id,
    Account, Balance, Deposit, Withdrawal,
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AccountLookupQuery {
    /// Lookup account by Ethereum address
    pub address: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountLookupResponse {
    pub accounts: Vec<Account>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountBalanceResponse {
    pub account_id: Uuid,
    pub account_label: String,
    pub account_number: String,
    pub account_status: String,
    pub balance: BalanceInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceInfo {
    pub currency: String,
    pub available_balance: String,
    pub available_balance_usdc: String,
    pub locked_balance: String,
    pub locked_balance_usdc: String,
    pub total_deposited: String,
    pub total_deposited_usdc: String,
    pub total_withdrawn: String,
    pub total_withdrawn_usdc: String,
    pub deposit_count: i32,
    pub withdrawal_count: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountDepositsResponse {
    pub account_id: Uuid,
    pub deposits: Vec<Deposit>,
    pub total_count: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountWithdrawalsResponse {
    pub account_id: Uuid,
    pub withdrawals: Vec<Withdrawal>,
    pub total_count: usize,
    pub has_more: bool,
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Lookup account by Ethereum address
#[utoipa::path(
    get,
    path = "/api/v1/accounts",
    params(AccountLookupQuery),
    responses(
        (status = 200, description = "Account found", body = AccountLookupResponse),
        (status = 404, description = "No account found for address")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn lookup_account(
    Query(params): Query<AccountLookupQuery>,
    State(state): State<ApiState>,
) -> Result<Json<AccountLookupResponse>, StatusCode> {
    if let Some(address) = params.address {
        info!("Looking up account by address: {}", address);
        
        let account = get_account_by_address(&state.pool, &address)
            .await
            .map_err(|e| {
                error!("Failed to lookup account: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        Ok(Json(AccountLookupResponse {
            accounts: account.into_iter().collect(),
        }))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// Get account details by ID
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account details", body = Account),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<Account>, StatusCode> {
    info!("Getting account: {}", account_id);
    
    let account = get_account_by_id(&state.pool, account_id)
        .await
        .map_err(|e| {
            error!("Failed to get account: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    
    Ok(Json(account))
}

/// Get account balance
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/balance",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account balance", body = AccountBalanceResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_balance_endpoint(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<AccountBalanceResponse>, StatusCode> {
    info!("Getting balance for account: {}", account_id);
    
    // Get account details
    let account = get_account_by_id(&state.pool, account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Get balance (default USDC)
    let balance = get_balance(&state.pool, account_id, &account.currency)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Convert to human-readable
    let divisor = rust_decimal::Decimal::new(1_000_000, 0);
    
    Ok(Json(AccountBalanceResponse {
        account_id: account.account_id,
        account_label: account.account_label,
        account_number: account.account_number,
        account_status: account.status,
        balance: BalanceInfo {
            currency: balance.currency.clone(),
            available_balance: balance.available_balance.to_string(),
            available_balance_usdc: (balance.available_balance / divisor).to_string(),
            locked_balance: balance.locked_balance.to_string(),
            locked_balance_usdc: (balance.locked_balance / divisor).to_string(),
            total_deposited: balance.total_deposited.to_string(),
            total_deposited_usdc: (balance.total_deposited / divisor).to_string(),
            total_withdrawn: balance.total_withdrawn.to_string(),
            total_withdrawn_usdc: (balance.total_withdrawn / divisor).to_string(),
            deposit_count: balance.deposit_count,
            withdrawal_count: balance.withdrawal_count,
        },
    }))
}

/// Get deposits for account
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/deposits",
    params(
        ("account_id" = Uuid, Path, description = "Account ID"),
        ("limit" = Option<i32>, Query, description = "Max results"),
        ("offset" = Option<i32>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Account deposits", body = AccountDepositsResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_deposits_endpoint(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<AccountDepositsResponse>, StatusCode> {
    info!("Getting deposits for account: {}", account_id);
    
    let deposits = get_deposits_by_account_id(&state.pool, account_id, None)
        .await
        .map_err(|e| {
            error!("Failed to get deposits: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let total_count = deposits.len();
    let has_more = false; // TODO: Implement pagination
    
    Ok(Json(AccountDepositsResponse {
        account_id,
        deposits,
        total_count,
        has_more,
    }))
}

/// Get withdrawals for account
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/withdrawals",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account withdrawals", body = AccountWithdrawalsResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_withdrawals_endpoint(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<AccountWithdrawalsResponse>, StatusCode> {
    info!("Getting withdrawals for account: {}", account_id);
    
    let withdrawals = get_withdrawals_by_account_id(&state.pool, account_id, None)
        .await
        .map_err(|e| {
            error!("Failed to get withdrawals: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let total_count = withdrawals.len();
    let has_more = false; // TODO: Implement pagination
    
    Ok(Json(AccountWithdrawalsResponse {
        account_id,
        withdrawals,
        total_count,
        has_more,
    }))
}
```

### 2. Add to Handlers Module (`api/src/handlers/mod.rs`)

**ADD:**
```rust
pub mod accounts;
```

**And re-export:**
```rust
pub use accounts::*;
```

### 3. Update Router (`api/src/api/router.rs`)

**ADD these routes (around line 132):**
```rust
// Account management endpoints
.route(
    "/api/v1/accounts",
    get(crate::handlers::accounts::lookup_account),
)
.route(
    "/api/v1/account/:account_id",
    get(crate::handlers::accounts::get_account),
)
.route(
    "/api/v1/account/:account_id/balance",
    get(crate::handlers::accounts::get_account_balance_endpoint),
)
.route(
    "/api/v1/account/:account_id/deposits",
    get(crate::handlers::accounts::get_account_deposits_endpoint),
)
.route(
    "/api/v1/account/:account_id/withdrawals",
    get(crate::handlers::accounts::get_account_withdrawals_endpoint),
)
```

**REMOVE these routes (around line 302-327):**
```rust
// DELETE ALL OF THESE:
.route(
    paths::USER_ACCOUNT_FINANCIAL_DEPOSIT,
    post(record_account_deposit),
)
.route(
    paths::USER_ACCOUNT_FINANCIAL_WITHDRAWAL,
    post(record_account_withdrawal),
)
.route(
    paths::USER_ACCOUNT_FINANCIAL,
    get(get_account_financial_transactions),
)
.route(
    paths::USER_FINANCIAL_TRANSACTIONS,
    get(get_user_financial_transactions),
)
.route(
    paths::USER_FINANCIAL_SUMMARY,
    get(get_user_financial_summary),
)
```

### 4. Update Paths (`api/src/config/paths.rs`)

**ADD (around line 200):**
```rust
// ============================================================================
// ACCOUNT MANAGEMENT PATHS
// ============================================================================

/// Account lookup (by address)
pub const ACCOUNTS: &str = api_path!("/accounts");

/// Get account by ID
pub const ACCOUNT_BY_ID: &str = api_path!("/account/{account_id}");

/// Account balance
pub const ACCOUNT_BALANCE: &str = api_path!("/account/{account_id}/balance");

/// Account deposits
pub const ACCOUNT_DEPOSITS: &str = api_path!("/account/{account_id}/deposits");

/// Account withdrawals  
pub const ACCOUNT_WITHDRAWALS: &str = api_path!("/account/{account_id}/withdrawals");

/// Get user's accounts
pub const USER_ACCOUNTS: &str = api_path!("/user/{user_id}/accounts");

/// Get user's balances (across all accounts)
pub const USER_BALANCES: &str = api_path!("/user/{user_id}/balances");

/// Get org's accounts
pub const ORG_ACCOUNTS: &str = api_path!("/org/{org_id}/accounts");

/// Get org's balances
pub const ORG_BALANCES: &str = api_path!("/org/{org_id}/balances");
```

**REMOVE (around line 347-393):**
```rust
// DELETE ENTIRE FINANCIAL TRANSACTION PATHS SECTION
// ============================================================================
// FINANCIAL TRANSACTION PATHS
// ============================================================================
// ... all paths from USER_ACCOUNT_FINANCIAL to MERCHANT_CREDIT_LINE_FINANCIAL
```

### 5. Update OpenAPI (`api/src/api/openapi.rs`)

**REMOVE from paths() (around line 271-277):**
```rust
// DELETE THESE:
crate::handlers::finance::transactions::record_account_deposit,
crate::handlers::finance::transactions::record_account_withdrawal,
crate::handlers::finance::transactions::get_account_financial_transactions,
crate::handlers::finance::transactions::get_user_financial_transactions,
crate::handlers::finance::transactions::get_user_financial_summary,
```

**ADD to paths():**
```rust
// Account endpoints
crate::handlers::accounts::lookup_account,
crate::handlers::accounts::get_account,
crate::handlers::accounts::get_account_balance_endpoint,
crate::handlers::accounts::get_account_deposits_endpoint,
crate::handlers::accounts::get_account_withdrawals_endpoint,
```

**REMOVE from components/schemas:**
```rust
// DELETE THESE:
AccountFinancialTransactionResponse,
CreateAccountFinancialTransactionRequest,
UnifiedFinancialTransactionResponse,
FinancialSummaryResponse,
PaginatedFinancialTransactionsResponse,
```

**ADD to components/schemas:**
```rust
// Account types
crate::handlers::accounts::AccountLookupQuery,
crate::handlers::accounts::AccountLookupResponse,
crate::handlers::accounts::AccountBalanceResponse,
crate::handlers::accounts::BalanceInfo,
crate::handlers::accounts::AccountDepositsResponse,
crate::handlers::accounts::AccountWithdrawalsResponse,
```

### 6. Delete Deprecated Files

```bash
# Remove deprecated transaction handlers
rm api/src/handlers/finance/transactions.rs

# Update finance/mod.rs to remove transactions module
# File: api/src/handlers/finance/mod.rs
# DELETE: pub mod transactions;
# DELETE: pub use transactions::*;
```

### 7. Update Import in `api/src/handlers/finance/mod.rs`

**REMOVE:**
```rust
pub mod transactions;
pub use transactions::*;
```

Keep only:
```rust
pub mod transaction_history;
pub mod withdrawals;

pub use transaction_history::*;
pub use withdrawals::*;
```

---

## Testing After Implementation

### Test 1: Account Auto-Creation

```bash
# 1. Start with fresh DB
make initDB

# 2. Create user
curl -X POST http://localhost:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{"email":"test@test.com","invite_code":"INVESTOR2025"}'

# Save user_id from response

# 3. Link wallet (triggers account creation)
curl -X PUT http://localhost:8080/api/v1/user/{user_id} \
  -H "Content-Type: application/json" \
  -d '{"wallet_address":"0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1"}'

# 4. Lookup account
curl "http://localhost:8080/api/v1/accounts?address=0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1"

# Expected: Returns account with account_id, account_label, status=ACTIVE
```

### Test 2: Balance Updates

```bash
# 1. Get account_id from test 1

# 2. Check initial balance (should be 0)
curl http://localhost:8080/api/v1/account/{account_id}/balance

# 3. Simulate deposit via SQL (in production, deposit monitor does this)
psql $DATABASE_URL << EOF
INSERT INTO deposits (account_id, address, amount, currency, transaction_hash, event_transaction_hash, block_number, event_timestamp, log_index, event_signature)
VALUES ('{account_id}', '0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1', 5000000, 'USDC', '0xabc123', '0xdef456', 12345678, extract(epoch from now()), 1, '0xddf252ad');
EOF

# 4. Confirm deposit (triggers balance update)
psql $DATABASE_URL -c "UPDATE deposits SET status = 'confirmed', confirmation_count = 12 WHERE transaction_hash = '0xabc123';"

# 5. Check balance again
curl http://localhost:8080/api/v1/account/{account_id}/balance

# Expected: available_balance_usdc = "5.000000"
```

### Test 3: Withdrawal Flow

```bash
# 1. Initiate withdrawal
curl -X POST http://localhost:8080/api/v1/withdrawals \
  -H "Content-Type: application/json" \
  -d '{
    "user_address":"0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
    "recipient_address":"0x1234567890123456789012345678901234567890",
    "amount_usdc":"2.5"
  }'

# Save withdrawal_id from response

# 2. Check balance (should be locked)
curl http://localhost:8080/api/v1/account/{account_id}/balance

# Expected: locked_balance_usdc = "2.5", available_balance_usdc = "2.5" (5 - 2.5)

# 3. Cancel withdrawal
curl -X POST http://localhost:8080/api/v1/withdrawals/{withdrawal_id}/cancel

# 4. Check balance (should be released)
curl http://localhost:8080/api/v1/account/{account_id}/balance

# Expected: locked_balance_usdc = "0", available_balance_usdc = "5.0"
```

---

## File Checklist

### Files to Create

- [ ] `api/src/handlers/accounts/mod.rs` (module declaration)
- [ ] `api/src/handlers/accounts/accounts.rs` (account endpoints)

### Files to Update

- [ ] `ethereum-client/src/deposit_monitor.rs` (use deposits table)
- [ ] `ethereum-client/src/withdrawal_monitor.rs` (use withdrawals table)
- [ ] `api/src/handlers/mod.rs` (add accounts module)
- [ ] `api/src/api/router.rs` (add account routes, remove /financial/* routes)
- [ ] `api/src/config/paths.rs` (add account paths, remove financial paths)
- [ ] `api/src/api/openapi.rs` (add account types, remove financial types)
- [ ] `api/src/handlers/finance/mod.rs` (remove transactions module)

### Files to Delete

- [ ] `api/src/handlers/finance/transactions.rs`

### Files Already Updated ✅

- [x] `db/migrations/003_consolidate_balance_tracking.sql`
- [x] `db/src/consolidated_balance_functions.rs`
- [x] `db/src/db.rs`
- [x] `db/src/lib.rs`
- [x] `api/src/services/balance_service.rs`
- [x] `api/src/services/withdrawal_service.rs`

---

## Validation Queries

After running migration and implementing changes, verify with:

```sql
-- Check tables exist
SELECT table_name FROM information_schema.tables 
WHERE table_name IN ('accounts', 'balances', 'deposits', 'withdrawals')
ORDER BY table_name;

-- Check old tables removed
SELECT table_name FROM information_schema.tables 
WHERE table_name IN ('user_balances', 'usdc_deposits', 'withdrawal_requests')
ORDER BY table_name;
-- Expected: No results

-- Check triggers exist
SELECT trigger_name, event_object_table
FROM information_schema.triggers 
WHERE trigger_name LIKE '%account%' OR trigger_name LIKE '%balance%'
ORDER BY event_object_table, trigger_name;

-- Test account auto-creation trigger
UPDATE identities SET wallet_address = '0x1234567890123456789012345678901234567890'
WHERE email = 'test@example.com';

-- Verify account was created
SELECT account_id, account_label, account_number, status
FROM accounts
WHERE user_id = (SELECT id FROM identities WHERE email = 'test@example.com');
-- Expected: One row with account_number = wallet_address, status = ACTIVE

-- Verify balance record exists
SELECT * FROM balances WHERE account_id IN (
    SELECT account_id FROM accounts WHERE user_id = (
        SELECT id FROM identities WHERE email = 'test@example.com'
    )
);
-- Expected: One row with available_balance = 0
```

---

## Final Steps

1. Complete code changes above (4-6 hours)
2. Run `make initDB`
3. Run tests (Step 1, 2, 3 above)
4. Verify all endpoints work
5. Update frontend using `FRONTEND_MIGRATION_BALANCE_CONSOLIDATION.md`
6. Deploy to staging
7. Full E2E testing

---

**Questions?** Refer to:
- `ACCOUNT_CENTRIC_MODEL.md` - Architecture
- `BALANCE_CONSOLIDATION_GUIDE.md` - Backend guide
- `IMPLEMENTATION_STATUS.md` - Current status
- `FRONTEND_MIGRATION_BALANCE_CONSOLIDATION.md` - Frontend guide

