# Account-Centric Financial Model

## Overview

Arda Credit uses an **account-centric** model for financial operations. Accounts are the primary entity for managing funds, with each account having exactly one Ethereum address (stored as `account_number`).

## Hierarchy

```
Organization/Company (org_id)
  └─ Account (account_id) - UUID
      └─ One Ethereum Address (account_number) - one-to-one relationship
          └─ Balances (by currency)
          └─ Deposits
          └─ Withdrawals
```

## Core Entities

### 1. **Accounts Table**

The `accounts` table is the primary entity:

```sql
accounts (
  account_id UUID PRIMARY KEY,           -- Main identifier
  org_id UUID,                           -- Owner organization (for company accounts)
  user_id UUID,                          -- Owner user (for individual accounts, backward compat)
  account_label VARCHAR(100),            -- Human-friendly name, e.g., "Operating USD"
  account_number VARCHAR(42) UNIQUE,     -- Ethereum address (one-to-one)
  account_type VARCHAR(50),              -- OPERATING, RESERVE, ESCROW, etc.
  currency VARCHAR(10) DEFAULT 'USDC',   -- Primary currency
  status VARCHAR(20) DEFAULT 'ACTIVE',   -- ACTIVE, INACTIVE, SUSPENDED
  opened_at TIMESTAMP,
  closed_at TIMESTAMP,
  created_at TIMESTAMP,
  updated_at TIMESTAMP
)
```

**Key Points:**
- `account_id` is a UUID (primary key)
- `account_number` is the Ethereum address (unique, one-to-one relationship)
- `status` controls whether account can perform operations
- Must have either `org_id` OR `user_id` (at least one owner)

### 2. **Balances Table**

Balances are keyed by `(account_id, currency)`:

```sql
balances (
  account_id UUID,                       -- Foreign key to accounts
  currency VARCHAR(10),                  -- USDC, ETH, etc.
  available_balance NUMERIC(78, 0),      -- Available for use
  locked_balance NUMERIC(78, 0),         -- Locked in deals/withdrawals
  total_deposited NUMERIC(78, 0),        -- Lifetime deposits
  total_withdrawn NUMERIC(78, 0),        -- Lifetime withdrawals
  deposit_count INTEGER,
  withdrawal_count INTEGER,
  PRIMARY KEY (account_id, currency)
)
```

### 3. **Deposits Table**

Deposits reference `account_id`:

```sql
deposits (
  id SERIAL PRIMARY KEY,
  account_id UUID,                       -- Account receiving deposit
  address VARCHAR(42),                   -- Specific address (same as account.account_number)
  amount NUMERIC(78, 0),
  currency VARCHAR(10),
  transaction_hash VARCHAR(66),
  status VARCHAR(20),                    -- pending, confirmed, processed, failed
  ...
)
```

### 4. **Withdrawals Table**

Withdrawals reference `account_id`:

```sql
withdrawals (
  id SERIAL PRIMARY KEY,
  account_id UUID,                       -- Account initiating withdrawal
  address VARCHAR(42),                   -- Source address (same as account.account_number)
  recipient_address VARCHAR(42),         -- Destination address
  amount NUMERIC(78, 0),
  currency VARCHAR(10),
  state VARCHAR(20),                     -- pending → validated → approved → submitted → confirmed
  request_source VARCHAR(20),            -- 'user' (UI) or 'contract' (direct)
  ...
)
```

## Automatic Account Creation

### For Users (Backward Compatibility)

When a user's `wallet_address` is set in the `identities` table, an account is **automatically created**:

```sql
-- Trigger: auto_create_account_for_user
-- Fires on: INSERT OR UPDATE OF wallet_address ON identities

-- Creates:
1. Account record with:
   - account_id: new UUID
   - user_id: identity.id
   - account_label: "{first_name} {last_name}" or "{email}" or "User Account"
   - account_number: wallet_address
   - account_type: "OPERATING"
   - status: "ACTIVE"

2. Balance record with:
   - account_id: new account_id
   - currency: "USDC"
   - all balances: 0
```

### For Organizations

Organizations will manually create accounts via API:

```
POST /api/v1/accounts
{
  "org_id": "company-uuid",
  "account_label": "Operating USD Account",
  "account_number": "0x742d35Cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1",
  "account_type": "OPERATING",
  "currency": "USDC"
}
```

## Account Status

### Status Values

- **ACTIVE**: Account can perform all operations
- **INACTIVE**: Account exists but cannot perform operations
- **SUSPENDED**: Account temporarily blocked (compliance, fraud, etc.)

### Status Validation

Account status is checked via triggers:

```sql
-- On withdrawal creation:
IF account.status = 'SUSPENDED' OR account.status = 'INACTIVE' THEN
  withdrawal.state = 'failed'
  withdrawal.error_message = 'Account is {status}'
  RAISE WARNING ...
END IF

-- On deposit creation:
IF account.status != 'ACTIVE' THEN
  RAISE WARNING (but allow deposit to be created)
END IF
```

**Design Decision**: Deposits are allowed even for non-ACTIVE accounts (to not lose funds), but withdrawals are blocked.

## API Endpoint Structure

All financial operations use **account_id**:

```
GET    /api/v1/account/{account_id}/balance
GET    /api/v1/account/{account_id}/deposits
GET    /api/v1/account/{account_id}/withdrawals
POST   /api/v1/account/{account_id}/withdrawals
DELETE /api/v1/account/{account_id}/withdrawals/{id}/cancel

// User convenience endpoints (aggregates across user's accounts)
GET    /api/v1/user/{user_id}/accounts
GET    /api/v1/user/{user_id}/balances

// Organization endpoints
GET    /api/v1/org/{org_id}/accounts
GET    /api/v1/org/{org_id}/balances
```

## Migration from Address-Centric to Account-Centric

### What Changed

**BEFORE** ❌ (Address-Centric):
```javascript
GET /api/v1/address/{address}/balance
GET /api/v1/address/{address}/deposits
```

**AFTER** ✅ (Account-Centric):
```javascript
GET /api/v1/account/{account_id}/balance
GET /api/v1/account/{account_id}/deposits
```

### Finding account_id from address

Since it's one-to-one, you can look up account by address:

```javascript
// Option 1: Client-side lookup
const account = await fetch(`/api/v1/accounts?address=${ethAddress}`);
const accountId = account.account_id;

// Option 2: Server provides account_id in user/wallet info
const user = await fetch(`/api/v1/user/{user_id}`);
const accountId = user.primary_account_id; // or user.accounts[0].account_id
```

### Convenience: Address-to-Account Lookup

We can add a convenience endpoint:

```
GET /api/v1/address/{address}/account
// Returns: { account_id, account_label, status, ... }
```

Then use returned `account_id` for subsequent calls.

## Implementation Impact

### Deposit Monitor

**OLD**:
```rust
create_deposit(pool, address, amount, ...)
```

**NEW**:
```rust
// create_deposit() now:
// 1. Looks up account_id by address
// 2. Creates orphan account if not found
// 3. Associates deposit with account_id
create_deposit(pool, address, amount, ...) // Same signature, different logic
```

### Withdrawal Service

**OLD**:
```rust
create_withdrawal_request(pool, user_address, recipient, amount, ...)
```

**NEW**:
```rust
// create_withdrawal_request() now:
// 1. Looks up account_id by address
// 2. Returns error if no account exists
// 3. Validates account status
create_withdrawal_request(pool, user_address, recipient, amount, ...)
```

### Balance Queries

**OLD**:
```rust
get_balance(pool, address, currency)
```

**NEW**:
```rust
get_balance(pool, account_id, currency)

// Or use convenience function:
let account = get_account_by_address(pool, address)?;
let balance = get_balance(pool, account.account_id, currency)?;
```

## Benefits of Account-Centric Model

1. **Business Alignment**: Accounts match real-world business entities
2. **Multi-Currency**: Each account naturally supports multiple currencies
3. **Organization Support**: Clear ownership (org vs. user)
4. **Account Labels**: Human-friendly names instead of just addresses
5. **Status Management**: Can suspend/inactivate accounts at business level
6. **Future Extensibility**: Can add account features (limits, permissions, etc.)

## Backward Compatibility

### For Existing Users

- When user links wallet address → account auto-created
- Existing code using `user.wallet_address` still works
- New account_id exposed in API responses for new flows

### For Frontend

- Frontend can continue using addresses for lookups
- Use address-to-account lookup endpoint
- Gradually migrate to using account_id directly

## Testing Account-Centric Flow

### Test Scenario 1: User Links Wallet

```
1. User registers → identity created (no wallet)
2. User links MetaMask → wallet_address set on identity
3. TRIGGER fires → account auto-created
4. Verify: accounts table has entry with account_number = wallet_address
5. Verify: balances table has entry for new account_id
```

### Test Scenario 2: Deposit Arrives Before Account

```
1. Deposit event detected at address 0x123...
2. No account exists for this address
3. create_deposit() auto-creates orphan account
4. Account labeled "Unlinked Account (0x123...)"
5. User can later claim this account by linking wallet
```

### Test Scenario 3: Account Status Blocking

```
1. Create account with status = 'ACTIVE'
2. Create withdrawal request → succeeds
3. Update account status = 'SUSPENDED'
4. Create withdrawal request → fails with error "Account is SUSPENDED"
5. Verify error logged in withdrawal.error_message
```

## Database Functions

### Account Functions

- `get_account_by_id(account_id)` - Get account by UUID
- `get_account_by_address(address)` - Get account by Ethereum address
- `get_accounts_by_user_id(user_id)` - Get all user accounts
- `get_accounts_by_org_id(org_id)` - Get all org accounts

### Balance Functions

- `get_balance(account_id, currency)` - Get balance for account
- `get_balances_for_account(account_id)` - Get all currencies
- `get_balances_by_user_id(user_id)` - Aggregate across user's accounts
- `get_balances_by_org_id(org_id)` - Aggregate across org's accounts

### Validation Functions

- `validate_account_active(account_id, operation)` - Throws if not active
- `get_available_balance(account_id, currency)` - Returns decimal (0 if none)
- `get_total_balance(account_id, currency)` - Returns available + locked

## Future Enhancements

With account-centric model, we can easily add:

1. **Account Limits**: Daily withdrawal limits, transaction limits
2. **Account Permissions**: Multi-signature requirements, approval workflows
3. **Account Types**: Specialized behavior for RESERVE vs OPERATING accounts
4. **Account Groups**: Related accounts under same organization
5. **Account Audit**: Track all operations per account
6. **Account Analytics**: Performance metrics per account

## Summary

The account-centric model provides a clean business abstraction over raw Ethereum addresses, enabling better organization, permissions, and future features while maintaining backward compatibility with existing user flows.

Key takeaway: **Always use `account_id` for operations, not addresses**. Addresses are implementation details stored in `account_number`.

