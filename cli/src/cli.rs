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
//!
//! ## Recent Changes
//!
//! Converted from loan-based CLI to deal-based CLI:
//! - Loan commands → Deal commands (consolidated with marketplace/transfer)
//! - Separate Marketplace commands → integrated into Deal commands
//! - Separate Transfer commands → integrated into Deal commands
//! - Updated terminology: borrower → originator, lender → investor
//!
//! ## Command Structure
//!
//! - **Deal**: Core deal operations, marketplace, and transfers (consolidated)
//! - **User**: User management and KYC operations
//! - **Pool**: Liquidity pool creation and management
//! - **Balance**: Balance tracking and withdrawal operations
//! - **Batch**: Batch processing and proof generation
//! - **Auth**: Authentication testing
//! - **Benchmark**: Performance testing (feature-gated)

use clap::{Parser, Subcommand};
use eyre::Result;
use std::env;
use tracing::{error, info};
use tracing_subscriber;

use arda_credit_api::BatchApiClient;

use crate::{
    auth::{handle_auth_command, AuthCommands},
    balance::{handle_balance_command, BalanceCommands},
    batch::{handle_batch_command, BatchCommands},
    deals::{handle_deal_command, DealCommands},
    user::{handle_user_command, UserCommands},
};

#[cfg(any(test, feature = "testing"))]
use crate::benchmarks::{handle_benchmark_command, BenchmarkCommands};

/// Main CLI application structure
#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Arda Credit CLI - Privacy-preserving deal operations")]
#[command(version)]
#[command(
    long_about = r#"Arda Credit CLI - Privacy-preserving deal system built on SP1 zero-knowledge proofs

This command-line interface provides access to all deal operations including:
• Deal origination, payments, and closeouts
• Deal marketplace trading (listing, browsing, purchasing)
• Deal ownership transfers between investors
• User management and KYC operations
• Liquidity pool creation and management
• Batch processing and proof generation

All deal operations maintain transaction privacy through zero-knowledge proofs
while enabling regulatory compliance and auditability.

For detailed command help, use: cli deal --help
"#
)]
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

    /// Deal operations (CRUD, payments, marketplace, transfers) - consolidated from separate categories
    #[command(subcommand)]
    Deal(DealCommands),

    /// Balance, wallet, and withdrawal operations
    #[command(subcommand)]
    Balance(BalanceCommands),

    /// Batch processing, proofs, and state operations
    #[command(subcommand)]
    Batch(BatchCommands),

    /// Magic link authentication testing
    #[command(subcommand)]
    Auth(AuthCommands),

    /// Performance benchmarks and analytics operations (requires testing feature)
    #[cfg(any(test, feature = "testing"))]
    #[command(subcommand)]
    Benchmark(BenchmarkCommands),
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
        Commands::Deal(cmd) => {
            handle_deal_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Balance(cmd) => {
            handle_balance_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Batch(cmd) => {
            handle_batch_command(&client, cmd, cli.verbose).await?;
        }
        Commands::Auth(cmd) => {
            handle_auth_command(&client, cmd, cli.verbose).await?;
        }
        #[cfg(any(test, feature = "testing"))]
        Commands::Benchmark(cmd) => {
            handle_benchmark_command(&client, cmd, cli.verbose).await?;
        }
    }

    info!("✅ CLI command completed successfully");
    Ok(())
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

// ============================================================================
// HELPER FUNCTIONS FOR CLI
// ============================================================================

/// Get the API base URL from environment or default
pub fn get_api_base_url() -> String {
    std::env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Handle API error responses consistently
pub async fn handle_api_error(response: reqwest::Response) -> Result<()> {
    let status = response.status();
    let url = response.url().clone();

    // Try to get the error response body
    let error_text = match response.text().await {
        Ok(text) => {
            // Try to parse as JSON to get a cleaner error message
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(json) => json
                    .get("error")
                    .or(json.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&text)
                    .to_string(),
                Err(_) => text,
            }
        }
        Err(_) => format!("HTTP {} - No response body", status),
    };

    error!("❌ API request failed: {} {} - {}", status, url, error_text);

    // Return a user-friendly error
    Err(eyre::eyre!(
        "API request failed: {} ({})",
        error_text,
        status
    ))
}
