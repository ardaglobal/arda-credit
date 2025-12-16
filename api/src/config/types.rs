//! Core API types and configurations
//!
//! This module contains the fundamental types and configurations used across
//! the API, extracted from the original rest.rs file.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Re-export the core types from router.rs
pub use crate::api::router::{ApiConfig, ApiState};

// ============================================================================
// SERVER ENVIRONMENT CONFIGURATION
// ============================================================================

/// Server environment for determining proof generation method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerEnvironment {
    /// Local development - uses local SP1 prover, no smart contract posting
    Local,
    /// Development environment - uses local SP1 prover, no smart contract posting
    Dev,
    /// Staging environment - uses Sindri/Groth16 proofs, smart contract posting on Anvil
    Staging,
    /// Sandbox environment - uses Sindri service, posts to Sepolia
    Sandbox,
    /// Production environment - uses Sindri service, posts to mainnet
    Production,
}

impl ServerEnvironment {
    /// Parse from environment variable string
    pub fn from_env() -> Self {
        std::env::var("SERVER_ENVIRONMENT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::Local)
    }

    /// Check if this environment should use local SP1 prover (core proofs, no contract posting)
    pub const fn use_local_prover(&self) -> bool {
        matches!(self, Self::Local | Self::Dev)
    }

    /// Check if this environment should use Sindri service
    pub const fn use_sindri(&self) -> bool {
        matches!(self, Self::Staging | Self::Sandbox | Self::Production)
    }

    /// Check if this environment should post to smart contracts
    pub const fn should_post_to_contract(&self) -> bool {
        matches!(self, Self::Staging | Self::Sandbox | Self::Production)
    }

    /// Check if this environment uses Anvil (local blockchain)
    pub const fn uses_anvil(&self) -> bool {
        matches!(self, Self::Local | Self::Dev | Self::Staging)
    }
}

impl std::str::FromStr for ServerEnvironment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "dev" => Ok(Self::Dev),
            "staging" => Ok(Self::Staging),
            "sandbox" => Ok(Self::Sandbox),
            "production" => Ok(Self::Production),
            _ => Err(format!(
                "Invalid server environment: '{}'. Valid options: local, dev, staging, sandbox, production",
                s
            )),
        }
    }
}

impl std::fmt::Display for ServerEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Dev => write!(f, "dev"),
            Self::Staging => write!(f, "staging"),
            Self::Sandbox => write!(f, "sandbox"),
            Self::Production => write!(f, "production"),
        }
    }
}

impl Default for ServerEnvironment {
    fn default() -> Self {
        Self::Local
    }
}

// ============================================================================
// CORE TYPES MOVED FROM REST.RS
// ============================================================================

/// User type enumeration for allowed user types
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    Investor,
    Originator,
    Admin,
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Investor => write!(f, "investor"),
            Self::Originator => write!(f, "originator"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for UserType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "investor" => Ok(Self::Investor),
            "originator" => Ok(Self::Originator),
            "admin" => Ok(Self::Admin),
            _ => Err(format!("Invalid user type: {}", s)),
        }
    }
}

/// Standard error response format
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(error: &str, message: &str) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
        }
    }
}

// Note: All types are now imported from rest.rs to avoid duplication
// This file serves as a bridge during the transition period
