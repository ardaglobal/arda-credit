# zkVM Integration Architecture

**Version:** 1.0  
**Last Updated:** December 12, 2024  
**Status:** Documentation

---

## Glossary

**zkVM (Zero-Knowledge Virtual Machine)** - The SP1 execution environment that generates zero-knowledge proofs. Runs deterministic code in a constrained environment.

**Business State (GlobalLoanState)** - In-memory HashMap-based state containing users, deals, and balances. Validates business logic during zkVM execution.

**Cryptographic State (ZkVMAds)** - Indexed Merkle Tree (IMT) Authenticated Data Structure. Generates state root commitments for on-chain verification.

**IMT (Indexed Merkle Tree)** - A specialized Merkle tree that supports efficient insertion and membership proofs. Used as our ADS implementation.

**ADS (Authenticated Data Structure)** - A data structure that allows proving properties about data cryptographically. Our IMT is the specific ADS implementation.

**State Roots** - Cryptographic commitments (32-byte hashes) representing the entire state of the IMT. Posted to smart contracts for verification.

**Nullifier** - A unique cryptographic commitment for each transaction. Prevents double-spending and ensures transactions can't be replayed.

**Critical Insight:** The zkVM maintains TWO states in parallel:
- **GlobalLoanState** (business state) = What happened (users created, deals made, balances changed)
- **ZkVMAds/IMT** (cryptographic state) = Cryptographic proof that it happened validly

These are complementary, not equivalent. Business state validates logic; cryptographic state provides proofs.

---

## Table of Contents

1. [Core Constraint: zkVM Isolation](#core-constraint-zkvm-isolation)
2. [Three-Layer Architecture](#three-layer-architecture)
3. [Complete Example: User Registration](#complete-example-user-registration)
4. [Dual State Concept](#dual-state-concept)
5. [Validation: Where It Lives](#validation-where-it-lives)
6. [Complete Flow Diagram](#complete-flow-diagram)
7. [Eventual Consistency Model](#eventual-consistency-model)
8. [Implementation Guidelines](#implementation-guidelines)

---

## Core Constraint: zkVM Isolation

The zkVM (Zero-Knowledge Virtual Machine) has strict architectural requirements that fundamentally shape our system design. The `lib/` directory contains code that runs inside the SP1 zkVM, which imposes the following constraints:

### The Two States in zkVM

During proof generation, the zkVM maintains **TWO distinct but complementary states**:

#### 1. Business State (GlobalLoanState)

**Structure:**
```rust
pub struct GlobalLoanState {
    pub users: HashMap<[u8; 32], User>,
    pub loans: HashMap<[u8; 32], Deal>,
    pub collateral: HashMap<[u8; 32], Collateral>,  // To be added
    pub balance_state: BalanceState,
    pub loan_terms: LoanTerms,
    pub transaction_count: u64,
    pub last_updated: u64,
}
```

**Purpose:** Validates business logic
- Does user exist?
- Is user KYC approved?
- Does user have sufficient balance?
- Is deal status valid for this operation?

**Example:** "User Alice (0x1234...) exists with $100 balance, Deal X (0xabcd...) has $50 outstanding balance"

#### 2. Cryptographic State (ZkVMAds - Indexed Merkle Tree)

**Structure:**
```rust
pub struct ZkVMAds {
    pub tree_height: u32,
    pub nullifiers: Vec<ZkVMNullifier>,
    pub nullifier_count: u64,
    pub next_available_index: u64,
}
```

**Purpose:** Generates cryptographic commitments (state roots)
- Inserts nullifiers for each transaction
- Computes Merkle tree roots
- Provides cryptographic proof of state

**Example:** "Current state root is 0xf1a2b3c4..., proving all transactions up to this point"

#### How They Work Together

```rust
// From program/src/main.rs
pub fn main() {
    let input = sp1_zkvm::io::read::<ZkVMBatchInput>();
    
    // Process business logic with GlobalLoanState
    let final_deal_state = process_deal_batch(
        input.initial_loan_state,  // ← Business state
        &input.batch
    ).expect("Failed to process deal batch");
    
    // Process cryptographic commitments with ZkVMAds/IMT
    let mut ads_state = input.initial_ads_state;  // ← Cryptographic state
    let state_transition = ads_state
        .process_transactions(&input.batch.transactions)
        .expect("Failed to process ADS state transition");
    
    // Output combines data from BOTH states:
    let public_values = LoanBatchPublicValues {
        previous_state_root: state_transition.old_root,    // From ZkVMAds
        new_state_root: state_transition.new_root,          // From ZkVMAds
        transaction_count: batch.transactions.len(),        // From both
        previousTotalBalance: initial_state.compute_total_user_balance(),  // From GlobalLoanState
        currentTotalBalance: final_state.compute_total_user_balance(),     // From GlobalLoanState
        // ...
    };
}
```

**Analogy:**
- **GlobalLoanState** = The ledger (records what transactions occurred and their effects)
- **ZkVMAds/IMT** = The notary seal (cryptographic proof the ledger is valid and unaltered)

Both are essential. The ledger tells you what happened; the seal proves it's legitimate.

### What lib/ Must Be

✅ **Pure State Transitions**
- Functions must be deterministic
- Same inputs always produce same outputs
- No random number generation
- No system time dependencies (timestamps must be inputs)

✅ **In-Memory Only**
- All state stored in HashMap and Vec data structures
- No file system access
- No persistent storage
- State exists only during proof generation

✅ **No External I/O**
- No database connections
- No network requests
- No file reads/writes
- No environment variable access

✅ **Reproducible**
- Anyone with the same inputs must get the same outputs
- This is critical for proof verification
- Enables independent verification of state transitions

### Why These Constraints Exist

The SP1 zkVM needs to:
1. **Prove correctness** - Every operation must be traceable and verifiable
2. **Run efficiently** - The proof generation process is computationally intensive
3. **Be deterministic** - Non-determinism breaks cryptographic proofs
4. **Work offline** - Proofs can be generated and verified independently

### Practical Implications

```rust
// ❌ WRONG - This cannot work in zkVM
pub fn process_transaction(tx: &Transaction) -> Result<()> {
    // Database access not allowed
    let user = sqlx::query!("SELECT * FROM users WHERE id = $1", tx.user_id)
        .fetch_one(&pool)
        .await?;
    
    // Network access not allowed
    let price = fetch_price_from_api().await?;
    
    // System time not allowed
    let now = SystemTime::now();
    
    Ok(())
}

// ✅ CORRECT - Pure in-memory state transition
pub fn process_transaction(
    state: &mut GlobalLoanState,
    tx: &LoanTransaction,
) -> Result<()> {
    // Check in-memory state
    let user = state.users.get(&tx.user_id)
        .ok_or(UserError::UserNotFound)?;
    
    // All data comes from inputs
    let balance = state.balance_state.get_balance(&tx.user_id);
    
    // Timestamp comes from transaction (input)
    let timestamp = tx.created_at;
    
    // Pure state mutation
    state.users.insert(tx.user_id, updated_user);
    
    Ok(())
}
```

---

## Three-Layer Architecture

Our system separates concerns across three distinct layers, each with specific responsibilities and constraints.

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: API Handler (api/src/handlers/)                  │
│  - HTTP request validation                                  │
│  - Authentication & authorization                           │
│  - Database writes (actual persistence)                     │
│  - Creates deal_transactions records                        │
│  - Response formatting                                      │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: Business Logic Bridge (api/src/services/)        │
│  - Orchestrates between API and zkVM                        │
│  - Converts database models to zkVM types                   │
│  - Transaction recording service                            │
│  - Does NOT execute zkVM logic directly                     │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: zkVM State Transitions (lib/src/)                │
│  - Pure state transition functions                          │
│  - Validation against in-memory state only                  │
│  - Balance calculations                                      │
│  - NO side effects (no DB, no I/O, no network)             │
└─────────────────────────────────────────────────────────────┘
```

### Layer 1: API Handler

**Location:** `api/src/handlers/`

**Responsibilities:**

1. **HTTP Request Validation**
   - Parse and validate request body
   - Check parameter formats (UUIDs, decimals, etc.)
   - Validate required fields

2. **Authentication & Authorization**
   - Verify user session/token
   - Check permissions (can user perform this action?)
   - Enforce access control rules

3. **Database Operations**
   - Write to business tables (users, deals, payments, collateral)
   - Query for business logic (does user exist? does deal exist?)
   - Update records as needed

4. **Transaction Recording**
   - Call TransactionRecordingService to create deal_transactions record
   - This record will be processed by zkVM later

5. **Response Formatting**
   - Return HTTP response with appropriate status codes
   - Format data for client consumption

**Key Insight:** The API layer writes to BOTH business tables AND the deal_transactions table. Business tables store the actual data; deal_transactions stores the audit trail for zkVM.

### Layer 2: Business Logic Bridge

**Location:** `api/src/services/`

**Responsibilities:**

1. **Transaction Recording Service**
   - **Implementation:** `api/src/services/transaction_recording.rs`
   - Creates deal_transactions records
   - Hashes UUIDs using SHA256 for privacy
   - Encodes metadata for zkVM consumption

2. **Data Conversion**
   - Converts database models (with UUIDs) to zkVM models (with hashes)
   - Prepares transaction batches
   - Handles serialization/deserialization

3. **Batch Orchestration**
   - Fetches pending transactions
   - Groups transactions into batches
   - Manages batch lifecycle (pending → processing → proven)

4. **Does NOT Execute zkVM**
   - This layer prepares inputs but doesn't run the zkVM
   - zkVM execution happens in background batch processor
   - Keeps concerns separated

### Layer 3: zkVM State Transitions

**Location:** `lib/src/`

**Responsibilities:**

1. **Pure State Transitions**
   ```rust
   pub fn process_transaction(
       &mut self,
       transaction: &LoanTransaction,
   ) -> Result<(), ProcessingError>
   ```

2. **In-Memory State Management**
   - GlobalLoanState contains HashMap of users, deals, balances
   - All state lives in RAM during proof generation
   - State is reconstructed from scratch for each batch

3. **Validation**
   - Check if user exists in zkVM state
   - Verify user has KYC approval in zkVM state
   - Ensure sufficient balance in zkVM state
   - Validate deal status transitions

4. **Balance Calculations**
   - Track user balances
   - Apply deposits, withdrawals, payments
   - Ensure balance conservation (system invariant)

**Key Files:**
- `lib/src/deal_processor.rs` - Main transaction processing
- `lib/src/user.rs` - User state management
- `lib/src/balance.rs` - Balance tracking
- `lib/src/deal.rs` - Deal state management
- `lib/src/transactions.rs` - Transaction type definitions

---

## Complete Example: User Registration

Let's trace a complete user registration through all three layers.

### Layer 1: API Handler

**File:** `api/src/handlers/users/users.rs`

```rust
use crate::services::transaction_recording::TransactionRecordingService;

#[instrument(skip(state), level = "info")]
pub async fn create_user_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    // 1. HTTP-level validation
    if request.email.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Email is required".to_string()));
    }
    
    validate_email_format(&request.email)?;
    
    // 2. Check business rules (database query OK here)
    let existing = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        request.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "User with this email already exists".to_string()
        ));
    }
    
    // 3. Create user in database (actual persistence)
    let user_id = Uuid::new_v4();
    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (id, email, user_type, kyc_status, created_at, updated_at)
        VALUES ($1, $2, $3, 'not_submitted', NOW(), NOW())
        RETURNING *
        "#,
        user_id,
        request.email,
        request.user_type,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // 4. Record transaction for zkVM batch processing
    let tx_service = TransactionRecordingService::new(state.pool.clone());
    tx_service.record_transaction(
        TransactionType::UserRegistration,
        user_id,
        None, // no deal_id for user registration
        Decimal::ZERO, // no amount
        Some(json!({
            "email": request.email,
            "user_type": request.user_type,
        })),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // 5. Return response
    let duration = start_time.elapsed();
    info!("User created in {:?}: {}", duration, user_id);
    
    Ok(Json(UserResponse {
        user_id: user.id.to_string(),
        email: user.email,
        user_type: user.user_type,
        kyc_status: user.kyc_status,
        created_at: user.created_at,
    }))
}
```

### Layer 2: Transaction Recording Service

**File:** `api/src/services/transaction_recording.rs`

```rust
use sha2::{Digest, Sha256};

pub struct TransactionRecordingService {
    pool: PgPool,
}

impl TransactionRecordingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn record_transaction(
        &self,
        transaction_type: TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: Decimal,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, DbError> {
        // 1. Generate transaction_id
        let transaction_id = Uuid::new_v4();
        
        // 2. Hash UUIDs with SHA256 for zkVM (privacy)
        let transaction_id_hash = Self::hash_uuid(&transaction_id);
        let user_id_hash = Self::hash_uuid(&user_id);
        let deal_id_hash = deal_id.map(|id| Self::hash_uuid(&id));
        
        // 3. Insert into deal_transactions table
        sqlx::query!(
            r#"
            INSERT INTO deal_transactions (
                id, transaction_id_hash, user_id_hash, deal_id_hash,
                transaction_type, amount, user_id, deal_id,
                metadata, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            "#,
            transaction_id,
            transaction_id_hash,
            user_id_hash,
            deal_id_hash,
            transaction_type.to_string(),
            amount,
            user_id,
            deal_id,
            metadata,
        )
        .execute(&self.pool)
        .await?;
        
        info!(
            "Recorded {} transaction: {} for user {}",
            transaction_type, transaction_id, user_id
        );
        
        // 4. Return transaction_id
        Ok(transaction_id)
    }
    
    fn hash_uuid(uuid: &Uuid) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(uuid.as_bytes());
        hasher.finalize().to_vec()
    }
}
```

### Layer 3: zkVM State Transition

**File:** `lib/src/user.rs` (already exists!)

```rust
impl UserState {
    /// Process user registration transaction
    ///
    /// State transition: None → User exists
    /// Validation: User must NOT already exist
    pub fn process_user_registration(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), UserError> {
        let user_id = transaction.user_id;
        
        // 1. Validation: User must NOT already exist in zkVM state
        if self.users.contains_key(&user_id) {
            return Err(UserError::UserAlreadyExists { user_id });
        }
        
        // 2. Extract user_type from metadata
        let metadata = transaction.metadata.as_bytes();
        let user_type_byte = metadata[0];
        let user_type = match user_type_byte {
            1 => UserType::Investor,
            2 => UserType::Originator,
            _ => return Err(UserError::InvalidUserType),
        };
        
        // 3. State transition: Create user in zkVM HashMap
        let user = User {
            user_id,
            user_type,
            kyc_status: KycStatus::NotSubmitted,
            created_at: transaction.created_at,
        };
        
        self.users.insert(user_id, user);
        
        info!(
            "zkVM: User registered: {:?}, type: {:?}",
            user_id, user_type
        );
        
        // 4. Return success
        Ok(())
    }
}
```

### What Happens Next

After the API handler returns, the transaction sits in `deal_transactions` with `included_in_batch_id = NULL`. Then:

1. **Background batch processor** runs (every 60 seconds, or when 10+ pending transactions)
2. **Fetches pending transactions** from deal_transactions table
3. **Converts to zkVM format** (LoanTransaction with hashed IDs)
4. **Creates batch** and updates `included_in_batch_id`
5. **Executes zkVM** which calls `process_user_registration()`
6. **Generates proof** via Sindri
7. **Posts to smart contract**

---

## Dual State Concept

A key architectural insight: **state exists in two places**, and they serve different purposes.

### Database State (PostgreSQL)

**Purpose:** Source of truth for business operations

**Characteristics:**
- Persistent and durable
- Queryable via SQL
- Used by API for all business logic
- Can be indexed for performance
- Supports complex queries (JOINs, aggregations)
- Mutable (can be updated in place)

**Tables:**
- `users` - User accounts with KYC status
- `deals` - Deal records with terms and balances
- `payments` - Payment history
- `collateral` - Collateral assets
- `deal_transactions` - Audit trail for zkVM

**Access Pattern:**
```rust
// API queries database directly
let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id)
    .fetch_one(&pool)
    .await?;

// Business logic uses database state
if user.kyc_status == "approved" {
    // Allow action
}
```

### zkVM State (In-Memory)

**Purpose:** Prove that database state transitions were valid

**Characteristics:**
- Ephemeral (exists only during proof generation)
- Reconstructed from scratch for each batch
- Immutable during proof (cannot be changed)
- Only accessible via zkVM execution
- Provides cryptographic proof of correctness

**Data Structures:**
```rust
pub struct GlobalLoanState {
    pub users: HashMap<[u8; 32], User>,
    pub loans: HashMap<[u8; 32], Deal>,
    pub balance_state: BalanceState,
    pub loan_terms: LoanTerms,
    pub transaction_count: u64,
    pub last_updated: u64,
}
```

**Access Pattern:**
```rust
// zkVM reconstructs state from transactions
let mut state = GlobalLoanState::new();

for transaction in batch.transactions {
    state.process_transaction(&transaction)?;
}

// State now reflects all transactions
// Can compute proofs about this state
```

### Why Two States?

The zkVM doesn't replace the database - it **proves the database is correct**!

```
Database State: "Alice has $100"
                     ↓
zkVM Proof: "I can prove that Alice legitimately received $100
            through valid transactions, and no other user lost
            that money without authorization"
```

**Think of it like double-entry bookkeeping:**
- Database = The ledger (what happened)
- zkVM = The audit (proof it was done correctly)

### State Synchronization

The two states stay synchronized through the transaction log:

```
1. User action → Database write + Transaction record
2. Batch processor → Collects transaction records
3. zkVM → Replays transactions, rebuilds state
4. Proof → Confirms database state is valid
```

If database and zkVM disagree, the zkVM will reject the transaction when generating the proof.

---

## Validation: Where It Lives

Validation happens at two different layers, and understanding when to use each is critical.

### Business Validation (API Layer - Layer 1)

**Can Access:** Database, external APIs, filesystem

**Examples:**

1. **User Existence Check**
   ```rust
   // Check if user already exists (prevents duplicates)
   let existing = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
       .fetch_optional(&pool)
       .await?;
   
   if existing.is_some() {
       return Err("User already exists");
   }
   ```

2. **Email Format Validation**
   ```rust
   fn validate_email(email: &str) -> Result<()> {
       let email_regex = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")?;
       if !email_regex.is_match(email) {
           return Err("Invalid email format");
       }
       Ok(())
   }
   ```

3. **Permission Checks**
   ```rust
   // Check if user has permission to create deal
   let membership = sqlx::query!(
       "SELECT role FROM company_memberships 
        WHERE user_id = $1 AND company_id = $2",
       user_id, company_id
   )
   .fetch_one(&pool)
   .await?;
   
   if membership.role != "admin" {
       return Err("Insufficient permissions");
   }
   ```

4. **Balance Check (Optimistic)**
   ```rust
   // Check user has sufficient balance (database state)
   let balance = get_user_balance(&pool, user_id).await?;
   if balance < withdrawal_amount {
       return Err("Insufficient balance"); // Reject early
   }
   ```

**When to Use:** 
- Before writing to database
- To prevent obviously invalid operations
- To provide immediate feedback to users
- For data that zkVM can't access (external APIs, complex queries)

### State Transition Validation (zkVM Layer - Layer 3)

**Can Access:** ONLY in-memory zkVM state

**Examples:**

1. **User Existence in zkVM State**
   ```rust
   // Check if user exists in zkVM state (NOT database)
   if !self.users.contains_key(&transaction.user_id) {
       return Err(ProcessingError::UserNotFound);
   }
   ```

2. **KYC Status in zkVM State**
   ```rust
   // Check KYC status from zkVM state
   let user = self.users.get(&transaction.user_id)
       .ok_or(ProcessingError::UserNotFound)?;
   
   if !user.is_kyc_approved() {
       return Err(ProcessingError::KycNotApproved);
   }
   ```

3. **Balance Check (Authoritative)**
   ```rust
   // Check balance from zkVM state (reconstructed from all transactions)
   let balance = self.balance_state.get_balance(&transaction.user_id);
   if balance < withdrawal_amount {
       return Err(ProcessingError::InsufficientBalance {
           requested: withdrawal_amount,
           available: balance,
       });
   }
   ```

4. **Deal Status Validation**
   ```rust
   // Check deal status in zkVM state
   let deal = self.loans.get(&transaction.deal_id)
       .ok_or(ProcessingError::DealNotFound)?;
   
   if deal.deal_status != LoanStatus::Active {
       return Err(ProcessingError::InvalidDealStatus);
   }
   ```

**When to Use:**
- During zkVM execution (batch processing)
- To prove transactions are valid
- For cryptographic guarantees
- When reconstructing state from transaction history

### Validation Overlap and Conflicts

Sometimes both layers validate the same thing, and they can disagree!

**Example Scenario:**

```
Time T1: User has $100 balance (database state)
         User makes payment: $50
         API checks: $100 >= $50 ✅ PASS
         Creates transaction record
         Returns success to user

Time T2: User makes another payment: $60
         API checks: $100 >= $60 ✅ PASS
         Creates transaction record  
         Returns success to user

Time T3: Batch processor runs
         zkVM state: Start with $0
         Process first transaction: $0 + $100 (deposit) = $100
         Process payment 1: $100 - $50 = $50 ✅
         Process payment 2: $50 - $60 = ??? ❌ FAIL!
         
         zkVM rejects second transaction!
```

This is **eventual consistency** - the API made optimistic decisions, but zkVM has final authority.

---

## Complete Flow Diagram

Here's the complete sequence from user action to on-chain proof:

```
┌─────────────────────────────────────────────────────────────────┐
│ TIME T1: User Action (HTTP Request)                             │
│ POST /api/v1/user { email: "alice@example.com", type: "investor" } │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: API Handler (api/src/handlers/users/users.rs)         │
│                                                                  │
│ 1. Validate email format ✅                                     │
│ 2. Query database: Does user exist?                             │
│    SELECT * FROM users WHERE email = 'alice@example.com'       │
│    Result: NOT FOUND ✅                                         │
│ 3. INSERT INTO users (id, email, user_type, ...)               │
│    Result: user_id = uuid-abc-123                               │
│ 4. Call TransactionRecordingService:                            │
│    record_transaction(UserRegistration, uuid-abc-123, ...)     │
│ 5. Return HTTP 201 Created                                      │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: Transaction Recording Service                          │
│ (api/src/services/transaction_recording.rs)                    │
│                                                                  │
│ 1. Generate transaction_id = uuid-tx-789                        │
│ 2. Hash UUIDs:                                                  │
│    - transaction_id_hash = SHA256(uuid-tx-789)                  │
│    - user_id_hash = SHA256(uuid-abc-123)                        │
│ 3. INSERT INTO deal_transactions (                              │
│      id = uuid-tx-789,                                          │
│      transaction_id_hash = 0xabcd...,                           │
│      user_id_hash = 0x1234...,                                  │
│      transaction_type = 'user_registration',                    │
│      metadata = { "email": "alice@...", "user_type": "..." },  │
│      included_in_batch_id = NULL  ← PENDING                    │
│    )                                                            │
│ 4. Return transaction_id                                        │
└─────────────────────────────────────────────────────────────────┘
                            ↓
                    Database State:
                    users: [Alice]
                    deal_transactions: [UserReg(Alice)] ← PENDING
                            ↓
                    Time passes...
                    (seconds to minutes)
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ TIME T2: Background Batch Processor Runs                        │
│ Triggered by: Timer (60s) OR Count (10+ txs) OR Manual         │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Batch Service (api/src/services/unified_batch_service.rs)      │
│                                                                  │
│ 1. Fetch pending transactions:                                  │
│    SELECT * FROM deal_transactions                              │
│    WHERE included_in_batch_id IS NULL                           │
│    LIMIT 50                                                      │
│    Result: [UserReg(Alice)]                                     │
│                                                                  │
│ 2. Create batch:                                                │
│    INSERT INTO proof_batches (id, ...) VALUES (42, ...)        │
│    batch_id = 42                                                │
│                                                                  │
│ 3. Update transactions:                                          │
│    UPDATE deal_transactions                                      │
│    SET included_in_batch_id = 42                                │
│    WHERE id = uuid-tx-789                                       │
│                                                                  │
│ 4. Convert to zkVM format:                                      │
│    LoanTransaction {                                            │
│      transaction_id: 0xabcd...,  ← hashed                      │
│      user_id: 0x1234...,         ← hashed                      │
│      deal_id: [0; 32],           ← zeros (no deal)             │
│      transaction_type: UserRegistration,                        │
│      amount: 0,                                                 │
│      created_at: timestamp,                                     │
│      metadata: [user_type_byte, ...],                          │
│    }                                                            │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: zkVM Execution (program/src/main.rs → lib/src/)       │
│                                                                  │
│ Input: ZkVMBatchInput {                                         │
│   initial_loan_state: GlobalLoanState {                         │
│     users: {},      ← Empty HashMap initially                  │
│     deals: {},                                                  │
│     balance_state: {},                                          │
│     transaction_count: 0,                                       │
│   },                                                            │
│   initial_ads_state: ZkVMAds {                                 │
│     merkle_tree: [...],                                         │
│   },                                                            │
│   batch: DealBatch {                                            │
│     batch_id: [42],                                             │
│     transactions: [UserReg(Alice)],                             │
│   }                                                             │
│ }                                                               │
│                                                                  │
│ Processing:                                                      │
│ 1. validate_transaction(&UserReg(Alice))                        │
│    ✅ Amount is 0 (valid for user registration)                │
│    ✅ deal_id is all zeros (valid for non-deal tx)            │
│                                                                  │
│ 2. process_transaction(&UserReg(Alice))                         │
│    ├─ Match: TransactionType::UserRegistration                 │
│    ├─ Check: users.contains_key(alice_id)? → No ✅            │
│    ├─ Extract: user_type from metadata → Investor              │
│    └─ Insert: users.insert(alice_id, User { ... })            │
│                                                                  │
│ 3. Update ADS Merkle tree:                                      │
│    ├─ old_root = hash(empty_state)                             │
│    ├─ Insert nullifier for user registration                    │
│    └─ new_root = hash(state_with_alice)                        │
│                                                                  │
│ Output: LoanBatchPublicValues {                                 │
│   previous_state_root: 0x0000...,                              │
│   new_state_root: 0xf1a2...,                                   │
│   transaction_count: 1,                                         │
│   batch_timestamp: T2,                                          │
│   transaction_types_hash: hash([UserRegistration]),            │
│   previousTotalBalance: 0,                                      │
│   currentTotalBalance: 0,                                       │
│ }                                                               │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Proof Generation (via Sindri Service)                           │
│                                                                  │
│ 1. Submit proof request to Sindri:                             │
│    POST https://sindri.app/api/v1/proof                        │
│    Body: {                                                      │
│      program_id: "arda-credit-sp1",                             │
│      inputs: ZkVMBatchInput,                                    │
│    }                                                            │
│                                                                  │
│ 2. Sindri generates SP1 proof:                                 │
│    - Compiles zkVM program                                      │
│    - Executes with inputs                                       │
│    - Generates cryptographic proof                              │
│    - Takes ~30 seconds to 2 minutes                            │
│                                                                  │
│ 3. Poll for completion:                                         │
│    GET https://sindri.app/api/v1/proof/{proof_id}/status       │
│    Status: "Pending" → "Ready"                                 │
│                                                                  │
│ 4. Retrieve proof data:                                         │
│    GET https://sindri.app/api/v1/proof/{proof_id}              │
│    Response: {                                                  │
│      proof_id: "proof_abc123",                                 │
│      status: "Ready",                                           │
│      proof_bytes: 0x...,                                       │
│      public_values: 0x...,                                     │
│      verifying_key: 0x...,                                     │
│    }                                                            │
│                                                                  │
│ 5. Update database:                                             │
│    UPDATE proof_batches                                         │
│    SET sindri_proof_id = 'proof_abc123',                       │
│        proof_status = 'proven',                                 │
│        proven_at = NOW()                                        │
│    WHERE id = 42                                                │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Smart Contract Submission                                        │
│ (Ethereum/Sepolia - ethereum_client crate)                     │
│                                                                  │
│ 1. Fetch proof data from Sindri                                │
│                                                                  │
│ 2. Get ADS state roots from database:                           │
│    SELECT prev_merkle_root, new_merkle_root                     │
│    FROM ads_state_commits                                       │
│    WHERE batch_id = 42                                          │
│                                                                  │
│ 3. Call ARDA smart contract:                                    │
│    ARDA.updateState(                                            │
│      stateId = 0x0000...,        ← previous state root         │
│      newStateRoot = 0xf1a2...,   ← new state root              │
│      proof = proof_bytes,        ← SP1 proof from Sindri       │
│      publicValues = abi_encoded  ← LoanBatchPublicValues       │
│    )                                                            │
│                                                                  │
│ 4. Wait for transaction confirmation:                           │
│    - Transaction sent to network                                │
│    - Miners include in block                                    │
│    - Confirmations accumulate                                   │
│    - Takes ~15 seconds on testnet, ~1-2 minutes on mainnet    │
│                                                                  │
│ 5. Update database:                                             │
│    UPDATE proof_batches                                         │
│    SET posted_to_contract = true,                              │
│        contract_tx_hash = '0x...',                              │
│        posted_at = NOW()                                        │
│    WHERE id = 42                                                │
│                                                                  │
│ 6. Smart contract state updated:                                │
│    - State root stored on-chain                                 │
│    - Anyone can verify the proof                                │
│    - Immutable record created                                   │
└─────────────────────────────────────────────────────────────────┘
                            ↓
                    ✅ COMPLETE ✅
        User registration proven valid and posted on-chain
```

### Timeline

- **T1 (API call):** < 100ms
- **T1 to T2 (waiting):** 1-60 seconds (depends on batch trigger)
- **T2 (batch creation):** < 1 second
- **T2 (zkVM execution):** < 1 second (local proof generation, or ~30s-2min for Sindri)
- **T2 (proof generation):** 30 seconds - 2 minutes (Sindri)
- **T2 (contract submission):** 15 seconds - 2 minutes (blockchain confirmation)

**Total:** 1-5 minutes from user action to on-chain proof

---

## Eventual Consistency Model

Our system uses **eventual consistency** between the optimistic API layer and the authoritative zkVM layer.

### Two Validation Points

1. **Optimistic Validation (API - Immediate)**
   - Checks current database state
   - Provides immediate feedback to user
   - Can be wrong if state changes quickly

2. **Authoritative Validation (zkVM - Delayed)**
   - Reconstructs state from transaction history
   - Cryptographically proves correctness
   - Final authority on validity

### When They Disagree

**Scenario:** Race condition on user balance

```rust
// Time T1: User has $100
let balance = get_balance_from_db(&pool, user_id).await?; // $100

// User makes payment #1: $60
// API checks: $100 >= $60 ✅ APPROVE
create_payment(&pool, user_id, deal_id, 60).await?;
record_transaction(DealPayment, user_id, deal_id, 60, ...).await?;

// User makes payment #2: $50 (before batch runs!)
// API checks: $100 >= $50 ✅ APPROVE (database still shows $100!)
create_payment(&pool, user_id, deal_id2, 50).await?;
record_transaction(DealPayment, user_id, deal_id2, 50, ...).await?;

// Batch runs
// zkVM reconstructs state:
// - Start: $0
// - Deposit: +$100 = $100
// - Payment #1: -$60 = $40
// - Payment #2: -$50 = -$10 ❌ REJECTED!

// zkVM throws InsufficientBalance error
```

### Handling Rejections

When zkVM rejects a transaction:

```rust
// In batch processor
match state.process_transaction(&transaction) {
    Ok(_) => {
        // Transaction valid - include in proof
        info!("✅ Transaction {} processed", transaction.transaction_id);
    }
    Err(e) => {
        // Transaction invalid - mark as failed
        error!("❌ Transaction {} failed: {}", transaction.transaction_id, e);
        
        // Update deal_transactions table
        sqlx::query!(
            "UPDATE deal_transactions 
             SET status = 'failed',
                 error_message = $1,
                 failed_at = NOW()
             WHERE transaction_id_hash = $2",
            e.to_string(),
            transaction.transaction_id
        )
        .execute(&pool)
        .await?;
        
        // Optionally: Rollback database state
        // This depends on business requirements:
        // - Do we revert the payment record?
        // - Do we notify the user?
        // - Do we retry with a smaller amount?
        
        // For now: Log and continue with other transactions
    }
}
```

### Rollback Strategies

**Option 1: Soft Failure (Recommended)**
- Mark transaction as failed in deal_transactions
- Keep payment record with status = 'failed'
- Notify user via email/webhook
- User can retry with corrected amount

**Option 2: Hard Rollback**
- Delete or mark payment as invalid
- Restore previous balance state
- More complex, requires careful handling

**Option 3: Pre-locking**
- Lock balance at API level
- Reserve funds until zkVM confirms
- More complex state management
- Better user experience

### Best Practices

1. **API validation is a courtesy, not a guarantee**
   - Help users avoid obvious mistakes
   - But don't rely on it for correctness

2. **zkVM validation is authoritative**
   - If zkVM rejects, transaction is invalid
   - Update business systems accordingly

3. **Design for failures**
   - Transactions can fail at zkVM stage
   - Have clear error handling paths
   - Provide user feedback mechanisms

4. **Monitor discrepancies**
   - Track how often API approves but zkVM rejects
   - High rates indicate API validation needs improvement

---

## Implementation Guidelines

### When Adding a New Transaction Type

Follow these steps:

1. **Define in lib/src/transactions.rs**
   ```rust
   pub enum TransactionType {
       // ... existing types ...
       NewTransactionType = 26,
   }
   ```

2. **Implement zkVM logic in lib/src/deal_processor.rs**
   ```rust
   match transaction.transaction_type {
       // ... existing cases ...
       TransactionType::NewTransactionType => {
           // Validate in-memory state
           // Apply state transition
           // Update relevant HashMaps
       }
   }
   ```

3. **Create/update API endpoint in api/src/handlers/**
   ```rust
   pub async fn new_operation_endpoint(...) -> Result<...> {
       // Validate inputs
       // Check business rules (DB queries OK)
       // Write to business tables
       // Call record_transaction()
       // Return response
   }
   ```

4. **Use TransactionRecordingService**
   ```rust
   let tx_service = TransactionRecordingService::new(pool.clone());
   tx_service.record_transaction(
       TransactionType::NewTransactionType,
       user_id,
       deal_id,
       amount,
       metadata,
   ).await?;
   ```

5. **Add tests**
   - Unit test zkVM logic in lib/src/
   - Integration test API endpoint in api/tests/
   - Update populate_deal_data.rs script

### Common Pitfalls

❌ **Don't access database from lib/**
```rust
// WRONG - lib/ cannot access database
let user = sqlx::query!("SELECT * FROM users WHERE id = $1", user_id)
    .fetch_one(&pool)
    .await?;
```

❌ **Don't execute zkVM from API handlers**
```rust
// WRONG - API shouldn't run zkVM directly
let mut state = GlobalLoanState::new();
state.process_transaction(&transaction)?;
```

❌ **Don't skip transaction recording**
```rust
// WRONG - Must record transaction for zkVM
let payment = create_payment(&pool, deal_id, amount).await?;
// Missing: record_transaction() call!
return Ok(payment);
```

❌ **Don't trust API validation alone**
```rust
// WRONG - API validation is not sufficient
if balance >= amount {
    // Don't assume this guarantees zkVM will approve
    update_balance(&pool, user_id, balance - amount).await?;
}
```

### Testing Strategy

1. **Unit Tests (lib/)**
   - Test pure state transitions
   - Test validation logic
   - Test edge cases
   - Run with `cargo test --lib`

2. **Integration Tests (api/tests/)**
   - Test API endpoints
   - Test TransactionRecordingService
   - Verify database writes
   - Run with `cargo test --test integration_tests`

3. **End-to-End Tests**
   - Use populate_deal_data.rs script
   - Manually trigger batch processing
   - Verify proof generation
   - Check contract posting

4. **zkVM Tests (program/)**
   - Test complete batch processing
   - Verify proof generation locally
   - Run with `cargo run --release` in program/

---

## Summary

### Key Takeaways

1. **Three Layers, Three Responsibilities**
   - API: HTTP validation + database writes + transaction recording
   - Bridge: Data conversion + orchestration
   - zkVM: Pure state transitions + proof generation

2. **Dual State is Essential**
   - Database: Source of truth for business
   - zkVM: Proof of correctness
   - They complement, not replace each other

3. **Validation Happens Twice**
   - API: Optimistic, immediate, helpful
   - zkVM: Authoritative, delayed, final

4. **Eventually Consistent**
   - API can approve what zkVM rejects
   - Design for failure cases
   - Have clear error handling

5. **lib/ Must Stay Pure**
   - No database, no I/O, no network
   - Deterministic and reproducible
   - This is non-negotiable for zkVM

---

## State Progression Model (MVP)

**Current implementation:** Process from genesis for every batch

```
Batch 1: S0 → S1 (process transactions 1-50)
Batch 2: S0 → S2 (process transactions 1-68, reprocessing 1-50 + new 51-68)
Batch 3: S0 → S3 (process transactions 1-85, reprocessing everything)
```

**Why this works:**
- Simple and correct
- No state snapshot management complexity
- Handles historical imports and real-time operations identically
- Each proof verifies complete transaction history

**Future optimization:** State snapshots for incremental processing
```
Batch 1: S0 → S1 (process tx 1-50, save snapshot)
Batch 2: S1 → S2 (load snapshot, process only tx 51-68)
Batch 3: S2 → S3 (load snapshot, process only tx 69-85)
```

## Related Documentation

- **[Batching and Transaction Ordering](batching-and-ordering.md)** - Simplified global timestamp sorting
- **[Final Implementation Audit](FINAL_IMPLEMENTATION_AUDIT.md)** - Complete system audit and verification
- **[Executive Summary](EXECUTIVE_SUMMARY.md)** - High-level overview for stakeholders

---

**Questions or Issues?**

If something in this architecture is unclear or you discover edge cases, update this document and the related audit reports. Keep documentation in sync with implementation.
