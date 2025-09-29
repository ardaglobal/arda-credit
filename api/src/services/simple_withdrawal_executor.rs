//! Simple withdrawal executor service
//!
//! This module provides a simple service to execute approved withdrawals on-chain
//! by calling the Loan contract's withdraw function using the ethereum_client.

use alloy_primitives::{Address, U256};
use anyhow::Result;
use arda_credit_db::get_validated_pending_withdrawals;
use ethereum_client::client::EthereumClient;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Configuration for the simple withdrawal executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWithdrawalExecutorConfig {
    /// Maximum number of withdrawals to process per execution batch
    pub batch_size: i32,
    /// Gas limit for individual withdrawal transactions
    pub gas_limit: u64,
    /// Whether to run in dry-run mode (no actual transactions)
    pub dry_run: bool,
}

impl Default for SimpleWithdrawalExecutorConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            gas_limit: 200000,
            dry_run: false,
        }
    }
}

/// Handle to the withdrawal executor service
#[derive(Clone)]
pub struct SimpleWithdrawalExecutorHandle {
    inner: Arc<RwLock<SimpleWithdrawalExecutorService>>,
}

/// The withdrawal executor service
pub struct SimpleWithdrawalExecutorService {
    pool: PgPool,
    ethereum_client: EthereumClient,
    config: SimpleWithdrawalExecutorConfig,
}

impl SimpleWithdrawalExecutorService {
    /// Create a new withdrawal executor service
    pub fn new(
        pool: PgPool,
        ethereum_client: EthereumClient,
        config: SimpleWithdrawalExecutorConfig,
    ) -> Self {
        Self {
            pool,
            ethereum_client,
            config,
        }
    }

    /// Execute a single withdrawal request
    pub async fn execute_withdrawal(&self, request_id: i32) -> Result<()> {
        info!("Executing withdrawal request {}", request_id);

        // Get the withdrawal request details
        let withdrawals = get_validated_pending_withdrawals(&self.pool, Some(1))
            .await
            .map_err(|e| {
                error!("Failed to get withdrawal request {}: {}", request_id, e);
                e
            })?;

        let withdrawal = withdrawals
            .iter()
            .find(|w| w.id == request_id)
            .ok_or_else(|| {
                error!(
                    "Withdrawal request {} not found or not in validated state",
                    request_id
                );
                anyhow::anyhow!("Withdrawal request not found or not validated")
            })?
            .clone();

        // Convert withdrawal amount to U256 (USDC has 6 decimals)
        let amount_usdc = withdrawal.amount_usdc.ok_or_else(|| {
            error!("Withdrawal request {} missing amount_usdc", request_id);
            anyhow::anyhow!("Missing withdrawal amount")
        })?;

        let amount_wei = decimal_to_u256(amount_usdc, 6)?;

        // Parse recipient address
        let recipient_address = Address::from_str(&withdrawal.recipient_address).map_err(|e| {
            error!(
                "Invalid recipient address {}: {}",
                withdrawal.recipient_address, e
            );
            e
        })?;

        info!(
            "Executing withdrawal {}: {} USDC to {}",
            request_id, amount_usdc, recipient_address
        );

        if self.config.dry_run {
            warn!(
                "[DRY RUN] Would execute withdrawal {} for {} USDC to {}",
                request_id, amount_usdc, recipient_address
            );
            return Ok(());
        }

        // Execute the withdrawal on-chain
        let (tx_hash, _receipt) = self
            .ethereum_client
            .call_withdraw_function(recipient_address, amount_wei, self.config.gas_limit)
            .await
            .map_err(|e| {
                error!(
                    "Failed to execute withdrawal {} on-chain: {}",
                    request_id, e
                );
                e
            })?;

        // Get block number from receipt
        let block_number = _receipt
            .as_ref()
            .and_then(|r| r.block_number)
            .map(|bn| bn as i64);

        let tx_hash_hex = format!("0x{}", hex::encode(tx_hash.as_slice()));

        info!(
            "Successfully executed withdrawal {} on-chain. TX: {}, Block: {:?}",
            request_id, tx_hash_hex, block_number
        );

        // Mark withdrawal as submitted in database
        if let Err(e) = arda_credit_db::mark_withdrawal_submitted(
            &self.pool,
            request_id,
            &tx_hash_hex,
            block_number,
        )
        .await
        {
            error!(
                "Failed to mark withdrawal {} as submitted in database: {}",
                request_id, e
            );
            return Err(anyhow::anyhow!("Failed to update database: {}", e));
        }

        info!("Withdrawal {} marked as submitted in database", request_id);

        Ok(())
    }

    /// Execute multiple withdrawal requests in batch
    pub async fn execute_batch_withdrawals(
        &self,
        request_ids: Vec<i32>,
    ) -> Result<Vec<Result<()>>> {
        let mut results = Vec::new();

        for request_id in request_ids {
            let result = self.execute_withdrawal(request_id).await;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute all pending validated withdrawals
    pub async fn execute_pending_withdrawals(&self) -> Result<usize> {
        info!("Executing all pending validated withdrawals");

        let pending_withdrawals =
            get_validated_pending_withdrawals(&self.pool, Some(self.config.batch_size))
                .await
                .map_err(|e| {
                    error!("Failed to get pending withdrawals: {}", e);
                    e
                })?;

        if pending_withdrawals.is_empty() {
            info!("No pending withdrawals to execute");
            return Ok(0);
        }

        info!(
            "Found {} pending withdrawals to execute",
            pending_withdrawals.len()
        );

        let mut executed_count = 0;

        for withdrawal in &pending_withdrawals {
            match self.execute_withdrawal(withdrawal.id).await {
                Ok(()) => {
                    executed_count += 1;
                    info!("Successfully executed withdrawal {}", withdrawal.id);
                }
                Err(e) => {
                    error!("Failed to execute withdrawal {}: {}", withdrawal.id, e);
                    // Continue with other withdrawals even if one fails
                }
            }
        }

        info!(
            "Executed {} out of {} pending withdrawals",
            executed_count,
            pending_withdrawals.len()
        );
        Ok(executed_count)
    }
}

impl SimpleWithdrawalExecutorHandle {
    /// Create a new handle to the withdrawal executor service
    pub fn new(service: SimpleWithdrawalExecutorService) -> Self {
        Self {
            inner: Arc::new(RwLock::new(service)),
        }
    }

    /// Execute a single withdrawal request
    pub async fn execute_withdrawal(&self, request_id: i32) -> Result<()> {
        let service = self.inner.read().await;
        service.execute_withdrawal(request_id).await
    }

    /// Execute multiple withdrawal requests in batch
    pub async fn execute_batch_withdrawals(
        &self,
        request_ids: Vec<i32>,
    ) -> Result<Vec<Result<()>>> {
        let service = self.inner.read().await;
        service.execute_batch_withdrawals(request_ids).await
    }

    /// Execute all pending validated withdrawals
    pub async fn execute_pending_withdrawals(&self) -> Result<usize> {
        let service = self.inner.read().await;
        service.execute_pending_withdrawals().await
    }
}

/// Create a simple withdrawal executor configuration from environment variables
///
/// This function reads the following environment variables:
/// - `WITHDRAWAL_EXECUTOR_BATCH_SIZE`: Maximum withdrawals to process per batch (default: 10)
/// - `WITHDRAWAL_EXECUTOR_GAS_LIMIT`: Gas limit for withdrawal transactions (default: 200000)
/// - `WITHDRAWAL_EXECUTOR_DRY_RUN`: Set to "true" to simulate withdrawals without executing (default: false)
///
/// Required Ethereum environment variables:
/// - `ETHEREUM_RPC_URL`: Ethereum RPC endpoint
/// - `ETHEREUM_CONTRACT_ADDRESS`: Loan contract address
/// - `ETHEREUM_WALLET_PRIVATE_KEY`: Private key for signing transactions
///
/// The executor will be enabled if all required Ethereum environment variables are present.
pub fn create_simple_withdrawal_executor_config() -> Option<SimpleWithdrawalExecutorConfig> {
    // Check if all required Ethereum environment variables are present
    let required_vars = [
        "ETHEREUM_RPC_URL",
        "ETHEREUM_CONTRACT_ADDRESS",
        "ETHEREUM_WALLET_PRIVATE_KEY",
    ];

    for var in &required_vars {
        if std::env::var(var).is_err() {
            info!(
                "Withdrawal executor disabled: missing required environment variable {}",
                var
            );
            return None;
        }
    }

    let config = SimpleWithdrawalExecutorConfig {
        batch_size: std::env::var("WITHDRAWAL_EXECUTOR_BATCH_SIZE")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .unwrap_or(10),
        gas_limit: std::env::var("WITHDRAWAL_EXECUTOR_GAS_LIMIT")
            .unwrap_or_else(|_| "200000".to_string())
            .parse()
            .unwrap_or(200000),
        dry_run: std::env::var("WITHDRAWAL_EXECUTOR_DRY_RUN")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true",
    };

    info!(
        "Created withdrawal executor config: batch_size={}, gas_limit={}, dry_run={}",
        config.batch_size, config.gas_limit, config.dry_run
    );
    Some(config)
}

/// Convert a Decimal to U256 with specified decimals
fn decimal_to_u256(decimal_value: rust_decimal::Decimal, decimals: u32) -> Result<U256> {
    // Multiply by 10^decimals to convert to integer representation
    let multiplier = 10u128.pow(decimals);
    let scaled_value = decimal_value * rust_decimal::Decimal::from(multiplier);

    // Convert to integer
    let integer_value = scaled_value
        .to_u128()
        .ok_or_else(|| anyhow::anyhow!("Amount too large to convert to U256"))?;

    Ok(U256::from(integer_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_decimal_to_u256() {
        // Test USDC conversion (6 decimals)
        let usdc_amount = Decimal::from_str("100.50").unwrap();
        let result = decimal_to_u256(usdc_amount, 6).unwrap();
        assert_eq!(result, U256::from(100_500_000u64)); // 100.50 * 10^6

        // Test small amount
        let small_amount = Decimal::from_str("0.001").unwrap();
        let result = decimal_to_u256(small_amount, 6).unwrap();
        assert_eq!(result, U256::from(1000u64)); // 0.001 * 10^6

        // Test zero
        let zero_amount = Decimal::ZERO;
        let result = decimal_to_u256(zero_amount, 6).unwrap();
        assert_eq!(result, U256::from(0u64));
    }

    #[test]
    fn test_config_defaults() {
        let config = SimpleWithdrawalExecutorConfig::default();
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.gas_limit, 200000);
        assert!(!config.dry_run);
    }
}
