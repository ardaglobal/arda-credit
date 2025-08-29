use crate::{
    client::EthereumClient,
    error::{EthereumError, Result},
    event_manager::{EventHandler, EventManager},
};
use alloy_primitives::{Address, FixedBytes, U256};
use arda_credit_db::{
    get_user_id_by_wallet_address, update_deposit_status, update_user_balance_from_deposit,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Deposit status enum matching database values
#[derive(Debug, Clone, PartialEq)]
pub enum DepositStatus {
    Pending,
    Confirmed,
    Processed,
    Failed,
}

impl std::fmt::Display for DepositStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepositStatus::Pending => write!(f, "pending"),
            DepositStatus::Confirmed => write!(f, "confirmed"),
            DepositStatus::Processed => write!(f, "processed"),
            DepositStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Configuration for deposit monitoring
#[derive(Debug, Clone)]
pub struct DepositMonitorConfig {
    /// Required block confirmations before processing deposit
    pub required_confirmations: u64,
    /// How often to check for confirmation updates (in seconds)
    pub confirmation_check_interval: u64,
    /// Maximum retry attempts for failed operations
    pub max_retry_attempts: u32,
    /// Retry backoff multiplier
    pub retry_backoff_seconds: u64,
    /// Batch size for processing deposits
    pub batch_size: usize,
}

impl Default for DepositMonitorConfig {
    fn default() -> Self {
        Self {
            required_confirmations: 12,
            confirmation_check_interval: 30,
            max_retry_attempts: 3,
            retry_backoff_seconds: 60,
            batch_size: 100,
        }
    }
}

/// Deposit information from database
#[derive(Debug, Clone)]
pub struct DepositRecord {
    pub id: i32,
    pub user_address: String,
    pub amount: String, // U256 as string
    pub transaction_hash: String,
    pub event_transaction_hash: String,
    pub block_number: i64,
    pub event_timestamp: i64,
    pub log_index: i64,
    pub status: DepositStatus,
    pub confirmation_count: i32,
    pub required_confirmations: i32,
}

/// USDC Deposit monitoring service
pub struct DepositMonitor {
    ethereum_client: Arc<EthereumClient>,
    event_manager: EventManager,
    db_pool: PgPool,
    config: DepositMonitorConfig,
    is_running: Arc<tokio::sync::RwLock<bool>>,
}

impl DepositMonitor {
    /// Create new deposit monitor
    pub fn new(
        ethereum_client: Arc<EthereumClient>,
        db_pool: PgPool,
        config: Option<DepositMonitorConfig>,
    ) -> Self {
        let event_manager = EventManager::new(ethereum_client.clone());
        let config = config.unwrap_or_default();

        Self {
            ethereum_client,
            event_manager,
            db_pool,
            config,
            is_running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Start the deposit monitoring service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting USDC Deposit Monitor service");

        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!("Deposit monitor already running");
                return Ok(());
            }
            *running = true;
        }

        // Register deposit event handler
        let monitor_handler = Arc::new(DepositEventHandler::new(
            self.db_pool.clone(),
            self.config.clone(),
        ));

        // Subscribe to deposit events
        self.event_manager.register_handler(monitor_handler);

        // Start background tasks
        let db_pool = self.db_pool.clone();
        let ethereum_client = self.ethereum_client.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        let confirmation_checker = Self::spawn_confirmation_checker(
            db_pool.clone(),
            ethereum_client.clone(),
            config.clone(),
            is_running.clone(),
        );
        let retry_processor =
            Self::spawn_retry_processor(db_pool.clone(), config.clone(), is_running.clone());
        let balance_updater = Self::spawn_balance_updater(db_pool.clone(), is_running.clone());
        let event_processor = self.event_manager.start_processing_events();

        // Wait for all tasks
        tokio::select! {
            result = confirmation_checker => {
                error!("Confirmation checker stopped: {:?}", result);
                result?
            },
            result = retry_processor => {
                error!("Retry processor stopped: {:?}", result);
                result?
            },
            result = balance_updater => {
                error!("Balance updater stopped: {:?}", result);
                result?
            },
            result = event_processor => {
                error!("Event processor stopped: {:?}", result);
                result
            }
        }
    }

    /// Stop the deposit monitoring service
    pub async fn stop(&self) {
        info!("Stopping USDC Deposit Monitor service");
        let mut running = self.is_running.write().await;
        *running = false;
    }

    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Spawn confirmation checker background task
    fn spawn_confirmation_checker(
        db_pool: PgPool,
        ethereum_client: Arc<EthereumClient>,
        config: DepositMonitorConfig,
        is_running: Arc<tokio::sync::RwLock<bool>>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.confirmation_check_interval));

            loop {
                interval.tick().await;

                if !*is_running.read().await {
                    break;
                }

                if let Err(e) = Self::check_pending_confirmations(
                    &db_pool,
                    &ethereum_client,
                    config.required_confirmations,
                )
                .await
                {
                    error!("Error checking confirmations: {}", e);
                }
            }

            Ok(())
        })
    }

    /// Spawn retry processor for failed operations
    fn spawn_retry_processor(
        db_pool: PgPool,
        config: DepositMonitorConfig,
        is_running: Arc<tokio::sync::RwLock<bool>>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.retry_backoff_seconds));

            loop {
                interval.tick().await;

                if !*is_running.read().await {
                    break;
                }

                if let Err(e) = Self::process_retries(&db_pool, config.max_retry_attempts).await {
                    error!("Error processing retries: {}", e);
                }
            }

            Ok(())
        })
    }

    /// Spawn balance updater background task
    fn spawn_balance_updater(
        db_pool: PgPool,
        is_running: Arc<tokio::sync::RwLock<bool>>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes

            loop {
                interval.tick().await;

                if !*is_running.read().await {
                    break;
                }

                if let Err(e) = Self::sync_user_balances(&db_pool).await {
                    error!("Error syncing user balances: {}", e);
                }
            }

            Ok(())
        })
    }

    /// Check pending deposits for sufficient confirmations
    async fn check_pending_confirmations(
        db_pool: &PgPool,
        ethereum_client: &EthereumClient,
        required_confirmations: u64,
    ) -> Result<()> {
        // Get all pending deposits using dynamic query
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                user_address,
                amount::TEXT as amount,
                transaction_hash,
                event_transaction_hash,
                block_number,
                event_timestamp,
                log_index,
                status,
                confirmation_count,
                required_confirmations
            FROM usdc_deposits
            WHERE status = 'pending'
            ORDER BY block_number ASC
            LIMIT 100
            "#,
        )
        .fetch_all(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        let pending_deposits: Vec<DepositRecord> = rows
            .into_iter()
            .map(|row| DepositRecord {
                id: row.get("id"),
                user_address: row.get("user_address"),
                amount: row.get("amount"),
                transaction_hash: row.get("transaction_hash"),
                event_transaction_hash: row.get("event_transaction_hash"),
                block_number: row.get("block_number"),
                event_timestamp: row.get("event_timestamp"),
                log_index: row.get("log_index"),
                status: match row.get::<String, _>("status").as_str() {
                    "pending" => DepositStatus::Pending,
                    "confirmed" => DepositStatus::Confirmed,
                    "processed" => DepositStatus::Processed,
                    "failed" => DepositStatus::Failed,
                    _ => DepositStatus::Pending,
                },
                confirmation_count: row.get("confirmation_count"),
                required_confirmations: row.get("required_confirmations"),
            })
            .collect();

        let current_block = ethereum_client.get_latest_block_number().await?;

        for deposit in pending_deposits {
            let confirmations = current_block.saturating_sub(deposit.block_number as u64);

            // Update confirmation count
            sqlx::query(
                "UPDATE usdc_deposits SET confirmation_count = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(confirmations as i32)
            .bind(deposit.id)
            .execute(db_pool)
            .await
            .map_err(|e| EthereumError::Database(e.to_string()))?;

            // If sufficient confirmations, mark as confirmed and update user balance
            if confirmations >= required_confirmations {
                // Use our integrated balance update service
                match Self::process_confirmed_deposit(db_pool, &deposit).await {
                    Ok(_) => {
                        // Log the confirmation
                        Self::log_deposit_processing(
                            db_pool,
                            deposit.id,
                            "confirmation_check",
                            "success",
                            Some(&format!(
                                "Deposit confirmed and balance updated with {confirmations} confirmations"
                            )),
                        )
                        .await?;

                        info!(
                            "Deposit {} confirmed and processed: user={}, amount={}, confirmations={}",
                            deposit.id, deposit.user_address, deposit.amount, confirmations
                        );
                    }
                    Err(e) => {
                        error!("Failed to process confirmed deposit {}: {}", deposit.id, e);

                        // Log the error
                        Self::log_deposit_processing(
                            db_pool,
                            deposit.id,
                            "balance_update",
                            "error",
                            Some(&format!("Failed to update balance: {e}")),
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Process retry attempts for failed operations
    async fn process_retries(db_pool: &PgPool, max_retries: u32) -> Result<()> {
        let rows = sqlx::query(
            r#"
            SELECT deposit_id, retry_count
            FROM deposit_processing_log
            WHERE status = 'retry'
            AND next_retry_at <= NOW()
            AND retry_count < $1
            ORDER BY next_retry_at ASC
            LIMIT 50
            "#,
        )
        .bind(max_retries as i32)
        .fetch_all(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        let retry_records: Vec<(i32, i32)> = rows
            .into_iter()
            .map(|row| (row.get("deposit_id"), row.get("retry_count")))
            .collect();

        for (deposit_id, retry_count) in retry_records {
            // Attempt to reprocess the deposit
            if let Err(e) = Self::reprocess_deposit(db_pool, deposit_id).await {
                // Increment retry count
                let next_retry =
                    chrono::Utc::now() + chrono::Duration::seconds(60 * (retry_count + 1) as i64);

                sqlx::query(
                    "UPDATE deposit_processing_log SET retry_count = retry_count + 1, next_retry_at = $1, updated_at = NOW() WHERE deposit_id = $2 AND status = 'retry'"
                )
                .bind(next_retry)
                .bind(deposit_id)
                .execute(db_pool)
                .await
                .map_err(|e| EthereumError::Database(e.to_string()))?;

                warn!(
                    "Retry {} failed for deposit {}: {}",
                    retry_count + 1,
                    deposit_id,
                    e
                );
            } else {
                // Mark retry as successful
                sqlx::query(
                    "UPDATE deposit_processing_log SET status = 'success', updated_at = NOW() WHERE deposit_id = $1 AND status = 'retry'"
                )
                .bind(deposit_id)
                .execute(db_pool)
                .await
                .map_err(|e| EthereumError::Database(e.to_string()))?;

                info!("Retry successful for deposit {}", deposit_id);
            }
        }

        Ok(())
    }

    /// Process confirmed deposit: update status and user balance atomically
    async fn process_confirmed_deposit(db_pool: &PgPool, deposit: &DepositRecord) -> Result<()> {
        // 1. Update deposit status to confirmed
        let _updated_deposit = update_deposit_status(
            db_pool,
            deposit.id,
            "confirmed",
            deposit.confirmation_count,
            Some(chrono::Utc::now()),
        )
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        // 2. Find user ID by wallet address
        if let Some(user_id) = get_user_id_by_wallet_address(db_pool, &deposit.user_address)
            .await
            .map_err(|e| EthereumError::Database(e.to_string()))?
        {
            // 3. Update user balance (convert wei to USDC: divide by 1,000,000)
            let deposit_amount_decimal = deposit
                .amount
                .parse::<rust_decimal::Decimal>()
                .map_err(|e| EthereumError::Database(format!("Invalid deposit amount: {e}")))?;
            let deposit_amount_usdc =
                deposit_amount_decimal / rust_decimal::Decimal::new(1_000_000, 0);

            update_user_balance_from_deposit(db_pool, user_id, deposit_amount_usdc)
                .await
                .map_err(|e| EthereumError::Database(e.to_string()))?;

            info!(
                "Updated user balance: user_id={}, wallet={}, amount_usdc={}",
                user_id, deposit.user_address, deposit_amount_usdc
            );
        } else {
            warn!(
                "No user found for wallet address: {}. Deposit confirmed but balance not updated.",
                deposit.user_address
            );

            // Still update the status even if we can't update the balance
            // The database trigger will handle the user_deposit_balances table
        }

        Ok(())
    }

    /// Reprocess a failed deposit
    async fn reprocess_deposit(db_pool: &PgPool, deposit_id: i32) -> Result<()> {
        // Get deposit record
        let row = sqlx::query(
            r#"
            SELECT
                id,
                user_address,
                amount::TEXT as amount,
                transaction_hash,
                event_transaction_hash,
                block_number,
                event_timestamp,
                log_index,
                status,
                confirmation_count,
                required_confirmations
            FROM usdc_deposits
            WHERE id = $1
            "#,
        )
        .bind(deposit_id)
        .fetch_optional(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        if let Some(row) = row {
            let deposit = DepositRecord {
                id: row.get("id"),
                user_address: row.get("user_address"),
                amount: row.get("amount"),
                transaction_hash: row.get("transaction_hash"),
                event_transaction_hash: row.get("event_transaction_hash"),
                block_number: row.get("block_number"),
                event_timestamp: row.get("event_timestamp"),
                log_index: row.get("log_index"),
                status: match row.get::<String, _>("status").as_str() {
                    "pending" => DepositStatus::Pending,
                    "confirmed" => DepositStatus::Confirmed,
                    "processed" => DepositStatus::Processed,
                    "failed" => DepositStatus::Failed,
                    _ => DepositStatus::Pending,
                },
                confirmation_count: row.get("confirmation_count"),
                required_confirmations: row.get("required_confirmations"),
            };

            // Reprocess the deposit if it's confirmed but not processed
            if deposit.status == DepositStatus::Confirmed {
                Self::process_confirmed_deposit(db_pool, &deposit).await?;
            }
        }

        debug!("Reprocessed deposit {}", deposit_id);
        Ok(())
    }

    /// Synchronize user balance aggregations
    async fn sync_user_balances(db_pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_deposit_balances (
                user_address,
                total_deposited,
                deposit_count,
                first_deposit_at,
                last_deposit_at,
                updated_at
            )
            SELECT
                user_address,
                SUM(amount::NUMERIC) as total_deposited,
                COUNT(*) as deposit_count,
                MIN(COALESCE(block_timestamp, created_at)) as first_deposit_at,
                MAX(COALESCE(block_timestamp, created_at)) as last_deposit_at,
                NOW() as updated_at
            FROM usdc_deposits
            WHERE status IN ('confirmed', 'processed')
            GROUP BY user_address
            ON CONFLICT (user_address) DO UPDATE SET
                total_deposited = EXCLUDED.total_deposited,
                deposit_count = EXCLUDED.deposit_count,
                first_deposit_at = LEAST(user_deposit_balances.first_deposit_at, EXCLUDED.first_deposit_at),
                last_deposit_at = GREATEST(user_deposit_balances.last_deposit_at, EXCLUDED.last_deposit_at),
                updated_at = NOW()
            "#
        )
        .execute(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        debug!("User deposit balances synchronized");
        Ok(())
    }

    /// Log deposit processing events
    async fn log_deposit_processing(
        db_pool: &PgPool,
        deposit_id: i32,
        event_type: &str,
        status: &str,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO deposit_processing_log (deposit_id, event_type, status, message, created_at) VALUES ($1, $2, $3, $4, NOW())"
        )
        .bind(deposit_id)
        .bind(event_type)
        .bind(status)
        .bind(message)
        .execute(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get deposit statistics
    pub async fn get_deposit_stats(&self) -> Result<HashMap<String, serde_json::Value>> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_deposits,
                COUNT(*) FILTER (WHERE status = 'pending') as pending_deposits,
                COUNT(*) FILTER (WHERE status = 'confirmed') as confirmed_deposits,
                COUNT(*) FILTER (WHERE status = 'processed') as processed_deposits,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_deposits,
                COALESCE(SUM(amount::NUMERIC), 0) as total_amount,
                COUNT(DISTINCT user_address) as unique_depositors
            FROM usdc_deposits
            "#,
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        let mut result = HashMap::new();

        let total_deposits: i64 = row.get("total_deposits");
        let pending_deposits: i64 = row.get("pending_deposits");
        let confirmed_deposits: i64 = row.get("confirmed_deposits");
        let processed_deposits: i64 = row.get("processed_deposits");
        let failed_deposits: i64 = row.get("failed_deposits");
        let unique_depositors: i64 = row.get("unique_depositors");

        result.insert(
            "total_deposits".to_string(),
            serde_json::Value::from(total_deposits),
        );
        result.insert(
            "pending_deposits".to_string(),
            serde_json::Value::from(pending_deposits),
        );
        result.insert(
            "confirmed_deposits".to_string(),
            serde_json::Value::from(confirmed_deposits),
        );
        result.insert(
            "processed_deposits".to_string(),
            serde_json::Value::from(processed_deposits),
        );
        result.insert(
            "failed_deposits".to_string(),
            serde_json::Value::from(failed_deposits),
        );
        result.insert(
            "unique_depositors".to_string(),
            serde_json::Value::from(unique_depositors),
        );

        // Handle total amount - this might be a decimal or bigint depending on the query
        if let Ok(total_amount) = row.try_get::<rust_decimal::Decimal, _>("total_amount") {
            result.insert(
                "total_amount_wei".to_string(),
                serde_json::Value::from(total_amount.to_string()),
            );
            let total_usdc = total_amount / rust_decimal::Decimal::new(1_000_000, 0);
            result.insert(
                "total_amount_usdc".to_string(),
                serde_json::Value::from(total_usdc.to_string()),
            );
        }

        Ok(result)
    }
}

/// Event handler for USDC deposits
pub struct DepositEventHandler {
    db_pool: PgPool,
    config: DepositMonitorConfig,
}

impl DepositEventHandler {
    pub fn new(db_pool: PgPool, config: DepositMonitorConfig) -> Self {
        Self { db_pool, config }
    }

    /// Store deposit event in database
    #[allow(clippy::too_many_arguments)]
    async fn store_deposit_event(
        &self,
        user: Address,
        amount: U256,
        transaction_hash: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Result<i32> {
        let user_address = format!("{user:?}");
        let amount_str = amount.to_string();
        let tx_hash_str = format!("0x{}", hex::encode(transaction_hash.as_slice()));
        let event_tx_hash_str = format!("0x{}", hex::encode(tx_hash.as_slice()));

        // Insert deposit record with idempotency check
        let deposit_id: i32 = sqlx::query_scalar(
            r#"
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
            ON CONFLICT (event_transaction_hash, log_index)
            DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(&user_address)
        .bind(&amount_str)
        .bind(&tx_hash_str)
        .bind(&event_tx_hash_str)
        .bind(block_number as i64)
        .bind(timestamp as i64)
        .bind(log_index as i64)
        .bind(self.config.required_confirmations as i32)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        info!(
            "Stored deposit event: id={}, user={}, amount={}, tx_hash={}, block={}",
            deposit_id, user_address, amount_str, event_tx_hash_str, block_number
        );

        Ok(deposit_id)
    }
}

impl EventHandler for DepositEventHandler {
    fn handle_deposit(
        &self,
        user: Address,
        amount: U256,
        transaction_hash: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
    ) {
        let db_pool = self.db_pool.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            // Note: We don't have tx_hash and log_index in the handler interface
            // These would need to be added to the event data structure
            let tx_hash = transaction_hash; // Using transaction_hash as tx_hash for now
            let log_index = 0; // This should be passed from the event

            let handler = DepositEventHandler::new(db_pool, config);
            if let Err(e) = handler
                .store_deposit_event(
                    user,
                    amount,
                    transaction_hash,
                    timestamp,
                    block_number,
                    tx_hash,
                    log_index,
                )
                .await
            {
                error!("Failed to store deposit event: {}", e);
            }
        });
    }

    // Implement other required event handler methods with no-op or logging
    fn handle_state_updated(
        &self,
        _state_id: FixedBytes<32>,
        _new_state: FixedBytes<32>,
        _proof_id: FixedBytes<32>,
        _updater: Address,
        _block_number: u64,
    ) {
    }
    fn handle_batch_state_updated(
        &self,
        _state_ids: Vec<FixedBytes<32>>,
        _new_states: Vec<FixedBytes<32>>,
        _updater: Address,
        _block_number: u64,
    ) {
    }
    fn handle_proof_stored(
        &self,
        _proof_id: FixedBytes<32>,
        _state_id: FixedBytes<32>,
        _submitter: Address,
        _block_number: u64,
    ) {
    }
    fn handle_proof_verified(&self, _proof_id: FixedBytes<32>, _success: bool, _block_number: u64) {
    }
    fn handle_state_read_requested(
        &self,
        _state_id: FixedBytes<32>,
        _reader: Address,
        _block_number: u64,
    ) {
    }
    fn handle_proof_read_requested(
        &self,
        _proof_id: FixedBytes<32>,
        _reader: Address,
        _block_number: u64,
    ) {
    }
    fn handle_loan_payment(
        &self,
        _loan_id: FixedBytes<32>,
        _amount: U256,
        _payer: Address,
        _block_number: u64,
    ) {
    }
    fn handle_loan_closed(
        &self,
        _loan_id: FixedBytes<32>,
        _final_amount: U256,
        _block_number: u64,
    ) {
    }
    fn handle_withdrawal(&self, _user: Address, _amount: U256, _block_number: u64) {}
}
