use clap::Parser;
use ethereum_client::{
    Config, DepositMonitor, DepositMonitorConfig, EthereumClient, EthereumError, Result,
};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Database URL
    #[arg(short, long, env = "DATABASE_URL")]
    database_url: String,

    /// Ethereum RPC URL
    #[arg(short, long, env = "ETHEREUM_RPC_URL")]
    ethereum_rpc_url: String,

    /// Loan contract address
    #[arg(short, long, env = "LOAN_CONTRACT_ADDRESS")]
    contract_address: String,

    /// Required block confirmations (default: 12)
    #[arg(long, default_value = "12")]
    confirmations: u64,

    /// Confirmation check interval in seconds (default: 30)
    #[arg(long, default_value = "30")]
    check_interval: u64,

    /// Maximum retry attempts (default: 3)
    #[arg(long, default_value = "3")]
    max_retries: u32,

    /// Retry backoff in seconds (default: 60)
    #[arg(long, default_value = "60")]
    retry_backoff: u64,

    /// Processing batch size (default: 100)
    #[arg(long, default_value = "100")]
    batch_size: usize,

    /// Log level (default: info)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Check service health and exit
    #[arg(long)]
    health_check: bool,

    /// Print statistics and exit
    #[arg(long)]
    stats: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "info" => LevelFilter::INFO,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .compact(),
        )
        .with(
            EnvFilter::builder()
                .with_default_directive(log_level.into())
                .from_env_lossy(),
        )
        .init();

    info!("Starting USDC Deposit Monitor");
    info!("Database URL: {}", mask_sensitive_data(&args.database_url));
    info!(
        "Ethereum RPC: {}",
        mask_sensitive_data(&args.ethereum_rpc_url)
    );
    info!("Contract: {}", args.contract_address);
    info!("Confirmations: {}", args.confirmations);

    // Initialize database connection
    let db_pool = create_database_pool(&args.database_url).await?;

    // Run health check if requested
    if args.health_check {
        return run_health_check(&db_pool, &args.ethereum_rpc_url, &args.contract_address).await;
    }

    // Print stats if requested
    if args.stats {
        return print_statistics(&db_pool).await;
    }

    // Initialize Ethereum client
    let ethereum_client =
        create_ethereum_client(&args.ethereum_rpc_url, &args.contract_address).await?;

    // Create deposit monitor configuration
    let monitor_config = DepositMonitorConfig {
        required_confirmations: args.confirmations,
        confirmation_check_interval: args.check_interval,
        max_retry_attempts: args.max_retries,
        retry_backoff_seconds: args.retry_backoff,
        batch_size: args.batch_size,
    };

    // Create and start deposit monitor
    let mut deposit_monitor =
        DepositMonitor::new(ethereum_client, db_pool.clone(), Some(monitor_config));

    // Handle shutdown signals
    let shutdown_signal = tokio::signal::ctrl_c();

    tokio::select! {
        result = deposit_monitor.start() => {
            error!("Deposit monitor stopped unexpectedly: {:?}", result);
            result
        }
        _ = shutdown_signal => {
            info!("Received shutdown signal");
            deposit_monitor.stop().await;
            info!("Deposit monitor stopped gracefully");
            Ok(())
        }
    }
}

/// Create database connection pool
async fn create_database_pool(database_url: &str) -> Result<PgPool> {
    info!("Connecting to database...");

    let pool = PgPool::connect(database_url)
        .await
        .map_err(|e| EthereumError::Database(format!("Failed to connect to database: {}", e)))?;

    // Test the connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| EthereumError::Database(format!("Database health check failed: {}", e)))?;

    info!("Database connection established");
    Ok(pool)
}

/// Create Ethereum client
async fn create_ethereum_client(
    rpc_url: &str,
    contract_address: &str,
) -> Result<Arc<EthereumClient>> {
    info!("Connecting to Ethereum node...");

    let config = Config {
        network: ethereum_client::config::NetworkConfig {
            name: "mainnet".to_string(),
            chain_id: 1,
            rpc_url: rpc_url
                .parse()
                .map_err(|e| EthereumError::Config(format!("Invalid RPC URL: {}", e)))?,
            ws_url: None,
            explorer_url: None,
            is_testnet: false,
        },
        contract: ethereum_client::config::ContractConfig {
            arda_credit_contract: contract_address.parse().map_err(|e| {
                EthereumError::InvalidAddress(format!("Invalid contract address: {}", e))
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

    let client = Arc::new(EthereumClient::new(config).await?);

    // Test the connection
    let latest_block = client.get_latest_block_number().await?;
    info!("Connected to Ethereum node, latest block: {}", latest_block);

    Ok(client)
}

/// Run health check
async fn run_health_check(db_pool: &PgPool, rpc_url: &str, contract_address: &str) -> Result<()> {
    info!("Running health check...");

    // Check database
    match sqlx::query("SELECT COUNT(*) as count FROM usdc_deposits")
        .fetch_one(db_pool)
        .await
    {
        Ok(row) => {
            let count: i64 = row.get("count");
            info!(
                "✓ Database connection healthy, {} deposits in database",
                count
            );
        }
        Err(e) => {
            error!("✗ Database health check failed: {}", e);
            return Err(EthereumError::Database(e.to_string()));
        }
    }

    // Check Ethereum connection
    match create_ethereum_client(rpc_url, contract_address).await {
        Ok(client) => {
            let latest_block = client.get_latest_block_number().await?;
            info!(
                "✓ Ethereum connection healthy, latest block: {}",
                latest_block
            );
        }
        Err(e) => {
            error!("✗ Ethereum health check failed: {}", e);
            return Err(e);
        }
    }

    // Check pending deposits
    let pending_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM usdc_deposits WHERE status = 'pending'")
            .fetch_one(db_pool)
            .await
            .map_err(|e| EthereumError::Database(e.to_string()))?;

    info!("✓ {} pending deposits awaiting confirmation", pending_count);

    info!("Health check passed!");
    Ok(())
}

/// Print service statistics
async fn print_statistics(db_pool: &PgPool) -> Result<()> {
    info!("Fetching service statistics...");

    // Create a temporary deposit monitor to use its stats function
    let dummy_config = Config::default();
    let dummy_client = Arc::new(
        EthereumClient::new(dummy_config)
            .await
            .unwrap_or_else(|_| panic!("Failed to create dummy client")),
    );
    let deposit_monitor = DepositMonitor::new(dummy_client, db_pool.clone(), None);

    match deposit_monitor.get_deposit_stats().await {
        Ok(stats) => {
            println!("\n=== USDC Deposit Monitor Statistics ===");
            for (key, value) in stats {
                println!("{}: {}", key.replace('_', " ").to_uppercase(), value);
            }
            println!();
        }
        Err(e) => {
            error!("Failed to fetch statistics: {}", e);
            return Err(e);
        }
    }

    // Recent deposits
    let recent_deposits = sqlx::query(
        r#"
        SELECT
            user_address,
            amount::TEXT as amount,
            status,
            confirmation_count,
            created_at
        FROM usdc_deposits
        ORDER BY created_at DESC
        LIMIT 10
        "#,
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| EthereumError::Database(e.to_string()))?;

    if !recent_deposits.is_empty() {
        println!("=== Recent Deposits ===");
        for deposit in recent_deposits {
            let amount_str: String = deposit.get("amount");
            let amount_usdc = amount_str.parse::<u64>().unwrap_or(0) as f64 / 1_000_000.0;
            let user_address: String = deposit.get("user_address");
            let status: String = deposit.get("status");
            let confirmation_count: i32 = deposit.get("confirmation_count");
            let created_at: chrono::DateTime<chrono::Utc> = deposit.get("created_at");
            println!(
                "{}: {} USDC from {} (status: {}, confirmations: {})",
                created_at.format("%Y-%m-%d %H:%M:%S"),
                amount_usdc,
                &user_address[..10],
                status,
                confirmation_count
            );
        }
        println!();
    }

    Ok(())
}

/// Mask sensitive data in URLs for logging
fn mask_sensitive_data(url: &str) -> String {
    if let Ok(parsed_url) = url::Url::parse(url) {
        let mut masked = parsed_url.clone();
        if parsed_url.password().is_some() {
            masked.set_password(Some("***")).ok();
        }
        if let Some(query) = parsed_url.query() {
            if query.contains("apikey") || query.contains("token") {
                masked.set_query(Some("***"));
            }
        }
        masked.to_string()
    } else {
        // If not a valid URL, just mask potential API keys
        if url.len() > 20 {
            format!("{}...{}", &url[..10], &url[url.len() - 4..])
        } else {
            url.to_string()
        }
    }
}
