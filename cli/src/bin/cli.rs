//! Arda Credit CLI - Privacy-preserving loan operations
//!
//! This CLI provides comprehensive loan operation capabilities including:
//! - User management and KYC operations
//! - Liquidity pool creation and management
//! - Loan origination, payments, and closeouts
//! - Balance and withdrawal management
//! - Batch processing and ZK proof generation
//! - Performance testing and analytics
//!
//! All operations maintain privacy through zero-knowledge proofs while enabling
//! regulatory compliance and transaction verification.

use arda_credit_cli::cli::run_cli;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    run_cli().await
}