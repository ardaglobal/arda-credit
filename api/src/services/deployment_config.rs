//! Centralized deployment configuration service
//!
//! This module provides a single source of truth for deployment configuration,
//! reading from deployment JSON files for local/staging environments.
//!
//! Features:
//! - Cached deployment configuration to avoid repeated file reads
//! - Environment-aware configuration loading
//! - Optional refresh capability for development
//! - Consistent error handling across the codebase

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

// ============================================================================
// DEPLOYMENT CONFIGURATION TYPES
// ============================================================================

/// Deployment configuration loaded from JSON files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentConfig {
    /// Deployed ARDA contract address
    pub arda: String,
    /// Deployed USDC contract address (MockUSDC for local/staging)
    pub usdc: String,
    /// Deployed faucet contract address
    pub faucet: String,
    /// Deployed verifier contract address
    pub verifier: String,
    /// Deployer wallet address
    pub deployer: String,
    /// Chain ID of the deployment network
    pub chain_id: u64,
    /// Block number where contracts were deployed
    #[serde(rename = "blockNumber")]
    pub block_number: u64,
    /// Deployment timestamp (Unix timestamp or ISO string)
    pub timestamp: serde_json::Value,
    /// Environment where this deployment was made
    pub environment: String,
    /// Optional program verification key
    #[serde(rename = "programVKey")]
    pub program_vkey: Option<String>,
}

/// Network configuration derived from deployment
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub chain_id: u64,
    pub rpc_url: String,
    pub block_explorer_url: String,
    pub deployment_block: u64,
}

/// Complete deployment and network information
#[derive(Debug, Clone)]
pub struct DeploymentInfo {
    pub config: DeploymentConfig,
    pub network: NetworkConfig,
    pub environment: String,
}

// ============================================================================
// DEPLOYMENT SERVICE
// ============================================================================

/// Centralized deployment configuration service
#[derive(Debug)]
pub struct DeploymentService {
    /// Cached deployment information
    cached_deployment: Arc<RwLock<Option<DeploymentInfo>>>,
}

impl Default for DeploymentService {
    fn default() -> Self {
        Self::new()
    }
}

impl DeploymentService {
    /// Create a new deployment service
    pub fn new() -> Self {
        Self {
            cached_deployment: Arc::new(RwLock::new(None)),
        }
    }

    /// Get deployment information for the current environment
    ///
    /// This method will:
    /// 1. Check if deployment is already cached
    /// 2. If not cached, load from deployment file
    /// 3. Cache the result for future calls
    pub fn get_deployment(&self) -> Result<DeploymentInfo, DeploymentError> {
        let environment = std::env::var("SERVER_ENVIRONMENT")
            .unwrap_or_else(|_| "local".to_string())
            .to_lowercase();

        // Check if we have cached deployment for this environment
        {
            let cached = self.cached_deployment.read().unwrap();
            if let Some(ref deployment) = *cached {
                if deployment.environment == environment {
                    return Ok(deployment.clone());
                }
            }
        }

        // Load deployment from file
        info!(
            "📁 Loading deployment configuration for environment: {}",
            environment
        );
        let deployment = self.load_deployment_from_file(&environment)?;

        // Cache the deployment
        {
            let mut cached = self.cached_deployment.write().unwrap();
            *cached = Some(deployment.clone());
        }

        Ok(deployment)
    }

    /// Force refresh the deployment configuration from file
    ///
    /// Useful during development when deployment files might change
    pub fn refresh_deployment(&self) -> Result<DeploymentInfo, DeploymentError> {
        let environment = std::env::var("SERVER_ENVIRONMENT")
            .unwrap_or_else(|_| "local".to_string())
            .to_lowercase();

        info!(
            "🔄 Refreshing deployment configuration for environment: {}",
            environment
        );

        let deployment = self.load_deployment_from_file(&environment)?;

        // Update cache
        {
            let mut cached = self.cached_deployment.write().unwrap();
            *cached = Some(deployment.clone());
        }

        Ok(deployment)
    }

    /// Load deployment configuration from JSON file
    fn load_deployment_from_file(
        &self,
        environment: &str,
    ) -> Result<DeploymentInfo, DeploymentError> {
        let deployment_file = format!("contracts/deployments/{}-deployment.json", environment);

        let deployment_data =
            std::fs::read_to_string(&deployment_file).map_err(|e| DeploymentError::FileRead {
                file: deployment_file.clone(),
                source: e,
            })?;

        let config: DeploymentConfig =
            serde_json::from_str(&deployment_data).map_err(|e| DeploymentError::JsonParse {
                file: deployment_file,
                source: e,
            })?;

        // Build network configuration
        let network = self.build_network_config(&config, environment)?;

        let deployment_info = DeploymentInfo {
            config,
            network,
            environment: environment.to_string(),
        };

        info!(
            "✅ Deployment loaded: ARDA={}, USDC={}, Faucet={}, Chain={}",
            deployment_info.config.arda,
            deployment_info.config.usdc,
            deployment_info.config.faucet,
            deployment_info.config.chain_id
        );

        Ok(deployment_info)
    }

    /// Build network configuration from deployment config
    fn build_network_config(
        &self,
        config: &DeploymentConfig,
        environment: &str,
    ) -> Result<NetworkConfig, DeploymentError> {
        let rpc_url = match environment {
            "local" | "dev" | "staging" => "http://127.0.0.1:8545".to_string(),
            _ => return Err(DeploymentError::UnsupportedEnvironment {
                environment: environment.to_string(),
                message:
                    "Non-local environments should use environment variables, not deployment files"
                        .to_string(),
            }),
        };

        let block_explorer_url = match config.chain_id {
            1 => "https://etherscan.io".to_string(),
            11155111 => "https://sepolia.etherscan.io".to_string(),
            5 => "https://goerli.etherscan.io".to_string(),
            31337 => "http://localhost:8545".to_string(),
            _ => {
                warn!(
                    "Unknown chain ID: {}, defaulting to localhost",
                    config.chain_id
                );
                "http://localhost:8545".to_string()
            }
        };

        Ok(NetworkConfig {
            chain_id: config.chain_id,
            rpc_url,
            block_explorer_url,
            deployment_block: config.block_number,
        })
    }

    /// Check if deployment is available for local/staging environments
    pub fn is_deployment_available(&self) -> bool {
        match self.get_deployment() {
            Ok(_) => true,
            Err(e) => {
                warn!("Deployment not available: {}", e);
                false
            }
        }
    }

    /// Get specific contract address
    pub fn get_contract_address(&self, contract: ContractType) -> Result<String, DeploymentError> {
        let deployment = self.get_deployment()?;

        let address = match contract {
            ContractType::Arda => &deployment.config.arda,
            ContractType::Usdc => &deployment.config.usdc,
            ContractType::Faucet => &deployment.config.faucet,
            ContractType::Verifier => &deployment.config.verifier,
        };

        Ok(address.clone())
    }
}

// ============================================================================
// CONTRACT TYPE ENUM
// ============================================================================

/// Available contract types in deployment
#[derive(Debug, Clone, Copy)]
pub enum ContractType {
    Arda,
    Usdc,
    Faucet,
    Verifier,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Deployment service errors
#[derive(Debug, thiserror::Error)]
pub enum DeploymentError {
    #[error("Failed to read deployment file '{file}': {source}")]
    FileRead {
        file: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse deployment JSON '{file}': {source}")]
    JsonParse {
        file: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Unsupported environment '{environment}': {message}")]
    UnsupportedEnvironment {
        environment: String,
        message: String,
    },

    #[error("Missing contract field '{field}' in deployment file")]
    MissingField { field: String },
}

// ============================================================================
// GLOBAL SERVICE INSTANCE
// ============================================================================

/// Global deployment service instance
///
/// This can be initialized once at startup and used throughout the application
static DEPLOYMENT_SERVICE: once_cell::sync::Lazy<DeploymentService> =
    once_cell::sync::Lazy::new(DeploymentService::new);

/// Get the global deployment service instance
pub fn deployment_service() -> &'static DeploymentService {
    &DEPLOYMENT_SERVICE
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Get deployment information using the global service
pub fn get_deployment() -> Result<DeploymentInfo, DeploymentError> {
    deployment_service().get_deployment()
}

/// Get specific contract address using the global service
pub fn get_contract_address(contract: ContractType) -> Result<String, DeploymentError> {
    deployment_service().get_contract_address(contract)
}

/// Check if deployment is available using the global service
pub fn is_deployment_available() -> bool {
    deployment_service().is_deployment_available()
}

/// Refresh deployment configuration using the global service
pub fn refresh_deployment() -> Result<DeploymentInfo, DeploymentError> {
    deployment_service().refresh_deployment()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deployment_service_creation() {
        let service = DeploymentService::new();
        assert!(service.cached_deployment.read().unwrap().is_none());
    }

    #[test]
    fn test_contract_type_coverage() {
        // Ensure we handle all contract types
        let contracts = [
            ContractType::Arda,
            ContractType::Usdc,
            ContractType::Faucet,
            ContractType::Verifier,
        ];

        for contract in contracts {
            match contract {
                ContractType::Arda => {}
                ContractType::Usdc => {}
                ContractType::Faucet => {}
                ContractType::Verifier => {}
            }
        }
    }
}
