use alloy_primitives::{Address, U256};
use anyhow::{Context, Result};
use arda_credit_db::{
    get_validated_pending_withdrawals, init_db_with_url, mark_withdrawal_submitted,
    update_withdrawal_request_state,
};
use clap::Parser;
use ethereum_client::{client::EthereumClient, config::Config};
use rust_decimal::prelude::*;
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;
use tracing::{debug, error, info};
use url::Url;

/// USDC Withdrawal Executor service
///
/// This service monitors approved withdrawal requests and executes them on the blockchain.
/// It processes withdrawals that have been approved by admins but not yet submitted to the blockchain.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Database connection URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// Ethereum RPC URL
    #[arg(long, env = "ETHEREUM_RPC_URL")]
    ethereum_rpc_url: String,

    /// Smart contract address
    #[arg(long, env = "ETHEREUM_CONTRACT_ADDRESS")]
    contract_address: String,

    /// Private key for transaction signing (hex format, with or without 0x prefix)
    #[arg(long, env = "ETHEREUM_PRIVATE_KEY")]
    private_key: String,

    /// Polling interval in seconds (default: 30)
    #[arg(long, default_value = "30")]
    poll_interval: u64,

    /// Gas limit for withdrawal transactions (default: 200000)
    #[arg(long, default_value = "200000")]
    gas_limit: u64,

    /// Maximum withdrawals to process per batch (default: 10)
    #[arg(long, default_value = "10")]
    batch_size: i32,

    /// Dry run mode - don't actually execute transactions
    #[arg(long)]
    dry_run: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!(
            "withdrawal_executor={},ethereum_client={}",
            log_level, log_level
        ))
        .init();

    info!("Starting USDC Withdrawal Executor service");
    info!("Database URL: {}", mask_database_url(&args.database_url));
    info!("Contract Address: {}", args.contract_address);
    info!("Poll interval: {}s", args.poll_interval);
    info!("Gas limit: {}", args.gas_limit);
    info!("Batch size: {}", args.batch_size);
    info!("Dry run mode: {}", args.dry_run);

    // Initialize database connection
    let db_pool = init_db_with_url(&args.database_url)
        .await
        .context("Failed to initialize database connection")?;

    // Initialize Ethereum client
    let config = create_ethereum_config(
        &args.ethereum_rpc_url,
        &args.contract_address,
        &args.private_key,
    )
    .context("Failed to create Ethereum config")?;

    let ethereum_client = EthereumClient::new(config)
        .await
        .context("Failed to initialize Ethereum client")?;

    info!("Withdrawal executor initialized successfully");

    // Main processing loop
    loop {
        if let Err(e) =
            process_pending_withdrawals(&db_pool, &ethereum_client, args.batch_size, args.dry_run)
                .await
        {
            error!("Error processing withdrawals: {}", e);
        }

        debug!(
            "Sleeping for {} seconds before next poll",
            args.poll_interval
        );
        sleep(Duration::from_secs(args.poll_interval)).await;
    }
}

/// Process all pending withdrawal requests
async fn process_pending_withdrawals(
    db_pool: &sqlx::PgPool,
    ethereum_client: &EthereumClient,
    batch_size: i32,
    dry_run: bool,
) -> Result<()> {
    // Get approved withdrawals that haven't been submitted yet
    let pending_withdrawals = get_validated_pending_withdrawals(db_pool, Some(batch_size))
        .await
        .context("Failed to get pending withdrawals")?;

    if pending_withdrawals.is_empty() {
        debug!("No pending withdrawals to process");
        return Ok(());
    }

    info!(
        "Processing {} pending withdrawals",
        pending_withdrawals.len()
    );

    for withdrawal in pending_withdrawals {
        if let Err(e) =
            process_single_withdrawal(db_pool, ethereum_client, &withdrawal, dry_run).await
        {
            error!("Failed to process withdrawal {}: {}", withdrawal.id, e);

            // Mark withdrawal as failed
            if let Err(db_err) = update_withdrawal_request_state(
                db_pool,
                withdrawal.id,
                "failed",
                Some(&format!("Execution failed: {}", e)),
            )
            .await
            {
                error!(
                    "Failed to mark withdrawal {} as failed: {}",
                    withdrawal.id, db_err
                );
            }
        }
    }

    Ok(())
}

/// Process a single withdrawal request
async fn process_single_withdrawal(
    db_pool: &sqlx::PgPool,
    ethereum_client: &EthereumClient,
    withdrawal: &arda_credit_db::WithdrawalRequest,
    dry_run: bool,
) -> Result<()> {
    let recipient_address = Address::from_str(&withdrawal.recipient_address)
        .context("Invalid recipient address format")?;

    // Convert USDC amount to wei (USDC has 6 decimals)
    let amount_usdc = withdrawal
        .amount_usdc
        .context("Missing amount_usdc field")?;

    // Convert Decimal to U256 (multiply by 10^6 for USDC decimals)
    let amount_wei = decimal_to_u256(amount_usdc, 6)?;

    info!(
        "Processing withdrawal {}: {} USDC to {} (wei: {})",
        withdrawal.id, amount_usdc, recipient_address, amount_wei
    );

    if dry_run {
        info!(
            "[DRY RUN] Would execute withdrawal {} for {} USDC to {}",
            withdrawal.id, amount_usdc, recipient_address
        );
        return Ok(());
    }

    // Execute the withdrawal on the blockchain
    let (tx_hash, receipt) = ethereum_client
        .call_withdraw_function(recipient_address, amount_wei, 200000)
        .await
        .context("Failed to execute withdrawal transaction")?;

    let block_number = receipt
        .as_ref()
        .map(|r| r.block_number.unwrap_or_default() as i64);

    info!(
        "Withdrawal {} executed successfully! TX Hash: {}, Block: {:?}",
        withdrawal.id, tx_hash, block_number
    );

    // Mark withdrawal as submitted in database
    mark_withdrawal_submitted(
        db_pool,
        withdrawal.id,
        &format!("0x{}", hex::encode(tx_hash)),
        block_number,
    )
    .await
    .context("Failed to mark withdrawal as submitted in database")?;

    info!(
        "Withdrawal {} marked as submitted in database",
        withdrawal.id
    );

    Ok(())
}

/// Convert a Decimal to U256 with specified decimals
fn decimal_to_u256(decimal_value: rust_decimal::Decimal, decimals: u32) -> Result<U256> {
    // Multiply by 10^decimals to convert to integer representation
    let multiplier = 10u128.pow(decimals);
    let scaled_value = decimal_value * rust_decimal::Decimal::from(multiplier);

    // Convert to integer
    let integer_value = scaled_value
        .to_u128()
        .context("Amount too large to convert to U256")?;

    Ok(U256::from(integer_value))
}

/// Create Ethereum configuration for the withdrawal executor
fn create_ethereum_config(
    rpc_url: &str,
    contract_address: &str,
    private_key: &str,
) -> Result<Config> {
    use alloy_primitives::FixedBytes;
    use alloy_signer_local::PrivateKeySigner;
    use ethereum_client::config::{
        ContractConfig, MonitoringConfig, NetworkConfig, RpcConfig, SignerConfig,
    };

    // Parse the private key to get the signer address
    let private_key_bytes = hex::decode(private_key.strip_prefix("0x").unwrap_or(private_key))
        .context("Invalid private key format")?;

    let fixed_bytes = FixedBytes::<32>::try_from(private_key_bytes.as_slice())
        .context("Private key must be 32 bytes")?;

    let signer = PrivateKeySigner::from_bytes(&fixed_bytes)
        .context("Failed to create signer from private key")?;

    let signer_address = signer.address();

    let config = Config {
        network: NetworkConfig {
            name: "sepolia".to_string(), // Default to sepolia testnet
            chain_id: 11155111,
            rpc_url: Url::parse(rpc_url).context("Invalid RPC URL format")?,
            ws_url: None,
            explorer_url: None,
            is_testnet: true,
        },
        contract: ContractConfig {
            arda_credit_contract: Address::from_str(contract_address)
                .context("Invalid contract address format")?,
            verifier_contract: Address::ZERO, // Not needed for withdrawals
            deployment_block: None,
        },
        rpc: RpcConfig {
            notify_addresses: vec![],
            rate_limit_per_second: 10,
        },
        signer: Some(SignerConfig {
            private_key: private_key.to_string(),
            address: signer_address,
        }),
        monitoring: MonitoringConfig {
            enable_event_monitoring: false, // We don't need event monitoring for withdrawals
            polling_interval_seconds: 30,
            max_block_range: 1000,
            retry_attempts: 3,
            timeout_seconds: 30,
        },
    };

    Ok(config)
}

/// Mask sensitive parts of the database URL for logging
fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            format!("{}:****{}", &url[..colon_pos], &url[at_pos..])
        } else {
            url.to_string()
        }
    } else {
        url.to_string()
    }
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
    }

    #[test]
    fn test_mask_database_url() {
        let url = "postgresql://user:password@localhost:5432/db";
        let masked = mask_database_url(url);
        assert_eq!(masked, "postgresql://user:****@localhost:5432/db");

        let url_no_password = "postgresql://localhost:5432/db";
        let masked = mask_database_url(url_no_password);
        assert_eq!(masked, "postgresql://localhost:5432/db");
    }
}
