//! Arda Credit CLI Library
//!
//! This library provides a comprehensive command-line interface for the Arda Credit
//! privacy-preserving loan platform. The CLI is organized into focused modules:
//!
//! - [`balance`] - Balance, wallet, and withdrawal operations
//! - [`batch`] - Batch processing, proofs, and state operations  
//! - [`cli`] - Core CLI setup and middleware configuration
//! - [`deals`] - Credit/deal operations (investor, merchant, credit lines)
//! - [`pools`] - Liquidity pool operations
//! - [`benchmarks`] - Performance benchmarks and analytics operations (feature-gated)
//! - [`user`] - User management, KYC, and invite code operations
//!
//! Each module contains focused functionality and should not exceed 1000 lines of code.
//! All public functions have comprehensive documentation and follow consistent error
//! handling patterns.

// Public module exports
pub mod auth;
pub mod balance;
pub mod batch;
pub mod cli;
pub mod deals;
pub mod marketplace;
pub mod pools;
pub mod transfers;
pub mod user;

// Performance benchmarks (available for both internal and integration tests)
#[cfg(any(test, feature = "testing"))]
pub mod benchmarks;

// Re-export commonly used types and functions
pub use auth::*;
pub use balance::*;
pub use batch::*;
pub use cli::*;
pub use deals::*;
pub use marketplace::*;
pub use pools::*;
pub use user::*;

// Conditionally re-export benchmark utilities
#[cfg(any(test, feature = "testing"))]
pub use benchmarks::*;

// External dependencies used across modules
pub use clap::{Parser, Subcommand};
pub use eyre::Result;
pub use std::env;
pub use std::fs;
pub use std::time::Instant;
pub use tracing::{error, info, warn};

// Arda Credit dependencies
pub use alloy_primitives::Address;
pub use arda_credit_api::BatchApiClient;

/// Validation error for CLI inputs
#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation error: {}", self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Validates Ethereum addresses for CLI inputs
pub fn validate_ethereum_address(address: &str) -> Result<Address, ValidationError> {
    address.parse().map_err(|_| ValidationError {
        message: format!("Invalid Ethereum address format: {}", address),
    })
}

/// Common CLI configuration and utilities
pub struct CliConfig {
    pub api_base_url: String,
    pub verbose: bool,
}

impl CliConfig {
    /// Creates a new CLI configuration from environment variables
    pub fn new() -> Self {
        let api_base_url =
            env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

        Self {
            api_base_url,
            verbose: false,
        }
    }

    /// Creates a BatchApiClient from the configuration
    pub fn create_client(&self) -> BatchApiClient {
        BatchApiClient::new(&self.api_base_url)
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self::new()
    }
}
