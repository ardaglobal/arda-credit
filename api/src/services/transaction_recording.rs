//! Transaction Recording Service
//!
//! This service records transactions in the state_transactions table for zkVM batch processing.
//! It provides a consistent interface for all API endpoints to create transaction records
//! that will later be batched and proven via zero-knowledge proofs.
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
//! ```rust,no_run
//! use api::services::TransactionRecordingService;
//! use arda_credit_lib::TransactionType;
//! use serde_json::json;
//! use rust_decimal::Decimal;
//! use uuid::Uuid;
//!
//! # async fn example(
//! #     state: api::config::types::ApiState,
//! #     user_id: Uuid,
//! #     deal_id: Uuid,
//! #     principal_amount: Decimal,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
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
//!         "investor_id": "...",
//!         "interest_rate_bps": 500,
//!     })),
//! ).await?;
//! # Ok(())
//! # }
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

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{error, info, instrument};
use uuid::Uuid;

use arda_credit_db::DbError;
use arda_credit_lib::{DealTransaction, MetadataArray};
use rust_decimal::prelude::ToPrimitive;

/// Service for recording transactions for zkVM processing
#[derive(Clone)]
pub struct TransactionRecordingService {
    pool: PgPool,
}

/// Request to record a transaction
#[derive(Debug, Clone)]
pub struct TransactionRecordingRequest {
    pub transaction_type: arda_credit_lib::TransactionType,
    pub user_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub amount: rust_decimal::Decimal,
    pub metadata: Option<serde_json::Value>,
}

impl TransactionRecordingRequest {
    /// Create a new transaction recording request
    pub fn new(
        transaction_type: arda_credit_lib::TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: rust_decimal::Decimal,
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

impl TransactionRecordingService {
    /// Create a new transaction recording service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

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

    /// Record a transaction for zkVM batch processing
    ///
    /// Creates a record in the state_transactions table with serialized DealTransaction data.
    /// The transaction will later be picked up by the batch processor, included in a zkVM proof,
    /// and posted to the smart contract.
    ///
    /// # Arguments
    /// * `deal_transaction` - Complete DealTransaction struct from arda-credit-lib
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The database ID that was created
    /// * `Err(DbError)` - Database error if insertion fails
    ///
    /// # Example
    /// ```rust,no_run
    /// use api::services::TransactionRecordingService;
    /// use arda_credit_lib::{DealTransaction, TransactionType, MetadataArray};
    /// use uuid::Uuid;
    ///
    /// # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    /// let tx_service = TransactionRecordingService::new(pool.clone());
    ///
    /// let deal_tx = DealTransaction {
    ///     transaction_id: [0u8; 32],
    ///     user_id: [0u8; 32],
    ///     originator_company_id: [0u8; 32],
    ///     investor_company_id: [0u8; 32],
    ///     deal_id: [0u8; 32],
    ///     transaction_type: TransactionType::UserRegistration,
    ///     amount: 0,
    ///     created_at: 0,
    ///     metadata: MetadataArray::empty(),
    /// };
    ///
    /// let db_id = tx_service.record_transaction(deal_tx).await?;
    /// # Ok(())
    /// # }
    /// ```
    /// Record a transaction for zkVM batch processing
    ///
    /// Creates a record in the state_transactions table with serialized DealTransaction data.
    /// The transaction will later be picked up by the batch processor, included in a zkVM proof,
    /// and posted to the smart contract.
    ///
    /// # Arguments
    /// * `deal_transaction` - Complete DealTransaction struct from arda-credit-lib
    /// * `original_user_id` - Original user UUID (before hashing) for query column
    /// * `original_deal_id` - Original deal UUID (before hashing) for query column, if applicable
    /// * `original_originator_company_id` - Original originator company UUID for query column, if applicable
    /// * `original_investor_company_id` - Original investor company UUID for query column, if applicable
    /// * `original_collateral_id` - Original collateral UUID for query column, if applicable
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The database ID that was created
    /// * `Err(DbError)` - Database error if insertion fails
    #[instrument(skip(self, deal_transaction), level = "info")]
    pub async fn record_transaction(
        &self,
        deal_transaction: DealTransaction,
        original_user_id: Option<Uuid>,
        original_deal_id: Option<Uuid>,
        original_originator_company_id: Option<Uuid>,
        original_investor_company_id: Option<Uuid>,
        original_collateral_id: Option<Uuid>,
    ) -> Result<Uuid, DbError> {
        let transaction_type = deal_transaction.transaction_type.to_string();

        // Serialize the DealTransaction using bincode
        let transaction_data =
            bincode::serde::encode_to_vec(&deal_transaction, bincode::config::standard()).map_err(
                |e| {
                    error!("Failed to serialize transaction: {}", e);
                    DbError::ValidationError(format!("Serialization error: {}", e))
                },
            )?;

        // Insert into state_transactions table with extracted UUIDs for querying
        let db_id = arda_credit_db::create_state_transaction(
            &self.pool,
            &transaction_type,
            &transaction_data,
            original_user_id,
            original_deal_id,
            original_originator_company_id,
            original_investor_company_id,
            original_collateral_id,
        )
        .await?;

        info!("Recorded {} transaction: db_id={}", transaction_type, db_id);

        // Record metrics
        metrics::counter!(
            "zkvm_transactions_recorded_total",
            "type" => transaction_type
        )
        .increment(1);

        Ok(db_id)
    }

    /// Helper to create and record a transaction from simplified inputs
    ///
    /// This is a convenience method for API handlers that constructs a complete
    /// DealTransaction struct and records it.
    ///
    /// # Arguments
    /// * `transaction_type` - Type of transaction
    /// * `user_id` - UUID of the user initiating the transaction
    /// * `deal_id` - Optional UUID of the deal
    /// * `amount` - Transaction amount (Decimal::ZERO for non-monetary)
    /// * `originator_company_id` - Optional originator company UUID
    /// * `investor_company_id` - Optional investor company UUID
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The database ID that was created
    /// * `Err(DbError)` - Database error if insertion fails
    #[instrument(skip(self), level = "info")]
    pub async fn record_transaction_simple(
        &self,
        transaction_type: arda_credit_lib::TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: rust_decimal::Decimal,
        originator_company_id: Option<Uuid>,
        investor_company_id: Option<Uuid>,
    ) -> Result<Uuid, DbError> {
        use arda_credit_lib::hash_uuid;

        // Generate transaction ID
        let transaction_id_uuid = Uuid::new_v4();
        let transaction_id = hash_uuid(&transaction_id_uuid);
        let user_id_hash = hash_uuid(&user_id);

        // Hash deal ID if present
        let deal_id_hash = deal_id.map(|id| hash_uuid(&id)).unwrap_or([0u8; 32]);

        // Hash company IDs if present
        let originator_hash = originator_company_id
            .map(|id| hash_uuid(&id))
            .unwrap_or([0u8; 32]);
        let investor_hash = investor_company_id
            .map(|id| hash_uuid(&id))
            .unwrap_or([0u8; 32]);

        // Convert amount to u64 (6 decimal places)
        let amount_u64 = (amount * rust_decimal::Decimal::from(1_000_000))
            .to_u64()
            .unwrap_or(0);

        // Get current timestamp
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Create DealTransaction
        let deal_transaction = DealTransaction {
            transaction_id,
            user_id: user_id_hash,
            originator_company_id: originator_hash,
            investor_company_id: investor_hash,
            deal_id: deal_id_hash,
            transaction_type,
            amount: amount_u64,
            created_at,
            metadata: MetadataArray::empty(),
        };

        // Pass original UUIDs (before hashing) for query columns
        self.record_transaction(
            deal_transaction,
            Some(user_id),
            deal_id,
            originator_company_id,
            investor_company_id,
            None, // collateral_id not available in this method
        )
        .await
    }

    /// Record a transaction with metadata (convenience method for handlers)
    ///
    /// This method matches the signature used by API handlers and constructs
    /// a complete DealTransaction with metadata encoding.
    ///
    /// # Arguments
    /// * `transaction_type` - Type of transaction
    /// * `user_id` - UUID of the user initiating the transaction
    /// * `deal_id` - Optional UUID of the deal
    /// * `amount` - Transaction amount (Decimal::ZERO for non-monetary)
    /// * `metadata` - Optional JSON metadata to encode in transaction
    /// * `originator_company_id` - Optional originator company UUID
    /// * `investor_company_id` - Optional investor company UUID
    /// * `collateral_id` - Optional collateral UUID
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The database ID that was created
    /// * `Err(DbError)` - Database error if insertion fails
    #[instrument(skip(self), level = "info")]
    pub async fn record_transaction_with_metadata(
        &self,
        transaction_type: arda_credit_lib::TransactionType,
        user_id: Uuid,
        deal_id: Option<Uuid>,
        amount: rust_decimal::Decimal,
        metadata: Option<serde_json::Value>,
        originator_company_id: Option<Uuid>,
        investor_company_id: Option<Uuid>,
        collateral_id: Option<Uuid>,
    ) -> Result<Uuid, DbError> {
        use arda_credit_lib::hash_uuid;

        // Generate transaction ID
        let transaction_id_uuid = Uuid::new_v4();
        let transaction_id = hash_uuid(&transaction_id_uuid);
        let user_id_hash = hash_uuid(&user_id);

        // Hash deal ID if present
        let deal_id_hash = deal_id.map(|id| hash_uuid(&id)).unwrap_or([0u8; 32]);

        // Hash company IDs if present
        let originator_hash = originator_company_id
            .map(|id| hash_uuid(&id))
            .unwrap_or([0u8; 32]);
        let investor_hash = investor_company_id
            .map(|id| hash_uuid(&id))
            .unwrap_or([0u8; 32]);

        // Convert amount to u64 (6 decimal places)
        let amount_u64 = (amount * rust_decimal::Decimal::from(1_000_000))
            .to_u64()
            .unwrap_or(0);

        // Get current timestamp
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Encode metadata
        let metadata_array = if let Some(meta) = metadata {
            // Serialize metadata JSON to bytes
            let metadata_bytes = serde_json::to_string(&meta)
                .unwrap_or_else(|_| "{}".to_string())
                .into_bytes();
            MetadataArray::from_bytes(&metadata_bytes)
        } else {
            MetadataArray::empty()
        };

        // Create DealTransaction
        let deal_transaction = DealTransaction {
            transaction_id,
            user_id: user_id_hash,
            originator_company_id: originator_hash,
            investor_company_id: investor_hash,
            deal_id: deal_id_hash,
            transaction_type,
            amount: amount_u64,
            created_at,
            metadata: metadata_array,
        };

        // Pass original UUIDs (before hashing) for query columns
        self.record_transaction(
            deal_transaction,
            Some(user_id),
            deal_id,
            originator_company_id,
            investor_company_id,
            collateral_id,
        )
        .await
    }

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
        use arda_credit_lib::hash_uuid;

        let mut tx = self.pool.begin().await?;
        let mut transaction_ids = Vec::with_capacity(transactions.len());

        for request in transactions {
            // Generate transaction ID and hash
            let transaction_id_uuid = Uuid::new_v4();
            let transaction_id = hash_uuid(&transaction_id_uuid);
            let user_id_hash = hash_uuid(&request.user_id);

            // Hash deal ID if present
            let deal_id_hash = request
                .deal_id
                .map(|id| hash_uuid(&id))
                .unwrap_or([0u8; 32]);

            // Convert amount to u64 (6 decimal places)
            let amount_u64 = (request.amount * rust_decimal::Decimal::from(1_000_000))
                .to_u64()
                .unwrap_or(0);

            // Get current timestamp
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Encode metadata
            let metadata_array = if let Some(meta) = &request.metadata {
                let metadata_bytes = serde_json::to_string(meta)
                    .unwrap_or_else(|_| "{}".to_string())
                    .into_bytes();
                MetadataArray::from_bytes(&metadata_bytes)
            } else {
                MetadataArray::empty()
            };

            // Create DealTransaction
            let deal_transaction = DealTransaction {
                transaction_id,
                user_id: user_id_hash,
                originator_company_id: [0u8; 32], // Not available in TransactionRecordingRequest
                investor_company_id: [0u8; 32],   // Not available in TransactionRecordingRequest
                deal_id: deal_id_hash,
                transaction_type: request.transaction_type,
                amount: amount_u64,
                created_at,
                metadata: metadata_array,
            };

            // Serialize transaction data
            let transaction_data =
                bincode::serde::encode_to_vec(&deal_transaction, bincode::config::standard())
                    .map_err(|e| {
                        error!("Failed to serialize transaction: {}", e);
                        DbError::ValidationError(format!("Serialization error: {}", e))
                    })?;

            // Insert using new schema
            let db_id = sqlx::query!(
                r#"
                INSERT INTO state_transactions (
                    transaction_type,
                    transaction_data,
                    user_id,
                    deal_id
                )
                VALUES ($1, $2, $3, $4)
                RETURNING id
                "#,
                request.transaction_type.to_string(),
                &transaction_data[..],
                request.user_id,
                request.deal_id,
            )
            .fetch_one(&mut *tx)
            .await?;

            transaction_ids.push(db_id.id);
        }

        tx.commit().await?;

        info!("Recorded batch of {} transactions", transaction_ids.len());

        metrics::counter!("zkvm_batch_transactions_recorded_total")
            .increment(transaction_ids.len() as u64);

        Ok(transaction_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    use std::str::FromStr;

    // Helper to create test database pool
    async fn setup_test_db() -> PgPool {
        // Use test database or in-memory SQLite
        let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/arda_credit_test".to_string()
        });

        sqlx::PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    #[ignore = "Requires test database - run with: cargo test --package api -- --ignored"]
    async fn test_record_user_registration() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());

        let user_id = Uuid::new_v4();
        let metadata = Some(serde_json::json!({
            "email": "test@example.com",
            "user_type": "investor"
        }));

        let result = service
            .record_transaction(
                arda_credit_lib::TransactionType::UserRegistration,
                user_id,
                None,
                rust_decimal::Decimal::ZERO,
                metadata,
            )
            .await;

        assert!(result.is_ok());
        let transaction_id = result.unwrap();

        // Verify record was created
        let record = sqlx::query!(
            "SELECT * FROM state_transactions WHERE id = $1",
            transaction_id
        )
        .fetch_one(&pool)
        .await
        .expect("Transaction should exist");

        assert_eq!(record.transaction_type, "user_registration");
        assert_eq!(record.user_id, Some(user_id));
        assert_eq!(record.deal_id, None);
        assert_eq!(record.amount, rust_decimal::Decimal::ZERO);
    }

    #[tokio::test]
    #[ignore = "Requires test database - run with: cargo test --package api -- --ignored"]
    async fn test_record_deal_origination() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());

        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();
        let amount = rust_decimal::Decimal::from_str("100000.50").unwrap();

        let result = service
            .record_transaction(
                arda_credit_lib::TransactionType::DealOrigination,
                user_id,
                Some(deal_id),
                amount,
                None,
            )
            .await;

        assert!(result.is_ok());
        let transaction_id = result.unwrap();

        // Verify record
        let record = sqlx::query!(
            "SELECT * FROM state_transactions WHERE id = $1",
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
    #[ignore = "Requires test database - run with: cargo test --package api -- --ignored"]
    async fn test_all_transaction_types() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());

        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();

        // Test each transaction type
        let types = vec![
            arda_credit_lib::TransactionType::UserRegistration,
            arda_credit_lib::TransactionType::DealOrigination,
            arda_credit_lib::TransactionType::DealPayment,
            arda_credit_lib::TransactionType::DealCloseout,
            arda_credit_lib::TransactionType::CollateralCreation,
            arda_credit_lib::TransactionType::CollateralUpdate,
            arda_credit_lib::TransactionType::CollateralDealAssociation,
            arda_credit_lib::TransactionType::CollateralDealDisassociation,
            arda_credit_lib::TransactionType::CollateralDetectionRequest,
            arda_credit_lib::TransactionType::CollateralDetectionUpdate,
        ];

        for tx_type in types {
            let result = service
                .record_transaction(
                    tx_type,
                    user_id,
                    Some(deal_id),
                    rust_decimal::Decimal::ZERO,
                    None,
                )
                .await;

            assert!(result.is_ok(), "Failed to record {} transaction", tx_type);
        }

        // Verify all were created
        let count = sqlx::query!(
            "SELECT COUNT(*) as count FROM state_transactions WHERE user_id = $1",
            user_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(count.count, Some(10));
    }

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

        assert_eq!(
            our_hash, expected_hash,
            "Hash must match test script implementation"
        );
    }

    #[tokio::test]
    #[ignore = "Requires test database - run with: cargo test --package api -- --ignored"]
    async fn test_batch_recording() {
        let pool = setup_test_db().await;
        let service = TransactionRecordingService::new(pool.clone());

        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();

        let requests = vec![
            TransactionRecordingRequest::new(
                arda_credit_lib::TransactionType::UserRegistration,
                user_id,
                None,
                rust_decimal::Decimal::ZERO,
                None,
            ),
            TransactionRecordingRequest::new(
                arda_credit_lib::TransactionType::DealOrigination,
                user_id,
                Some(deal_id),
                rust_decimal::Decimal::from(100000),
                None,
            ),
            TransactionRecordingRequest::new(
                arda_credit_lib::TransactionType::DealPayment,
                user_id,
                Some(deal_id),
                rust_decimal::Decimal::from(50000),
                None,
            ),
        ];

        let result = service.record_transactions_batch(requests).await;

        assert!(result.is_ok());
        let transaction_ids = result.unwrap();
        assert_eq!(transaction_ids.len(), 3);

        // Verify all created
        for tx_id in transaction_ids {
            let exists = sqlx::query!("SELECT id FROM state_transactions WHERE id = $1", tx_id)
                .fetch_optional(&pool)
                .await
                .unwrap();

            assert!(exists.is_some());
        }
    }
}
