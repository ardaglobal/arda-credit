use ethereum_client::{Config, DepositMonitor, DepositMonitorConfig, EthereumClient, Result};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("USDC Deposit Monitor Example");

    // Configuration - in production these would come from environment variables
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/arda_credit_db".to_string()
    });

    let ethereum_rpc_url = std::env::var("ETHEREUM_RPC_URL")
        .unwrap_or_else(|_| "https://eth-mainnet.alchemyapi.io/v2/demo".to_string());

    let contract_address = std::env::var("LOAN_CONTRACT_ADDRESS")
        .unwrap_or_else(|_| "0x8e202F059E1B4d8b9150bC5D2198B843e5eFF8De".to_string());

    // Connect to database
    info!(
        "Connecting to database: {}",
        mask_database_url(&database_url)
    );
    let db_pool = PgPool::connect(&database_url)
        .await
        .map_err(|e| ethereum_client::EthereumError::Database(e.to_string()))?;

    // Test database connection
    sqlx::query("SELECT 1")
        .fetch_one(&db_pool)
        .await
        .map_err(|e| {
            ethereum_client::EthereumError::Database(format!("Database test failed: {}", e))
        })?;

    info!("✓ Database connected successfully");

    // Create Ethereum client
    info!(
        "Connecting to Ethereum RPC: {}",
        mask_rpc_url(&ethereum_rpc_url)
    );
    let config = Config {
        network: ethereum_client::config::NetworkConfig {
            name: "mainnet".to_string(),
            chain_id: 1,
            rpc_url: ethereum_rpc_url.parse().map_err(|e| {
                ethereum_client::EthereumError::Config(format!("Invalid RPC URL: {}", e))
            })?,
            ws_url: None,
            explorer_url: None,
            is_testnet: false,
        },
        contract: ethereum_client::config::ContractConfig {
            arda_credit_contract: contract_address.parse().map_err(|e| {
                ethereum_client::EthereumError::InvalidAddress(format!(
                    "Invalid contract address: {}",
                    e
                ))
            })?,
            verifier_contract: "0x0000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            deployment_block: None,
        },
        rpc: ethereum_client::config::RpcConfig {
            notify_addresses: vec![],
            rate_limit_per_second: 10,
        },
        signer: None,
        monitoring: ethereum_client::config::MonitoringConfig {
            enable_event_monitoring: true,
            polling_interval_seconds: 12,
            max_block_range: 1000,
            retry_attempts: 3,
            timeout_seconds: 30,
        },
    };

    let ethereum_client = Arc::new(EthereumClient::new(config).await?);
    let latest_block = ethereum_client.get_latest_block_number().await?;
    info!(
        "✓ Ethereum connected successfully, latest block: {}",
        latest_block
    );

    // Create deposit monitor configuration
    let monitor_config = DepositMonitorConfig {
        required_confirmations: 3,       // Lower for testing
        confirmation_check_interval: 10, // Check every 10 seconds
        max_retry_attempts: 2,
        retry_backoff_seconds: 30,
        batch_size: 50,
    };

    info!("Creating deposit monitor with config:");
    info!(
        "  Required confirmations: {}",
        monitor_config.required_confirmations
    );
    info!(
        "  Check interval: {} seconds",
        monitor_config.confirmation_check_interval
    );
    info!("  Max retries: {}", monitor_config.max_retry_attempts);

    // Create deposit monitor
    let mut deposit_monitor =
        DepositMonitor::new(ethereum_client, db_pool.clone(), Some(monitor_config));

    // Show initial statistics
    info!("Fetching initial deposit statistics...");
    match deposit_monitor.get_deposit_stats().await {
        Ok(stats) => {
            for (key, value) in &stats {
                info!("  {}: {}", key.replace('_', " "), value);
            }
        }
        Err(e) => {
            info!("Could not fetch initial stats: {}", e);
        }
    }

    // Run the deposit monitor for a short time (30 seconds) as an example
    info!("Starting deposit monitor for 30 seconds...");
    info!("The monitor will:");
    info!("  1. Listen for new Deposit events from the smart contract");
    info!("  2. Store them in the database with 'pending' status");
    info!("  3. Monitor block confirmations");
    info!("  4. Update status to 'confirmed' after required confirmations");
    info!("  5. Aggregate user balances automatically");

    // Run with timeout for demonstration
    let monitor_result = timeout(Duration::from_secs(30), deposit_monitor.start()).await;

    match monitor_result {
        Ok(Ok(())) => {
            info!("Deposit monitor completed successfully");
        }
        Ok(Err(e)) => {
            info!("Deposit monitor stopped with error: {}", e);
        }
        Err(_) => {
            info!("Deposit monitor example timeout reached (30 seconds)");
            deposit_monitor.stop().await;
        }
    }

    // Show final statistics
    info!("Fetching final deposit statistics...");
    match deposit_monitor.get_deposit_stats().await {
        Ok(stats) => {
            for (key, value) in &stats {
                info!("  {}: {}", key.replace('_', " "), value);
            }
        }
        Err(e) => {
            info!("Could not fetch final stats: {}", e);
        }
    }

    info!("Example completed!");
    info!("");
    info!("To run the full deposit monitor service:");
    info!("  cargo run --bin deposit_monitor -- \\");
    info!("    --database-url \"{}\" \\", database_url);
    info!("    --ethereum-rpc-url \"{}\" \\", ethereum_rpc_url);
    info!("    --contract-address \"{}\"", contract_address);

    Ok(())
}

fn mask_database_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            format!(
                "postgresql://***@{}:{}/***",
                host,
                parsed.port().unwrap_or(5432)
            )
        } else {
            "postgresql://***".to_string()
        }
    } else {
        "***".to_string()
    }
}

fn mask_rpc_url(url: &str) -> String {
    if url.contains("localhost") || url.contains("127.0.0.1") {
        url.to_string()
    } else if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            format!("{}://{}/***/", parsed.scheme(), host)
        } else {
            "***".to_string()
        }
    } else {
        "***".to_string()
    }
}
