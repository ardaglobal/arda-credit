# Transaction Table Consolidation

## Overview

This document explains the consolidation of transaction tables in the Arda Credit system to provide a cleaner separation between **state transition functions** (ZK transactions) and **financial transactions** (money movement).

## Problem Statement

The system previously had multiple transaction tables with overlapping responsibilities:
- `arithmetic_transactions` - Demo/test arithmetic operations
- `incoming_transactions` - Legacy queue for pending arithmetic transactions
- `deal_transactions` - Deal lifecycle state transitions
- `account_financial_transactions` - User deposits/withdrawals
- `credit_line_financial_transactions` - Credit line operations (deprecated)
- `collateral_transactions` - Collateral lifecycle audit trail

This led to:
1. **State bloat** - Multiple tables tracking similar ZK state transitions
2. **Terminology confusion** - Unclear distinction between state transitions and financial transactions
3. **Code complexity** - Different APIs for similar operations

## Solution: Consolidated Architecture

### State Transactions (ZK State Transition Functions)

**Single Table**: `state_transactions`

Consolidates all zero-knowledge state transition function transactions into a unified audit trail.

**Schema:**
```sql
CREATE TABLE state_transactions (
    id UUID PRIMARY KEY,
    transaction_id_hash BYTEA NOT NULL UNIQUE,  -- SHA256 for zkVM
    transaction_type VARCHAR(50) NOT NULL,       -- See TransactionType enum
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
    
    -- Hashed identifiers (for zkVM privacy)
    user_id_hash BYTEA,
    deal_id_hash BYTEA,
    
    -- Foreign key references (for database queries)
    user_id UUID REFERENCES users(id),
    deal_id UUID REFERENCES deals(id),
    payment_id UUID REFERENCES deal_payments(id),
    
    -- Batch processing
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    
    -- Metadata
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

**Transaction Types:**
- User lifecycle: `user_registration`, `kyc_submission`, `kyc_verification`, `investor_registration`
- Company lifecycle: `company_registration`, `company_update`
- Deal lifecycle: `deal_origination`, `deal_payment`, `deal_closeout`
- Financial operations: `user_deposit`, `user_withdrawal`
- Document operations: `verify_document`
- Collateral operations: `collateral_creation`, `collateral_update`, `collateral_deal_association`, etc.
- State management: `state_regenesis`

**Key Features:**
- Unified audit trail for all ZK state transitions
- Privacy-preserving via hashed identifiers
- Batch processing integration for ZK proofs
- Flexible metadata for transaction-specific data

### Financial Transactions (Money Movement)

Financial transactions remain separate from state transitions with clear terminology:

#### 1. Deposits & Withdrawals
**Table**: `account_financial_transactions`
**Alias View**: `deposits_withdrawals`

User interactions with the system (user ↔ system):
- **DEPOSIT**: User deposits USDC into the system
- **WITHDRAWAL**: User withdraws USDC from the system

```sql
CREATE TABLE account_financial_transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    transaction_type VARCHAR(20) NOT NULL,  -- DEPOSIT, WITHDRAWAL
    amount DECIMAL(20,6) NOT NULL,
    currency VARCHAR(10) DEFAULT 'USDC',
    transaction_hash VARCHAR(66) NOT NULL,
    blockchain_status VARCHAR(20) DEFAULT 'CONFIRMED',
    block_number BIGINT,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

#### 2. Payments
**Table**: `deal_payments`
**Alias View**: `payments`

Money movement between accounts (account ↔ account):
- Deal-related payments
- Inter-account transfers

```sql
CREATE TABLE deal_payments (
    id UUID PRIMARY KEY,
    deal_id UUID NOT NULL,
    payer_id UUID NOT NULL,
    amount DECIMAL(18, 6) NOT NULL,
    payment_type VARCHAR(20) NOT NULL,  -- regular, principal, interest, penalty
    transaction_hash VARCHAR(66),
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

#### 3. Deprecated Tables
- `credit_line_financial_transactions` - Deprecated (merchants removed from system)
- `arithmetic_transactions` - Kept for backward compatibility (test/demo only)

### Audit Trail Tables

Separate audit trail tables for specific entities:

#### Collateral Transactions
**Table**: `collateral_transactions`

Lifecycle events for collateral assets:
```sql
CREATE TABLE collateral_transactions (
    transaction_id UUID PRIMARY KEY,
    collateral_id UUID NOT NULL,
    transaction_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    deal_id UUID,
    metadata JSONB DEFAULT '{}'
);
```

## Migration Strategy

### Phase 1: Create New Infrastructure (✅ Completed)

**Migration**: `079_consolidate_transaction_tables.sql`

1. Create `state_transactions` table
2. Migrate data from `deal_transactions`
3. Create backward compatibility views:
   - `deal_transactions_view` (maps to state_transactions)
   - `incoming_transactions_view` (maps to state_transactions with type=arithmetic_operation)
   - `deposits_withdrawals` (alias for account_financial_transactions)
   - `payments` (alias for deal_payments)
4. Add helper functions:
   - `get_user_state_transactions(user_id, limit, offset)`
   - `get_deal_state_transactions(deal_id, limit)`
   - `get_pending_state_transactions(limit)`

### Phase 2: Update Application Code (✅ Completed)

1. **Database Layer** (`db/src/`):
   - Add `StateTransaction` struct
   - Add state transaction functions:
     - `create_state_transaction()`
     - `get_pending_state_transactions()`
     - `get_deal_state_transactions()`
     - `get_user_state_transactions()`
     - `mark_state_transactions_batched()`
     - `create_batch_from_state_transactions()`
   - Update exports in `lib.rs`

2. **API Layer** (`api/src/services/`):
   - Update `transaction_recording.rs` to use `state_transactions`
   - Update `batch_converter.rs` to query `state_transactions`
   - Update `state_builder.rs` to use `state_transactions`

3. **Library** (`lib/src/transactions.rs`):
   - Add comprehensive documentation explaining consolidation
   - Clarify distinction between state and financial transactions

### Phase 3: Testing & Validation (⏭️ Next)

1. Run database migration: `make initDB`
2. Run test suite: `make test`
3. Verify backward compatibility via views
4. Check API endpoints still function correctly

### Phase 4: Deprecation (Future)

Once all code is updated and tested:

1. Drop old tables:
   ```sql
   DROP TABLE deal_transactions;
   DROP TABLE incoming_transactions;
   ```
2. Optionally rename views to replace tables:
   ```sql
   DROP VIEW deposits_withdrawals;
   ALTER TABLE account_financial_transactions RENAME TO deposits_withdrawals;
   ```

## Benefits

### 1. Clean Separation of Concerns

**State Transactions:**
- All ZK state transition functions in one place
- Single API for recording state changes
- Unified batch processing

**Financial Transactions:**
- Clear distinction: Deposits, Withdrawals, Payments
- Separate from state transition logic
- Easier to understand and audit

### 2. Improved Code Clarity

**Before:**
```rust
// Confusing - which table?
insert_into_deal_transactions(...)
insert_into_incoming_transactions(...)
```

**After:**
```rust
// Clear purpose
create_state_transaction(...)        // For ZK state transitions
record_deposit(...)                  // For financial deposits
record_payment(...)                  // For financial payments
```

### 3. Better Database Organization

**Before:**
```
├── arithmetic_transactions (test)
├── incoming_transactions (queue)
├── deal_transactions (state)
├── account_financial_transactions (deposit/withdrawal)
└── credit_line_financial_transactions (deprecated)
```

**After:**
```
State Transitions (ZK):
  └── state_transactions (consolidated)

Financial Operations:
  ├── account_financial_transactions (deposits/withdrawals)
  └── deal_payments (payments)

Audit Trails:
  └── collateral_transactions (collateral lifecycle)
```

### 4. Simplified API Development

New transaction types only need to be added in one place:
1. Add to `TransactionType` enum
2. Record in `state_transactions` table
3. Automatic batching and proof generation

## API Usage Examples

### Recording State Transactions

```rust
use arda_credit_lib::TransactionType;

// Record a deal origination state transaction
let tx_id = tx_service.record_transaction(
    TransactionType::DealOrigination,
    user_id,
    Some(deal_id),
    principal_amount,
    Some(json!({
        "investor_id": investor_id.to_string(),
        "interest_rate_bps": 500,
    })),
).await?;
```

### Querying State Transactions

```rust
// Get all state transactions for a deal
let transactions = get_deal_state_transactions(&pool, deal_id).await?;

// Get pending transactions awaiting batch processing
let pending = get_pending_state_transactions(&pool, Some(100)).await?;

// Get user's transaction history
let user_txs = get_user_state_transactions(&pool, user_id, Some(50), Some(0)).await?;
```

### Creating Batches

```rust
// Create batch from pending state transactions
let batch = create_batch_from_state_transactions(&pool, 100, None).await?;

// Mark transactions as batched
mark_state_transactions_batched(&pool, &tx_ids, batch_id).await?;
```

## Database Functions

### get_user_state_transactions
Get transaction history for a user with batch information:
```sql
SELECT * FROM get_user_state_transactions(
    p_user_id := 'user-uuid',
    p_limit := 100,
    p_offset := 0
);
```

### get_deal_state_transactions
Get complete audit trail for a deal:
```sql
SELECT * FROM get_deal_state_transactions(
    p_deal_id := 'deal-uuid',
    p_limit := 100
);
```

### get_pending_state_transactions
Get unbatched transactions ready for processing:
```sql
SELECT * FROM get_pending_state_transactions(
    p_limit := 100
);
```

## Backward Compatibility

Views maintain compatibility with existing code:

```sql
-- Old code using deal_transactions still works
SELECT * FROM deal_transactions WHERE deal_id = 'uuid';

-- New code uses state_transactions
SELECT * FROM state_transactions WHERE deal_id = 'uuid';

-- Both return the same data (view maps to new table)
```

## Future Enhancements

1. **Audit Trail API**: REST endpoints for querying state transaction history
2. **State Snapshots**: Periodic checkpoints of full state at batch boundaries
3. **Transaction Replay**: Rebuild state by replaying state_transactions
4. **Analytics**: Aggregate queries over consolidated transaction data
5. **Real-time Monitoring**: WebSocket subscriptions to state transaction stream

## See Also

- [Architecture Overview](three-component-overview.md)
- [ZK Architecture](../zk/architecture.md)
- [Database Migrations](../../db/migrations/)
- [Transaction Types](../../lib/src/transactions.rs)
