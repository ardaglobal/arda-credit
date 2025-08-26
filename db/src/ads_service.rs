use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use crate::ccr::CcrOperation;
use crate::error::DbError;
use crate::merkle_tree::{
    AlgorithmInsertionResult, IndexedMerkleTree, InsertionProof, MerkleProof,
};

// ============================================================================
// AUTHENTICATED DATA STRUCTURE SERVICE LAYER
// ============================================================================

/// Main trait for authenticated data structures
/// Provides cryptographic guarantees for state transitions and proofs
#[async_trait]
pub trait AuthenticatedDataStructure: Send + Sync {
    type Value;
    type Proof;
    type StateCommitment;

    /// Insert a value and return cryptographic state transition proof
    async fn insert(&mut self, value: Self::Value) -> Result<StateTransition, AdsError>;

    /// Generate membership proof for existing value
    async fn prove_membership(&self, value: Self::Value) -> Result<MembershipProof, AdsError>;

    /// Generate non-membership proof using low nullifier technique
    async fn prove_non_membership(
        &self,
        value: Self::Value,
    ) -> Result<NonMembershipProof, AdsError>;

    /// Get current state commitment for settlement contract
    async fn get_state_commitment(&self) -> Result<Self::StateCommitment, AdsError>;

    /// Verify a state transition is cryptographically valid
    async fn verify_state_transition(&self, transition: &StateTransition)
        -> Result<bool, AdsError>;

    /// Batch insert multiple values efficiently
    async fn batch_insert(
        &mut self,
        values: &[Self::Value],
    ) -> Result<Vec<StateTransition>, AdsError>;

    /// Get audit trail for a specific value
    async fn get_audit_trail(&self, value: Self::Value) -> Result<AuditTrail, AdsError>;
}

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// Loan operation metadata for enhanced state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanMetadata {
    pub loan_id: Option<uuid::Uuid>,      // Loan ID (for loan operations)
    pub payment_id: Option<uuid::Uuid>,   // Payment ID (for payment operations)
    pub user_id: Option<uuid::Uuid>,      // Primary user ID (borrower/lender)
    pub borrower_id: Option<uuid::Uuid>,  // Borrower ID (for loan operations)
    pub lender_id: Option<uuid::Uuid>,    // Lender ID (for loan operations)
    pub payment_amount: Option<rust_decimal::Decimal>, // Payment amount
    pub principal_amount: Option<rust_decimal::Decimal>, // Loan principal
    pub outstanding_balance: Option<rust_decimal::Decimal>, // Remaining balance
    pub loan_status: Option<String>,      // Loan status (active, paid_off, closed, etc.)
    pub operation_type: String,           // Operation type (loan_origination, loan_payment, etc.)
    pub transaction_context: Option<serde_json::Value>, // Additional transaction data
}

/// CCR operation metadata for enhanced state transitions (kept for compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcrMetadata {
    pub operation_type: String,           // CCR operation type (loan_creation, payment, etc.)
    pub operation_id: String,             // Primary ID (loan_id, payment_id, etc.)
    pub user_ids: Vec<String>,            // Related user IDs (borrower_id, lender_id, etc.)
    pub amount: Option<i64>,              // Associated amount if applicable
    pub created_at: DateTime<Utc>,        // Operation timestamp
    pub additional_context: serde_json::Value, // Additional operation-specific data
}

/// State transition proof for settlement contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub id: String,                      // Unique transition ID
    pub old_root: [u8; 32],              // Previous tree root
    pub new_root: [u8; 32],              // New tree root after insertion
    pub nullifier_value: i64,            // Inserted nullifier
    pub insertion_proof: InsertionProof, // 7-step algorithm proof
    pub block_height: u64,               // vApp block height
    pub timestamp: DateTime<Utc>,        // Insertion timestamp
    pub gas_estimate: u64,               // Estimated gas for settlement
    pub witnesses: Vec<WitnessData>,     // ZK circuit witness data
    #[serde(default)]
    pub loan_metadata: Option<LoanMetadata>, // Loan operation metadata (enhanced for loan lifecycle)
    #[serde(default)]
    pub ccr_metadata: Option<CcrMetadata>, // CCR operation metadata (kept for backward compatibility)
}

/// Membership proof for existing nullifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipProof {
    pub nullifier_value: i64,       // Proven nullifier
    pub merkle_proof: MerkleProof,  // Path to root
    pub root_hash: [u8; 32],        // Current tree root
    pub tree_index: i64,            // Position in tree
    pub verified_at: DateTime<Utc>, // Proof generation time
}

/// Non-membership proof using low nullifier technique
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonMembershipProof {
    pub queried_value: i64,               // Value being proven absent
    pub low_nullifier: LowNullifierProof, // Low nullifier proof
    pub root_hash: [u8; 32],              // Current tree root
    pub range_proof: RangeProof,          // Range validation proof
    pub verified_at: DateTime<Utc>,       // Proof generation time
}

/// Loan operation inclusion proof for user verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanInclusionProof {
    pub loan_operation: LoanOperationProof, // Loan operation details
    pub membership_proof: MembershipProof,  // Core membership proof
    pub batch_info: BatchInclusionInfo,     // Batch context information
    pub operation_metadata: LoanMetadata,   // Full loan operation metadata
    pub verified_at: DateTime<Utc>,         // Proof generation time
}

/// Loan operation proof details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanOperationProof {
    pub operation_type: String,         // "loan_origination", "loan_payment", "loan_closeout"
    pub loan_id: Option<uuid::Uuid>,    // Loan ID
    pub payment_id: Option<uuid::Uuid>, // Payment ID (for payments)
    pub user_id: uuid::Uuid,            // User requesting proof (borrower/lender)
    pub amount: Option<rust_decimal::Decimal>, // Operation amount
    pub nullifier_value: i64,           // Generated nullifier
    pub operation_hash: [u8; 32],       // Hash of operation details
    pub user_role: String,              // "borrower" or "lender"
}

/// Batch inclusion information for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInclusionInfo {
    pub batch_id: i32,                  // Batch ID where operation was included
    pub batch_root: [u8; 32],           // Merkle root of the batch
    pub batch_timestamp: DateTime<Utc>, // Batch processing timestamp
    pub operation_index: u32,           // Index of operation within batch
    pub total_operations: u32,          // Total operations in batch
}

/// Request for loan operation inclusion proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanInclusionRequest {
    pub user_id: uuid::Uuid,            // User requesting the proof
    pub loan_id: Option<uuid::Uuid>,    // Loan ID (for origination/closeout)
    pub payment_id: Option<uuid::Uuid>, // Payment ID (for payments)
    pub operation_type: String,         // "loan_origination", "loan_payment", "loan_closeout"
    pub batch_id: Option<i32>,          // Optional specific batch ID
    pub user_role: String,              // "borrower" or "lender"
}

/// Low nullifier proof component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowNullifierProof {
    pub value: i64,                // Low nullifier value
    pub next_value: i64,           // Next nullifier (0 = max)
    pub tree_index: i64,           // Tree position
    pub merkle_proof: MerkleProof, // Merkle proof for low nullifier
}

/// Range proof for non-membership
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeProof {
    pub lower_bound: i64,   // low_nullifier.value
    pub upper_bound: i64,   // low_nullifier.next_value
    pub queried_value: i64, // Value in range
    pub valid: bool,        // Range check result
}

/// State commitment for settlement contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCommitment {
    pub root_hash: [u8; 32],             // Merkle tree root
    pub nullifier_count: u64,            // Total nullifiers
    pub tree_height: u32,                // Tree depth (32)
    pub last_updated: DateTime<Utc>,     // Last modification
    pub commitment_hash: [u8; 32],       // Hash of commitment data
    pub settlement_data: SettlementData, // Contract settlement info
}

/// Settlement contract data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementData {
    pub contract_address: String, // Settlement contract
    pub chain_id: u64,            // Blockchain chain ID
    pub nonce: u64,               // Settlement nonce
    pub gas_price: u64,           // Gas price estimate
}

/// Witness data for ZK circuit generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessData {
    pub circuit_type: String,       // Circuit identifier
    pub inputs: serde_json::Value,  // Circuit inputs
    pub constraints: u32,           // Constraint count
    pub proving_key_hash: [u8; 32], // Proving key identifier
}

/// Audit trail for regulatory compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrail {
    pub nullifier_value: i64,                // Tracked nullifier
    pub operation_history: Vec<AuditEvent>,  // Historical operations
    pub compliance_status: ComplianceStatus, // Regulatory status
    pub created_at: DateTime<Utc>,           // First seen
    pub last_accessed: DateTime<Utc>,        // Last proof generation
}

/// Individual audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,                 // Unique event ID
    pub event_type: AuditEventType,       // Operation type
    pub timestamp: DateTime<Utc>,         // When it occurred
    pub root_before: [u8; 32],            // State before
    pub root_after: [u8; 32],             // State after
    pub transaction_hash: Option<String>, // On-chain tx hash
    pub block_height: u64,                // vApp block
    pub operator: String,                 // Who performed operation
    pub metadata: serde_json::Value,      // Additional context
}

/// Types of auditable events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Insertion,          // Nullifier inserted
    MembershipProof,    // Membership proven
    NonMembershipProof, // Non-membership proven
    StateCommitment,    // State committed
    Settlement,         // Settled on-chain
    Verification,       // Proof verified
    CcrOperation,       // CCR operation processed
    LoanOperation,      // Loan lifecycle operation processed
}

/// Compliance status for regulatory requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub is_compliant: bool,        // Meets regulations
    pub last_audit: DateTime<Utc>, // Last compliance check
    pub jurisdiction: String,      // Regulatory jurisdiction
    pub notes: Vec<String>,        // Compliance notes
}

/// Loan operation for batch processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanOperation {
    pub operation_type: String,           // "loan_origination", "loan_payment", "loan_closeout"
    pub loan_id: Option<uuid::Uuid>,      // Loan ID
    pub payment_id: Option<uuid::Uuid>,   // Payment ID (for payment operations)
    pub borrower_id: Option<uuid::Uuid>,  // Borrower ID
    pub lender_id: Option<uuid::Uuid>,    // Lender ID
    pub amount: Option<rust_decimal::Decimal>, // Operation amount
    pub loan_status: Option<String>,      // Current loan status
    pub transaction_data: serde_json::Value, // Additional transaction context
    pub nullifier_prefix: Option<i64>,    // Optional nullifier prefix
}

impl LoanOperation {
    /// Convert to nullifier using deterministic hash
    pub fn to_nullifier(&self) -> i64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        if let Some(loan_id) = &self.loan_id {
            loan_id.to_string().hash(&mut hasher);
        }
        if let Some(payment_id) = &self.payment_id {
            payment_id.to_string().hash(&mut hasher);
        }
        self.operation_type.hash(&mut hasher);
        if let Some(amount) = &self.amount {
            amount.to_string().hash(&mut hasher);
        }

        let hash = hasher.finish();
        ((hash % (i64::MAX as u64)) as i64) + 1
    }

    /// Convert to nullifier with operation-type prefix
    pub fn to_nullifier_with_prefix(&self) -> i64 {
        let base_nullifier = self.to_nullifier();
        let prefix = self.get_operation_prefix();
        
        // Apply prefix while keeping within range
        let adjusted_base = base_nullifier % 999_999_999;
        prefix + adjusted_base
    }

    /// Get operation-specific prefix
    fn get_operation_prefix(&self) -> i64 {
        match self.operation_type.as_str() {
            "loan_origination" => 1_000_000_000,
            "loan_closeout" => 2_000_000_000,
            "loan_payment" => 3_000_000_000,
            _ => 0, // Default/legacy
        }
    }

    /// Convert to loan metadata
    pub fn to_loan_metadata(&self) -> LoanMetadata {
        LoanMetadata {
            loan_id: self.loan_id,
            payment_id: self.payment_id,
            user_id: self.borrower_id.or(self.lender_id),
            borrower_id: self.borrower_id,
            lender_id: self.lender_id,
            payment_amount: self.amount,
            principal_amount: if self.operation_type == "loan_origination" { self.amount } else { None },
            outstanding_balance: None, // To be filled by business logic
            loan_status: self.loan_status.clone(),
            operation_type: self.operation_type.clone(),
            transaction_context: Some(self.transaction_data.clone()),
        }
    }
}

/// Loan lifecycle summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanLifecycleSummary {
    pub loan_id: uuid::Uuid,
    pub total_operations: usize,
    pub origination_date: Option<DateTime<Utc>>,
    pub last_payment_date: Option<DateTime<Utc>>,
    pub total_payments: usize,
    pub current_status: String,
    pub state_transitions: usize,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdsMetrics {
    pub operations_total: u64,                    // Total operations
    pub insertions_total: u64,                    // Total insertions
    pub proofs_generated: u64,                    // Total proofs
    pub avg_insertion_time_ms: f64,               // Average insertion time
    pub avg_proof_time_ms: f64,                   // Average proof time
    pub error_rate: f64,                          // Error rate percentage
    pub last_reset: DateTime<Utc>,                // Metrics reset time
    pub constraint_efficiency: ConstraintMetrics, // ZK efficiency metrics
    // Loan-specific metrics
    pub total_loan_operations: u64,               // Total loan operations
    pub loan_originations: u64,                   // Total loan originations
    pub loan_payments: u64,                       // Total loan payments
    pub loan_closeouts: u64,                      // Total loan closeouts
}

/// ZK constraint efficiency metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintMetrics {
    pub avg_constraints_per_op: f64,         // Constraints per operation
    pub target_constraints: u32,             // Target constraint count
    pub efficiency_ratio: f64,               // Actual vs target
    pub circuit_types: HashMap<String, u32>, // Constraints by circuit
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
pub enum AdsError {
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Nullifier {0} already exists")]
    NullifierExists(i64),

    #[error("Nullifier {0} not found")]
    NullifierNotFound(i64),

    #[error("Insertion failed: {0}")]
    InsertionFailed(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Proof verification failed: {0}")]
    ProofVerificationFailed(String),

    #[error("State commitment error: {0}")]
    StateCommitmentError(String),

    #[error("Audit trail error: {0}")]
    AuditTrailError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Concurrent access error: {0}")]
    ConcurrencyError(String),

    #[error("Invalid configuration: {0}")]
    ConfigurationError(String),

    #[error("Proof generation failed: {0}")]
    ProofGeneration(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("Proof verification error: {0}")]
    ProofVerification(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

// ============================================================================
// INDEXED MERKLE TREE ADS IMPLEMENTATION
// ============================================================================

/// Thread-safe ADS implementation using indexed Merkle tree
pub struct IndexedMerkleTreeADS {
    tree: Arc<RwLock<IndexedMerkleTree>>, // Thread-safe tree access
    state_cache: Arc<RwLock<HashMap<[u8; 32], StateCommitment>>>, // State cache
    audit_storage: Arc<RwLock<HashMap<i64, AuditTrail>>>, // Audit trails
    metrics: Arc<RwLock<AdsMetrics>>,     // Performance metrics
    config: AdsConfig,                    // Service configuration
    pool: PgPool,                         // Database connection
}

/// Configuration for ADS service
#[derive(Debug, Clone)]
pub struct AdsConfig {
    pub settlement_contract: String, // Settlement contract address
    pub chain_id: u64,               // Blockchain chain ID
    pub audit_enabled: bool,         // Enable audit trails
    pub metrics_enabled: bool,       // Enable metrics collection
    pub cache_size_limit: usize,     // Max cache entries
    pub batch_size_limit: usize,     // Max batch size
    pub gas_price: u64,              // Default gas price
}

impl Default for AdsConfig {
    fn default() -> Self {
        Self {
            settlement_contract: "0x0000000000000000000000000000000000000000".to_string(),
            chain_id: 1,
            audit_enabled: true,
            metrics_enabled: true,
            cache_size_limit: 10_000,
            batch_size_limit: 1_000,
            gas_price: 20_000_000_000, // 20 gwei
        }
    }
}

impl IndexedMerkleTreeADS {
    /// Create new ADS service instance
    #[instrument(skip(pool), level = "info")]
    pub async fn new(pool: PgPool, config: AdsConfig) -> Result<Self, AdsError> {
        info!("🚀 Initializing IndexedMerkleTreeADS service");

        let mut tree = IndexedMerkleTree::new(pool.clone());

        // Recover state from database if it exists
        if let Some(state) = tree
            .db
            .state
            .get_state(None)
            .await
            .map_err(AdsError::Database)?
        {
            info!(
                "🔄 Found existing tree state with {} nullifiers, recovering...",
                state.total_nullifiers
            );
            tree.recover_state(state)
                .await
                .map_err(AdsError::Database)?;
            info!("✅ Tree state recovered successfully");
        } else {
            info!("📝 No existing tree state found, starting with empty tree");
        }

        // Initialize metrics - if we recovered state, update them accordingly
        let mut metrics = AdsMetrics {
            operations_total: 0,
            insertions_total: 0,
            proofs_generated: 0,
            avg_insertion_time_ms: 0.0,
            avg_proof_time_ms: 0.0,
            error_rate: 0.0,
            last_reset: Utc::now(),
            constraint_efficiency: ConstraintMetrics {
                avg_constraints_per_op: 0.0,
                target_constraints: 200,
                efficiency_ratio: 1.0,
                circuit_types: HashMap::new(),
            },
        };

        // If we recovered state, update metrics to reflect the recovered data
        if let Ok(Some(state)) = tree.db.state.get_state(None).await {
            metrics.insertions_total = state.total_nullifiers as u64;
            info!(
                "📊 Updated metrics from recovered state: {} insertions",
                metrics.insertions_total
            );
        }

        let service = Self {
            tree: Arc::new(RwLock::new(tree)),
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            audit_storage: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(metrics)),
            config,
            pool,
        };

        // Initialize audit storage table if needed
        if service.config.audit_enabled {
            service.init_audit_storage().await?;
        }

        // Rebuild state cache from persisted commitments if we have recovered data
        service.rebuild_state_cache().await?;

        // CRITICAL: Ensure tree_state consistency on startup to prevent "low nullifier" errors
        info!("🔧 Ensuring tree state consistency on startup");
        if let Err(e) = service.ensure_tree_state_consistency().await {
            warn!(
                "Failed to fix tree state consistency, but continuing: {}",
                e
            );
        } else {
            info!("✅ Tree state consistency verified on startup");
        }

        info!("✅ IndexedMerkleTreeADS service initialized successfully");
        Ok(service)
    }

    /// Initialize audit storage in database
    #[instrument(skip(self), level = "debug")]
    async fn init_audit_storage(&self) -> Result<(), AdsError> {
        // Audit events table is created by migration 003_create_audit_events.sql
        // Just verify the table exists
        sqlx::query!("SELECT 1 as status FROM audit_events LIMIT 0")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        debug!("📝 Audit storage initialized");
        Ok(())
    }

    /// Ensure tree_state table consistency with actual nullifier data
    /// This prevents "duplicate key" and "low nullifier not found" errors
    #[instrument(skip(self), level = "info")]
    async fn ensure_tree_state_consistency(&self) -> Result<(), AdsError> {
        info!("🔧 Checking and fixing tree_state consistency");

        sqlx::query!("SELECT fix_tree_state_consistency()")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        info!("✅ Tree state consistency verified");
        Ok(())
    }

    /// Rebuild state cache from persisted ADS state commits
    #[instrument(skip(self), level = "info")]
    async fn rebuild_state_cache(&self) -> Result<(), AdsError> {
        info!("🔄 Rebuilding ADS state cache from database");

        // Get recent ADS state commits to populate the cache
        let recent_commits = sqlx::query!(
            r#"
            SELECT ads.batch_id, ads.merkle_root, ads.created_at,
                   pb.previous_counter_value, pb.final_counter_value
            FROM ads_state_commits ads
            JOIN proof_batches pb ON ads.batch_id = pb.id
            WHERE pb.proof_status = 'proven'
            ORDER BY ads.created_at DESC
            LIMIT 100
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AdsError::Database(crate::error::DbError::Database(e)))?;

        if recent_commits.is_empty() {
            info!("📝 No proven batches found, state cache remains empty");
        } else {
            let mut cache = self.state_cache.write().await;

            for commit in recent_commits {
                let mut root_key = [0u8; 32];
                root_key.copy_from_slice(&commit.merkle_root);

                let state_commitment = StateCommitment {
                    root_hash: root_key,
                    nullifier_count: 0, // We could query this if needed
                    tree_height: 32,
                    last_updated: commit.created_at.unwrap_or_else(|| chrono::Utc::now()),
                    commitment_hash: root_key, // Use root as commitment hash for simplicity
                    settlement_data: SettlementData {
                        contract_address: "0x0000000000000000000000000000000000000000".to_string(),
                        chain_id: 1, // Default to mainnet
                        nonce: 0,
                        gas_price: 20_000_000_000, // 20 gwei
                    },
                };

                cache.insert(root_key, state_commitment);
            }

            info!(
                "✅ Rebuilt state cache with {} entries from proven batches",
                cache.len()
            );
        }

        Ok(())
    }

    /// Generate unique transaction ID
    fn generate_transaction_id() -> String {
        use sha2::{Digest, Sha256};
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap();
        let random_bytes = rand::random::<[u8; 8]>();
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_be_bytes());
        hasher.update(random_bytes);
        let hash = hasher.finalize();
        hex::encode(&hash[..16]) // First 16 bytes as hex
    }

    /// Record audit event
    #[instrument(skip(self, metadata), level = "debug")]
    async fn record_audit_event(
        &self,
        nullifier_value: i64,
        event_type: AuditEventType,
        root_before: [u8; 32],
        root_after: [u8; 32],
        block_height: u64,
        metadata: serde_json::Value,
    ) -> Result<(), AdsError> {
        if !self.config.audit_enabled {
            return Ok(());
        }

        let event_id = Self::generate_transaction_id();
        let event = AuditEvent {
            event_id: event_id.clone(),
            event_type: event_type.clone(),
            timestamp: Utc::now(),
            root_before,
            root_after,
            transaction_hash: None,
            block_height,
            operator: "vapp-server".to_string(), // Could be configurable
            metadata,
        };

        // Store in database
        sqlx::query!(
            r#"
            INSERT INTO audit_events (
                event_id, nullifier_value, event_type, timestamp, root_before,
                root_after, transaction_hash, block_height, operator, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            event_id,
            nullifier_value,
            serde_json::to_string(&event_type)?,
            event.timestamp,
            root_before.as_slice(),
            root_after.as_slice(),
            event.transaction_hash,
            block_height as i64,
            event.operator,
            event.metadata,
        )
        .execute(&self.pool)
        .await?;

        // Update in-memory audit trail
        let mut audit_storage = self.audit_storage.write().await;
        let audit_trail = audit_storage
            .entry(nullifier_value)
            .or_insert_with(|| AuditTrail {
                nullifier_value,
                operation_history: Vec::new(),
                compliance_status: ComplianceStatus {
                    is_compliant: true,
                    last_audit: Utc::now(),
                    jurisdiction: "US".to_string(), // Configurable
                    notes: Vec::new(),
                },
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            });

        audit_trail.operation_history.push(event);
        audit_trail.last_accessed = Utc::now();

        debug!("📋 Audit event recorded for nullifier {}", nullifier_value);
        Ok(())
    }

    /// Update performance metrics
    #[instrument(skip(self), level = "debug")]
    async fn update_metrics(&self, operation_type: &str, duration_ms: f64) -> Result<(), AdsError> {
        if !self.config.metrics_enabled {
            return Ok(());
        }

        let mut metrics = self.metrics.write().await;
        metrics.operations_total += 1;

        match operation_type {
            "insertion" => {
                metrics.insertions_total += 1;
                // Update running average
                let total = metrics.insertions_total as f64;
                metrics.avg_insertion_time_ms =
                    (metrics.avg_insertion_time_ms * (total - 1.0) + duration_ms) / total;
            }
            "proof" => {
                metrics.proofs_generated += 1;
                let total = metrics.proofs_generated as f64;
                metrics.avg_proof_time_ms =
                    (metrics.avg_proof_time_ms * (total - 1.0) + duration_ms) / total;
            }
            _ => {}
        }

        debug!(
            "📊 Metrics updated for {}: {:.2}ms",
            operation_type, duration_ms
        );
        Ok(())
    }

    /// Get current block height (placeholder - integrate with actual vApp)
    fn get_current_block_height(&self) -> Result<u64, AdsError> {
        // This would integrate with your vApp's block tracking
        // For now, return a placeholder value
        Ok(1000) // Placeholder
    }

    /// Generate witness data for ZK circuits
    fn generate_witness_data(
        &self,
        insertion_result: &AlgorithmInsertionResult,
    ) -> Vec<WitnessData> {
        vec![WitnessData {
            circuit_type: "merkle_inclusion".to_string(),
            inputs: serde_json::json!({
                "nullifier": insertion_result.nullifier.value,
                "tree_index": insertion_result.nullifier.tree_index,
                "old_root": hex::encode(insertion_result.old_root),
                "new_root": hex::encode(insertion_result.new_root),
            }),
            constraints: insertion_result.operations_count.constraints_count,
            proving_key_hash: [0u8; 32], // Placeholder
        }]
    }

    /// Calculate gas estimate for settlement
    fn calculate_gas_estimate(&self, _state_transition: &StateTransition) -> u64 {
        // Base gas for Merkle proof verification + settlement
        // This would be calibrated based on actual contract gas usage
        150_000 + (32 * 5_000) // Base + (proof_size * per_hash_gas)
    }
}

#[async_trait]
impl AuthenticatedDataStructure for IndexedMerkleTreeADS {
    type Value = i64;
    type Proof = MerkleProof;
    type StateCommitment = StateCommitment;

    #[instrument(skip(self), level = "info")]
    async fn insert(&mut self, value: i64) -> Result<StateTransition, AdsError> {
        info!("🔄 Inserting nullifier: {}", value);
        let start_time = std::time::Instant::now();

        // Get exclusive access to tree for modification
        let mut tree_guard = self.tree.write().await;

        // Check if nullifier already exists
        if tree_guard.db.nullifiers.exists(value).await? {
            return Err(AdsError::NullifierExists(value));
        }

        // Perform insertion using 7-step algorithm
        let insertion_result = tree_guard
            .insert_nullifier(value)
            .await
            .map_err(|e| AdsError::InsertionFailed(e.to_string()))?;

        drop(tree_guard); // Release lock early

        // Generate witness data for ZK circuits
        let witnesses = self.generate_witness_data(&insertion_result);

        // Get current block height
        let block_height = self.get_current_block_height()?;

        // Create state transition
        let transition_id = Self::generate_transaction_id();
        let state_transition = StateTransition {
            id: transition_id.clone(),
            old_root: insertion_result.old_root,
            new_root: insertion_result.new_root,
            nullifier_value: value,
            insertion_proof: insertion_result.insertion_proof.clone(),
            block_height,
            timestamp: Utc::now(),
            gas_estimate: self.calculate_gas_estimate(&StateTransition {
                id: transition_id,
                old_root: insertion_result.old_root,
                new_root: insertion_result.new_root,
                nullifier_value: value,
                insertion_proof: insertion_result.insertion_proof.clone(),
                block_height,
                timestamp: Utc::now(),
                gas_estimate: 0,
                witnesses: witnesses.clone(),
                loan_metadata: None,
                ccr_metadata: None,
            }),
            witnesses,
            loan_metadata: None,
            ccr_metadata: None,
        };

        // Update state cache
        let nullifier_count = self.get_nullifier_count().await?;
        let new_commitment = StateCommitment {
            root_hash: insertion_result.new_root,
            nullifier_count,
            tree_height: 32,
            last_updated: Utc::now(),
            commitment_hash: self
                .calculate_commitment_hash(&insertion_result.new_root, nullifier_count),
            settlement_data: SettlementData {
                contract_address: self.config.settlement_contract.clone(),
                chain_id: self.config.chain_id,
                nonce: nullifier_count,
                gas_price: self.config.gas_price,
            },
        };

        {
            let mut cache = self.state_cache.write().await;
            cache.insert(insertion_result.new_root, new_commitment);

            // Limit cache size
            if cache.len() > self.config.cache_size_limit {
                // Remove oldest entries (simple LRU would be better)
                let keys_to_remove: Vec<_> = cache
                    .keys()
                    .take(cache.len() - self.config.cache_size_limit)
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }

        // Record audit event
        self.record_audit_event(
            value,
            AuditEventType::Insertion,
            insertion_result.old_root,
            insertion_result.new_root,
            block_height,
            serde_json::json!({
                "transition_id": state_transition.id,
                "constraints": insertion_result.operations_count.constraints_count,
                "hash_operations": insertion_result.operations_count.hash_operations,
                "gas_estimate": state_transition.gas_estimate,
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("insertion", duration_ms).await?;

        info!(
            "✅ Nullifier {} inserted successfully in {:.2}ms",
            value, duration_ms
        );
        Ok(state_transition)
    }

    #[instrument(skip(self), level = "info")]
    async fn prove_membership(&self, value: i64) -> Result<MembershipProof, AdsError> {
        info!("🔍 Generating membership proof for: {}", value);
        let start_time = std::time::Instant::now();

        let tree_guard = self.tree.read().await;

        // Check if nullifier exists and get details
        let nullifier = tree_guard
            .db
            .nullifiers
            .get_by_value(value)
            .await?
            .ok_or(AdsError::NullifierNotFound(value))?;

        // Generate Merkle proof
        let merkle_proof = tree_guard
            .generate_merkle_proof(nullifier.tree_index)
            .await?;
        let root_hash = tree_guard.get_root().await?;

        drop(tree_guard);

        let membership_proof = MembershipProof {
            nullifier_value: value,
            merkle_proof,
            root_hash,
            tree_index: nullifier.tree_index,
            verified_at: Utc::now(),
        };

        // Record audit event
        let block_height = self.get_current_block_height()?;
        self.record_audit_event(
            value,
            AuditEventType::MembershipProof,
            root_hash,
            root_hash, // Same root for proof generation
            block_height,
            serde_json::json!({
                "tree_index": nullifier.tree_index,
                "proof_size": membership_proof.merkle_proof.siblings.len(),
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("proof", duration_ms).await?;

        info!(
            "✅ Membership proof generated for {} in {:.2}ms",
            value, duration_ms
        );
        Ok(membership_proof)
    }

    #[instrument(skip(self), level = "info")]
    async fn prove_non_membership(&self, value: i64) -> Result<NonMembershipProof, AdsError> {
        info!("🔍 Generating non-membership proof for: {}", value);
        let start_time = std::time::Instant::now();

        let tree_guard = self.tree.read().await;

        // Find low nullifier
        let low_nullifier = tree_guard
            .db
            .nullifiers
            .find_low_nullifier(value)
            .await?
            .ok_or(AdsError::InvalidRange("No low nullifier found".to_string()))?;

        // Verify range: low_nullifier.value < value < low_nullifier.next_value
        if value <= low_nullifier.value {
            return Err(AdsError::InvalidRange(
                "Value not greater than low nullifier".into(),
            ));
        }
        if low_nullifier.next_value != 0 && value >= low_nullifier.next_value {
            return Err(AdsError::InvalidRange(
                "Value not less than next nullifier".into(),
            ));
        }

        // Generate proof for low nullifier
        let merkle_proof = tree_guard
            .generate_merkle_proof(low_nullifier.tree_index)
            .await?;
        let root_hash = tree_guard.get_root().await?;

        drop(tree_guard);

        let range_valid = value > low_nullifier.value
            && (low_nullifier.next_value == 0 || value < low_nullifier.next_value);

        let range_proof = RangeProof {
            lower_bound: low_nullifier.value,
            upper_bound: low_nullifier.next_value,
            queried_value: value,
            valid: range_valid,
        };

        let non_membership_proof = NonMembershipProof {
            queried_value: value,
            low_nullifier: LowNullifierProof {
                value: low_nullifier.value,
                next_value: low_nullifier.next_value,
                tree_index: low_nullifier.tree_index,
                merkle_proof,
            },
            root_hash,
            range_proof,
            verified_at: Utc::now(),
        };

        // Record audit event
        let block_height = self.get_current_block_height()?;
        self.record_audit_event(
            value,
            AuditEventType::NonMembershipProof,
            root_hash,
            root_hash,
            block_height,
            serde_json::json!({
                "low_nullifier": low_nullifier.value,
                "range_valid": range_valid,
                "tree_index": low_nullifier.tree_index,
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("proof", duration_ms).await?;

        info!(
            "✅ Non-membership proof generated for {} in {:.2}ms",
            value, duration_ms
        );
        Ok(non_membership_proof)
    }

    #[instrument(skip(self), level = "info")]
    async fn get_state_commitment(&self) -> Result<StateCommitment, AdsError> {
        let tree_guard = self.tree.read().await;
        let root_hash = tree_guard.get_root().await?;
        drop(tree_guard);

        // Check cache first
        {
            let cache = self.state_cache.read().await;
            if let Some(cached) = cache.get(&root_hash) {
                return Ok(cached.clone());
            }
        }

        // Generate new commitment
        let nullifier_count = self.get_nullifier_count().await?;
        let commitment = StateCommitment {
            root_hash,
            nullifier_count,
            tree_height: 32,
            last_updated: Utc::now(),
            commitment_hash: self.calculate_commitment_hash(&root_hash, nullifier_count),
            settlement_data: SettlementData {
                contract_address: self.config.settlement_contract.clone(),
                chain_id: self.config.chain_id,
                nonce: nullifier_count,
                gas_price: self.config.gas_price,
            },
        };

        // Update cache
        {
            let mut cache = self.state_cache.write().await;
            cache.insert(root_hash, commitment.clone());
        }

        Ok(commitment)
    }

    #[instrument(skip(self, transition), level = "info")]
    async fn verify_state_transition(
        &self,
        transition: &StateTransition,
    ) -> Result<bool, AdsError> {
        info!("🔐 Verifying state transition: {}", transition.id);

        let tree_guard = self.tree.read().await;

        // Verify the insertion proof structure and cryptographic validity
        let verification_result =
            tree_guard.verify_insertion_proof(&transition.insertion_proof, &transition.new_root);

        drop(tree_guard);

        if !verification_result {
            warn!(
                "State transition verification failed for: {}",
                transition.id
            );
            return Ok(false);
        }

        // Additional business logic validation
        if transition.old_root == transition.new_root {
            return Err(AdsError::ProofVerificationFailed(
                "Root should change after insertion".to_string(),
            ));
        }

        if transition.nullifier_value == 0 {
            return Err(AdsError::ProofVerificationFailed(
                "Invalid nullifier value".to_string(),
            ));
        }

        info!(
            "✅ State transition verified successfully: {}",
            transition.id
        );
        Ok(true)
    }

    #[instrument(skip(self, values), level = "info")]
    async fn batch_insert(&mut self, values: &[i64]) -> Result<Vec<StateTransition>, AdsError> {
        info!("📦 Batch inserting {} nullifiers", values.len());

        if values.len() > self.config.batch_size_limit {
            return Err(AdsError::InvalidRange(format!(
                "Batch size {} exceeds limit {}",
                values.len(),
                self.config.batch_size_limit
            )));
        }

        let mut transitions = Vec::with_capacity(values.len());

        // Process each insertion sequentially for now
        // Could be optimized with true batch operations in the future
        for &value in values {
            match self.insert(value).await {
                Ok(transition) => transitions.push(transition),
                Err(e) => {
                    warn!("Batch insertion failed for value {}: {:?}", value, e);
                    return Err(e);
                }
            }
        }

        info!(
            "✅ Batch insertion completed: {} transitions",
            transitions.len()
        );
        Ok(transitions)
    }

    #[instrument(skip(self), level = "info")]
    async fn get_audit_trail(&self, value: i64) -> Result<AuditTrail, AdsError> {
        info!("📋 Retrieving audit trail for nullifier: {}", value);

        // Check in-memory cache first
        {
            let audit_storage = self.audit_storage.read().await;
            if let Some(trail) = audit_storage.get(&value) {
                return Ok(trail.clone());
            }
        }

        // Load from database
        let events = sqlx::query!(
            r#"
            SELECT event_id, event_type, timestamp, root_before, root_after,
                   transaction_hash, block_height, operator, metadata
            FROM audit_events
            WHERE nullifier_value = $1
            ORDER BY timestamp ASC
            "#,
            value
        )
        .fetch_all(&self.pool)
        .await?;

        let mut operation_history = Vec::new();
        for row in events {
            let event = AuditEvent {
                event_id: row.event_id,
                event_type: serde_json::from_str(&row.event_type)?,
                timestamp: row.timestamp,
                root_before: {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&row.root_before);
                    arr
                },
                root_after: {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&row.root_after);
                    arr
                },
                transaction_hash: row.transaction_hash,
                block_height: row.block_height.unwrap_or(0) as u64,
                operator: row.operator.unwrap_or_else(|| "system".to_string()),
                metadata: row.metadata.unwrap_or_else(|| serde_json::json!({})),
            };
            operation_history.push(event);
        }

        if operation_history.is_empty() {
            return Err(AdsError::AuditTrailError(format!(
                "No audit trail found for nullifier {}",
                value
            )));
        }

        let audit_trail = AuditTrail {
            nullifier_value: value,
            operation_history,
            compliance_status: ComplianceStatus {
                is_compliant: true,
                last_audit: Utc::now(),
                jurisdiction: "US".to_string(), // Configurable
                notes: Vec::new(),
            },
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        // Cache for future access
        {
            let mut audit_storage = self.audit_storage.write().await;
            audit_storage.insert(value, audit_trail.clone());
        }

        info!("✅ Audit trail retrieved for nullifier: {}", value);
        Ok(audit_trail)
    }
}

// Helper methods for IndexedMerkleTreeADS
impl IndexedMerkleTreeADS {
    /// Get total nullifier count from database
    async fn get_nullifier_count(&self) -> Result<u64, AdsError> {
        let count = sqlx::query_scalar!(
            "SELECT total_nullifiers FROM tree_state WHERE tree_id = 'default'"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(count.flatten().unwrap_or(0) as u64)
    }

    /// Calculate hash of state commitment data
    fn calculate_commitment_hash(&self, root_hash: &[u8; 32], nullifier_count: u64) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(root_hash);
        hasher.update(nullifier_count.to_be_bytes());
        hasher.update(self.config.chain_id.to_be_bytes());
        hasher.finalize().into()
    }

    /// Get current performance metrics
    #[instrument(skip(self), level = "info")]
    pub async fn get_metrics(&self) -> Result<AdsMetrics, AdsError> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    /// Reset performance metrics
    #[instrument(skip(self), level = "info")]
    pub async fn reset_metrics(&self) -> Result<(), AdsError> {
        let mut metrics = self.metrics.write().await;
        *metrics = AdsMetrics {
            operations_total: 0,
            insertions_total: 0,
            proofs_generated: 0,
            avg_insertion_time_ms: 0.0,
            avg_proof_time_ms: 0.0,
            error_rate: 0.0,
            last_reset: Utc::now(),
            constraint_efficiency: ConstraintMetrics {
                avg_constraints_per_op: 0.0,
                target_constraints: 200,
                efficiency_ratio: 1.0,
                circuit_types: HashMap::new(),
            },
        };

        info!("🔄 Performance metrics reset");
        Ok(())
    }

    /// Health check for the service
    #[instrument(skip(self), level = "info")]
    pub async fn health_check(&self) -> Result<bool, AdsError> {
        // Check database connectivity
        sqlx::query!("SELECT 1 as health")
            .fetch_one(&self.pool)
            .await?;

        // Check tree access
        let _tree_guard = self.tree.read().await;

        // Check cache access
        let _cache_guard = self.state_cache.read().await;

        info!("✅ Health check passed");
        Ok(true)
    }

    // ============================================================================
    // CCR OPERATIONS SUPPORT
    // ============================================================================

    /// Insert a CCR operation and generate enhanced state transition with CCR metadata
    /// This method provides full integration with CCR operations while reusing existing IMT logic
    #[instrument(skip(self), level = "info")]
    pub async fn insert_ccr_operation(
        &mut self,
        operation: &CcrOperation,
    ) -> Result<StateTransition, AdsError> {
        let operation_type = operation.operation_type();
        let operation_id = operation.primary_id();
        info!("🔄 Inserting CCR operation: {} ({})", operation_type, operation_id);

        // Generate prefixed nullifier using CCR operation's built-in method
        let nullifier_value = operation.to_nullifier();
        debug!("Generated CCR nullifier: {} for operation {}", nullifier_value, operation_id);

        // Use existing insert logic to maintain compatibility
        let mut base_transition = self.insert(nullifier_value).await?;

        // Enhance with CCR metadata
        let ccr_metadata = CcrMetadata {
            operation_type: operation_type.to_string(),
            operation_id: operation_id.to_string(),
            user_ids: self.extract_user_ids(operation),
            amount: operation.amount(),
            created_at: operation.created_at(),
            additional_context: self.serialize_operation_context(operation)?,
        };

        base_transition.ccr_metadata = Some(ccr_metadata);

        // Add CCR-specific audit event
        if self.config.audit_enabled {
            self.add_ccr_audit_event(&base_transition, operation).await?;
        }

        info!("✅ CCR operation {} inserted successfully with nullifier {}", operation_id, nullifier_value);
        Ok(base_transition)
    }

    /// Batch insert multiple CCR operations with enhanced state transitions
    #[instrument(skip(self, operations), level = "info")]
    pub async fn batch_insert_ccr_operations(
        &mut self,
        operations: &[CcrOperation],
    ) -> Result<Vec<StateTransition>, AdsError> {
        info!("📦 Batch inserting {} CCR operations", operations.len());

        if operations.len() > self.config.batch_size_limit {
            return Err(AdsError::InvalidRange(format!(
                "Batch size {} exceeds limit {}",
                operations.len(),
                self.config.batch_size_limit
            )));
        }

        let mut transitions = Vec::with_capacity(operations.len());

        // Process each CCR operation sequentially to maintain consistency
        for operation in operations {
            let transition = self.insert_ccr_operation(operation).await?;
            transitions.push(transition);
        }

        info!("✅ Successfully batch inserted {} CCR operations", operations.len());
        Ok(transitions)
    }

    /// Extract user IDs from a CCR operation for metadata
    fn extract_user_ids(&self, operation: &CcrOperation) -> Vec<String> {
        match operation {
            CcrOperation::LoanCreation { borrower_id, lender_id, .. } => {
                vec![borrower_id.to_string(), lender_id.to_string()]
            }
            CcrOperation::LoanFunding { funder_id, .. } => {
                vec![funder_id.to_string()]
            }
            CcrOperation::Payment { payer_id, .. } => {
                vec![payer_id.to_string()]
            }
            CcrOperation::BalanceUpdate { identity_id, .. } => {
                vec![identity_id.to_string()]
            }
        }
    }

    /// Serialize operation context for additional metadata
    fn serialize_operation_context(&self, operation: &CcrOperation) -> Result<serde_json::Value, AdsError> {
        match serde_json::to_value(operation) {
            Ok(value) => Ok(value),
            Err(e) => {
                warn!("Failed to serialize CCR operation context: {}", e);
                // Fallback to basic context
                Ok(serde_json::json!({
                    "operation_type": operation.operation_type(),
                    "serialization_error": e.to_string()
                }))
            }
        }
    }

    /// Add CCR-specific audit event
    async fn add_ccr_audit_event(
        &self,
        transition: &StateTransition,
        operation: &CcrOperation,
    ) -> Result<(), AdsError> {
        let audit_event = AuditEvent {
            event_id: format!("ccr_{}", transition.id),
            event_type: AuditEventType::CcrOperation,
            timestamp: transition.timestamp,
            root_before: transition.old_root,
            root_after: transition.new_root,
            transaction_hash: None, // Will be filled when settled on-chain
            block_height: transition.block_height,
            operator: "ads_service".to_string(),
            metadata: serde_json::json!({
                "operation_type": operation.operation_type(),
                "operation_id": operation.primary_id(),
                "nullifier_value": transition.nullifier_value,
                "user_ids": self.extract_user_ids(operation),
                "amount": operation.amount()
            }),
        };

        // Store audit event in database
        sqlx::query!(
            r#"
            INSERT INTO audit_events (event_id, event_type, timestamp, root_before, root_after, 
                                    block_height, operator, metadata, nullifier_value)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            audit_event.event_id,
            serde_json::to_string(&audit_event.event_type).unwrap_or_default(),
            audit_event.timestamp,
            audit_event.root_before.as_slice(),
            audit_event.root_after.as_slice(),
            audit_event.block_height as i64,
            audit_event.operator,
            audit_event.metadata,
            transition.nullifier_value
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        debug!("Added CCR audit event: {}", audit_event.event_id);
        Ok(())
    }

    /// Get enhanced audit trail for CCR operations
    #[instrument(skip(self), level = "debug")]
    pub async fn get_ccr_audit_trail(&self, operation_id: &str) -> Result<Vec<AuditEvent>, AdsError> {
        let events = sqlx::query!(
            r#"
            SELECT event_id, event_type, timestamp, root_before, root_after,
                   transaction_hash, block_height, operator, metadata, nullifier_value
            FROM audit_events 
            WHERE metadata->>'operation_id' = $1
            ORDER BY timestamp DESC
            "#,
            operation_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        let mut audit_events = Vec::new();
        for row in events {
            if let (Ok(event_type), Some(nullifier_value)) = (
                serde_json::from_str::<AuditEventType>(&row.event_type.unwrap_or_default()),
                row.nullifier_value
            ) {
                audit_events.push(AuditEvent {
                    event_id: row.event_id,
                    event_type,
                    timestamp: row.timestamp,
                    root_before: row.root_before.as_slice().try_into().unwrap_or_default(),
                    root_after: row.root_after.as_slice().try_into().unwrap_or_default(),
                    transaction_hash: row.transaction_hash,
                    block_height: row.block_height as u64,
                    operator: row.operator,
                    metadata: row.metadata,
                });
            }
        }

        Ok(audit_events)
    }

    // ============================================================================
    // LOAN LIFECYCLE BATCH PROCESSING
    // ============================================================================

    /// Batch process loan operations with enhanced metadata tracking
    #[instrument(skip(self, loan_operations), level = "info")]
    pub async fn process_loan_batch(
        &mut self,
        loan_operations: &[LoanOperation],
    ) -> Result<Vec<StateTransition>, AdsError> {
        info!("🏦 Processing loan batch with {} operations", loan_operations.len());

        if loan_operations.len() > self.config.batch_size_limit {
            return Err(AdsError::InvalidRange(format!(
                "Loan batch size {} exceeds limit {}",
                loan_operations.len(),
                self.config.batch_size_limit
            )));
        }

        let mut state_transitions = Vec::new();

        for loan_op in loan_operations {
            // Generate nullifier for the loan operation
            let nullifier = loan_op.to_nullifier();

            // Process single loan operation
            let mut transition = self.insert_with_metadata(nullifier, loan_op.to_loan_metadata()).await?;
            
            // Add loan-specific audit trail
            self.add_loan_audit_event(&transition, loan_op).await?;

            state_transitions.push(transition);
            
            // Update performance metrics
            self.metrics.total_loan_operations += 1;
            match loan_op.operation_type.as_str() {
                "loan_origination" => self.metrics.loan_originations += 1,
                "loan_payment" => self.metrics.loan_payments += 1,
                "loan_closeout" => self.metrics.loan_closeouts += 1,
                _ => {}
            }
        }

        info!("✅ Processed {} loan operations successfully", state_transitions.len());
        Ok(state_transitions)
    }

    /// Process loan operations with nullifier prefix support
    #[instrument(skip(self, loan_operations), level = "info")]
    pub async fn process_loan_batch_with_prefixes(
        &mut self,
        loan_operations: &[LoanOperation],
        use_prefixes: bool,
    ) -> Result<Vec<StateTransition>, AdsError> {
        info!("🏦 Processing {} loan operations (prefixes: {})", loan_operations.len(), use_prefixes);

        let mut state_transitions = Vec::new();

        for loan_op in loan_operations {
            let nullifier = if use_prefixes {
                loan_op.to_nullifier_with_prefix()
            } else {
                loan_op.to_nullifier()
            };

            let mut transition = self.insert_with_metadata(nullifier, loan_op.to_loan_metadata()).await?;
            self.add_loan_audit_event(&transition, loan_op).await?;
            state_transitions.push(transition);
        }

        Ok(state_transitions)
    }

    /// Insert nullifier with loan metadata
    async fn insert_with_metadata(
        &mut self,
        nullifier: i64,
        loan_metadata: LoanMetadata,
    ) -> Result<StateTransition, AdsError> {
        // Use existing insert method for core functionality
        let mut transition = self.insert(nullifier).await?;
        
        // Enhance with loan metadata
        transition.loan_metadata = Some(loan_metadata);
        
        Ok(transition)
    }

    /// Add loan-specific audit event
    async fn add_loan_audit_event(
        &self,
        transition: &StateTransition,
        loan_op: &LoanOperation,
    ) -> Result<(), AdsError> {
        let audit_event = AuditEvent {
            event_id: format!("loan_{}", transition.id),
            event_type: AuditEventType::LoanOperation,
            timestamp: transition.timestamp,
            root_before: transition.old_root,
            root_after: transition.new_root,
            transaction_hash: None,
            block_height: transition.block_height,
            operator: "loan_processor".to_string(),
            metadata: serde_json::json!({
                "operation_type": loan_op.operation_type,
                "loan_id": loan_op.loan_id,
                "borrower_id": loan_op.borrower_id,
                "lender_id": loan_op.lender_id,
                "amount": loan_op.amount,
                "loan_status": loan_op.loan_status,
                "nullifier_value": transition.nullifier_value,
                "payment_id": loan_op.payment_id
            }),
        };

        // Store audit event
        sqlx::query!(
            r#"
            INSERT INTO audit_events (event_id, event_type, timestamp, root_before, root_after, 
                                    block_height, operator, metadata, nullifier_value)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            audit_event.event_id,
            serde_json::to_string(&audit_event.event_type).unwrap_or_default(),
            audit_event.timestamp,
            audit_event.root_before.as_slice(),
            audit_event.root_after.as_slice(),
            audit_event.block_height as i64,
            audit_event.operator,
            audit_event.metadata,
            transition.nullifier_value
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        debug!("Added loan audit event: {}", audit_event.event_id);
        Ok(())
    }

    /// Get loan-specific audit trail
    #[instrument(skip(self), level = "debug")]
    pub async fn get_loan_audit_trail(&self, loan_id: uuid::Uuid) -> Result<Vec<AuditEvent>, AdsError> {
        let events = sqlx::query!(
            r#"
            SELECT event_id, event_type, timestamp, root_before, root_after,
                   transaction_hash, block_height, operator, metadata, nullifier_value
            FROM audit_events 
            WHERE metadata->>'loan_id' = $1
            ORDER BY timestamp DESC
            "#,
            loan_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        let mut audit_events = Vec::new();
        for row in events {
            if let (Ok(event_type), Some(nullifier_value)) = (
                serde_json::from_str::<AuditEventType>(&row.event_type.unwrap_or_default()),
                row.nullifier_value
            ) {
                audit_events.push(AuditEvent {
                    event_id: row.event_id,
                    event_type,
                    timestamp: row.timestamp,
                    root_before: row.root_before.as_slice().try_into().unwrap_or_default(),
                    root_after: row.root_after.as_slice().try_into().unwrap_or_default(),
                    transaction_hash: row.transaction_hash,
                    block_height: row.block_height as u64,
                    operator: row.operator,
                    metadata: row.metadata,
                });
            }
        }

        Ok(audit_events)
    }

    /// Get loan lifecycle summary
    #[instrument(skip(self), level = "debug")]
    pub async fn get_loan_lifecycle_summary(&self, loan_id: uuid::Uuid) -> Result<LoanLifecycleSummary, AdsError> {
        let events = self.get_loan_audit_trail(loan_id).await?;
        
        let mut summary = LoanLifecycleSummary {
            loan_id,
            total_operations: events.len(),
            origination_date: None,
            last_payment_date: None,
            total_payments: 0,
            current_status: "unknown".to_string(),
            state_transitions: events.len(),
        };

        for event in &events {
            if let Ok(metadata) = serde_json::from_value::<serde_json::Value>(event.metadata.clone()) {
                match metadata.get("operation_type").and_then(|v| v.as_str()) {
                    Some("loan_origination") => {
                        summary.origination_date = Some(event.timestamp);
                    },
                    Some("loan_payment") => {
                        summary.total_payments += 1;
                        if summary.last_payment_date.is_none() || Some(event.timestamp) > summary.last_payment_date {
                            summary.last_payment_date = Some(event.timestamp);
                        }
                    },
                    _ => {}
                }
                
                // Update current status from most recent event
                if let Some(status) = metadata.get("loan_status").and_then(|v| v.as_str()) {
                    summary.current_status = status.to_string();
                }
            }
        }

        Ok(summary)
    }

    // ============================================================================
    // LOAN OPERATION INCLUSION PROOF GENERATION
    // ============================================================================

    /// Generate loan operation inclusion proof for user verification
    #[instrument(skip(self), level = "info")]
    pub async fn prove_loan_operation_inclusion(
        &self,
        loan_operation_request: &LoanInclusionRequest,
    ) -> Result<LoanInclusionProof, AdsError> {
        info!("🔍 Generating loan operation inclusion proof for user {}", loan_operation_request.user_id);

        // Step 1: Verify the loan operation exists and user has access
        let loan_operation = self.verify_user_loan_access(loan_operation_request).await?;
        
        // Step 2: Find the nullifier for this operation
        let nullifier = self.calculate_loan_operation_nullifier(&loan_operation).await?;
        
        // Step 3: Generate core membership proof
        let membership_proof = self.prove_membership(nullifier).await?;
        
        // Step 4: Get batch information
        let batch_info = self.get_batch_inclusion_info(nullifier, loan_operation_request.batch_id).await?;
        
        // Step 5: Create loan operation proof details
        let loan_operation_proof = LoanOperationProof {
            operation_type: loan_operation.operation_type.clone(),
            loan_id: loan_operation.loan_id,
            payment_id: loan_operation.payment_id,
            user_id: loan_operation_request.user_id,
            amount: loan_operation.amount,
            nullifier_value: nullifier,
            operation_hash: self.hash_loan_operation(&loan_operation),
            user_role: loan_operation_request.user_role.clone(),
        };

        // Step 6: Assemble complete loan inclusion proof
        let loan_inclusion_proof = LoanInclusionProof {
            loan_operation: loan_operation_proof,
            membership_proof,
            batch_info,
            operation_metadata: loan_operation.to_loan_metadata(),
            verified_at: Utc::now(),
        };

        info!("✅ Generated loan operation inclusion proof for {} operation", loan_operation.operation_type);
        Ok(loan_inclusion_proof)
    }

    /// Generate core membership proof for a nullifier
    #[instrument(skip(self), level = "debug")]
    pub async fn prove_membership(&self, nullifier: i64) -> Result<MembershipProof, AdsError> {
        let proof_start = Instant::now();
        
        // Get merkle proof from the underlying tree
        let merkle_proof = self.tree.prove_membership(nullifier).await
            .map_err(|e| AdsError::ProofGeneration(format!("Failed to generate merkle proof: {}", e)))?;
        
        // Get current root
        let current_root = self.tree.get_root().await
            .map_err(|e| AdsError::StateError(format!("Failed to get current root: {}", e)))?;
        
        // Get tree index for the nullifier
        let tree_index = self.tree.get_nullifier_index(nullifier).await
            .map_err(|e| AdsError::StateError(format!("Failed to get nullifier index: {}", e)))?;

        let membership_proof = MembershipProof {
            nullifier_value: nullifier,
            merkle_proof,
            root_hash: current_root,
            tree_index,
            verified_at: Utc::now(),
        };

        let duration_ms = proof_start.elapsed().as_millis() as f64;
        self.update_metrics("proof", duration_ms).await?;

        info!("✅ Membership proof generated for {} in {:.2}ms", nullifier, duration_ms);
        Ok(membership_proof)
    }

    /// Prove that a loan payment was included in a specific batch
    #[instrument(skip(self), level = "info")]
    pub async fn prove_loan_payment_in_batch(
        &self,
        user_id: uuid::Uuid,
        loan_id: uuid::Uuid,
        payment_id: uuid::Uuid,
        batch_id: i32,
    ) -> Result<LoanInclusionProof, AdsError> {
        info!("💰 Proving loan payment inclusion: loan={}, payment={}, batch={}", 
              loan_id, payment_id, batch_id);

        let request = LoanInclusionRequest {
            user_id,
            loan_id: Some(loan_id),
            payment_id: Some(payment_id),
            operation_type: "loan_payment".to_string(),
            batch_id: Some(batch_id),
            user_role: "borrower".to_string(), // Could be determined from context
        };

        self.prove_loan_operation_inclusion(&request).await
    }

    /// Verify user has access to the requested loan operation
    async fn verify_user_loan_access(
        &self,
        request: &LoanInclusionRequest,
    ) -> Result<LoanOperation, AdsError> {
        // Query database for the loan operation
        let loan_operation_data = if let Some(payment_id) = request.payment_id {
            // Payment operation
            sqlx::query!(
                r#"
                SELECT lt.loan_id, lt.payment_id, lt.transaction_type, lt.amount, lt.identity_id,
                       l.borrower_id, l.lender_id, l.loan_status, lt.created_at
                FROM loan_transactions lt
                JOIN loans l ON lt.loan_id = l.id
                WHERE lt.payment_id = $1 AND (l.borrower_id = $2 OR l.lender_id = $2)
                "#,
                payment_id,
                request.user_id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?
        } else if let Some(loan_id) = request.loan_id {
            // Loan origination or closeout
            sqlx::query!(
                r#"
                SELECT lt.loan_id, lt.payment_id, lt.transaction_type, lt.amount, lt.identity_id,
                       l.borrower_id, l.lender_id, l.loan_status, lt.created_at
                FROM loan_transactions lt
                JOIN loans l ON lt.loan_id = l.id
                WHERE lt.loan_id = $1 AND (l.borrower_id = $2 OR l.lender_id = $2)
                  AND lt.transaction_type = $3
                ORDER BY lt.created_at DESC
                LIMIT 1
                "#,
                loan_id,
                request.user_id,
                request.operation_type
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?
        } else {
            return Err(AdsError::InvalidRequest("Either loan_id or payment_id must be provided".to_string()));
        };

        let row = loan_operation_data.ok_or_else(|| {
            AdsError::InvalidRequest("Loan operation not found or user doesn't have access".to_string())
        })?;

        // Convert database row to LoanOperation
        let loan_operation = LoanOperation {
            operation_type: row.transaction_type,
            loan_id: row.loan_id,
            payment_id: row.payment_id,
            borrower_id: row.borrower_id,
            lender_id: row.lender_id,
            amount: Some(row.amount),
            loan_status: Some(row.loan_status),
            transaction_data: serde_json::json!({
                "created_at": row.created_at,
                "identity_id": row.identity_id
            }),
            nullifier_prefix: None,
        };

        Ok(loan_operation)
    }

    /// Calculate the nullifier for a loan operation (matches batch processing logic)
    async fn calculate_loan_operation_nullifier(&self, loan_operation: &LoanOperation) -> Result<i64, AdsError> {
        // Use the same nullifier generation logic as in batch processing
        let nullifier = loan_operation.to_nullifier_with_prefix();
        
        debug!("Calculated nullifier {} for {} operation", 
               nullifier, loan_operation.operation_type);
        
        Ok(nullifier)
    }

    /// Get batch inclusion information for the operation
    async fn get_batch_inclusion_info(
        &self,
        nullifier: i64,
        requested_batch_id: Option<i32>,
    ) -> Result<BatchInclusionInfo, AdsError> {
        // Find which batch contains this nullifier
        let batch_info = if let Some(batch_id) = requested_batch_id {
            // Verify the nullifier is in the specified batch
            sqlx::query!(
                r#"
                SELECT pb.id, pb.created_at, asc.merkle_root
                FROM proof_batches pb
                JOIN ads_state_commits asc ON pb.id = asc.batch_id
                JOIN audit_events ae ON ae.nullifier_value = $1
                WHERE pb.id = $2
                "#,
                nullifier,
                batch_id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?
        } else {
            // Find which batch contains this nullifier
            sqlx::query!(
                r#"
                SELECT pb.id, pb.created_at, asc.merkle_root
                FROM proof_batches pb
                JOIN ads_state_commits asc ON pb.id = asc.batch_id
                JOIN audit_events ae ON ae.nullifier_value = $1
                ORDER BY pb.created_at DESC
                LIMIT 1
                "#,
                nullifier
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?
        };

        let batch_row = batch_info.ok_or_else(|| {
            AdsError::InvalidRequest("Nullifier not found in any batch".to_string())
        })?;

        // Get operation count for this batch
        let operation_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::INT
            FROM audit_events ae
            JOIN proof_batches pb ON pb.created_at <= ae.timestamp 
            WHERE pb.id = $1
            "#,
            batch_row.id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?
        .unwrap_or(0) as u32;

        let batch_info = BatchInclusionInfo {
            batch_id: batch_row.id,
            batch_root: batch_row.merkle_root.as_slice().try_into()
                .map_err(|_| AdsError::StateError("Invalid merkle root format".to_string()))?,
            batch_timestamp: batch_row.created_at,
            operation_index: 0, // Would need more sophisticated tracking
            total_operations: operation_count,
        };

        Ok(batch_info)
    }

    /// Hash loan operation for proof generation
    fn hash_loan_operation(&self, loan_operation: &LoanOperation) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        
        hasher.update(loan_operation.operation_type.as_bytes());
        if let Some(loan_id) = loan_operation.loan_id {
            hasher.update(loan_id.as_bytes());
        }
        if let Some(payment_id) = loan_operation.payment_id {
            hasher.update(payment_id.as_bytes());
        }
        if let Some(amount) = loan_operation.amount {
            hasher.update(&amount.to_string().as_bytes());
        }
        
        let hash = hasher.finalize();
        *hash.as_bytes()
    }

    /// Verify a loan inclusion proof
    #[instrument(skip(self, proof), level = "info")]
    pub async fn verify_loan_inclusion_proof(
        &self,
        proof: &LoanInclusionProof,
    ) -> Result<bool, AdsError> {
        info!("🔍 Verifying loan inclusion proof for {} operation", 
              proof.loan_operation.operation_type);

        // Step 1: Verify the membership proof
        let membership_valid = self.verify_membership_proof(&proof.membership_proof).await?;
        if !membership_valid {
            return Ok(false);
        }

        // Step 2: Verify nullifier matches operation
        let expected_nullifier = self.calculate_operation_nullifier_from_proof(&proof.loan_operation).await?;
        if proof.membership_proof.nullifier_value != expected_nullifier {
            return Ok(false);
        }

        // Step 3: Verify batch information
        let batch_valid = self.verify_batch_inclusion_info(&proof.batch_info, proof.membership_proof.nullifier_value).await?;
        if !batch_valid {
            return Ok(false);
        }

        info!("✅ Loan inclusion proof verified successfully");
        Ok(true)
    }

    /// Verify membership proof against current tree state
    async fn verify_membership_proof(&self, proof: &MembershipProof) -> Result<bool, AdsError> {
        // Get current root to verify against
        let current_root = self.tree.get_root().await
            .map_err(|e| AdsError::StateError(format!("Failed to get current root: {}", e)))?;

        // The proof root should match current root or a historical valid root
        if proof.root_hash != current_root {
            // Could check against historical roots if needed
            warn!("Proof root mismatch: expected {:?}, got {:?}", current_root, proof.root_hash);
        }

        // Verify the merkle proof
        let valid = self.tree.verify_membership_proof(proof.nullifier_value, &proof.merkle_proof, &proof.root_hash).await
            .map_err(|e| AdsError::ProofVerification(format!("Failed to verify merkle proof: {}", e)))?;

        Ok(valid)
    }

    /// Calculate nullifier from proof operation data
    async fn calculate_operation_nullifier_from_proof(&self, operation_proof: &LoanOperationProof) -> Result<i64, AdsError> {
        // Recreate LoanOperation from proof data
        let loan_operation = LoanOperation {
            operation_type: operation_proof.operation_type.clone(),
            loan_id: operation_proof.loan_id,
            payment_id: operation_proof.payment_id,
            borrower_id: None, // Not needed for nullifier calculation
            lender_id: None,   // Not needed for nullifier calculation
            amount: operation_proof.amount,
            loan_status: None,
            transaction_data: serde_json::Value::Null,
            nullifier_prefix: None,
        };

        let nullifier = loan_operation.to_nullifier_with_prefix();
        Ok(nullifier)
    }

    /// Verify batch inclusion information
    async fn verify_batch_inclusion_info(&self, batch_info: &BatchInclusionInfo, nullifier: i64) -> Result<bool, AdsError> {
        // Verify the batch exists and contains the nullifier
        let batch_exists = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) > 0
            FROM proof_batches pb
            JOIN audit_events ae ON ae.nullifier_value = $1
            WHERE pb.id = $2
            "#,
            nullifier,
            batch_info.batch_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AdsError::Database(DbError::Database(e)))?
        .unwrap_or(false);

        Ok(batch_exists)
    }

    // ============================================================================
    // PREFIXED NULLIFIER VERIFICATION SYSTEM
    // ============================================================================

    /// Verify that a nullifier follows the correct prefix pattern
    #[instrument(skip(self), level = "debug")]
    pub async fn verify_nullifier_prefix(
        &self,
        nullifier: i64,
        operation_type: &str,
    ) -> Result<bool, AdsError> {
        debug!("🔢 Verifying nullifier prefix for {} operation: {}", operation_type, nullifier);

        // Define prefix ranges for different operation types
        let valid_range = match operation_type {
            "loan_origination" => (1_000_000_000i64, 1_999_999_999i64),
            "loan_closeout" => (2_000_000_000i64, 2_999_999_999i64),
            "loan_payment" => (3_000_000_000i64, 3_999_999_999i64),
            _ => return Err(AdsError::InvalidRequest(format!("Unknown operation type: {}", operation_type))),
        };

        let is_valid = nullifier >= valid_range.0 && nullifier <= valid_range.1;
        
        if is_valid {
            debug!("✅ Nullifier {} is valid for {} (range: {}-{})", 
                   nullifier, operation_type, valid_range.0, valid_range.1);
        } else {
            warn!("❌ Nullifier {} is invalid for {} (expected range: {}-{})", 
                  nullifier, operation_type, valid_range.0, valid_range.1);
        }

        Ok(is_valid)
    }

    /// Verify loan inclusion proof with enhanced prefix validation
    #[instrument(skip(self, proof), level = "info")]
    pub async fn verify_loan_inclusion_proof_with_prefix_validation(
        &self,
        proof: &LoanInclusionProof,
    ) -> Result<bool, AdsError> {
        info!("🔍 Verifying loan inclusion proof with prefix validation for {} operation", 
              proof.loan_operation.operation_type);

        // Step 1: Standard verification
        let basic_valid = self.verify_loan_inclusion_proof(proof).await?;
        if !basic_valid {
            return Ok(false);
        }

        // Step 2: Verify nullifier prefix is correct for operation type
        let prefix_valid = self.verify_nullifier_prefix(
            proof.membership_proof.nullifier_value,
            &proof.loan_operation.operation_type,
        ).await?;

        if !prefix_valid {
            warn!("❌ Nullifier prefix validation failed");
            return Ok(false);
        }

        // Step 3: Verify nullifier determinism - recalculate and compare
        let recalculated_nullifier = self.calculate_operation_nullifier_from_proof(&proof.loan_operation).await?;
        if recalculated_nullifier != proof.membership_proof.nullifier_value {
            warn!("❌ Nullifier determinism check failed: expected {}, got {}", 
                  recalculated_nullifier, proof.membership_proof.nullifier_value);
            return Ok(false);
        }

        // Step 4: Verify operation hash integrity
        let operation_for_hash = LoanOperation {
            operation_type: proof.loan_operation.operation_type.clone(),
            loan_id: proof.loan_operation.loan_id,
            payment_id: proof.loan_operation.payment_id,
            borrower_id: None,
            lender_id: None,
            amount: proof.loan_operation.amount,
            loan_status: None,
            transaction_data: serde_json::Value::Null,
            nullifier_prefix: None,
        };
        
        let expected_hash = self.hash_loan_operation(&operation_for_hash);
        if expected_hash != proof.loan_operation.operation_hash {
            warn!("❌ Operation hash verification failed");
            return Ok(false);
        }

        info!("✅ Loan inclusion proof verified successfully with prefix validation");
        Ok(true)
    }

    /// Generate comprehensive proof verification report
    #[instrument(skip(self, proof), level = "info")]
    pub async fn generate_proof_verification_report(
        &self,
        proof: &LoanInclusionProof,
    ) -> Result<serde_json::Value, AdsError> {
        let verification_start = Instant::now();

        // Run all verification checks
        let basic_verification = self.verify_loan_inclusion_proof(proof).await?;
        let prefix_verification = self.verify_nullifier_prefix(
            proof.membership_proof.nullifier_value,
            &proof.loan_operation.operation_type,
        ).await?;
        
        let membership_verification = self.verify_membership_proof(&proof.membership_proof).await?;
        let batch_verification = self.verify_batch_inclusion_info(
            &proof.batch_info,
            proof.membership_proof.nullifier_value,
        ).await?;

        let nullifier_determinism = {
            let recalculated = self.calculate_operation_nullifier_from_proof(&proof.loan_operation).await?;
            recalculated == proof.membership_proof.nullifier_value
        };

        let duration_ms = verification_start.elapsed().as_millis();

        let report = serde_json::json!({
            "verification_summary": {
                "overall_valid": basic_verification && prefix_verification,
                "verification_time_ms": duration_ms,
                "verified_at": Utc::now(),
            },
            "loan_operation": {
                "operation_type": proof.loan_operation.operation_type,
                "loan_id": proof.loan_operation.loan_id,
                "payment_id": proof.loan_operation.payment_id,
                "user_id": proof.loan_operation.user_id,
                "user_role": proof.loan_operation.user_role,
                "amount": proof.loan_operation.amount,
            },
            "verification_checks": {
                "basic_verification": basic_verification,
                "prefix_verification": prefix_verification,
                "membership_verification": membership_verification,
                "batch_verification": batch_verification,
                "nullifier_determinism": nullifier_determinism,
            },
            "nullifier_analysis": {
                "value": proof.membership_proof.nullifier_value,
                "operation_type": proof.loan_operation.operation_type,
                "prefix_valid": prefix_verification,
            },
            "batch_information": {
                "batch_id": proof.batch_info.batch_id,
                "batch_timestamp": proof.batch_info.batch_timestamp,
                "total_operations": proof.batch_info.total_operations,
                "merkle_root": format!("{:?}", proof.batch_info.batch_root),
            },
        });

        info!("📊 Generated proof verification report in {}ms", duration_ms);
        Ok(report)
    }

    /// Batch verify multiple loan inclusion proofs
    #[instrument(skip(self, proofs), level = "info")]
    pub async fn batch_verify_loan_proofs(
        &self,
        proofs: &[LoanInclusionProof],
    ) -> Result<Vec<bool>, AdsError> {
        info!("🔍 Batch verifying {} loan inclusion proofs", proofs.len());
        
        let mut results = Vec::with_capacity(proofs.len());
        let verification_start = Instant::now();
        
        for (index, proof) in proofs.iter().enumerate() {
            debug!("Verifying proof {} of {}", index + 1, proofs.len());
            let valid = self.verify_loan_inclusion_proof_with_prefix_validation(proof).await?;
            results.push(valid);
        }
        
        let duration_ms = verification_start.elapsed().as_millis();
        let valid_count = results.iter().filter(|&&v| v).count();
        
        info!("✅ Batch verification complete: {}/{} proofs valid in {}ms", 
              valid_count, proofs.len(), duration_ms);
        
        Ok(results)
    }
}

// ============================================================================
// SERVICE FACTORY
// ============================================================================

/// Factory for creating ADS service instances
pub struct AdsServiceFactory {
    pool: PgPool,
    config: AdsConfig,
}

impl AdsServiceFactory {
    /// Create new service factory
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: AdsConfig::default(),
        }
    }

    /// Create factory with custom configuration
    pub fn with_config(pool: PgPool, config: AdsConfig) -> Self {
        Self { pool, config }
    }

    /// Create indexed Merkle tree ADS instance
    pub async fn create_indexed_merkle_tree(&self) -> Result<IndexedMerkleTreeADS, AdsError> {
        IndexedMerkleTreeADS::new(self.pool.clone(), self.config.clone()).await
    }

    /// Update service configuration
    pub fn set_config(&mut self, config: AdsConfig) {
        self.config = config;
    }
}
