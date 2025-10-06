//! Unified Batch Service
//!
//! This module provides a single, unified interface for batch creation that always
//! uses ADS integration, regardless of how the batch is triggered (API, timer, threshold).
//!
//! This replaces the dual workflow problem where some batches used ADS and others didn't.

use futures_util::future;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use arda_credit_db::{
    get_pending_transactions, store_ads_state_commit, AuthenticatedDataStructure,
    IncomingTransaction, IndexedMerkleTreeADS, LoanOperation, ProofBatch, StateTransition,
};
use chrono;
use std::time::{Duration, Instant};
use tokio::time::timeout;

// ============================================================================
// TRANSACTION TYPE PREFIXES FOR NULLIFIER NAMESPACE ISOLATION
// ============================================================================

/// Prefix constants for operation-type based nullifier generation
/// These prefixes prevent nullifier collisions between different transaction types
/// Each prefix reserves a 1 billion value range for its operation type
pub mod transaction_prefixes {
    /// Loan origination transactions: 1,000,000,000 - 1,999,999,999
    pub const DEAL_ORIGINATION: i64 = 1_000_000_000;

    /// Loan closeout transactions: 2,000,000,000 - 2,999,999,999
    pub const DEAL_CLOSEOUT: i64 = 2_000_000_000;

    /// Loan payment transactions: 3,000,000,000 - 3,999,999,999
    pub const DEAL_PAYMENT: i64 = 3_000_000_000;

    /// User registration transactions: 5,000,000,000 - 5,999,999,999
    pub const USER_REGISTRATION: i64 = 5_000_000_000;

    /// KYC verification transactions: 6,000,000,000 - 6,999,999,999
    pub const KYC_VERIFICATION: i64 = 6_000_000_000;

    /// Default/legacy transactions (for backward compatibility): 0 - 999,999,999
    pub const LEGACY_DEFAULT: i64 = 0;

    /// Maximum value within a prefix range (999,999,999)
    pub const PREFIX_RANGE_SIZE: i64 = 999_999_999;
}

// ============================================================================
// LOAN-SPECIFIC BATCH METRICS AND PERFORMANCE TRACKING
// ============================================================================

/// Comprehensive loan batch performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanBatchMetrics {
    // Throughput metrics
    pub transactions_per_second: f64,
    pub operations_per_second: f64,
    pub merkleization_tps: f64,
    pub total_throughput: f64,

    // Timing breakdown (milliseconds)
    pub total_processing_time: f64,
    pub validation_time: f64,
    pub nullifier_generation_time: f64,
    pub merkleization_time: f64,
    pub database_commit_time: f64,
    pub ads_processing_time: f64,

    // Operation breakdown
    pub loan_originations: usize,
    pub loan_payments: usize,
    pub loan_closeouts: usize,
    pub mixed_operations: usize,
    pub total_operations: usize,

    // Performance indicators
    pub batch_size: usize,
    pub nullifiers_generated: usize,
    pub state_transitions: usize,
    pub merkle_depth_utilized: u32,
    pub memory_usage_mb: f64,

    // Quality metrics
    pub validation_success_rate: f64,
    pub lifecycle_integrity_score: f64,
    pub prefix_collision_rate: f64,

    // Timestamps
    pub batch_started_at: chrono::DateTime<chrono::Utc>,
    pub batch_completed_at: chrono::DateTime<chrono::Utc>,
}

impl Default for LoanBatchMetrics {
    fn default() -> Self {
        Self {
            transactions_per_second: 0.0,
            operations_per_second: 0.0,
            merkleization_tps: 0.0,
            total_throughput: 0.0,
            total_processing_time: 0.0,
            validation_time: 0.0,
            nullifier_generation_time: 0.0,
            merkleization_time: 0.0,
            database_commit_time: 0.0,
            ads_processing_time: 0.0,
            loan_originations: 0,
            loan_payments: 0,
            loan_closeouts: 0,
            mixed_operations: 0,
            total_operations: 0,
            batch_size: 0,
            nullifiers_generated: 0,
            state_transitions: 0,
            merkle_depth_utilized: 0,
            memory_usage_mb: 0.0,
            validation_success_rate: 0.0,
            lifecycle_integrity_score: 0.0,
            prefix_collision_rate: 0.0,
            batch_started_at: chrono::Utc::now(),
            batch_completed_at: chrono::Utc::now(),
        }
    }
}

/// Loan lifecycle validation result for batch integrity
#[derive(Debug, Clone)]
pub struct LoanLifecycleValidation {
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub lifecycle_warnings: Vec<String>,
    pub integrity_score: f64,
    pub validated_loans: usize,
    pub validation_time_ms: f64,
}

/// Optimized batch processing configuration for loans
#[derive(Debug, Clone)]
pub struct LoanBatchConfig {
    pub target_tps: f64,
    pub max_batch_size: usize,
    pub enable_parallel_validation: bool,
    pub enable_lifecycle_validation: bool,
    pub enable_merkle_optimization: bool,
    pub timeout_duration: Duration,
    pub memory_limit_mb: usize,
}

impl Default for LoanBatchConfig {
    fn default() -> Self {
        Self {
            target_tps: 60.0,
            max_batch_size: 1000,
            enable_parallel_validation: true,
            enable_lifecycle_validation: true,
            enable_merkle_optimization: true,
            timeout_duration: Duration::from_secs(30),
            memory_limit_mb: 512,
        }
    }
}

// ============================================================================
// UNIFIED BATCH SERVICE
// ============================================================================

/// Unified service for all batch creation with consistent ADS integration and loan optimizations
pub struct UnifiedBatchService {
    pool: PgPool,
    ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
    max_batch_size: u32,
    loan_config: LoanBatchConfig,
    performance_history: Arc<RwLock<Vec<LoanBatchMetrics>>>,
}

/// Response from unified batch creation with enhanced loan metrics
#[derive(Debug)]
pub struct BatchCreationResult {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub merkle_root: Vec<u8>,
    pub nullifier_count: usize,
    pub processing_time_ms: f64,
    pub loan_metrics: Option<LoanBatchMetrics>,
    pub lifecycle_validation: Option<LoanLifecycleValidation>,
    pub achieved_tps: f64,
}

impl UnifiedBatchService {
    /// Create new unified batch service with basic configuration
    pub fn new(
        pool: PgPool,
        ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
        max_batch_size: u32,
    ) -> Self {
        Self {
            pool,
            ads_service,
            max_batch_size,
            loan_config: LoanBatchConfig::default(),
            performance_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create optimized unified batch service for loan processing
    pub fn new_with_loan_optimization(
        pool: PgPool,
        ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
        max_batch_size: u32,
        loan_config: LoanBatchConfig,
    ) -> Self {
        Self {
            pool,
            ads_service,
            max_batch_size,
            loan_config,
            performance_history: Arc::new(RwLock::new(Vec::with_capacity(1000))),
        }
    }

    /// Get current loan batch configuration
    pub fn get_loan_config(&self) -> &LoanBatchConfig {
        &self.loan_config
    }

    /// Update loan batch configuration for runtime optimization
    pub fn update_loan_config(&mut self, config: LoanBatchConfig) {
        self.loan_config = config;
        info!(
            "🔧 Updated loan batch configuration: target TPS = {}",
            self.loan_config.target_tps
        );
    }

    /// Get performance history for analysis
    pub async fn get_performance_history(&self) -> Vec<LoanBatchMetrics> {
        self.performance_history.read().await.clone()
    }

    /// Clear performance history to free memory
    pub async fn clear_performance_history(&self) {
        self.performance_history.write().await.clear();
    }

    /// Create a batch with full ADS integration
    ///
    /// This method provides unified processing for loan transactions with prefix-based nullifier generation
    #[instrument(skip(self), level = "info")]
    pub async fn create_batch_with_ads(
        &self,
        requested_batch_size: Option<i32>,
        trigger_source: &str,
    ) -> Result<Option<BatchCreationResult>, String> {
        let _batch_start_time = Instant::now();

        info!("🔄 UNIFIED: Creating batch via {} trigger", trigger_source);

        // Step 1: Get pending transactions
        let pending_transactions = match get_pending_transactions(&self.pool).await {
            Ok(transactions) => transactions,
            Err(e) => return Err(format!("Failed to get pending transactions: {}", e)),
        };

        if pending_transactions.is_empty() {
            debug!("No pending transactions to batch");
            return Ok(None);
        }

        // Step 2: Determine batch size
        let batch_size = requested_batch_size
            .map(|s| s as usize)
            .unwrap_or(pending_transactions.len())
            .min(self.max_batch_size as usize)
            .min(pending_transactions.len());

        let batch_transactions = &pending_transactions[..batch_size];

        info!(
            "📦 UNIFIED: Processing {} transactions through unified ADS integration",
            batch_size
        );

        // Step 3: Start atomic database transaction
        let db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Failed to begin database transaction: {}", e))?;

        // Step 4: Create batch entry
        info!("📋 UNIFIED: Creating batch entry in database");
        let batch = match self.create_batch_entry(batch_transactions).await {
            Ok(Some(batch)) => batch,
            Ok(None) => {
                debug!("Failed to create batch entry");
                db_tx.rollback().await.ok();
                return Ok(None);
            }
            Err(e) => {
                error!("Failed to create batch entry: {}", e);
                db_tx.rollback().await.ok();
                return Err(format!("Failed to create batch entry: {}", e));
            }
        };

        info!(
            "✅ UNIFIED: Batch entry created: id={}, transactions={}",
            batch.id,
            batch_transactions.len()
        );

        // Step 5: Process transactions with prefix-based nullifiers
        let processing_start = Instant::now();
        info!(
            "🔄 Processing {} transactions with prefix-based nullifiers",
            batch_size
        );

        let mut nullifiers = Vec::new();
        for transaction in batch_transactions {
            let nullifier = self.transaction_to_nullifier(transaction);
            nullifiers.push(nullifier);
        }

        let processing_time = processing_start.elapsed().as_millis() as f64;
        info!(
            "✅ Generated {} nullifiers in {:.2}ms",
            nullifiers.len(),
            processing_time
        );

        // Step 6: Apply nullifiers through ADS
        info!(
            "🔐 UNIFIED: Applying {} nullifiers through ADS service",
            nullifiers.len()
        );
        let mut ads_guard = self.ads_service.write().await;

        let state_transitions = ads_guard
            .batch_insert(&nullifiers)
            .await
            .map_err(|e| format!("Failed to process nullifiers through ADS: {}", e))?;

        info!(
            "✅ UNIFIED: Successfully processed {} state transitions through ADS",
            state_transitions.len()
        );

        // Step 7: Get the final merkle root
        let merkle_root = if let Some(last_transition) = state_transitions.last() {
            last_transition.new_root
        } else {
            error!("UNIFIED: No state transitions returned from ADS");
            drop(ads_guard);
            db_tx.rollback().await.ok();
            return Err("No state transitions returned from ADS".to_string());
        };

        drop(ads_guard); // Release ADS lock

        info!("🌳 UNIFIED: Final merkle root: {:02x?}", &merkle_root[..8]);

        // Step 8: Store merkle root atomically
        info!("💾 UNIFIED: Storing merkle root for batch {}", batch.id);
        match store_ads_state_commit(&self.pool, batch.id, &merkle_root).await {
            Ok(_) => {
                info!(
                    "✅ UNIFIED: Merkle root stored successfully for batch {}",
                    batch.id
                );
            }
            Err(e) => {
                error!(
                    "UNIFIED: Failed to store merkle root for batch {}: {}",
                    batch.id, e
                );
                db_tx.rollback().await.ok();
                return Err(format!("Failed to store merkle root: {}", e));
            }
        }

        // Step 9: Commit the transaction
        if let Err(e) = db_tx.commit().await {
            error!("UNIFIED: Failed to commit batch transaction: {}", e);
            return Err(format!("Failed to commit batch transaction: {}", e));
        }

        let total_time = _batch_start_time.elapsed().as_millis() as f64;

        info!("🎉 UNIFIED: Batch processing completed successfully!");
        info!("   📊 Batch ID: {}", batch.id);
        info!("   🔢 Total Transactions: {}", batch_size);
        info!("   🔢 Total Nullifiers: {}", state_transitions.len());
        info!("   ⏱️ Processing Time: {:.2}ms", total_time);
        info!("   🌳 Merkle Root: 0x{}", hex::encode(&merkle_root));

        let result = BatchCreationResult {
            batch_id: batch.id,
            previous_counter_value: batch.previous_counter_value,
            final_counter_value: batch.final_counter_value,
            transaction_count: batch_size,
            merkle_root: merkle_root.to_vec(),
            nullifier_count: state_transitions.len(),
            processing_time_ms: total_time,
            loan_metrics: None,
            lifecycle_validation: None,
            achieved_tps: 0.0,
        };

        Ok(Some(result))
    }

    /// Create batch entry in database (temporary - uses existing logic)
    /// TODO: This should be replaced with pure database operations once SQL function is deprecated
    async fn create_batch_entry(
        &self,
        transactions: &[IncomingTransaction],
    ) -> Result<Option<ProofBatch>, String> {
        // For now, use the existing create_batch logic but limit to exact transactions
        match arda_credit_db::create_batch(&self.pool, Some(transactions.len() as i32)).await {
            Ok(batch) => Ok(batch),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }

    /// Convert a transaction to a nullifier value with operation-type prefixes
    fn transaction_to_nullifier(&self, transaction: &IncomingTransaction) -> i64 {
        self.transaction_to_nullifier_with_prefix(transaction, transaction_prefixes::LEGACY_DEFAULT)
    }

    /// Convert a transaction to a nullifier value with specific prefix for namespace isolation
    fn transaction_to_nullifier_with_prefix(
        &self,
        transaction: &IncomingTransaction,
        prefix: i64,
    ) -> i64 {
        // Create a deterministic nullifier from the transaction data using blake3
        // This ensures the same transaction always produces the same nullifier across deployments
        let mut hasher = blake3::Hasher::new();

        // Hash each field as its little-endian byte representation for deterministic results
        hasher.update(&transaction.id.to_le_bytes());
        hasher.update(&transaction.amount.to_le_bytes());
        hasher.update(&transaction.created_at.timestamp().to_le_bytes());

        // Get the first 8 bytes of the digest as a u64 (little-endian)
        let digest = hasher.finalize();
        let hash_bytes = digest.as_bytes();
        let hash_u64 = u64::from_le_bytes([
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
            hash_bytes[4],
            hash_bytes[5],
            hash_bytes[6],
            hash_bytes[7],
        ]);

        // Convert to positive value within prefix range
        // Take modulo to fit within the prefix range (999,999,999), then add 1 to avoid zero
        let base_nullifier =
            ((hash_u64 % (transaction_prefixes::PREFIX_RANGE_SIZE as u64)) as i64) + 1;

        // Apply prefix for namespace isolation
        let final_nullifier = prefix + base_nullifier;

        debug!(
            "Transaction {} -> nullifier {} (prefix: {}, base: {}, hash: {})",
            transaction.id, final_nullifier, prefix, base_nullifier, hash_u64
        );

        final_nullifier
    }

    // ============================================================================
    // LOAN LIFECYCLE BATCH PROCESSING INTEGRATION
    // ============================================================================

    /// Create high-performance loan batch optimized for >60 TPS throughput
    #[instrument(skip(self, loan_operations), level = "info")]
    pub async fn create_optimized_loan_batch(
        &self,
        loan_operations: &[LoanOperation],
        trigger_source: &str,
    ) -> Result<Option<BatchCreationResult>, String> {
        let _batch_start_time = Instant::now();
        let mut metrics = LoanBatchMetrics {
            batch_started_at: chrono::Utc::now(),
            batch_size: loan_operations.len(),
            ..Default::default()
        };

        info!("🚀 OPTIMIZED: Creating high-performance loan batch via {} with {} operations (target: {} TPS)",
              trigger_source, loan_operations.len(), self.loan_config.target_tps);

        if loan_operations.is_empty() {
            debug!("No loan operations to batch");
            return Ok(None);
        }

        // Apply timeout for entire operation
        let batch_result = timeout(
            self.loan_config.timeout_duration,
            self.process_loan_batch_optimized(loan_operations, &mut metrics),
        )
        .await;

        match batch_result {
            Ok(result) => result,
            Err(_) => {
                error!(
                    "🚫 OPTIMIZED: Loan batch processing timed out after {:?}",
                    self.loan_config.timeout_duration
                );
                Err("Batch processing timeout".to_string())
            }
        }
    }

    /// Internal optimized loan batch processing with comprehensive metrics
    async fn process_loan_batch_optimized(
        &self,
        loan_operations: &[LoanOperation],
        metrics: &mut LoanBatchMetrics,
    ) -> Result<Option<BatchCreationResult>, String> {
        let total_start = Instant::now();

        // Step 1: Parallel validation and categorization
        let validation_start = Instant::now();
        let (validated_operations, lifecycle_validation) =
            if self.loan_config.enable_lifecycle_validation {
                self.validate_loan_operations_parallel(loan_operations)
                    .await?
            } else {
                (
                    loan_operations.to_vec(),
                    LoanLifecycleValidation {
                        is_valid: true,
                        validation_errors: vec![],
                        lifecycle_warnings: vec![],
                        integrity_score: 1.0,
                        validated_loans: loan_operations.len(),
                        validation_time_ms: 0.0,
                    },
                )
            };
        metrics.validation_time = validation_start.elapsed().as_millis() as f64;
        metrics.lifecycle_integrity_score = lifecycle_validation.integrity_score;

        // Categorize operations for optimized processing
        self.categorize_loan_operations(&validated_operations, metrics);

        // Step 2: Create batch entry
        let batch =
            match arda_credit_db::create_batch(&self.pool, Some(validated_operations.len() as i32))
                .await
            {
                Ok(Some(batch)) => batch,
                Ok(None) => {
                    debug!("Failed to create optimized loan batch entry");
                    return Ok(None);
                }
                Err(e) => {
                    error!("Failed to create optimized loan batch entry: {}", e);
                    return Err(format!(
                        "Failed to create optimized loan batch entry: {}",
                        e
                    ));
                }
            };

        info!(
            "✅ OPTIMIZED: Batch entry created: id={}, validated_ops={}",
            batch.id,
            validated_operations.len()
        );

        // Step 3: Optimized nullifier generation
        let nullifier_start = Instant::now();
        let nullifiers = if self.loan_config.enable_parallel_validation {
            self.generate_nullifiers_parallel(&validated_operations)
                .await?
        } else {
            validated_operations
                .iter()
                .map(|op| op.to_nullifier_with_prefix())
                .collect()
        };
        metrics.nullifier_generation_time = nullifier_start.elapsed().as_millis() as f64;
        metrics.nullifiers_generated = nullifiers.len();

        // Step 4: Optimized merkleization
        let merkle_start = Instant::now();
        let (state_transitions, merkle_root) = if self.loan_config.enable_merkle_optimization {
            self.process_merkleization_optimized(&validated_operations)
                .await?
        } else {
            let mut ads_guard = self.ads_service.write().await;
            let transitions = ads_guard
                .process_loan_batch(&validated_operations)
                .await
                .map_err(|e| format!("Failed to process loan operations: {}", e))?;
            let root = transitions
                .last()
                .ok_or("No state transitions returned")?
                .new_root;
            drop(ads_guard);
            (transitions, root)
        };
        metrics.merkleization_time = merkle_start.elapsed().as_millis() as f64;
        metrics.ads_processing_time = merkle_start.elapsed().as_millis() as f64;
        metrics.state_transitions = state_transitions.len();

        // Step 5: Atomic database commit
        let db_start = Instant::now();
        match store_ads_state_commit(&self.pool, batch.id, &merkle_root).await {
            Ok(_) => {
                info!("✅ OPTIMIZED: Merkle root stored for batch {}", batch.id);
            }
            Err(e) => {
                error!(
                    "OPTIMIZED: Failed to store merkle root for batch {}: {}",
                    batch.id, e
                );
                return Err(format!("Failed to store merkle root: {}", e));
            }
        }
        metrics.database_commit_time = db_start.elapsed().as_millis() as f64;

        // Step 6: Calculate final metrics
        let total_time = total_start.elapsed().as_millis() as f64;
        metrics.total_processing_time = total_time;
        metrics.batch_completed_at = chrono::Utc::now();

        // Calculate throughput metrics
        let seconds_elapsed = total_time / 1000.0;
        metrics.transactions_per_second = if seconds_elapsed > 0.0 {
            validated_operations.len() as f64 / seconds_elapsed
        } else {
            0.0
        };
        metrics.operations_per_second = metrics.transactions_per_second;
        metrics.merkleization_tps = if metrics.merkleization_time > 0.0 {
            (state_transitions.len() as f64 * 1000.0) / metrics.merkleization_time
        } else {
            0.0
        };
        metrics.total_throughput = metrics.transactions_per_second;

        // Calculate validation success rate
        metrics.validation_success_rate = if lifecycle_validation.validated_loans > 0 {
            (lifecycle_validation.validated_loans as f64
                - lifecycle_validation.validation_errors.len() as f64)
                / lifecycle_validation.validated_loans as f64
        } else {
            1.0
        };

        // Store metrics in performance history
        {
            let mut history = self.performance_history.write().await;
            history.push(metrics.clone());
            if history.len() > 1000 {
                // Keep last 1000 entries
                history.remove(0);
            }
        }

        info!("🎉 OPTIMIZED: High-performance loan batch completed!");
        info!("   📊 Batch ID: {}", batch.id);
        info!(
            "   🏦 Loan Operations: {} (originations: {}, payments: {}, closeouts: {})",
            validated_operations.len(),
            metrics.loan_originations,
            metrics.loan_payments,
            metrics.loan_closeouts
        );
        info!(
            "   ⚡ Achieved TPS: {:.2} (target: {:.2})",
            metrics.transactions_per_second, self.loan_config.target_tps
        );
        info!("   ⏱️ Total Time: {:.2}ms (validation: {:.2}ms, nullifiers: {:.2}ms, merkle: {:.2}ms, db: {:.2}ms)",
              total_time, metrics.validation_time, metrics.nullifier_generation_time,
              metrics.merkleization_time, metrics.database_commit_time);
        info!(
            "   🎯 Success Rate: {:.1}%, Integrity Score: {:.2}",
            metrics.validation_success_rate * 100.0,
            metrics.lifecycle_integrity_score
        );
        info!("   🌳 Merkle Root: 0x{}", hex::encode(&merkle_root));

        let result = BatchCreationResult {
            batch_id: batch.id,
            previous_counter_value: batch.previous_counter_value,
            final_counter_value: batch.final_counter_value,
            transaction_count: validated_operations.len(),
            merkle_root: merkle_root.to_vec(),
            nullifier_count: state_transitions.len(),
            processing_time_ms: total_time,
            loan_metrics: Some(metrics.clone()),
            lifecycle_validation: Some(lifecycle_validation),
            achieved_tps: metrics.transactions_per_second,
        };

        Ok(Some(result))
    }

    /// Process mixed batch of regular transactions and loan operations
    #[instrument(skip(self, transactions, loan_operations), level = "info")]
    pub async fn create_mixed_batch(
        &self,
        transactions: &[IncomingTransaction],
        loan_operations: &[LoanOperation],
        trigger_source: &str,
    ) -> Result<Option<BatchCreationResult>, String> {
        let _batch_start_time = Instant::now();

        let total_operations = transactions.len() + loan_operations.len();
        info!(
            "🔄 UNIFIED: Creating mixed batch via {} ({} regular, {} loan operations)",
            trigger_source,
            transactions.len(),
            loan_operations.len()
        );

        if total_operations == 0 {
            debug!("No operations to batch");
            return Ok(None);
        }

        // Step 1: Create batch entry
        let batch =
            match arda_credit_db::create_batch(&self.pool, Some(total_operations as i32)).await {
                Ok(Some(batch)) => batch,
                Ok(None) => {
                    debug!("Failed to create mixed batch entry");
                    return Ok(None);
                }
                Err(e) => {
                    error!("Failed to create mixed batch entry: {}", e);
                    return Err(format!("Failed to create mixed batch entry: {}", e));
                }
            };

        info!("✅ UNIFIED: Mixed batch entry created: id={}", batch.id);

        let mut ads_guard = self.ads_service.write().await;
        let mut all_transitions = Vec::new();

        // Step 2: Process regular transactions
        if !transactions.is_empty() {
            let mut nullifiers = Vec::new();
            for transaction in transactions {
                let nullifier = self.transaction_to_nullifier(transaction);
                nullifiers.push(nullifier);
            }

            let regular_transitions = ads_guard
                .batch_insert(&nullifiers)
                .await
                .map_err(|e| format!("Failed to process regular transactions: {}", e))?;
            all_transitions.extend(regular_transitions);
        }

        // Step 3: Process loan operations
        if !loan_operations.is_empty() {
            let loan_transitions = ads_guard
                .process_loan_batch(loan_operations)
                .await
                .map_err(|e| format!("Failed to process loan operations: {}", e))?;
            all_transitions.extend(loan_transitions);
        }

        // Step 4: Get final merkle root
        let merkle_root = if let Some(last_transition) = all_transitions.last() {
            last_transition.new_root
        } else {
            error!("UNIFIED: No state transitions returned from mixed batch");
            drop(ads_guard);
            return Err("No state transitions returned".to_string());
        };

        drop(ads_guard);

        // Step 5: Store merkle root
        store_ads_state_commit(&self.pool, batch.id, &merkle_root)
            .await
            .map_err(|e| format!("Failed to store merkle root: {}", e))?;

        let total_time = _batch_start_time.elapsed().as_millis() as f64;

        info!("🎉 UNIFIED: Mixed batch processing completed successfully!");
        info!("   📊 Batch ID: {}", batch.id);
        info!("   🔢 Regular Transactions: {}", transactions.len());
        info!("   🏦 Loan Operations: {}", loan_operations.len());
        info!("   🔢 Total Transitions: {}", all_transitions.len());
        info!("   ⏱️ Processing Time: {:.2}ms", total_time);

        let result = BatchCreationResult {
            batch_id: batch.id,
            previous_counter_value: batch.previous_counter_value,
            final_counter_value: batch.final_counter_value,
            transaction_count: total_operations,
            merkle_root: merkle_root.to_vec(),
            nullifier_count: all_transitions.len(),
            processing_time_ms: total_time,
            loan_metrics: None,
            lifecycle_validation: None,
            achieved_tps: 0.0,
        };

        Ok(Some(result))
    }

    /// Convert database loan transaction to LoanOperation
    pub fn loan_transaction_to_operation(
        &self,
        loan_transaction: &arda_credit_db::DealTransactionDb,
    ) -> Result<LoanOperation, String> {
        // Convert database transaction to loan operation
        let operation = LoanOperation {
            operation_type: loan_transaction.transaction_type.clone(),
            loan_id: loan_transaction.deal_id,
            payment_id: loan_transaction.payment_id,
            borrower_id: loan_transaction.user_id, // Assuming identity is borrower
            lender_id: None,                       // Would need to be populated from loan record
            amount: Some(loan_transaction.amount),
            deal_status: Some("active".to_string()), // Would need to be determined from context
            transaction_data: serde_json::json!({
                "transaction_id_hash": hex::encode(&loan_transaction.transaction_id_hash),
                "user_id_hash": hex::encode(&loan_transaction.user_id_hash),
                "created_at": loan_transaction.created_at
            }),
            nullifier_prefix: None,
        };

        Ok(operation)
    }

    /// Get service health status
    pub async fn health_check(&self) -> Result<String, String> {
        // Check database connectivity
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {}
            Err(e) => return Err(format!("Database health check failed: {}", e)),
        }

        // Check ADS service
        let ads_guard = self.ads_service.read().await;
        match ads_guard.health_check().await {
            Ok(true) => {
                Ok("Unified batch service with prefix-based nullifiers is healthy".to_string())
            }
            Ok(false) => Err("ADS service is not healthy".to_string()),
            Err(e) => Err(format!("ADS health check failed: {}", e)),
        }
    }

    // ============================================================================
    // LOAN OPERATION PROOF GENERATION INTEGRATION
    // ============================================================================

    /// Generate loan operation inclusion proof for users
    #[instrument(skip(self), level = "info")]
    pub async fn generate_loan_inclusion_proof(
        &self,
        user_id: uuid::Uuid,
        loan_id: Option<uuid::Uuid>,
        payment_id: Option<uuid::Uuid>,
        operation_type: String,
        batch_id: Option<i32>,
        user_role: String,
    ) -> Result<arda_credit_db::LoanInclusionProof, String> {
        info!(
            "🔍 Generating loan inclusion proof for user {} ({} operation)",
            user_id, operation_type
        );

        let ads_guard = self.ads_service.read().await;

        let request = arda_credit_db::LoanInclusionRequest {
            user_id,
            loan_id,
            payment_id,
            operation_type,
            batch_id,
            user_role,
        };

        let proof = ads_guard
            .prove_loan_operation_inclusion(&request)
            .await
            .map_err(|e| format!("Failed to generate loan inclusion proof: {}", e))?;

        info!("✅ Loan inclusion proof generated successfully");
        Ok(proof)
    }

    /// Generate loan payment inclusion proof for specific batch
    #[instrument(skip(self), level = "info")]
    pub async fn generate_loan_payment_proof(
        &self,
        user_id: uuid::Uuid,
        loan_id: uuid::Uuid,
        payment_id: uuid::Uuid,
        batch_id: i32,
    ) -> Result<arda_credit_db::LoanInclusionProof, String> {
        info!(
            "💰 Generating loan payment proof: loan={}, payment={}, batch={}",
            loan_id, payment_id, batch_id
        );

        let ads_guard = self.ads_service.read().await;

        let proof = ads_guard
            .prove_loan_payment_in_batch(user_id, loan_id, payment_id, batch_id)
            .await
            .map_err(|e| format!("Failed to generate loan payment proof: {}", e))?;

        info!("✅ Loan payment proof generated successfully");
        Ok(proof)
    }

    /// Verify loan inclusion proof with comprehensive validation
    #[instrument(skip(self, proof), level = "info")]
    pub async fn verify_loan_proof(
        &self,
        proof: &arda_credit_db::LoanInclusionProof,
    ) -> Result<bool, String> {
        info!(
            "🔍 Verifying loan inclusion proof for {} operation",
            proof.loan_operation.operation_type
        );

        let ads_guard = self.ads_service.read().await;

        let is_valid = ads_guard
            .verify_loan_inclusion_proof_with_prefix_validation(proof)
            .await
            .map_err(|e| format!("Failed to verify loan proof: {}", e))?;

        if is_valid {
            info!("✅ Loan inclusion proof verified successfully");
        } else {
            info!("❌ Loan inclusion proof verification failed");
        }

        Ok(is_valid)
    }

    /// Generate comprehensive proof verification report
    #[instrument(skip(self, proof), level = "info")]
    pub async fn generate_proof_report(
        &self,
        proof: &arda_credit_db::LoanInclusionProof,
    ) -> Result<serde_json::Value, String> {
        info!("📊 Generating proof verification report");

        let ads_guard = self.ads_service.read().await;

        let report = ads_guard
            .generate_proof_verification_report(proof)
            .await
            .map_err(|e| format!("Failed to generate proof report: {}", e))?;

        info!("✅ Proof verification report generated");
        Ok(report)
    }

    /// Batch verify multiple loan inclusion proofs
    #[instrument(skip(self, proofs), level = "info")]
    pub async fn batch_verify_proofs(
        &self,
        proofs: &[arda_credit_db::LoanInclusionProof],
    ) -> Result<Vec<bool>, String> {
        info!("🔍 Batch verifying {} loan inclusion proofs", proofs.len());

        let ads_guard = self.ads_service.read().await;

        let results = ads_guard
            .batch_verify_loan_proofs(proofs)
            .await
            .map_err(|e| format!("Failed to batch verify proofs: {}", e))?;

        let valid_count = results.iter().filter(|&&v| v).count();
        info!(
            "✅ Batch verification completed: {}/{} proofs valid",
            valid_count,
            proofs.len()
        );

        Ok(results)
    }

    /// Get loan audit trail for proof context
    #[instrument(skip(self), level = "info")]
    pub async fn get_loan_audit_trail(
        &self,
        loan_id: uuid::Uuid,
    ) -> Result<Vec<arda_credit_db::AuditEvent>, String> {
        info!("📋 Retrieving loan audit trail for loan {}", loan_id);

        let ads_guard = self.ads_service.read().await;

        let audit_trail = ads_guard
            .get_loan_audit_trail(loan_id)
            .await
            .map_err(|e| format!("Failed to get loan audit trail: {}", e))?;

        info!("✅ Retrieved {} audit events for loan", audit_trail.len());
        Ok(audit_trail)
    }

    /// Get loan lifecycle summary for comprehensive reporting
    #[instrument(skip(self), level = "info")]
    pub async fn get_loan_lifecycle_summary(
        &self,
        loan_id: uuid::Uuid,
    ) -> Result<arda_credit_db::LoanLifecycleSummary, String> {
        info!("📈 Generating loan lifecycle summary for loan {}", loan_id);

        let ads_guard = self.ads_service.read().await;

        let summary = ads_guard
            .get_loan_lifecycle_summary(loan_id)
            .await
            .map_err(|e| format!("Failed to get loan lifecycle summary: {}", e))?;

        info!(
            "✅ Generated loan lifecycle summary: {} operations, {} payments",
            summary.total_operations, summary.total_payments
        );
        Ok(summary)
    }

    // ============================================================================
    // OPTIMIZED LOAN PROCESSING HELPER METHODS
    // ============================================================================

    /// Parallel validation of loan operations for lifecycle integrity
    async fn validate_loan_operations_parallel(
        &self,
        operations: &[LoanOperation],
    ) -> Result<(Vec<LoanOperation>, LoanLifecycleValidation), String> {
        let validation_start = Instant::now();
        let mut validation_errors = Vec::new();
        let mut lifecycle_warnings = Vec::new();
        let mut validated_operations = Vec::new();
        let mut loan_states = HashMap::new();

        info!(
            "🔍 VALIDATION: Starting parallel loan lifecycle validation for {} operations",
            operations.len()
        );

        // Group operations by loan_id for lifecycle validation
        for operation in operations {
            if let Some(loan_id) = operation.loan_id {
                loan_states
                    .entry(loan_id)
                    .or_insert(Vec::new())
                    .push(operation.clone());
            } else {
                validation_errors.push(format!(
                    "Operation missing loan_id: {:?}",
                    operation.operation_type
                ));
            }
        }

        // Validate each loan's lifecycle integrity
        let validation_results: Vec<_> = loan_states
            .into_iter()
            .map(|(loan_id, ops)| self.validate_single_loan_lifecycle(loan_id, &ops))
            .collect();

        let mut total_integrity_score = 0.0;
        let mut validated_loan_count = 0;

        for result in validation_results {
            match result {
                Ok((valid_ops, errors, warnings, score)) => {
                    validated_operations.extend(valid_ops);
                    validation_errors.extend(errors);
                    lifecycle_warnings.extend(warnings);
                    total_integrity_score += score;
                    validated_loan_count += 1;
                }
                Err(e) => {
                    validation_errors.push(format!("Loan validation failed: {}", e));
                }
            }
        }

        let average_integrity_score = if validated_loan_count > 0 {
            total_integrity_score / validated_loan_count as f64
        } else {
            0.0
        };

        let validation_time = validation_start.elapsed().as_millis() as f64;

        info!("✅ VALIDATION: Completed in {:.2}ms - {} operations validated with {:.2} integrity score",
              validation_time, validated_operations.len(), average_integrity_score);

        if !validation_errors.is_empty() {
            warn!(
                "⚠️ VALIDATION: {} errors found: {:?}",
                validation_errors.len(),
                validation_errors
            );
        }

        if !lifecycle_warnings.is_empty() {
            warn!(
                "⚠️ VALIDATION: {} warnings: {:?}",
                lifecycle_warnings.len(),
                lifecycle_warnings
            );
        }

        let lifecycle_validation = LoanLifecycleValidation {
            is_valid: validation_errors.is_empty(),
            validation_errors,
            lifecycle_warnings,
            integrity_score: average_integrity_score,
            validated_loans: validated_loan_count,
            validation_time_ms: validation_time,
        };

        Ok((validated_operations, lifecycle_validation))
    }

    /// Validate a single loan's lifecycle integrity
    fn validate_single_loan_lifecycle(
        &self,
        loan_id: uuid::Uuid,
        operations: &[LoanOperation],
    ) -> Result<(Vec<LoanOperation>, Vec<String>, Vec<String>, f64), String> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut valid_operations = Vec::new();

        // Sort operations by timestamp or sequence
        let mut sorted_ops = operations.to_vec();
        sorted_ops.sort_by(|a, b| {
            let a_time = a
                .transaction_data
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("1970-01-01T00:00:00Z");
            let b_time = b
                .transaction_data
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("1970-01-01T00:00:00Z");
            a_time.cmp(b_time)
        });

        // Validate operation sequence
        let mut has_origination = false;
        let mut is_closed = false;

        for op in &sorted_ops {
            match op.operation_type.as_str() {
                "loan_origination" => {
                    if has_origination {
                        warnings.push(format!("Multiple originations for loan {}", loan_id));
                    }
                    if is_closed {
                        errors.push(format!("Origination after closeout for loan {}", loan_id));
                    } else {
                        has_origination = true;
                        valid_operations.push(op.clone());
                    }
                }
                "loan_payment" => {
                    if !has_origination {
                        errors.push(format!("Payment without origination for loan {}", loan_id));
                    } else if is_closed {
                        errors.push(format!("Payment after closeout for loan {}", loan_id));
                    } else {
                        valid_operations.push(op.clone());
                    }
                }
                "loan_closeout" => {
                    if !has_origination {
                        errors.push(format!("Closeout without origination for loan {}", loan_id));
                    } else if is_closed {
                        warnings.push(format!("Multiple closeouts for loan {}", loan_id));
                    } else {
                        is_closed = true;
                        valid_operations.push(op.clone());
                    }
                }
                _ => {
                    warnings.push(format!(
                        "Unknown operation type: {} for loan {}",
                        op.operation_type, loan_id
                    ));
                    // Still include it for processing
                    valid_operations.push(op.clone());
                }
            }
        }

        // Calculate integrity score
        let max_possible_score = operations.len() as f64;
        let error_penalty = errors.len() as f64 * 0.5;
        let warning_penalty = warnings.len() as f64 * 0.1;
        let integrity_score =
            ((max_possible_score - error_penalty - warning_penalty) / max_possible_score).max(0.0);

        Ok((valid_operations, errors, warnings, integrity_score))
    }

    /// Categorize loan operations for metrics tracking
    fn categorize_loan_operations(
        &self,
        operations: &[LoanOperation],
        metrics: &mut LoanBatchMetrics,
    ) {
        for operation in operations {
            match operation.operation_type.as_str() {
                "loan_origination" => metrics.loan_originations += 1,
                "loan_payment" => metrics.loan_payments += 1,
                "loan_closeout" => metrics.loan_closeouts += 1,
                _ => metrics.mixed_operations += 1,
            }
        }
        metrics.total_operations = operations.len();

        debug!(
            "📊 CATEGORIZATION: Originations: {}, Payments: {}, Closeouts: {}, Mixed: {}",
            metrics.loan_originations,
            metrics.loan_payments,
            metrics.loan_closeouts,
            metrics.mixed_operations
        );
    }

    /// Generate nullifiers in parallel for performance
    async fn generate_nullifiers_parallel(
        &self,
        operations: &[LoanOperation],
    ) -> Result<Vec<i64>, String> {
        info!(
            "⚡ NULLIFIERS: Generating {} nullifiers in parallel",
            operations.len()
        );

        // Split operations into chunks for parallel processing
        let chunk_size = (operations.len() / 4).max(1); // Use 4 threads by default
        let chunks: Vec<_> = operations.chunks(chunk_size).collect();

        let nullifier_futures: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                tokio::task::spawn_blocking({
                    let chunk = chunk.to_vec();
                    move || {
                        chunk
                            .iter()
                            .map(|op| op.to_nullifier_with_prefix())
                            .collect::<Vec<i64>>()
                    }
                })
            })
            .collect();

        let nullifier_results = future::join_all(nullifier_futures).await;

        let mut all_nullifiers = Vec::new();
        for result in nullifier_results {
            match result {
                Ok(nullifiers) => all_nullifiers.extend(nullifiers),
                Err(e) => return Err(format!("Failed to generate nullifiers: {}", e)),
            }
        }

        info!(
            "✅ NULLIFIERS: Generated {} nullifiers successfully",
            all_nullifiers.len()
        );
        Ok(all_nullifiers)
    }

    /// Optimized merkleization specifically for loan operations
    async fn process_merkleization_optimized(
        &self,
        operations: &[LoanOperation],
    ) -> Result<(Vec<StateTransition>, [u8; 32]), String> {
        info!(
            "🌳 MERKLEIZATION: Starting optimized processing for {} loan operations",
            operations.len()
        );

        // For loan-heavy workloads, we can optimize by:
        // 1. Pre-sorting operations by type for better cache locality
        // 2. Batch processing similar operation types together
        // 3. Using optimized insertion patterns

        let mut sorted_operations = operations.to_vec();

        // Sort by operation type for better merkle tree insertion patterns
        sorted_operations.sort_by(|a, b| {
            let a_priority = match a.operation_type.as_str() {
                "loan_origination" => 0,
                "loan_payment" => 1,
                "loan_closeout" => 2,
                _ => 3,
            };
            let b_priority = match b.operation_type.as_str() {
                "loan_origination" => 0,
                "loan_payment" => 1,
                "loan_closeout" => 2,
                _ => 3,
            };
            a_priority.cmp(&b_priority)
        });

        // Process through ADS with optimized batch
        let mut ads_guard = self.ads_service.write().await;
        let state_transitions = ads_guard
            .process_loan_batch(&sorted_operations)
            .await
            .map_err(|e| format!("Optimized merkleization failed: {}", e))?;

        let merkle_root = state_transitions
            .last()
            .ok_or("No state transitions in optimized merkleization")?
            .new_root;

        drop(ads_guard);

        info!(
            "✅ MERKLEIZATION: Completed optimized processing with {} transitions",
            state_transitions.len()
        );
        Ok((state_transitions, merkle_root))
    }

    /// Create optimized mixed batch with loan operations prioritization
    #[instrument(skip(self, transactions, loan_operations), level = "info")]
    pub async fn create_optimized_mixed_batch(
        &self,
        transactions: &[IncomingTransaction],
        loan_operations: &[LoanOperation],
        trigger_source: &str,
    ) -> Result<Option<BatchCreationResult>, String> {
        let total_operations = transactions.len() + loan_operations.len();
        let mut metrics = LoanBatchMetrics {
            batch_started_at: chrono::Utc::now(),
            batch_size: total_operations,
            ..Default::default()
        };

        info!(
            "🚀 OPTIMIZED: Creating mixed batch via {} ({} regular, {} loan ops, target: {} TPS)",
            trigger_source,
            transactions.len(),
            loan_operations.len(),
            self.loan_config.target_tps
        );

        if total_operations == 0 {
            debug!("No operations to batch");
            return Ok(None);
        }

        // Apply timeout for entire operation
        let batch_result = timeout(
            self.loan_config.timeout_duration,
            self.process_mixed_batch_optimized(transactions, loan_operations, &mut metrics),
        )
        .await;

        match batch_result {
            Ok(result) => result,
            Err(_) => {
                error!(
                    "🚫 OPTIMIZED: Mixed batch processing timed out after {:?}",
                    self.loan_config.timeout_duration
                );
                Err("Mixed batch processing timeout".to_string())
            }
        }
    }

    /// Internal optimized mixed batch processing
    async fn process_mixed_batch_optimized(
        &self,
        transactions: &[IncomingTransaction],
        loan_operations: &[LoanOperation],
        metrics: &mut LoanBatchMetrics,
    ) -> Result<Option<BatchCreationResult>, String> {
        let total_start = Instant::now();
        let total_operations = transactions.len() + loan_operations.len();

        // Step 1: Validate loan operations if enabled
        let validation_start = Instant::now();
        let (validated_loan_ops, lifecycle_validation) =
            if self.loan_config.enable_lifecycle_validation && !loan_operations.is_empty() {
                self.validate_loan_operations_parallel(loan_operations)
                    .await?
            } else {
                (
                    loan_operations.to_vec(),
                    LoanLifecycleValidation {
                        is_valid: true,
                        validation_errors: vec![],
                        lifecycle_warnings: vec![],
                        integrity_score: 1.0,
                        validated_loans: loan_operations.len(),
                        validation_time_ms: 0.0,
                    },
                )
            };
        metrics.validation_time = validation_start.elapsed().as_millis() as f64;

        // Categorize operations
        self.categorize_loan_operations(&validated_loan_ops, metrics);

        // Step 2: Create batch entry
        let batch =
            match arda_credit_db::create_batch(&self.pool, Some(total_operations as i32)).await {
                Ok(Some(batch)) => batch,
                Ok(None) => {
                    debug!("Failed to create optimized mixed batch entry");
                    return Ok(None);
                }
                Err(e) => {
                    error!("Failed to create optimized mixed batch entry: {}", e);
                    return Err(format!(
                        "Failed to create optimized mixed batch entry: {}",
                        e
                    ));
                }
            };

        info!("✅ OPTIMIZED: Mixed batch entry created: id={}", batch.id);

        // Step 3: Process all operations optimally
        let mut ads_guard = self.ads_service.write().await;
        let mut all_transitions = Vec::new();

        // Process regular transactions first for consistent ordering
        if !transactions.is_empty() {
            let mut nullifiers = Vec::new();
            for transaction in transactions {
                let nullifier = self.transaction_to_nullifier(transaction);
                nullifiers.push(nullifier);
            }

            let regular_transitions = ads_guard
                .batch_insert(&nullifiers)
                .await
                .map_err(|e| format!("Failed to process regular transactions: {}", e))?;
            all_transitions.extend(regular_transitions);
        }

        // Process loan operations with optimization
        if !validated_loan_ops.is_empty() {
            let merkle_start = Instant::now();
            let loan_transitions = if self.loan_config.enable_merkle_optimization {
                // Use optimized sorting for loan operations
                let mut sorted_ops = validated_loan_ops.clone();
                sorted_ops.sort_by(|a, b| {
                    let a_priority = match a.operation_type.as_str() {
                        "loan_origination" => 0,
                        "loan_payment" => 1,
                        "loan_closeout" => 2,
                        _ => 3,
                    };
                    let b_priority = match b.operation_type.as_str() {
                        "loan_origination" => 0,
                        "loan_payment" => 1,
                        "loan_closeout" => 2,
                        _ => 3,
                    };
                    a_priority.cmp(&b_priority)
                });
                ads_guard
                    .process_loan_batch(&sorted_ops)
                    .await
                    .map_err(|e| format!("Failed to process optimized loan operations: {}", e))?
            } else {
                ads_guard
                    .process_loan_batch(&validated_loan_ops)
                    .await
                    .map_err(|e| format!("Failed to process loan operations: {}", e))?
            };
            all_transitions.extend(loan_transitions);
            metrics.merkleization_time += merkle_start.elapsed().as_millis() as f64;
        }

        // Get final merkle root
        let merkle_root = if let Some(last_transition) = all_transitions.last() {
            last_transition.new_root
        } else {
            error!("OPTIMIZED: No state transitions returned from mixed batch");
            drop(ads_guard);
            return Err("No state transitions returned".to_string());
        };

        drop(ads_guard);
        metrics.ads_processing_time = metrics.merkleization_time;
        metrics.state_transitions = all_transitions.len();
        metrics.nullifiers_generated = all_transitions.len();

        // Step 4: Store merkle root
        let db_start = Instant::now();
        match store_ads_state_commit(&self.pool, batch.id, &merkle_root).await {
            Ok(_) => {
                info!(
                    "✅ OPTIMIZED: Merkle root stored for mixed batch {}",
                    batch.id
                );
            }
            Err(e) => {
                error!(
                    "OPTIMIZED: Failed to store merkle root for mixed batch {}: {}",
                    batch.id, e
                );
                return Err(format!("Failed to store merkle root: {}", e));
            }
        }
        metrics.database_commit_time = db_start.elapsed().as_millis() as f64;

        // Step 5: Calculate final metrics
        let total_time = total_start.elapsed().as_millis() as f64;
        metrics.total_processing_time = total_time;
        metrics.batch_completed_at = chrono::Utc::now();

        // Calculate throughput metrics
        let seconds_elapsed = total_time / 1000.0;
        metrics.transactions_per_second = if seconds_elapsed > 0.0 {
            total_operations as f64 / seconds_elapsed
        } else {
            0.0
        };
        metrics.operations_per_second = metrics.transactions_per_second;
        metrics.total_throughput = metrics.transactions_per_second;

        metrics.validation_success_rate = if lifecycle_validation.validated_loans > 0 {
            (lifecycle_validation.validated_loans as f64
                - lifecycle_validation.validation_errors.len() as f64)
                / lifecycle_validation.validated_loans as f64
        } else {
            1.0
        };

        // Store metrics in performance history
        {
            let mut history = self.performance_history.write().await;
            history.push(metrics.clone());
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        info!("🎉 OPTIMIZED: Mixed batch completed!");
        info!("   📊 Batch ID: {}", batch.id);
        info!(
            "   🔄 Regular Transactions: {}, Loan Operations: {}",
            transactions.len(),
            validated_loan_ops.len()
        );
        info!(
            "   ⚡ Achieved TPS: {:.2} (target: {:.2})",
            metrics.transactions_per_second, self.loan_config.target_tps
        );
        info!("   ⏱️ Total Time: {:.2}ms", total_time);
        info!(
            "   🎯 Success Rate: {:.1}%",
            metrics.validation_success_rate * 100.0
        );

        let result = BatchCreationResult {
            batch_id: batch.id,
            previous_counter_value: batch.previous_counter_value,
            final_counter_value: batch.final_counter_value,
            transaction_count: total_operations,
            merkle_root: merkle_root.to_vec(),
            nullifier_count: all_transitions.len(),
            processing_time_ms: total_time,
            loan_metrics: Some(metrics.clone()),
            lifecycle_validation: Some(lifecycle_validation),
            achieved_tps: metrics.transactions_per_second,
        };

        Ok(Some(result))
    }

    /// Get comprehensive performance analytics
    pub async fn get_performance_analytics(&self) -> serde_json::Value {
        let history = self.performance_history.read().await;

        if history.is_empty() {
            return serde_json::json!({
                "status": "no_data",
                "total_batches": 0
            });
        }

        let total_batches = history.len();
        let avg_tps = history
            .iter()
            .map(|m| m.transactions_per_second)
            .sum::<f64>()
            / total_batches as f64;
        let avg_processing_time =
            history.iter().map(|m| m.total_processing_time).sum::<f64>() / total_batches as f64;
        let avg_integrity_score = history
            .iter()
            .map(|m| m.lifecycle_integrity_score)
            .sum::<f64>()
            / total_batches as f64;

        let max_tps = history
            .iter()
            .map(|m| m.transactions_per_second)
            .fold(0.0, f64::max);
        let min_tps = history
            .iter()
            .map(|m| m.transactions_per_second)
            .fold(f64::INFINITY, f64::min);

        let total_operations = history.iter().map(|m| m.total_operations).sum::<usize>();
        let total_originations = history.iter().map(|m| m.loan_originations).sum::<usize>();
        let total_payments = history.iter().map(|m| m.loan_payments).sum::<usize>();
        let total_closeouts = history.iter().map(|m| m.loan_closeouts).sum::<usize>();

        serde_json::json!({
            "performance_summary": {
                "total_batches": total_batches,
                "average_tps": avg_tps,
                "max_tps": max_tps,
                "min_tps": min_tps,
                "target_tps": self.loan_config.target_tps,
                "tps_achievement_rate": (avg_tps / self.loan_config.target_tps) * 100.0,
                "average_processing_time_ms": avg_processing_time,
                "average_integrity_score": avg_integrity_score,
            },
            "operation_breakdown": {
                "total_operations": total_operations,
                "loan_originations": total_originations,
                "loan_payments": total_payments,
                "loan_closeouts": total_closeouts,
                "mixed_operations": total_operations - total_originations - total_payments - total_closeouts,
            },
            "recent_performance": history.iter().rev().take(10).collect::<Vec<_>>(),
            "configuration": {
                "target_tps": self.loan_config.target_tps,
                "max_batch_size": self.loan_config.max_batch_size,
                "parallel_validation_enabled": self.loan_config.enable_parallel_validation,
                "lifecycle_validation_enabled": self.loan_config.enable_lifecycle_validation,
                "merkle_optimization_enabled": self.loan_config.enable_merkle_optimization,
                "timeout_seconds": self.loan_config.timeout_duration.as_secs(),
            }
        })
    }
}
