//! Core CLI setup and command definitions
//!
//! This module provides the main CLI application structure, argument parsing,
//! and command routing. It serves as the entry point for all CLI operations
//! and coordinates with specialized modules for specific functionality.
//!
//! ## Architecture
//!
//! The CLI is organized around the following principles:
//! - Single responsibility: Each module handles one functional area
//! - Consistent error handling: All operations use the same Result patterns
//! - Comprehensive documentation: All commands have detailed help text
//! - Validation: Input validation happens at the CLI level before delegation

use clap::{Parser, Subcommand};
use eyre::Result;
use std::env;
use tracing::{error, info};
use tracing_subscriber;

use arda_credit_api::BatchApiClient;

use crate::{
    balance::{BalanceCommands, handle_balance_command},
    batch::{BatchCommands, handle_batch_command},
    loans::{LoanCommands, handle_loan_command},
    pools::{PoolCommands, handle_pool_command},
    testing::{TestingCommands, handle_testing_command},
    user::{UserCommands, handle_user_command},
};

/// Main CLI application structure
#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Arda Credit CLI - Privacy-preserving loan operations")]
#[command(version)]
#[command(long_about = "
Arda Credit CLI provides comprehensive loan operation capabilities including:
- User management and KYC operations
- Liquidity pool creation and management  
- Loan origination, payments, and closeouts
- Balance and withdrawal management
- Batch processing and ZK proof generation
- Performance testing and analytics

All operations maintain privacy through zero-knowledge proofs while enabling
regulatory compliance and transaction verification.")]
pub struct Cli {
    /// API server base URL
    #[arg(
        long,
        env = "ARDA_CREDIT_API_URL",
        default_value = "http://localhost:8080"
    )]
    pub api_url: String,

    /// Enable verbose output across all commands
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Main command categories for the Arda Credit CLI
#[derive(Subcommand)]
pub enum Commands {
    /// User management, KYC, and invite code operations
    #[command(subcommand)]
    User(UserCommands),

    /// Liquidity pool creation and management
    #[command(subcommand)]
    Pool(PoolCommands),

    /// Credit/loan operations (investors, merchants, credit lines)
    #[command(subcommand)]
    Loan(LoanCommands),

    /// Balance, wallet, and withdrawal operations
    #[command(subcommand)]
    Balance(BalanceCommands),

    /// Batch processing, proofs, and state operations
    #[command(subcommand)]
    Batch(BatchCommands),

    /// Testing, performance, and analytics operations
    #[command(subcommand)]
    Test(TestingCommands),

    /// Legacy commands for backward compatibility (deprecated)
    #[command(flatten)]
    Legacy(LegacyCommands),
}

/// Legacy commands maintained for backward compatibility
/// These will be removed in a future version
#[derive(Subcommand)]
pub enum LegacyCommands {
    /// Submit a new transaction to the batch processing queue
    #[deprecated(note = "Use 'batch submit-transaction' instead")]
    SubmitTransaction {
        /// Transaction amount to add to the counter
        #[arg(short, long)]
        amount: i32,
    },
    
    /// View all pending (unbatched) transactions  
    #[deprecated(note = "Use 'batch view-pending' instead")]
    ViewPending,
    
    /// Check API server health
    #[deprecated(note = "Use 'batch health-check' instead")]
    HealthCheck,
    
    /// Trigger batch creation and get contract submission data
    #[deprecated(note = "Use 'batch trigger' instead")]
    TriggerBatch {
        /// Maximum number of transactions to include in batch
        #[arg(long, default_value = "10")]
        batch_size: Option<i32>,
        /// Show detailed output including private information
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Initialize CLI application with logging and configuration
pub fn init_cli() -> Result<()> {
    // Setup structured logging with environment-based filtering
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "cli=info".to_string()))
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("🚀 Arda Credit CLI starting...");
    Ok(())
}

/// Main CLI command dispatcher
pub async fn run_cli() -> Result<()> {
    init_cli()?;

    let cli = Cli::parse();
    let client = BatchApiClient::new(&cli.api_url);

    info!("📡 Connected to API server: {}", cli.api_url);

    match cli.command {
        Commands::User(cmd) => {
            handle_user_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Pool(cmd) => {
            handle_pool_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Loan(cmd) => {
            handle_loan_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Balance(cmd) => {
            handle_balance_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Batch(cmd) => {
            handle_batch_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Test(cmd) => {
            handle_testing_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Legacy(cmd) => {
            handle_legacy_command(&client, cmd, cli.verbose).await?;
        }
    }

    info!("✅ CLI command completed successfully");
    Ok(())
}

/// Handle legacy commands with deprecation warnings
async fn handle_legacy_command(
    client: &BatchApiClient,
    command: LegacyCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        #[allow(deprecated)]
        LegacyCommands::SubmitTransaction { amount } => {
            eprintln!("⚠️  WARNING: 'submit-transaction' is deprecated. Use 'batch submit-transaction' instead.");
            crate::batch::submit_transaction(client, amount).await
        }
        #[allow(deprecated)]
        LegacyCommands::ViewPending => {
            eprintln!("⚠️  WARNING: 'view-pending' is deprecated. Use 'batch view-pending' instead.");
            crate::batch::view_pending_transactions(client).await
        }
        #[allow(deprecated)]
        LegacyCommands::HealthCheck => {
            eprintln!("⚠️  WARNING: 'health-check' is deprecated. Use 'batch health-check' instead.");
            crate::batch::health_check(client).await
        }
        #[allow(deprecated)]
        LegacyCommands::TriggerBatch { batch_size, verbose: cmd_verbose } => {
            eprintln!("⚠️  WARNING: 'trigger-batch' is deprecated. Use 'batch trigger' instead.");
            crate::batch::trigger_batch(client, batch_size, cmd_verbose || verbose).await
        }
    }
}

/// CLI middleware for common operations
pub struct CliMiddleware;

impl CliMiddleware {
    /// Validate required environment variables
    pub fn validate_environment() -> Result<()> {
        // Check for required environment variables
        if env::var("ARDA_CREDIT_API_URL").is_err() {
            info!("💡 ARDA_CREDIT_API_URL not set, using default: http://localhost:8080");
        }

        Ok(())
    }

    /// Setup authentication context (for future use)
    pub fn setup_auth_context() -> Result<()> {
        // Future: Setup authentication context here
        // This could include API key validation, JWT tokens, etc.
        Ok(())
    }

    /// Validate API connectivity
    pub async fn validate_connectivity(client: &BatchApiClient) -> Result<()> {
        match crate::batch::health_check(client).await {
            Ok(_) => {
                info!("✅ API server connectivity validated");
                Ok(())
            }
            Err(e) => {
                error!("❌ API server connectivity failed: {}", e);
                Err(e)
            }
        }
    }
}