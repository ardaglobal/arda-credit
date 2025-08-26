//! Background Batch Processing Service
//!
//! This module provides automatic batch processing with multiple triggers:
//! - Timer-based: Every 1 minute
//! - Count-based: When 10+ transactions are pending
//! - Manual: Via API trigger
//!
//! The service runs in the background alongside the API server.

use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, instrument, warn};

use crate::rest::ApiConfig;
use alloy_primitives::{Bytes, FixedBytes};
use arda_credit_db::{
    get_batch_by_id, get_pending_transactions, get_proven_unposted_batches,
    mark_batch_posted_to_contract_with_tx_hash, update_batch_proof, IndexedMerkleTreeADS,
};
use arda_credit_lib::proof::{generate_batch_proof, BatchProofGenerationRequest, ProofSystem};
use ethereum_client::{Config as EthConfig, EthereumClient};

// ============================================================================
// BATCH PROCESSOR CONFIGURATION
// ============================================================================

/// Configuration for the background batch processor
#[derive(Debug, Clone)]
pub struct BatchProcessorConfig {
    /// Timer interval for periodic batching (default: 1 minute)
    pub timer_interval_seconds: u64,

    /// Minimum number of pending transactions to trigger batching (default: 10)
    pub count_trigger_threshold: usize,

    /// Maximum batch size (should match API config)
    pub max_batch_size: u32,

    /// Whether to enable the background processor
    pub enabled: bool,

    /// Minimum time between batches to avoid too frequent processing
    pub min_batch_interval_seconds: u64,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            timer_interval_seconds: 60, // 1 minute
            count_trigger_threshold: 10,
            max_batch_size: 50,
            enabled: true,
            min_batch_interval_seconds: 5, // Minimum 5 seconds between batches
        }
    }
}

impl From<&ApiConfig> for BatchProcessorConfig {
    fn from(api_config: &ApiConfig) -> Self {
        Self {
            max_batch_size: api_config.max_batch_size,
            ..Default::default()
        }
    }
}

// ============================================================================
// BATCH PROCESSOR COMMANDS
// ============================================================================

/// Commands that can be sent to the batch processor
#[derive(Debug, Clone)]
pub enum BatchProcessorCommand {
    /// Trigger manual batch processing
    TriggerBatch,

    /// Stop the batch processor
    Stop,

    /// Update configuration
    UpdateConfig(BatchProcessorConfig),

    /// Get current status
    GetStatus,
}

/// Response from batch processor operations
#[derive(Debug, Clone)]
pub struct BatchProcessorResponse {
    pub success: bool,
    pub message: String,
    pub batch_id: Option<i32>,
    pub transaction_count: Option<usize>,
}

/// Statistics about the batch processor
#[derive(Debug, Clone, Default)]
pub struct BatchProcessorStats {
    pub total_batches_created: u64,
    pub total_transactions_processed: u64,
    pub last_batch_time: Option<Instant>,
    pub timer_triggers: u64,
    pub count_triggers: u64,
    pub manual_triggers: u64,
    pub errors: u64,
}

// ============================================================================
// BACKGROUND BATCH PROCESSOR
// ============================================================================

/// Background batch processor service
pub struct BackgroundBatchProcessor {
    config: BatchProcessorConfig,
    pool: PgPool,
    command_rx: mpsc::UnboundedReceiver<BatchProcessorCommand>,
    stats: Arc<RwLock<BatchProcessorStats>>,
    ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
}

/// Handle for communicating with the background batch processor
#[derive(Clone)]
pub struct BatchProcessorHandle {
    command_tx: mpsc::UnboundedSender<BatchProcessorCommand>,
    stats: Arc<RwLock<BatchProcessorStats>>,
}

impl BatchProcessorHandle {
    /// Trigger manual batch processing
    pub fn trigger_batch(&self) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::TriggerBatch)
            .map_err(|e| format!("Failed to send trigger command: {}", e))
    }

    /// Stop the batch processor
    pub fn stop(&self) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::Stop)
            .map_err(|e| format!("Failed to send stop command: {}", e))
    }

    /// Update processor configuration
    pub fn update_config(&self, config: BatchProcessorConfig) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::UpdateConfig(config))
            .map_err(|e| format!("Failed to send config update: {}", e))
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> BatchProcessorStats {
        self.stats.read().await.clone()
    }
}

impl BackgroundBatchProcessor {
    /// Create a new background batch processor
    pub fn new(
        pool: PgPool,
        config: BatchProcessorConfig,
        ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
    ) -> (Self, BatchProcessorHandle) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(RwLock::new(BatchProcessorStats::default()));

        let processor = Self {
            config,
            pool,
            command_rx,
            stats: stats.clone(),
            ads_service,
        };

        let handle = BatchProcessorHandle { command_tx, stats };

        (processor, handle)
    }

    /// Start the background batch processor
    #[instrument(skip(self), level = "info")]
    pub async fn run(mut self) {
        if !self.config.enabled {
            info!("🔄 Background batch processor is disabled");
            return;
        }

        info!("🚀 Starting background batch processor");
        info!(
            "⏰ Timer interval: {} seconds",
            self.config.timer_interval_seconds
        );
        info!(
            "📊 Count trigger threshold: {} transactions",
            self.config.count_trigger_threshold
        );
        info!("📦 Max batch size: {}", self.config.max_batch_size);

        // Start the continuous batch monitoring service
        let monitor_pool = self.pool.clone();
        tokio::spawn(async move {
            Self::run_batch_monitor_service(monitor_pool).await;
        });

        let mut timer = interval(Duration::from_secs(self.config.timer_interval_seconds));
        let mut last_batch_time = Instant::now();

        loop {
            tokio::select! {
                // Handle timer-based triggers
                _ = timer.tick() => {
                    if self.should_process_batch(last_batch_time) {
                        match self.process_batch("timer").await {
                            Ok(Some(_)) => {
                                last_batch_time = Instant::now();
                                self.increment_timer_triggers().await;
                            }
                            Ok(None) => {
                                debug!("Timer trigger: No transactions to batch");
                            }
                            Err(e) => {
                                error!("Timer trigger batch processing failed: {}", e);
                                self.increment_errors().await;
                            }
                        }
                    }
                }

                // Handle commands
                Some(command) = self.command_rx.recv() => {
                    match command {
                        BatchProcessorCommand::TriggerBatch => {
                            info!("🔄 Manual batch trigger received");
                            match self.process_batch("manual").await {
                                Ok(Some(_)) => {
                                    last_batch_time = Instant::now();
                                    self.increment_manual_triggers().await;
                                }
                                Ok(None) => {
                                    debug!("Manual trigger: No transactions to batch");
                                }
                                Err(e) => {
                                    error!("Manual trigger batch processing failed: {}", e);
                                    self.increment_errors().await;
                                }
                            }
                        }

                        BatchProcessorCommand::Stop => {
                            info!("🛑 Stopping background batch processor");
                            break;
                        }

                        BatchProcessorCommand::UpdateConfig(new_config) => {
                            info!("⚙️ Updating batch processor configuration");
                            self.config = new_config;
                            timer = interval(Duration::from_secs(self.config.timer_interval_seconds));
                        }

                        BatchProcessorCommand::GetStatus => {
                            let stats = self.stats.read().await;
                            debug!("📊 Batch processor stats: {:?}", *stats);
                        }
                    }
                }

                // Check count-based trigger periodically (every 10 seconds)
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    if self.should_process_batch(last_batch_time) {
                        match self.check_count_trigger().await {
                            Ok(true) => {
                                match self.process_batch("count").await {
                                    Ok(Some(_)) => {
                                        last_batch_time = Instant::now();
                                        self.increment_count_triggers().await;
                                    }
                                    Ok(None) => {
                                        debug!("Count trigger: No transactions to batch");
                                    }
                                    Err(e) => {
                                        error!("Count trigger batch processing failed: {}", e);
                                        self.increment_errors().await;
                                    }
                                }
                            }
                            Ok(false) => {
                                // Not enough transactions for count trigger
                            }
                            Err(e) => {
                                error!("Failed to check count trigger: {}", e);
                                self.increment_errors().await;
                            }
                        }
                    }
                }
            }
        }

        info!("✅ Background batch processor stopped");
    }

    /// Check if we should process a batch (respecting minimum interval)
    fn should_process_batch(&self, last_batch_time: Instant) -> bool {
        let elapsed = last_batch_time.elapsed();
        elapsed.as_secs() >= self.config.min_batch_interval_seconds
    }

    /// Check if count trigger should fire
    async fn check_count_trigger(&self) -> Result<bool, String> {
        match get_pending_transactions(&self.pool).await {
            Ok(transactions) => {
                let count = transactions.len();
                debug!("📊 Current pending transactions: {}", count);
                Ok(count >= self.config.count_trigger_threshold)
            }
            Err(e) => Err(format!("Failed to get pending transactions: {}", e)),
        }
    }

    /// Process a batch of transactions using unified ADS-integrated service
    #[instrument(skip(self), level = "info")]
    async fn process_batch(&self, trigger_type: &str) -> Result<Option<i32>, String> {
        info!(
            "🔄 UNIFIED: Processing batch via {} trigger (using unified service)",
            trigger_type
        );

        // Use unified batch service for consistent ADS integration
        let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
            self.pool.clone(),
            self.ads_service.clone(),
            self.config.max_batch_size,
        );

        match unified_service
            .create_batch_with_ads(None, trigger_type)
            .await
        {
            Ok(Some(result)) => {
                info!(
                    "✅ UNIFIED: Batch processed successfully via {}: id={}, transactions={}, nullifiers={}, merkle_root=0x{}",
                    trigger_type,
                    result.batch_id,
                    result.transaction_count,
                    result.nullifier_count,
                    hex::encode(&result.merkle_root[..8])
                );

                // Update statistics
                self.update_stats(result.batch_id, result.transaction_count)
                    .await;

                // Trigger proof generation asynchronously
                self.trigger_proof_generation(result.batch_id);

                Ok(Some(result.batch_id))
            }
            Ok(None) => {
                debug!("UNIFIED: No transactions available to batch");
                Ok(None)
            }
            Err(e) => {
                error!(
                    "UNIFIED: Failed to process batch via {}: {}",
                    trigger_type, e
                );
                Err(e)
            }
        }
    }

    /// Update processor statistics
    async fn update_stats(&self, _batch_id: i32, transaction_count: usize) {
        let mut stats = self.stats.write().await;
        stats.total_batches_created += 1;
        stats.total_transactions_processed += transaction_count as u64;
        stats.last_batch_time = Some(Instant::now());

        info!(
            "📊 Updated stats: batches={}, transactions={}",
            stats.total_batches_created, stats.total_transactions_processed
        );
    }

    async fn increment_timer_triggers(&self) {
        self.stats.write().await.timer_triggers += 1;
    }

    async fn increment_count_triggers(&self) {
        self.stats.write().await.count_triggers += 1;
    }

    async fn increment_manual_triggers(&self) {
        self.stats.write().await.manual_triggers += 1;
    }

    async fn increment_errors(&self) {
        self.stats.write().await.errors += 1;
    }

    /// Continuous batch monitoring service that runs independently
    async fn run_batch_monitor_service(pool: PgPool) {
        info!("🔄 Starting continuous batch monitoring service...");

        let mut interval = tokio::time::interval(Duration::from_secs(30)); // Check every 30 seconds
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // Phase 1: Submit proofs for batches missing Sindri proof IDs
            if let Err(e) = Self::submit_missing_proofs(&pool).await {
                error!("❌ Failed to submit missing proofs: {}", e);
            }

            // Phase 2: Update status for pending proofs
            if let Err(e) = Self::update_proof_statuses(&pool).await {
                error!("❌ Failed to update proof statuses: {}", e);
            }

            // Phase 3: Post proven batches to smart contract
            if let Err(e) = Self::post_proven_batches_to_contract(&pool).await {
                error!("❌ Failed to post proven batches to contract: {}", e);
            }

            // Small delay before next cycle
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    /// Phase 1: Submit proofs for batches missing Sindri proof IDs
    async fn submit_missing_proofs(pool: &PgPool) -> Result<(), String> {
        let batches_missing_proofs = sqlx::query!(
            "SELECT id FROM proof_batches
             WHERE sindri_proof_id IS NULL
                OR sindri_proof_id = ''
                OR sindri_proof_id LIKE 'failed_%'
                OR sindri_proof_id LIKE 'error_%'
             ORDER BY id ASC
             LIMIT 5" // Process in small batches to avoid overwhelming
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to query batches missing proof IDs: {}", e))?;

        if batches_missing_proofs.is_empty() {
            return Ok(()); // No work to do
        }

        info!(
            "🔍 Found {} batches missing valid proof IDs, submitting...",
            batches_missing_proofs.len()
        );

        for batch_record in batches_missing_proofs {
            let batch_id = batch_record.id;

            // Submit proof asynchronously and immediately store the proof ID
            tokio::spawn({
                let pool = pool.clone();
                async move {
                    if let Err(e) = Self::submit_proof_fast(&pool, batch_id).await {
                        error!("❌ Failed to submit proof for batch {}: {}", batch_id, e);
                    }
                }
            });

            // Small delay to avoid overwhelming Sindri
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        Ok(())
    }

    /// Phase 2: Update status for batches with pending proofs
    async fn update_proof_statuses(pool: &PgPool) -> Result<(), String> {
        let pending_batches = sqlx::query!(
            "SELECT id, sindri_proof_id FROM proof_batches
             WHERE proof_status = 'pending'
               AND sindri_proof_id IS NOT NULL
               AND sindri_proof_id != ''
               AND sindri_proof_id NOT LIKE 'failed_%'
               AND sindri_proof_id NOT LIKE 'error_%'
             ORDER BY id ASC
             LIMIT 10" // Check statuses in small batches
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to query pending batches: {}", e))?;

        if pending_batches.is_empty() {
            return Ok(()); // No pending proofs to check
        }

        info!(
            "🔍 Checking status for {} pending proofs...",
            pending_batches.len()
        );

        for batch_record in pending_batches {
            let batch_id = batch_record.id;
            let proof_id = batch_record.sindri_proof_id.unwrap_or_default();

            if proof_id.is_empty() {
                continue;
            }

            // Check proof status asynchronously
            tokio::spawn({
                let pool = pool.clone();
                let proof_id = proof_id.clone();
                async move {
                    if let Err(e) =
                        Self::check_and_update_proof_status(&pool, batch_id, &proof_id).await
                    {
                        error!(
                            "❌ Failed to check status for batch {} (proof {}): {}",
                            batch_id, proof_id, e
                        );
                    }
                }
            });

            // Small delay between status checks
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Fast proof submission that immediately stores the proof ID
    async fn submit_proof_fast(pool: &PgPool, batch_id: i32) -> Result<(), String> {
        info!("🚀 Fast proof submission for batch {}", batch_id);

        // Get batch details and transaction amounts
        let batch = get_batch_by_id(pool, batch_id)
            .await
            .map_err(|e| format!("Failed to get batch {}: {}", batch_id, e))?;

        let transaction_amounts: Vec<i32> = sqlx::query!(
            "SELECT amount FROM incoming_transactions WHERE id = ANY($1) ORDER BY id",
            &batch.transaction_ids
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get transaction amounts: {}", e))?
        .into_iter()
        .map(|row| row.amount)
        .collect();

        let initial_balance = batch.previous_counter_value as i32;

        // Create proof request
        let proof_request = BatchProofGenerationRequest {
            initial_balance,
            transactions: transaction_amounts,
            proof_system: ProofSystem::default(),
            generate_fixtures: false,
        };

        // Submit to Sindri and immediately store the proof ID
        match generate_batch_proof(proof_request).await {
            Ok(proof_response) => {
                info!(
                    "✅ Got Sindri proof ID for batch {}: {}",
                    batch_id, proof_response.proof_id
                );

                // Immediately store the proof ID in database
                let status = match proof_response.status.as_str() {
                    "Ready" => "proven",
                    "Failed" => "failed",
                    _ => "pending",
                };

                if let Err(e) =
                    update_batch_proof(pool, batch_id, &proof_response.proof_id, status).await
                {
                    error!("❌ Failed to store proof ID for batch {}: {}", batch_id, e);
                    return Err(format!("Failed to store proof ID: {}", e));
                }

                info!(
                    "📝 Stored proof ID {} for batch {} with status {}",
                    proof_response.proof_id, batch_id, status
                );
                Ok(())
            }
            Err(e) => {
                error!("❌ Sindri submission failed for batch {}: {}", batch_id, e);

                // Store error state
                let error_id = format!("error_{}", batch_id);
                if let Err(update_err) =
                    update_batch_proof(pool, batch_id, &error_id, "failed").await
                {
                    error!(
                        "❌ Failed to store error state for batch {}: {}",
                        batch_id, update_err
                    );
                }

                Err(format!("Sindri submission failed: {}", e))
            }
        }
    }

    /// Check proof status on Sindri and update database
    async fn check_and_update_proof_status(
        pool: &PgPool,
        batch_id: i32,
        proof_id: &str,
    ) -> Result<(), String> {
        use arda_credit_lib::proof::get_sindri_proof_info;

        match get_sindri_proof_info(proof_id).await {
            Ok(proof_info) => {
                let new_status = match proof_info.status {
                    sindri::JobStatus::Ready => "proven",
                    sindri::JobStatus::Failed => "failed",
                    _ => "pending", // Keep as pending
                };

                // Only update if status changed
                if new_status != "pending" {
                    if let Err(e) = update_batch_proof(pool, batch_id, proof_id, new_status).await {
                        error!("❌ Failed to update status for batch {}: {}", batch_id, e);
                        return Err(format!("Failed to update status: {}", e));
                    }
                    info!("📝 Updated batch {} status to {}", batch_id, new_status);
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to check proof status for {}: {}", proof_id, e);
                Err(format!("Failed to check proof status: {}", e))
            }
        }
    }

    /// Trigger proof generation for a batch asynchronously
    fn trigger_proof_generation(&self, batch_id: i32) {
        // The continuous monitoring service will pick up this batch automatically
        // No need to do anything here - just log that the batch was created
        info!(
            "📦 Batch {} created - monitoring service will handle proof generation",
            batch_id
        );
    }

    /// Generate a ZK proof for a specific batch
    pub async fn generate_proof_for_batch(pool: &PgPool, batch_id: i32) -> Result<(), String> {
        info!("🔐 Starting proof generation for batch: {}", batch_id);

        // Get batch details
        info!("📋 Fetching batch details for batch {}", batch_id);
        let batch = get_batch_by_id(pool, batch_id).await.map_err(|e| {
            error!("❌ Failed to get batch {}: {}", batch_id, e);
            format!("Failed to get batch {}: {}", batch_id, e)
        })?;
        info!(
            "✅ Retrieved batch details: transaction_ids={:?}",
            batch.transaction_ids
        );

        // Get individual transaction amounts for the batch
        info!("📋 Fetching transaction amounts for batch {}", batch_id);
        let transaction_amounts: Vec<i32> = sqlx::query!(
            "SELECT amount FROM incoming_transactions WHERE id = ANY($1) ORDER BY id",
            &batch.transaction_ids
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!(
                "❌ Failed to get transaction amounts for batch {}: {}",
                batch_id, e
            );
            format!("Failed to get transaction amounts: {}", e)
        })?
        .into_iter()
        .map(|row| row.amount)
        .collect();
        info!(
            "✅ Retrieved transaction amounts: {:?}",
            transaction_amounts
        );

        let initial_balance = batch.previous_counter_value as i32;
        let expected_final = initial_balance + transaction_amounts.iter().sum::<i32>();
        let actual_final = batch.final_counter_value as i32;

        info!(
            "🧮 Batch {} calculation check: initial={}, sum={}, expected_final={}, actual_final={}",
            batch_id,
            initial_balance,
            transaction_amounts.iter().sum::<i32>(),
            expected_final,
            actual_final
        );

        // Sanity check
        if expected_final != actual_final {
            error!(
                "❌ Batch {} transaction sum mismatch: expected {}, got {}",
                batch_id, expected_final, actual_final
            );
            return Err(format!(
                "Batch {} transaction sum mismatch: expected {}, got {}",
                batch_id, expected_final, actual_final
            ));
        }

        info!(
            "📊 Batch {} proof parameters: initial={}, transactions={:?}, final={}",
            batch_id, initial_balance, transaction_amounts, actual_final
        );

        // Create batch proof generation request
        info!(
            "📝 Creating batch proof generation request for batch {}",
            batch_id
        );
        let proof_request = BatchProofGenerationRequest {
            initial_balance,
            transactions: transaction_amounts.clone(),
            proof_system: ProofSystem::default(), // Use Groth16 by default
            generate_fixtures: false,             // Don't generate fixtures in production
        };
        info!(
            "✅ Created proof request: initial={}, transactions={:?}, proof_system={:?}",
            proof_request.initial_balance, proof_request.transactions, proof_request.proof_system
        );

        // Generate proof via Sindri
        info!(
            "🚀 Submitting batch proof request to Sindri for batch {}",
            batch_id
        );
        match generate_batch_proof(proof_request).await {
            Ok(proof_response) => {
                info!(
                    "✅ Proof submitted to Sindri for batch {}: proof_id={}",
                    batch_id, proof_response.proof_id
                );

                // Update batch with Sindri proof ID and appropriate status
                let status = match proof_response.status.as_str() {
                    "Ready" => "proven",
                    "Failed" => "failed",
                    _ => "pending", // Default for "Pending" or other statuses
                };

                if let Err(e) =
                    update_batch_proof(pool, batch_id, &proof_response.proof_id, status).await
                {
                    error!("Failed to update batch {} with proof ID: {}", batch_id, e);
                    return Err(format!("Failed to update batch with proof ID: {}", e));
                }

                info!(
                    "📝 Updated batch {} with Sindri proof ID: {} (status: {})",
                    batch_id, proof_response.proof_id, status
                );

                if status == "failed" {
                    Err(format!(
                        "Sindri proof generation failed for batch {}",
                        batch_id
                    ))
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                error!(
                    "Failed to submit proof request to Sindri for batch {}: {}",
                    batch_id, e
                );

                // Only update status to failed, don't create a fake proof ID
                // This handles cases where we couldn't even submit the request to Sindri
                if let Err(update_err) = update_batch_proof(
                    pool,
                    batch_id,
                    &format!("error_{}", batch_id), // Temporary ID to indicate submission error
                    "failed",
                )
                .await
                {
                    error!(
                        "Failed to update batch {} status to failed: {}",
                        batch_id, update_err
                    );
                }

                Err(format!("Failed to submit proof request: {}", e))
            }
        }
    }

    /// Phase 3: Post proven batches to smart contract
    async fn post_proven_batches_to_contract(pool: &PgPool) -> Result<(), String> {
        // Get proven batches that haven't been posted to contract yet
        let unposted_batches = get_proven_unposted_batches(pool, Some(5))
            .await
            .map_err(|e| format!("Failed to get proven unposted batches: {}", e))?;

        if unposted_batches.is_empty() {
            return Ok(()); // No work to do
        }

        info!(
            "🔗 Found {} proven batches to post to smart contract",
            unposted_batches.len()
        );

        // Log batch details for debugging
        for batch in &unposted_batches {
            debug!("📦 Batch {} ready for contract posting:", batch.id);
            debug!("   Sindri proof ID: {:?}", batch.sindri_proof_id);
            debug!(
                "   Counter: {} -> {}",
                batch.previous_counter_value, batch.final_counter_value
            );
            debug!("   Transaction count: {}", batch.transaction_ids.len());
            debug!("   Status: {}", batch.proof_status);
        }

        // Try to initialize Ethereum client (graceful fallback if not configured)
        let eth_client = match EthConfig::from_env() {
            Ok(config) => match EthereumClient::new(config).await {
                Ok(client) => Some(client),
                Err(e) => {
                    error!("❌ Failed to initialize Ethereum client: {}", e);
                    error!("   Smart contract posting will be skipped");
                    return Ok(());
                }
            },
            Err(e) => {
                error!("❌ Ethereum configuration not found: {}", e);
                error!("   Smart contract posting will be skipped");
                return Ok(());
            }
        };

        let eth_client = eth_client.unwrap();

        for batch in unposted_batches {
            // Submit to contract with retry logic and confirmation waiting
            match Self::submit_batch_with_retry(pool, &eth_client, &batch, 3).await {
                Ok(tx_hash) => {
                    info!(
                        "✅ Batch {} confirmed on-chain with tx hash: {}",
                        batch.id,
                        hex::encode(tx_hash)
                    );

                    // Mark as posted only after successful confirmation with transaction hash
                    let tx_hash_hex = format!("0x{}", hex::encode(tx_hash));
                    if let Err(e) =
                        mark_batch_posted_to_contract_with_tx_hash(pool, batch.id, &tx_hash_hex)
                            .await
                    {
                        error!(
                            "❌ Failed to mark batch {} as posted after confirmation: {}",
                            batch.id, e
                        );
                        // Even if marking fails, the transaction was successful on-chain
                        // Log this as a critical issue for manual resolution
                        error!("🚨 CRITICAL: Batch {} was successfully posted to contract (tx: {}) but database marking failed", batch.id, tx_hash_hex);
                        error!("    This batch may be re-submitted unless manually marked as posted in the database");
                    } else {
                        info!(
                            "✅ Successfully posted and confirmed batch {} to contract with tx: {}",
                            batch.id, tx_hash_hex
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "❌ Failed to submit and confirm batch {} to contract after retries: {}",
                        batch.id, e
                    );
                    continue; // Continue with next batch
                }
            }

            // Small delay between submissions to avoid overwhelming the network
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Ok(())
    }

    /// Submit a batch to smart contract with retry logic
    async fn submit_batch_with_retry(
        pool: &PgPool,
        eth_client: &EthereumClient,
        batch: &arda_credit_db::ProofBatch,
        max_retries: u32,
    ) -> Result<FixedBytes<32>, String> {
        let mut attempts = 0;
        let mut last_error = String::new();

        while attempts < max_retries {
            attempts += 1;
            info!(
                "🔄 Attempting to submit batch {} (attempt {} of {})",
                batch.id, attempts, max_retries
            );

            match Self::submit_batch_to_contract_with_confirmation(pool, eth_client, batch).await {
                Ok(tx_hash) => {
                    if attempts > 1 {
                        info!(
                            "✅ Batch {} submitted successfully after {} attempts",
                            batch.id, attempts
                        );
                    }
                    return Ok(tx_hash);
                }
                Err(e) => {
                    last_error = e.clone();
                    error!(
                        "❌ Attempt {} failed for batch {}: {}",
                        attempts, batch.id, e
                    );

                    if attempts < max_retries {
                        let backoff_seconds = 2_u64.pow(attempts - 1); // Exponential backoff: 1s, 2s, 4s, etc.
                        warn!(
                            "⏳ Retrying batch {} submission in {} seconds...",
                            batch.id, backoff_seconds
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_seconds)).await;
                    }
                }
            }
        }

        error!(
            "❌ All {} attempts failed for batch {}",
            max_retries, batch.id
        );
        Err(format!(
            "Failed after {} retries. Last error: {}",
            max_retries, last_error
        ))
    }

    /// Submit a single batch to the smart contract and wait for confirmation
    async fn submit_batch_to_contract_with_confirmation(
        _pool: &PgPool,
        eth_client: &EthereumClient,
        batch: &arda_credit_db::ProofBatch,
    ) -> Result<FixedBytes<32>, String> {
        info!("🚀 Submitting batch {} to smart contract", batch.id);

        // Get the Sindri proof ID from the batch
        let sindri_proof_id = batch
            .sindri_proof_id
            .as_ref()
            .ok_or_else(|| "Batch has no Sindri proof ID".to_string())?;

        // Fetch actual proof data from Sindri
        info!(
            "📥 Fetching real proof data from Sindri for proof ID: {}",
            sindri_proof_id
        );
        let proof_data = match arda_credit_lib::proof::get_sindri_proof_data(sindri_proof_id).await
        {
            Ok(data) => {
                info!("✅ Successfully retrieved proof data from Sindri");
                info!("   Proof size: {} bytes", data.proof_bytes.len());
                info!("   Public values size: {} bytes", data.public_values.len());
                info!("   Verifying key size: {} bytes", data.verifying_key.len());
                data
            }
            Err(e) => {
                error!("❌ Failed to retrieve proof data from Sindri: {}", e);
                return Err(format!("Failed to fetch proof data from Sindri: {}", e));
            }
        };

        // For now, use a random 32-byte hash for state management
        // until the ADS state root issue is fixed
        let state_id = FixedBytes::from_slice(
            &alloy_primitives::keccak256(format!("batch_{}", batch.id).as_bytes())[..32],
        );
        let new_state_root = FixedBytes::from_slice(
            &alloy_primitives::keccak256(
                format!("state_root_{}", batch.final_counter_value).as_bytes(),
            )[..32],
        );

        // Use real proof data from Sindri
        let proof_bytes = Bytes::from(proof_data.proof_bytes);

        // IMPORTANT: Smart contract expects ABI-encoded public values
        // The contract calls abi.decode() on the public values, so we must keep them ABI-encoded
        let public_values = Bytes::from(proof_data.public_values.clone());

        // Log decoded values for debugging
        if proof_data.public_values.len() >= 224 {
            // Updated length for LoanBatchPublicValues with balance fields (7 * 32 bytes)
            use alloy_sol_types::SolType;
            use arda_credit_lib::LoanBatchPublicValues;

            match LoanBatchPublicValues::abi_decode(&proof_data.public_values) {
                Ok(decoded) => {
                    info!("✅ Using ABI-encoded public values (contract expects this format)");
                    info!(
                        "   State: previous={:?}, new={:?}, transactions={}, timestamp={}",
                        decoded.previous_state_root,
                        decoded.new_state_root,
                        decoded.transaction_count,
                        decoded.batch_timestamp
                    );
                    info!(
                        "   Portfolio: previous_balance=${:.2}, current_balance=${:.2}, delta=${:.2}",
                        decoded.previousTotalBalance as f64 / 100.0,
                        decoded.currentTotalBalance as f64 / 100.0,
                        if decoded.currentTotalBalance >= decoded.previousTotalBalance {
                            (decoded.currentTotalBalance - decoded.previousTotalBalance) as f64 / 100.0
                        } else {
                            -((decoded.previousTotalBalance - decoded.currentTotalBalance) as f64 / 100.0)
                        }
                    );
                }
                Err(e) => {
                    warn!("⚠️ Failed to decode public values for logging: {}", e);
                }
            }
        }

        info!("🔐 Submitting real SP1 proof to smart contract");
        info!("   State ID: {}", state_id);
        info!("   New state root: {}", new_state_root);

        // Submit to smart contract and wait for confirmation
        match eth_client
            .update_state(state_id, new_state_root, proof_bytes, public_values)
            .await
        {
            Ok(result) => {
                // Extract transaction hash from result
                let tx_hash = result
                    .transaction_hash
                    .ok_or_else(|| "Transaction hash not available in result".to_string())?;

                info!(
                    "✅ Batch {} submitted and confirmed on-chain with REAL proof data!",
                    batch.id
                );
                info!("   Transaction hash: {}", hex::encode(tx_hash));
                info!("   Block number: {:?}", result.block_number);
                info!("   State ID: {}", result.state_id);
                info!("   New state root: {}", result.new_state_root);
                info!("   Used Sindri proof ID: {}", sindri_proof_id);

                // Return the transaction hash for tracking
                Ok(tx_hash)
            }
            Err(e) => {
                error!("❌ Failed to submit batch {} to contract: {}", batch.id, e);
                Err(format!("Smart contract submission failed: {}", e))
            }
        }
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Start the background batch processor service
pub async fn start_batch_processor(
    pool: PgPool,
    config: BatchProcessorConfig,
    ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
) -> BatchProcessorHandle {
    let (processor, handle) = BackgroundBatchProcessor::new(pool, config, ads_service);

    // Spawn the processor in the background
    tokio::spawn(async move {
        processor.run().await;
    });

    handle
}

/// Create batch processor configuration from API config
pub fn create_batch_processor_config(api_config: &ApiConfig) -> BatchProcessorConfig {
    BatchProcessorConfig::from(api_config)
}
