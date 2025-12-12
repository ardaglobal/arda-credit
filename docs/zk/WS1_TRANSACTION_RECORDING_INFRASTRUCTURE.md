# Work Stream 1: Transaction Recording Infrastructure

**Epic:** EPIC-1  
**Priority:** CRITICAL (Blocks all other work streams)  
**Effort:** 1 day (8 hours)  
**Owner:** Backend/Infrastructure Engineer  
**Dependencies:** None  

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Requirements](#technical-requirements)
3. [Implementation Guide](#implementation-guide)
4. [Testing Strategy](#testing-strategy)
5. [Edge Cases & Error Handling](#edge-cases--error-handling)
6. [Performance Considerations](#performance-considerations)
7. [Documentation Requirements](#documentation-requirements)
8. [Validation Checklist](#validation-checklist)

---

## Overview

### Purpose

Create the foundational service that records transactions for zkVM batch processing. This service bridges the gap between API endpoints (which create business records) and the zkVM proof system (which validates state transitions).

### Context

**Current State:**
- API endpoints create records in business tables (`users`, `deals`, `payments`, etc.)
- ❌ API endpoints do NOT create records in `deal_transactions` table
- zkVM batch processor has nothing to process
- Only test script (`scripts/populate_deal_data.rs`) creates deal_transactions

**Target State:**
- API endpoints create records in BOTH business tables AND deal_transactions
- TransactionRecordingService provides consistent interface
- All state-changing operations recorded for zkVM
- Batch processor has transactions to prove

### Why This Blocks Everything

Every other work stream depends on this service:
- **WS-2 (Deal Ops):** Needs service to record deal transactions
- **WS-3 (Collateral zkVM):** Needs transactions to process
- **WS-4 (Collateral API):** Needs service to record collateral transactions
- **WS-5 (Testing):** Needs service to test
- **WS-6 (Test Scripts):** Could use same service for consistency

---

## Technical Requirements

### Functional Requirements

**FR-1:** Service must record transactions for all transaction types
- UserRegistration (#1)
- DealOrigination (#4)
- DealPayment (#5)
- DealCloseout (#6)
- CollateralCreation (#20)
- CollateralUpdate (#21)
- CollateralDealAssociation (#22)
- CollateralDealDisassociation (#23)
- CollateralDetectionRequest (#24)
- CollateralDetectionUpdate (#25)

**FR-2:** Service must hash UUIDs using SHA256
- Converts UUIDs to 32-byte hashes for privacy
- Must match hashing logic in test script
- Consistent with zkVM expectations

**FR-3:** Service must insert into deal_transactions table
- All required fields populated
- Timestamps use database NOW()
- Metadata stored as JSONB

**FR-4:** Service must return transaction_id
- Caller can track transaction
- Used for logging and debugging
- Links to business records

**FR-5:** Service must handle errors gracefully
- Database errors (connection, constraint violations)
- Invalid inputs (bad UUIDs, invalid transaction types)
- Duplicate transactions

### Non-Functional Requirements

**NFR-1: Performance**
- Transaction recording must add <50ms to API latency
- Batch operations should be supported
- No blocking on proof generation

**NFR-2: Reliability**
- Transaction recording should not fail API requests
- Consider async/fire-and-forget if needed
- Implement retry logic for transient failures

**NFR-3: Observability**
- Log all transaction creations
- Metrics for transaction types
- Error tracking and alerting

**NFR-4: Maintainability**
- Clear API surface
- Well-documented
- Easy to use from any endpoint

---

## Implementation Guide

### Step 1: Create Service File Structure

**File:** `api/src/services/transaction_recording.rs`

**Location:** Create new file in services directory

**Initial Structure:**
```rust
//! Transaction Recording Service
//!
//! This service records transactions in the deal_transactions table for zkVM batch processing.
//! It provides a consistent interface for all API endpoints to create transaction records
//! that will later be batched and proven via zero-knowledge proofs.

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::config::types::ErrorResponse;
use arda_credit_db::DbError;

/// Service for recording transactions for zkVM processing
#[derive(Clone)]
pub struct TransactionRecordingService {
    pool: PgPool,
}

impl TransactionRecordingService {
    /// Create a new transaction recording service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    // Methods will be added in subsequent steps
}
```

**Effort:** 15 minutes

### Step 2: Implement UUID Hashing

**Add to TransactionRecordingService:**

```rust
impl TransactionRecordingService {
    /// Hash a UUID using SHA256 for privacy in zkVM
    ///
    /// This converts a 16-byte UUID into a 32-byte hash, which is used
    /// in the zkVM for privacy (prevents linking transactions to real UUIDs).
    ///
    /// # Arguments
    /// * `uuid` - The UUID to hash
    ///
    /// # Returns
    /// A 32-byte hash as Vec<u8>
    fn hash_uuid(uuid: &Uuid) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(uuid.as_bytes());
        hasher.finalize().to_vec()
    }
    
    /// Hash an optional UUID, returning None if input is None
    fn hash_optional_uuid(uuid: Option<&Uuid>) -> Option<Vec<u8>> {
        uuid.map(Self::hash_uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_uuid_deterministic() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        
        // Hash should be deterministic
        let hash1 = TransactionRecordingService::hash_uuid(&uuid);
        let hash2 = TransactionRecordingService::hash_uuid(&uuid);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // SHA256 produces 32 bytes
    }
    
    #[test]
    fn test_hash_uuid_unique() {
        let uuid1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let uuid2 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        
        let hash1 = TransactionRecordingService::hash_uuid(&uuid1);
        let hash2 = TransactionRecordingService::hash_uuid(&uuid2);
        
        assert_ne!(hash1, hash2); // Different UUIDs should produce different hashes
    }
    
    #[test]
    fn test_hash_optional_uuid() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        
        let hash_some = TransactionRecordingService::hash_optional_uuid(Some(&uuid));
        assert!(hash_some.is_some());
        
        let hash_none = TransactionRecordingService::hash_optional_uuid(None);
        assert!(hash_none.is_none());
    }
}
```

**Validation:**
- Run `cargo test hash_uuid` to verify tests pass
- Hashes should be 32 bytes
- Same UUID always produces same hash
- Different UUIDs produce different hashes

**Effort:** 1 hour

### Step 3: Implement record_transaction Method

**Add to TransactionRecordingService:**

```rust
use arda_credit_lib::TransactionType;
use rust_decimal::Decimal;

impl TransactionRecordingService {
    /// Record a transaction for zkVM batch processing
    ///
    /// Creates a record in the deal_transactions table that will later be
    /// picked up by the batch processor, included in a zkVM proof, and
    /// posted to the smart contract.
    ///
    /// # Arguments
    /// * `transaction_type` - Type of transaction (UserRegistration, DealOrigination, etc.)
    /// * `user_id` - UUID of the user initiating the transaction
    /// * `deal_id` - Optional UUID of the deal (None for non-deal transactions)
    /// * `amount` - Transaction amount (Decimal::ZERO for non-monetary transactions)
    /// * `metadata` - Optional JSON metadata with additional context
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The transaction_id that was created
    /// * `Err(DbError)` - Database error if insertion fails
    ///
    /// # Example
    /// ```rust
    /// let tx_service = TransactionRecordingService::new(pool.clone());
    /// let tx_id = tx_service.record_transaction(
    ///     TransactionType::UserRegistration,
    ///     user_id,
    ///     None,
    ///     Decimal::ZERO,
    ///     Some(json!({ "email": "user@example.com" })),
    /// ).await?;
    /// ```
    #[instrument(skip(self, metadata), level = "info")]
    pub async fn record_transaction(
        &self,
        transaction_type: TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: Decimal,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, DbError> {
        // Generate new transaction ID
        let transaction_id = Uuid::new_v4();
        
        // Hash UUIDs for zkVM privacy
        let transaction_id_hash = Self::hash_uuid(&transaction_id);
        let user_id_hash = Self::hash_uuid(&user_id);
        let deal_id_hash = deal_id.map(|id| Self::hash_uuid(&id));
        
        // Convert deal_id_hash Option<Vec<u8>> to Option<&[u8]> for query
        let deal_id_hash_ref = deal_id_hash.as_deref();
        
        // Insert into deal_transactions table
        sqlx::query!(
            r#"
            INSERT INTO deal_transactions (
                id,
                transaction_id_hash,
                user_id_hash,
                deal_id_hash,
                transaction_type,
                amount,
                user_id,
                deal_id,
                metadata,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            "#,
            transaction_id,
            &transaction_id_hash[..],  // Convert Vec<u8> to &[u8]
            &user_id_hash[..],
            deal_id_hash_ref,
            transaction_type.to_string(),
            amount,
            user_id,
            deal_id,
            metadata,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Failed to record {} transaction for user {}: {}",
                transaction_type, user_id, e
            );
            DbError::from(e)
        })?;
        
        info!(
            "Recorded {} transaction: {} for user {} (deal: {:?})",
            transaction_type,
            transaction_id,
            user_id,
            deal_id
        );
        
        // Record metrics
        metrics::counter!(
            "zkvm_transactions_recorded_total",
            "type" => transaction_type.to_string()
        )
        .increment(1);
        
        Ok(transaction_id)
    }
}
```

**Validation:**
- Compile check: `cargo check --package api`
- Verify SQL query syntax
- Check all fields match table schema

**Effort:** 3 hours

### Step 4: Add Batch Recording Method (Optional but Recommended)

**Add to TransactionRecordingService:**

```rust
impl TransactionRecordingService {
    /// Record multiple transactions in a single database transaction
    ///
    /// This is more efficient than calling record_transaction() multiple times
    /// when you need to record several related transactions atomically.
    ///
    /// # Arguments
    /// * `transactions` - Vector of transaction recording requests
    ///
    /// # Returns
    /// * `Ok(Vec<Uuid>)` - Transaction IDs created (in same order as input)
    /// * `Err(DbError)` - Database error (rolls back all if any fail)
    #[instrument(skip(self, transactions), level = "info")]
    pub async fn record_transactions_batch(
        &self,
        transactions: Vec<TransactionRecordingRequest>,
    ) -> Result<Vec<Uuid>, DbError> {
        let mut tx = self.pool.begin().await?;
        let mut transaction_ids = Vec::with_capacity(transactions.len());
        
        for request in transactions {
            let transaction_id = Uuid::new_v4();
            let transaction_id_hash = Self::hash_uuid(&transaction_id);
            let user_id_hash = Self::hash_uuid(&request.user_id);
            let deal_id_hash = request.deal_id.map(|id| Self::hash_uuid(&id));
            let deal_id_hash_ref = deal_id_hash.as_deref();
            
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
                &transaction_id_hash[..],
                &user_id_hash[..],
                deal_id_hash_ref,
                request.transaction_type.to_string(),
                request.amount,
                request.user_id,
                request.deal_id,
                request.metadata,
            )
            .execute(&mut *tx)
            .await?;
            
            transaction_ids.push(transaction_id);
        }
        
        tx.commit().await?;
        
        info!(
            "Recorded batch of {} transactions",
            transaction_ids.len()
        );
        
        metrics::counter!("zkvm_batch_transactions_recorded_total")
            .increment(transaction_ids.len() as u64);
        
        Ok(transaction_ids)
    }
}

/// Request to record a transaction
#[derive(Debug, Clone)]
pub struct TransactionRecordingRequest {
    pub transaction_type: TransactionType,
    pub user_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub amount: Decimal,
    pub metadata: Option<serde_json::Value>,
}

impl TransactionRecordingRequest {
    /// Create a new transaction recording request
    pub fn new(
        transaction_type: TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: Decimal,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            transaction_type,
            user_id,
            deal_id,
            amount,
            metadata,
        }
    }
}
```

**Use Case:** When creating a deal with collateral, you might want to record both DealOrigination and CollateralDealAssociation atomically.

**Effort:** 1.5 hours

### Step 5: Add Service to ApiState

**File:** `api/src/api/router.rs`

**Modify ApiState struct:**

```rust
/// API state containing the database pool, configuration, and services
#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub config: ApiConfig,
    pub batch_processor: Option<BatchProcessorHandle>,
    pub ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
    pub session_service: Arc<SessionService>,
    pub magic_link_service: Option<Arc<crate::services::magic_link_service::MagicLinkService>>,
    pub team_invitation_service: Option<Arc<crate::services::team_invitation_service::TeamInvitationService>>,
    pub withdrawal_service: WithdrawalTransactionService,
    pub balance_service: BalanceVerificationService,
    pub withdrawal_executor: Option<crate::services::simple_withdrawal_executor::SimpleWithdrawalExecutorHandle>,
    pub metrics_handle: Option<Arc<crate::utils::metrics::MetricsHandle>>,
    pub email_rate_limiter: Option<Arc<crate::middleware::middleware::EmailRateLimiter>>,
    pub metrics_cache: Arc<MetricsCache>,
    pub event_publisher: Option<Arc<crate::events::CollateralEventPublisher>>,
    // NEW: Add transaction recording service
    pub transaction_recording: Arc<TransactionRecordingService>,
}
```

**Update service initialization** in server startup:

```rust
// In api/src/bin/server.rs or wherever ApiState is initialized

let transaction_recording = Arc::new(TransactionRecordingService::new(pool.clone()));

let state = ApiState {
    pool: pool.clone(),
    config,
    batch_processor: Some(batch_processor_handle),
    ads_service: ads_service.clone(),
    session_service: session_service.clone(),
    // ... other services ...
    transaction_recording,  // NEW
};
```

**Effort:** 30 minutes

### Step 6: Export Service from Module

**File:** `api/src/services/mod.rs`

**Add:**
```rust
pub mod transaction_recording;
pub use transaction_recording::{TransactionRecordingService, TransactionRecordingRequest};
```

**File:** `api/src/lib.rs`

**Verify export chain:**
```rust
pub use services::{
    // ... existing exports ...
    TransactionRecordingService,
    TransactionRecordingRequest,
};
```

**Effort:** 15 minutes

---

## Testing Strategy

### Unit Tests (EPIC-1.4)

**File:** `api/src/services/transaction_recording.rs` (add to bottom of file)

**Test Coverage Required:**

#### Test 1: Basic Transaction Recording

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    
    // Helper to create test database pool
    async fn setup_test_db() -> PgPool {
        // Use test database or in-memory SQLite
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/arda_credit_test".to_string());
        
        sqlx::PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }
    
    #[tokio::test]
    async fn test_record_user_registration() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());
        
        let user_id = Uuid::new_v4();
        let metadata = Some(serde_json::json!({
            "email": "test@example.com",
            "user_type": "investor"
        }));
        
        let result = service.record_transaction(
            TransactionType::UserRegistration,
            user_id,
            None,
            Decimal::ZERO,
            metadata,
        ).await;
        
        assert!(result.is_ok());
        let transaction_id = result.unwrap();
        
        // Verify record was created
        let record = sqlx::query!(
            "SELECT * FROM deal_transactions WHERE id = $1",
            transaction_id
        )
        .fetch_one(&pool)
        .await
        .expect("Transaction should exist");
        
        assert_eq!(record.transaction_type, "user_registration");
        assert_eq!(record.user_id, Some(user_id));
        assert_eq!(record.deal_id, None);
        assert_eq!(record.amount, Decimal::ZERO);
    }
    
    #[tokio::test]
    async fn test_record_deal_origination() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());
        
        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();
        let amount = Decimal::from_str("100000.50").unwrap();
        
        let result = service.record_transaction(
            TransactionType::DealOrigination,
            user_id,
            Some(deal_id),
            amount,
            None,
        ).await;
        
        assert!(result.is_ok());
        let transaction_id = result.unwrap();
        
        // Verify record
        let record = sqlx::query!(
            "SELECT * FROM deal_transactions WHERE id = $1",
            transaction_id
        )
        .fetch_one(&pool)
        .await
        .expect("Transaction should exist");
        
        assert_eq!(record.transaction_type, "deal_origination");
        assert_eq!(record.user_id, Some(user_id));
        assert_eq!(record.deal_id, Some(deal_id));
        assert_eq!(record.amount, amount);
    }
    
    #[tokio::test]
    async fn test_all_transaction_types() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());
        
        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();
        
        // Test each transaction type
        let types = vec![
            TransactionType::UserRegistration,
            TransactionType::DealOrigination,
            TransactionType::DealPayment,
            TransactionType::DealCloseout,
            TransactionType::CollateralCreation,
            TransactionType::CollateralUpdate,
            TransactionType::CollateralDealAssociation,
            TransactionType::CollateralDealDisassociation,
            TransactionType::CollateralDetectionRequest,
            TransactionType::CollateralDetectionUpdate,
        ];
        
        for tx_type in types {
            let result = service.record_transaction(
                tx_type,
                user_id,
                Some(deal_id),
                Decimal::ZERO,
                None,
            ).await;
            
            assert!(
                result.is_ok(),
                "Failed to record {} transaction",
                tx_type
            );
        }
        
        // Verify all were created
        let count = sqlx::query!(
            "SELECT COUNT(*) as count FROM deal_transactions WHERE user_id = $1",
            user_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(count.count, Some(10));
    }
}
```

#### Test 2: UUID Hashing Consistency

**Add to tests module:**

```rust
#[tokio::test]
async fn test_hash_consistency_with_test_script() {
    // Verify our hashing matches the test script's hashing
    let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    
    // Our implementation
    let our_hash = TransactionRecordingService::hash_uuid(&uuid);
    
    // Expected from test script (scripts/populate_deal_data.rs:570)
    let mut hasher = Sha256::new();
    hasher.update(uuid.as_bytes());
    let expected_hash: Vec<u8> = hasher.finalize().to_vec();
    
    assert_eq!(our_hash, expected_hash, "Hash must match test script implementation");
}
```

#### Test 3: Batch Recording

```rust
#[tokio::test]
async fn test_batch_recording() {
    let pool = setup_test_db().await;
    let service = TransactionRecordingService::new(pool.clone());
    
    let user_id = Uuid::new_v4();
    let deal_id = Uuid::new_v4();
    
    let requests = vec![
        TransactionRecordingRequest::new(
            TransactionType::UserRegistration,
            user_id,
            None,
            Decimal::ZERO,
            None,
        ),
        TransactionRecordingRequest::new(
            TransactionType::DealOrigination,
            user_id,
            Some(deal_id),
            Decimal::from(100000),
            None,
        ),
        TransactionRecordingRequest::new(
            TransactionType::DealPayment,
            user_id,
            Some(deal_id),
            Decimal::from(50000),
            None,
        ),
    ];
    
    let result = service.record_transactions_batch(requests).await;
    
    assert!(result.is_ok());
    let transaction_ids = result.unwrap();
    assert_eq!(transaction_ids.len(), 3);
    
    // Verify all created
    for tx_id in transaction_ids {
        let exists = sqlx::query!(
            "SELECT id FROM deal_transactions WHERE id = $1",
            tx_id
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        
        assert!(exists.is_some());
    }
}
```

#### Test 4: Error Handling

```rust
#[tokio::test]
async fn test_duplicate_transaction_id_handling() {
    // Note: This shouldn't happen with Uuid::new_v4(), but test error handling
    let pool = setup_test_db().await;
    let service = TransactionRecordingService::new(pool.clone());
    
    let user_id = Uuid::new_v4();
    
    // First insertion should succeed
    let result1 = service.record_transaction(
        TransactionType::UserRegistration,
        user_id,
        None,
        Decimal::ZERO,
        None,
    ).await;
    
    assert!(result1.is_ok());
}

#[tokio::test]
async fn test_invalid_foreign_key_handling() {
    let pool = setup_test_db().await;
    let service = TransactionRecordingService::new(pool.clone());
    
    let user_id = Uuid::new_v4();
    let fake_deal_id = Uuid::new_v4(); // Deal doesn't exist
    
    // Should succeed - deal_id is optional reference
    let result = service.record_transaction(
        TransactionType::DealPayment,
        user_id,
        Some(fake_deal_id),
        Decimal::from(100),
        None,
    ).await;
    
    // Depending on schema, this might fail or succeed
    // Document expected behavior
}
```

**Validation:**
- Run `cargo test --package api transaction_recording`
- All tests should pass
- Check code coverage: `cargo tarpaulin --package api`

**Effort:** 2 hours

### Step 7: Add Logging & Metrics (EPIC-1.5)

**Add to implementation:**

```rust
use metrics;

impl TransactionRecordingService {
    #[instrument(skip(self, metadata), level = "info")]
    pub async fn record_transaction(
        &self,
        transaction_type: TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: Decimal,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, DbError> {
        let start_time = std::time::Instant::now();
        
        // ... existing implementation ...
        
        // Record metrics
        let duration = start_time.elapsed();
        
        metrics::histogram!(
            "zkvm_transaction_recording_duration_seconds",
            "type" => transaction_type.to_string()
        )
        .record(duration.as_secs_f64());
        
        metrics::counter!(
            "zkvm_transactions_recorded_total",
            "type" => transaction_type.to_string()
        )
        .increment(1);
        
        if amount > Decimal::ZERO {
            metrics::histogram!(
                "zkvm_transaction_amount_usdc",
                "type" => transaction_type.to_string()
            )
            .record(amount.to_f64().unwrap_or(0.0));
        }
        
        info!(
            "Recorded {} transaction {} in {:?}",
            transaction_type,
            transaction_id,
            duration
        );
        
        Ok(transaction_id)
    }
}
```

**Metrics to Track:**
- `zkvm_transactions_recorded_total{type}` - Counter by transaction type
- `zkvm_transaction_recording_duration_seconds{type}` - Histogram of latency
- `zkvm_transaction_amount_usdc{type}` - Histogram of amounts
- `zkvm_transaction_recording_errors_total{type}` - Counter of failures

**Effort:** 1 hour (can overlap with Step 3)

---

## Edge Cases & Error Handling

### Error Case 1: Database Connection Failure

**Scenario:** Database is temporarily unavailable

**Handling:**
```rust
pub async fn record_transaction(...) -> Result<Uuid, DbError> {
    match sqlx::query!(...).execute(&self.pool).await {
        Ok(_) => Ok(transaction_id),
        Err(e) => {
            error!("Database error recording transaction: {}", e);
            metrics::counter!("zkvm_transaction_recording_errors_total").increment(1);
            Err(DbError::from(e))
        }
    }
}
```

**Recommendation:** Let API endpoint decide whether to fail or retry

### Error Case 2: Invalid UUID Format

**Scenario:** Caller passes invalid UUID (should be prevented by types)

**Handling:** Not applicable - Rust's type system prevents this

### Error Case 3: Null/Missing Deal ID for Deal Transactions

**Scenario:** DealPayment transaction with deal_id = None

**Handling:**
```rust
// Validate in caller, not service
// Service accepts Option<Uuid> and records whatever is passed
// zkVM validation will catch inconsistencies later
```

**Rationale:** Service is generic, business validation happens elsewhere

### Error Case 4: Large Metadata (>1MB)

**Scenario:** Caller passes huge metadata JSON

**Handling:**
```rust
pub async fn record_transaction(
    &self,
    transaction_type: TransactionType,
    user_id: Uuid,
    deal_id: Option<Uuid>,
    amount: Decimal,
    metadata: Option<serde_json::Value>,
) -> Result<Uuid, DbError> {
    // Validate metadata size
    if let Some(ref meta) = metadata {
        let serialized = serde_json::to_string(meta)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;
        
        if serialized.len() > 1_000_000 {  // 1MB limit
            error!("Metadata too large: {} bytes", serialized.len());
            return Err(DbError::ValidationError(
                "Metadata exceeds 1MB limit".to_string()
            ));
        }
    }
    
    // ... rest of implementation
}
```

**Effort:** 30 minutes

### Error Case 5: Concurrent Transaction Recording

**Scenario:** Multiple API requests try to record transactions simultaneously

**Handling:** PostgreSQL handles this naturally with MVCC
- No special handling needed
- Each INSERT gets unique transaction_id
- No conflicts expected

**Validation:** Load test in EPIC-5.6

### Error Case 6: Transaction Type Not Supported

**Scenario:** Future transaction type added but not in enum

**Handling:** Rust's type system prevents this
- Can only pass valid TransactionType values
- Compiler error if invalid type used

---

## Performance Considerations

### Benchmark Targets

**Target:** <50ms added latency to API requests

**Measurement:**
```rust
let start = std::time::Instant::now();
let tx_id = service.record_transaction(...).await?;
let duration = start.elapsed();

assert!(duration.as_millis() < 50, "Transaction recording too slow");
```

### Optimization Strategies

**Strategy 1: Database Connection Pooling**
- Use existing PgPool (already optimized)
- No additional connections needed

**Strategy 2: Async Fire-and-Forget (If Needed)**

If transaction recording adds too much latency:

```rust
pub async fn record_transaction_async(
    &self,
    transaction_type: TransactionType,
    user_id: Uuid,
    deal_id: Option<Uuid>,
    amount: Decimal,
    metadata: Option<serde_json::Value>,
) -> Uuid {
    let transaction_id = Uuid::new_v4();
    
    // Clone data for async task
    let pool = self.pool.clone();
    let tx_id = transaction_id;
    
    // Fire and forget
    tokio::spawn(async move {
        if let Err(e) = Self::record_transaction_internal(
            &pool, transaction_id, transaction_type, user_id, deal_id, amount, metadata
        ).await {
            error!("Async transaction recording failed: {}", e);
        }
    });
    
    transaction_id  // Return immediately
}
```

**Trade-off:** 
- ✅ Lower API latency
- ❌ Can't guarantee transaction was recorded
- ❌ Harder to debug failures

**Recommendation:** Start with synchronous, optimize only if needed

**Strategy 3: Batch Recording for Bulk Operations**

Use `record_transactions_batch()` when creating multiple related records:

```rust
// Instead of:
for item in items {
    service.record_transaction(...).await?;  // N database calls
}

// Do this:
let requests: Vec<_> = items.iter().map(|item| {
    TransactionRecordingRequest::new(...)
}).collect();

service.record_transactions_batch(requests).await?;  // 1 database transaction
```

### Database Index Requirements

**Verify these indexes exist** (should be in migrations):

```sql
-- For finding pending transactions
CREATE INDEX IF NOT EXISTS idx_deal_transactions_pending 
ON deal_transactions(created_at) 
WHERE included_in_batch_id IS NULL;

-- For querying by deal
CREATE INDEX IF NOT EXISTS idx_deal_transactions_deal_id 
ON deal_transactions(deal_id);

-- For querying by user  
CREATE INDEX IF NOT EXISTS idx_deal_transactions_user_id 
ON deal_transactions(user_id);

-- For batch processor queries
CREATE INDEX IF NOT EXISTS idx_deal_transactions_batch_id 
ON deal_transactions(included_in_batch_id);
```

If missing, create migration in `db/migrations/`

---

## Documentation Requirements

### Code Documentation

**Add to service file:**

```rust
//! Transaction Recording Service
//!
//! This service provides a centralized interface for recording state-changing operations
//! in the `deal_transactions` table for zkVM batch processing.
//!
//! ## Architecture
//!
//! The service sits at Layer 2 of our three-layer architecture:
//! - Layer 1 (API Handlers): Call this service after database writes
//! - Layer 2 (This Service): Records transactions for zkVM processing
//! - Layer 3 (zkVM): Processes transactions and generates proofs
//!
//! ## Usage
//!
//! ```rust
//! use crate::services::TransactionRecordingService;
//!
//! // Get service from ApiState
//! let tx_service = state.transaction_recording.clone();
//!
//! // Record a transaction
//! let tx_id = tx_service.record_transaction(
//!     TransactionType::DealOrigination,
//!     user_id,
//!     Some(deal_id),
//!     principal_amount,
//!     Some(json!({
//!         "investor_id": investor_id.to_string(),
//!         "interest_rate_bps": 500,
//!     })),
//! ).await?;
//! ```
//!
//! ## Privacy & Hashing
//!
//! UUIDs are hashed using SHA256 before storage to maintain privacy in the zkVM:
//! - transaction_id_hash = SHA256(transaction_id)
//! - user_id_hash = SHA256(user_id)
//! - deal_id_hash = SHA256(deal_id) [if present]
//!
//! This prevents linking zkVM transactions back to specific database records.
//!
//! ## Error Handling
//!
//! The service propagates database errors to the caller. It's the caller's
//! responsibility to decide whether to:
//! - Fail the API request
//! - Retry the transaction recording
//! - Continue without recording (not recommended)
//!
//! ## See Also
//!
//! - [zkVM Architecture](../../docs/zk/architecture.md)
//! - [Implementation Plan](../../docs/zk/IMPLEMENTATION_PLAN.md)
```

### API Documentation

**Create:** `docs/zk/transaction-recording-service.md`

**Content:**
- Service purpose and architecture
- Usage examples for each transaction type
- Error handling guide
- Performance characteristics
- Troubleshooting common issues

**Effort:** 30 minutes

### Update Existing Docs

**Update:** `docs/zk/architecture.md`

Add reference to TransactionRecordingService in Layer 2 section:

```markdown
### Layer 2: Business Logic Bridge

The TransactionRecordingService is the primary component of Layer 2:

**Implementation:** `api/src/services/transaction_recording.rs`

**Responsibilities:**
- Generates transaction_id (Uuid::new_v4())
- Hashes UUIDs with SHA256 for privacy
- Inserts into deal_transactions table
- Returns transaction_id to caller

**Usage from API handlers:**
```rust
let tx_service = state.transaction_recording.clone();
let tx_id = tx_service.record_transaction(...).await?;
```
```

**Effort:** 15 minutes

---

## Validation Checklist

### Before Marking EPIC-1 Complete

- [ ] **EPIC-1.1:** Service file created with proper structure
- [ ] **EPIC-1.2:** UUID hashing implemented and tested
- [ ] **EPIC-1.3:** `record_transaction()` method complete
- [ ] **EPIC-1.4:** Batch recording implemented (optional but recommended)
- [ ] **EPIC-1.5:** Service added to ApiState
- [ ] **EPIC-1.6:** Module exports configured
- [ ] **EPIC-1.7:** All unit tests passing
- [ ] **EPIC-1.8:** Logging instrumented with tracing
- [ ] **EPIC-1.9:** Metrics added (Prometheus counters/histograms)
- [ ] **EPIC-1.10:** Code documentation complete
- [ ] **EPIC-1.11:** API documentation created
- [ ] **EPIC-1.12:** Code reviewed by peer
- [ ] **EPIC-1.13:** No linter warnings
- [ ] **EPIC-1.14:** Compiles without errors
- [ ] **EPIC-1.15:** Ready for WS-2 team to start integration

### Integration Test

**Manual validation before handoff:**

```rust
// Create test endpoint (temporary)
#[cfg(debug_assertions)]
pub async fn test_transaction_recording_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user_id = Uuid::new_v4();
    let deal_id = Uuid::new_v4();
    
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::UserRegistration,
            user_id,
            None,
            Decimal::ZERO,
            Some(json!({ "test": true })),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(json!({
        "transaction_id": tx_id,
        "user_id": user_id,
        "message": "Transaction recorded successfully"
    })))
}

// Test it:
// curl http://localhost:8080/api/v1/test/record-transaction
//
// Then check database:
// SELECT * FROM deal_transactions ORDER BY created_at DESC LIMIT 1;
```

---

## Common Issues & Troubleshooting

### Issue 1: "Table deal_transactions does not exist"

**Cause:** Database migration not run

**Solution:**
```bash
# Run migrations
make initDB

# Or manually
sqlx migrate run
```

### Issue 2: "Type TransactionType not found"

**Cause:** Missing import from lib

**Solution:**
```rust
use arda_credit_lib::TransactionType;
```

### Issue 3: "Cannot convert Vec<u8> to &[u8]"

**Cause:** sqlx query! expects slice, not Vec

**Solution:**
```rust
// Wrong:
.bind(transaction_id_hash)

// Correct:
.bind(&transaction_id_hash[..])
```

### Issue 4: Tests fail with "Connection refused"

**Cause:** Test database not running

**Solution:**
```bash
# Start test database
docker compose up -d postgres

# Or use TEST_DATABASE_URL env var
export TEST_DATABASE_URL="postgresql://localhost/test_db"
cargo test
```

### Issue 5: "Metadata serialization error"

**Cause:** Metadata contains non-serializable types

**Solution:**
- Only use JSON-compatible types in metadata
- Test metadata with `serde_json::to_value()` before passing

---

## Code Review Guidelines

### What to Check

**Architecture:**
- [ ] Service only has PgPool dependency (no business logic)
- [ ] No zkVM execution in this service
- [ ] Generic enough to handle all transaction types
- [ ] Follows existing service patterns in `api/src/services/`

**Code Quality:**
- [ ] Proper error handling (no unwrap/expect in production code)
- [ ] Instrumented with tracing (#[instrument])
- [ ] Metrics added for observability
- [ ] Comments explain "why", not "what"
- [ ] No TODOs or FIXMEs left

**Testing:**
- [ ] Unit tests cover happy path
- [ ] Unit tests cover error cases
- [ ] Tests use test database (not production)
- [ ] Tests clean up after themselves
- [ ] Code coverage >80%

**Documentation:**
- [ ] Module-level docs explain purpose
- [ ] Public functions have doc comments
- [ ] Examples provided for common use cases
- [ ] Links to architecture docs

**Performance:**
- [ ] No N+1 queries
- [ ] Batch method provided for bulk operations
- [ ] No blocking operations
- [ ] Metrics show <50ms latency

---

## Handoff to Other Work Streams

### For WS-2 (Deal Operations Integration)

**What they need:**
```rust
// Service is ready to use
let tx_service = state.transaction_recording.clone();

// Example: In create_deal_endpoint
let tx_id = tx_service.record_transaction(
    TransactionType::DealOrigination,
    originator_id,
    Some(deal_id),
    principal_amount,
    Some(json!({
        "investor_id": investor_id.to_string(),
        "interest_rate_bps": interest_rate_bps,
    })),
).await?;
```

**Documentation:** Point them to:
- `docs/zk/architecture.md` - Layer 2 section
- `api/src/services/transaction_recording.rs` - Usage examples
- This document - For understanding design decisions

### For WS-4 (Collateral API Integration)

**What they need:**
```rust
// Same service, different transaction types
let tx_id = tx_service.record_transaction(
    TransactionType::CollateralCreation,
    creator_id,
    None,  // No deal yet
    Decimal::ZERO,  // Non-monetary
    Some(json!({
        "collateral_type": "real_estate",
        "name": "123 Main St Property",
    })),
).await?;
```

### For WS-6 (Test Scripts)

**What they can use:**
- Optional: Refactor test script to use this service instead of direct SQL
- Or keep direct SQL for independence
- Either approach is valid

---

## Timeline Breakdown

### Hour-by-Hour Plan (8 hours total)

**Hours 1-2: Setup & Structure**
- Create service file
- Add basic structure
- Implement UUID hashing
- Write hashing tests
- **Deliverable:** Structure in place, hashing works

**Hours 3-5: Core Implementation**
- Implement record_transaction() method
- Add error handling
- Add logging and metrics
- Write core unit tests
- **Deliverable:** Service functional, tested

**Hours 6-7: Integration & Polish**
- Implement batch recording (optional)
- Add to ApiState
- Update module exports
- Write integration tests
- **Deliverable:** Service integrated, ready to use

**Hour 8: Documentation & Validation**
- Write code documentation
- Create API documentation
- Run full test suite
- Create test endpoint for manual validation
- **Deliverable:** EPIC-1 complete, ready for handoff

---

## Acceptance Criteria (Final Check)

Before marking EPIC-1 as complete, verify:

### Functional
- [ ] Can record UserRegistration transactions
- [ ] Can record DealOrigination transactions
- [ ] Can record DealPayment transactions
- [ ] Can record DealCloseout transactions
- [ ] Can record all 6 CollateralXxx transactions
- [ ] UUIDs are hashed with SHA256
- [ ] Records appear in deal_transactions table
- [ ] Transaction IDs are returned to caller

### Quality
- [ ] All unit tests pass (`cargo test transaction_recording`)
- [ ] Code coverage >80%
- [ ] No compiler warnings
- [ ] No clippy warnings
- [ ] Formatted with rustfmt

### Performance
- [ ] Transaction recording adds <50ms latency
- [ ] No blocking operations
- [ ] Handles 100+ transactions/second

### Observability
- [ ] Logged with tracing (info level minimum)
- [ ] Metrics exported to Prometheus
- [ ] Errors logged with error! macro
- [ ] Can debug transaction recording from logs

### Documentation
- [ ] Module docs explain purpose and usage
- [ ] Public methods have doc comments
- [ ] Examples provided
- [ ] Links to architecture docs
- [ ] API documentation created

### Integration
- [ ] Service added to ApiState
- [ ] Accessible from all handlers via `state.transaction_recording`
- [ ] Module properly exported
- [ ] Can be imported with `use crate::services::TransactionRecordingService;`

### Handoff
- [ ] Demo'd to WS-2 team
- [ ] Demo'd to WS-4 team
- [ ] Questions answered
- [ ] Usage examples shared
- [ ] Ready for integration

---

## Reference Implementation

### Complete Service Code

See the implementation guide above for the complete code. The final file should be ~400-500 lines including:
- Struct definition
- `record_transaction()` method
- `record_transactions_batch()` method
- `hash_uuid()` helper
- Error handling
- Logging & metrics
- Unit tests
- Documentation

### Database Schema Reference

**Table:** `deal_transactions`

```sql
CREATE TABLE deal_transactions (
    id UUID PRIMARY KEY,
    transaction_id_hash BYTEA NOT NULL,
    user_id_hash BYTEA NOT NULL,
    deal_id_hash BYTEA,
    transaction_type TEXT NOT NULL,
    amount DECIMAL NOT NULL DEFAULT 0,
    user_id UUID,
    deal_id UUID REFERENCES deals(id),
    payment_id UUID,
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

**Key Fields:**
- `id` - UUID for this transaction record
- `transaction_id_hash` - SHA256(id) for zkVM
- `user_id_hash` - SHA256(user_id) for zkVM
- `deal_id_hash` - SHA256(deal_id) for zkVM (nullable)
- `transaction_type` - String like "deal_origination"
- `amount` - Decimal amount (0 for non-monetary)
- `included_in_batch_id` - NULL until batched, then batch ID

---

## Success Metrics

### Definition of Done

**EPIC-1 is complete when:**

1. ✅ All code committed and reviewed
2. ✅ All tests passing in CI
3. ✅ Documentation merged
4. ✅ WS-2 and WS-4 teams can start their work
5. ✅ Demo shows transaction recording working
6. ✅ No blockers for dependent work streams

### Measurement

**After completion, verify:**

```bash
# 1. Service compiles
cargo build --package api

# 2. Tests pass
cargo test --package api transaction_recording

# 3. Can use in code
grep -r "transaction_recording" api/src/

# 4. Ready for integration
cargo doc --package api --open
# Check TransactionRecordingService docs
```

---

## Appendix: Quick Reference

### Import Statements Needed

```rust
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{error, info, instrument};
use uuid::Uuid;
use rust_decimal::Decimal;
use arda_credit_lib::TransactionType;
use metrics;
```

### Common Transaction Recording Patterns

**Pattern 1: Non-Deal Transaction (User Registration)**
```rust
tx_service.record_transaction(
    TransactionType::UserRegistration,
    user_id,
    None,  // No deal
    Decimal::ZERO,  // No amount
    Some(json!({ "email": email })),
).await?
```

**Pattern 2: Deal Transaction with Amount**
```rust
tx_service.record_transaction(
    TransactionType::DealOrigination,
    originator_id,
    Some(deal_id),
    principal_amount,
    Some(json!({ "investor_id": investor_id.to_string() })),
).await?
```

**Pattern 3: Collateral Transaction**
```rust
tx_service.record_transaction(
    TransactionType::CollateralCreation,
    creator_id,
    None,  // Not yet associated with deal
    Decimal::ZERO,
    Some(json!({ "collateral_type": "real_estate" })),
).await?
```

### Testing Commands

```bash
# Run all transaction recording tests
cargo test --package api transaction_recording

# Run with output
cargo test --package api transaction_recording -- --nocapture

# Check coverage
cargo tarpaulin --package api --lib -- transaction_recording

# Lint check
cargo clippy --package api

# Format check
cargo fmt --package api -- --check
```

---

**End of Work Stream 1 Implementation Plan**

**Next:** After EPIC-1 is complete, proceed to:
- Work Stream 2 (Deal Operations Integration)
- Work Stream 3 (Collateral zkVM Implementation)
- Work Stream 6 (Test Scripts - can start in parallel)
