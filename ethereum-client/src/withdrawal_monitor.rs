use crate::{
    client::EthereumClient,
    error::{EthereumError, Result},
    event_manager::{EventHandler, EventManager},
};
use alloy_primitives::{Address, FixedBytes, U256};
use arda_credit_db::{
    get_user_id_by_wallet_address, update_user_balance_from_withdrawal_tx,
    update_withdrawal_status_tx,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Withdrawal status enum matching database values
#[derive(Debug, Clone, PartialEq)]
pub enum WithdrawalStatus {
    Pending,
    Confirmed,
    Processed,
    Failed,
}

impl std::fmt::Display for WithdrawalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WithdrawalStatus::Pending => write!(f, "pending"),
            WithdrawalStatus::Confirmed => write!(f, "confirmed"),
            WithdrawalStatus::Processed => write!(f, "processed"),
            WithdrawalStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Configuration for withdrawal monitoring
#[derive(Debug, Clone)]
pub struct WithdrawalMonitorConfig {
    /// Required block confirmations before processing withdrawal
    pub required_confirmations: u64,
    /// How often to check for confirmation updates (in seconds)
    pub confirmation_check_interval: u64,
    /// Maximum retry attempts for failed operations
    pub max_retry_attempts: u32,
    /// Retry backoff multiplier
    pub retry_backoff_seconds: u64,
    /// Batch size for processing withdrawals
    pub batch_size: usize,
}

impl Default for WithdrawalMonitorConfig {
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

/// Withdrawal information from database
#[derive(Debug, Clone)]
pub struct WithdrawalRecord {
    pub id: i32,
    pub user_address: String,
    pub recipient_address: String,
    pub amount: String, // U256 as string
    pub transaction_hash: String,
    pub event_transaction_hash: String,
    pub block_number: i64,
    pub event_timestamp: i64,
    pub log_index: i64,
    pub status: WithdrawalStatus,
    pub confirmation_count: i32,
    pub required_confirmations: i32,
}

/// USDC Withdrawal monitoring service
pub struct WithdrawalMonitor {
    ethereum_client: Arc<EthereumClient>,
    event_manager: EventManager,
    db_pool: PgPool,
    config: WithdrawalMonitorConfig,
    is_running: Arc<tokio::sync::RwLock<bool>>,
}

impl WithdrawalMonitor {
    /// Create new withdrawal monitor
    pub fn new(
        ethereum_client: Arc<EthereumClient>,
        db_pool: PgPool,
        config: Option<WithdrawalMonitorConfig>,
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

    /// Start the withdrawal monitoring service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting USDC Withdrawal Monitor service");

        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!("Withdrawal monitor already running");
                return Ok(());
            }
            *running = true;
        }

        // Register withdrawal event handler
        let monitor_handler = Arc::new(WithdrawalEventHandler::new(
            self.db_pool.clone(),
            self.config.clone(),
        ));

        // Subscribe to withdrawal events
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

    /// Stop the withdrawal monitoring service
    pub async fn stop(&self) {
        info!("Stopping USDC Withdrawal Monitor service");
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
        config: WithdrawalMonitorConfig,
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
                    error!("Error checking withdrawal confirmations: {}", e);
                }
            }

            Ok(())
        })
    }

    /// Spawn retry processor for failed operations
    fn spawn_retry_processor(
        db_pool: PgPool,
        config: WithdrawalMonitorConfig,
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
                    error!("Error processing withdrawal retries: {}", e);
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
                    error!("Error syncing user withdrawal balances: {}", e);
                }
            }

            Ok(())
        })
    }

    /// Check pending withdrawals for sufficient confirmations
    async fn check_pending_confirmations(
        db_pool: &PgPool,
        ethereum_client: &EthereumClient,
        required_confirmations: u64,
    ) -> Result<()> {
        // Get all pending withdrawals
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                user_address,
                recipient_address,
                amount::TEXT as amount,
                transaction_hash,
                event_transaction_hash,
                block_number,
                event_timestamp,
                log_index,
                status,
                confirmation_count,
                required_confirmations
            FROM usdc_withdrawals
            WHERE status = 'pending'
            ORDER BY block_number ASC
            LIMIT 100
            "#,
        )
        .fetch_all(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        let pending_withdrawals: Vec<WithdrawalRecord> = rows
            .into_iter()
            .map(|row| WithdrawalRecord {
                id: row.get("id"),
                user_address: row.get("user_address"),
                recipient_address: row.get("recipient_address"),
                amount: row.get("amount"),
                transaction_hash: row.get("transaction_hash"),
                event_transaction_hash: row.get("event_transaction_hash"),
                block_number: row.get("block_number"),
                event_timestamp: row.get("event_timestamp"),
                log_index: row.get("log_index"),
                status: match row.get::<String, _>("status").as_str() {
                    "pending" => WithdrawalStatus::Pending,
                    "confirmed" => WithdrawalStatus::Confirmed,
                    "processed" => WithdrawalStatus::Processed,
                    "failed" => WithdrawalStatus::Failed,
                    _ => WithdrawalStatus::Pending,
                },
                confirmation_count: row.get("confirmation_count"),
                required_confirmations: row.get("required_confirmations"),
            })
            .collect();

        let current_block = ethereum_client.get_latest_block_number().await?;

        for withdrawal in pending_withdrawals {
            let confirmations = current_block.saturating_sub(withdrawal.block_number as u64);

            // Update confirmation count
            sqlx::query(
                "UPDATE usdc_withdrawals SET confirmation_count = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(confirmations as i32)
            .bind(withdrawal.id)
            .execute(db_pool)
            .await
            .map_err(|e| EthereumError::Database(e.to_string()))?;

            // If sufficient confirmations, mark as confirmed and update user balance
            if confirmations >= required_confirmations {
                // Use our integrated balance update service
                match Self::process_confirmed_withdrawal(db_pool, &withdrawal).await {
                    Ok(_) => {
                        // Log the confirmation
                        Self::log_withdrawal_processing(
                            db_pool,
                            withdrawal.id,
                            "confirmation_check",
                            "success",
                            Some(&format!(
                                "Withdrawal confirmed and balance updated with {confirmations} confirmations"
                            )),
                        )
                        .await?;

                        info!(
                            "Withdrawal {} confirmed and processed: user={}, recipient={}, amount={}, confirmations={}",
                            withdrawal.id, withdrawal.user_address, withdrawal.recipient_address, withdrawal.amount, confirmations
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to process confirmed withdrawal {}: {}",
                            withdrawal.id, e
                        );

                        // Log the error
                        Self::log_withdrawal_processing(
                            db_pool,
                            withdrawal.id,
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
            SELECT withdrawal_id, retry_count
            FROM withdrawal_processing_log
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
            .map(|row| (row.get("withdrawal_id"), row.get("retry_count")))
            .collect();

        for (withdrawal_id, retry_count) in retry_records {
            // Attempt to reprocess the withdrawal
            if let Err(e) = Self::reprocess_withdrawal(db_pool, withdrawal_id).await {
                // Increment retry count
                let next_retry =
                    chrono::Utc::now() + chrono::Duration::seconds(60 * (retry_count + 1) as i64);

                sqlx::query(
                    "UPDATE withdrawal_processing_log SET retry_count = retry_count + 1, next_retry_at = $1, updated_at = NOW() WHERE withdrawal_id = $2 AND status = 'retry'"
                )
                .bind(next_retry)
                .bind(withdrawal_id)
                .execute(db_pool)
                .await
                .map_err(|e| EthereumError::Database(e.to_string()))?;

                warn!(
                    "Retry {} failed for withdrawal {}: {}",
                    retry_count + 1,
                    withdrawal_id,
                    e
                );
            } else {
                // Mark retry as successful
                sqlx::query(
                    "UPDATE withdrawal_processing_log SET status = 'success', updated_at = NOW() WHERE withdrawal_id = $1 AND status = 'retry'"
                )
                .bind(withdrawal_id)
                .execute(db_pool)
                .await
                .map_err(|e| EthereumError::Database(e.to_string()))?;

                info!("Retry successful for withdrawal {}", withdrawal_id);
            }
        }

        Ok(())
    }

    /// Process confirmed withdrawal: update status and user balance atomically
    async fn process_confirmed_withdrawal(
        db_pool: &PgPool,
        withdrawal: &WithdrawalRecord,
    ) -> Result<()> {
        // Begin a database transaction for atomic operations
        let mut tx = db_pool
            .begin()
            .await
            .map_err(|e| EthereumError::Database(format!("Failed to begin transaction: {e}")))?;

        // 1. Update withdrawal status to confirmed within transaction
        let _updated_withdrawal = update_withdrawal_status_tx(
            &mut tx,
            withdrawal.id,
            "confirmed",
            withdrawal.confirmation_count,
            Some(chrono::Utc::now()),
        )
        .await
        .map_err(|e| EthereumError::Database(format!("Failed to update withdrawal status: {e}")))?;

        // 2. Find user ID by wallet address
        if let Some(user_id) = get_user_id_by_wallet_address(db_pool, &withdrawal.user_address)
            .await
            .map_err(|e| EthereumError::Database(e.to_string()))?
        {
            // 3. Update user balance atomically (convert wei to USDC: divide by 1,000,000)
            let withdrawal_amount_decimal = withdrawal
                .amount
                .parse::<rust_decimal::Decimal>()
                .map_err(|e| EthereumError::Database(format!("Invalid withdrawal amount: {e}")))?;
            let withdrawal_amount_usdc =
                withdrawal_amount_decimal / rust_decimal::Decimal::new(1_000_000, 0);

            // Debit user balance for withdrawal
            update_user_balance_from_withdrawal_tx(&mut tx, user_id, withdrawal_amount_usdc)
                .await
                .map_err(|e| EthereumError::Database(format!("Failed to update balance: {e}")))?;

            info!(
                "Updated user balance: user_id={}, wallet={}, recipient={}, amount_usdc={}",
                user_id,
                withdrawal.user_address,
                withdrawal.recipient_address,
                withdrawal_amount_usdc
            );
        } else {
            warn!(
                "No user found for wallet address: {}. Withdrawal confirmed but balance not updated.",
                withdrawal.user_address
            );

            // Still commit the transaction even if we can't update the balance
            // The database trigger will handle the user_withdrawal_balances table
        }

        // Commit the transaction - ensures both operations succeed or both fail
        tx.commit()
            .await
            .map_err(|e| EthereumError::Database(format!("Failed to commit transaction: {e}")))?;

        Ok(())
    }

    /// Reprocess a failed withdrawal
    async fn reprocess_withdrawal(db_pool: &PgPool, withdrawal_id: i32) -> Result<()> {
        // Get withdrawal record
        let row = sqlx::query(
            r#"
            SELECT
                id,
                user_address,
                recipient_address,
                amount::TEXT as amount,
                transaction_hash,
                event_transaction_hash,
                block_number,
                event_timestamp,
                log_index,
                status,
                confirmation_count,
                required_confirmations
            FROM usdc_withdrawals
            WHERE id = $1
            "#,
        )
        .bind(withdrawal_id)
        .fetch_optional(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        if let Some(row) = row {
            let withdrawal = WithdrawalRecord {
                id: row.get("id"),
                user_address: row.get("user_address"),
                recipient_address: row.get("recipient_address"),
                amount: row.get("amount"),
                transaction_hash: row.get("transaction_hash"),
                event_transaction_hash: row.get("event_transaction_hash"),
                block_number: row.get("block_number"),
                event_timestamp: row.get("event_timestamp"),
                log_index: row.get("log_index"),
                status: match row.get::<String, _>("status").as_str() {
                    "pending" => WithdrawalStatus::Pending,
                    "confirmed" => WithdrawalStatus::Confirmed,
                    "processed" => WithdrawalStatus::Processed,
                    "failed" => WithdrawalStatus::Failed,
                    _ => WithdrawalStatus::Pending,
                },
                confirmation_count: row.get("confirmation_count"),
                required_confirmations: row.get("required_confirmations"),
            };

            // Reprocess the withdrawal if it's confirmed but not processed
            if withdrawal.status == WithdrawalStatus::Confirmed {
                Self::process_confirmed_withdrawal(db_pool, &withdrawal).await?;
            }
        }

        debug!("Reprocessed withdrawal {}", withdrawal_id);
        Ok(())
    }

    /// Synchronize user balance aggregations
    async fn sync_user_balances(db_pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_withdrawal_balances (
                user_address,
                total_withdrawn,
                withdrawal_count,
                first_withdrawal_at,
                last_withdrawal_at,
                updated_at
            )
            SELECT
                user_address,
                SUM(amount::NUMERIC) as total_withdrawn,
                COUNT(*) as withdrawal_count,
                MIN(COALESCE(block_timestamp, created_at)) as first_withdrawal_at,
                MAX(COALESCE(block_timestamp, created_at)) as last_withdrawal_at,
                NOW() as updated_at
            FROM usdc_withdrawals
            WHERE status IN ('confirmed', 'processed')
            GROUP BY user_address
            ON CONFLICT (user_address) DO UPDATE SET
                total_withdrawn = EXCLUDED.total_withdrawn,
                withdrawal_count = EXCLUDED.withdrawal_count,
                first_withdrawal_at = LEAST(user_withdrawal_balances.first_withdrawal_at, EXCLUDED.first_withdrawal_at),
                last_withdrawal_at = GREATEST(user_withdrawal_balances.last_withdrawal_at, EXCLUDED.last_withdrawal_at),
                updated_at = NOW()
            "#
        )
        .execute(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        debug!("User withdrawal balances synchronized");
        Ok(())
    }

    /// Log withdrawal processing events
    async fn log_withdrawal_processing(
        db_pool: &PgPool,
        withdrawal_id: i32,
        event_type: &str,
        status: &str,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO withdrawal_processing_log (withdrawal_id, event_type, status, message, created_at) VALUES ($1, $2, $3, $4, NOW())"
        )
        .bind(withdrawal_id)
        .bind(event_type)
        .bind(status)
        .bind(message)
        .execute(db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get withdrawal statistics
    pub async fn get_withdrawal_stats(&self) -> Result<HashMap<String, serde_json::Value>> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_withdrawals,
                COUNT(*) FILTER (WHERE status = 'pending') as pending_withdrawals,
                COUNT(*) FILTER (WHERE status = 'confirmed') as confirmed_withdrawals,
                COUNT(*) FILTER (WHERE status = 'processed') as processed_withdrawals,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_withdrawals,
                COALESCE(SUM(amount::NUMERIC), 0) as total_amount,
                COUNT(DISTINCT user_address) as unique_withdrawers
            FROM usdc_withdrawals
            "#,
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| EthereumError::Database(e.to_string()))?;

        let mut result = HashMap::new();

        let total_withdrawals: i64 = row.get("total_withdrawals");
        let pending_withdrawals: i64 = row.get("pending_withdrawals");
        let confirmed_withdrawals: i64 = row.get("confirmed_withdrawals");
        let processed_withdrawals: i64 = row.get("processed_withdrawals");
        let failed_withdrawals: i64 = row.get("failed_withdrawals");
        let unique_withdrawers: i64 = row.get("unique_withdrawers");

        result.insert(
            "total_withdrawals".to_string(),
            serde_json::Value::from(total_withdrawals),
        );
        result.insert(
            "pending_withdrawals".to_string(),
            serde_json::Value::from(pending_withdrawals),
        );
        result.insert(
            "confirmed_withdrawals".to_string(),
            serde_json::Value::from(confirmed_withdrawals),
        );
        result.insert(
            "processed_withdrawals".to_string(),
            serde_json::Value::from(processed_withdrawals),
        );
        result.insert(
            "failed_withdrawals".to_string(),
            serde_json::Value::from(failed_withdrawals),
        );
        result.insert(
            "unique_withdrawers".to_string(),
            serde_json::Value::from(unique_withdrawers),
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

/// Event handler for USDC withdrawals
pub struct WithdrawalEventHandler {
    db_pool: PgPool,
    config: WithdrawalMonitorConfig,
}

impl WithdrawalEventHandler {
    pub fn new(db_pool: PgPool, config: WithdrawalMonitorConfig) -> Self {
        Self { db_pool, config }
    }

    /// Store withdrawal event in database
    #[allow(clippy::too_many_arguments)]
    async fn store_withdrawal_event(
        &self,
        user: Address,
        to: Address, // recipient address
        amount: U256,
        transaction_hash: FixedBytes<32>,
        timestamp: u64,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Result<i32> {
        let user_address = format!("{user:?}");
        let recipient_address = format!("{to:?}");
        let amount_str = amount.to_string();
        let tx_hash_str = format!("0x{}", hex::encode(transaction_hash.as_slice()));
        let event_tx_hash_str = format!("0x{}", hex::encode(tx_hash.as_slice()));

        // Insert withdrawal record with idempotency check
        let withdrawal_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO usdc_withdrawals (
                user_address,
                recipient_address,
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
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', 0, $9, NOW())
            ON CONFLICT (event_transaction_hash, log_index)
            DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(&user_address)
        .bind(&recipient_address)
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
            "Stored withdrawal event: id={}, user={}, recipient={}, amount={}, tx_hash={}, block={}",
            withdrawal_id, user_address, recipient_address, amount_str, event_tx_hash_str, block_number
        );

        Ok(withdrawal_id)
    }
}

impl EventHandler for WithdrawalEventHandler {
    fn handle_withdrawal(
        &self,
        user: Address,
        to: Address,
        amount: U256,
        timestamp: u64,
        block_number: u64,
    ) {
        let db_pool = self.db_pool.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            // Note: transaction_hash and log_index should come from the actual event
            // For now, we'll use placeholder values
            let transaction_hash = FixedBytes::<32>::ZERO; // This should come from the actual event
            let tx_hash = transaction_hash; // Using transaction_hash as tx_hash for now
            let log_index = 0; // This should be passed from the event

            let handler = WithdrawalEventHandler::new(db_pool, config);
            if let Err(e) = handler
                .store_withdrawal_event(
                    user,
                    to,
                    amount,
                    transaction_hash,
                    timestamp,
                    block_number,
                    tx_hash,
                    log_index,
                )
                .await
            {
                error!("Failed to store withdrawal event: {}", e);
            }
        });
    }

    // Implement other required event handler methods with no-op or logging
    fn handle_deposit(
        &self,
        _user: Address,
        _amount: U256,
        _transaction_hash: FixedBytes<32>,
        _timestamp: u64,
        _block_number: u64,
    ) {
    }
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
}
