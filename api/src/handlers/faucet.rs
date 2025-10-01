//! Faucet Information API Handler
//!
//! Provides faucet contract information for local/staging environments.
//! Used by frontend to automatically trigger faucet requests during onboarding.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};
use utoipa::ToSchema;

/// Faucet information response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FaucetInfoResponse {
    /// Whether faucet is available in this environment
    pub available: bool,

    /// Current server environment
    pub environment: String,

    /// Faucet contract address (if available)
    pub faucet_address: Option<String>,

    /// USDC contract address (if available)
    pub usdc_address: Option<String>,

    /// Amount distributed per request (in USDC, not wei)
    pub amount_per_request: Option<String>,

    /// Cooldown period in seconds
    pub cooldown_seconds: Option<u64>,

    /// Ethereum RPC URL for local/staging
    pub rpc_url: Option<String>,

    /// Chain ID
    pub chain_id: Option<String>,

    /// Suggested MetaMask network configuration
    pub network_config: Option<NetworkConfig>,
}

/// MetaMask network configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NetworkConfig {
    pub network_name: String,
    pub rpc_url: String,
    pub chain_id: String,
    pub currency_symbol: String,
    pub block_explorer_url: Option<String>,
}

/// Query parameters for checking user's faucet status
#[derive(Debug, Deserialize, ToSchema)]
pub struct FaucetStatusQuery {
    /// Ethereum wallet address to check
    pub address: String,
}

/// User's faucet status response
#[derive(Debug, Serialize, ToSchema)]
pub struct FaucetStatusResponse {
    /// Whether user can request funds now
    pub can_request_now: bool,

    /// Time until next request in seconds (0 if available now)
    pub time_until_next_request: u64,

    /// User's current USDC balance (wei)
    pub user_balance: Option<String>,

    /// Faucet contract balance (wei)
    pub faucet_balance: Option<String>,

    /// Error message if status check failed
    pub error: Option<String>,
}

/// Get faucet information endpoint
///
/// Returns faucet contract details and environment information.
/// Used by frontend to determine if faucet UI should be shown.
#[utoipa::path(
    get,
    path = "/api/v1/faucet/info",
    tag = "Dev",
    responses(
        (status = 200, description = "Faucet information retrieved", body = FaucetInfoResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_faucet_info() -> Result<Json<FaucetInfoResponse>, StatusCode> {
    info!("📋 Fetching faucet information");

    // Get server environment
    let environment = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    // Faucet only available in local/staging
    let is_local_env = matches!(environment.as_str(), "local" | "staging");

    if !is_local_env {
        info!("🌍 Faucet not available in {} environment", environment);
        return Ok(Json(FaucetInfoResponse {
            available: false,
            environment,
            faucet_address: None,
            usdc_address: None,
            amount_per_request: None,
            cooldown_seconds: None,
            rpc_url: None,
            chain_id: None,
            network_config: None,
        }));
    }

    // Try to load deployment file for local/staging
    let deployment_file = format!("contracts/deployments/{}-deployment.json", environment);
    let deployment_data = match std::fs::read_to_string(&deployment_file) {
        Ok(data) => data,
        Err(e) => {
            warn!("Failed to read deployment file {}: {}", deployment_file, e);
            return Ok(Json(FaucetInfoResponse {
                available: false,
                environment,
                faucet_address: None,
                usdc_address: None,
                amount_per_request: None,
                cooldown_seconds: None,
                rpc_url: None,
                chain_id: None,
                network_config: Some(NetworkConfig {
                    network_name: "Local Anvil Devnet".to_string(),
                    rpc_url: "http://127.0.0.1:8545".to_string(),
                    chain_id: "31337".to_string(),
                    currency_symbol: "ETH".to_string(),
                    block_explorer_url: None,
                }),
            }));
        }
    };

    // Parse deployment JSON
    let deployment: serde_json::Value = match serde_json::from_str(&deployment_data) {
        Ok(json) => json,
        Err(e) => {
            warn!("Failed to parse deployment JSON: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let faucet_address = deployment["faucet"].as_str().map(String::from);
    let usdc_address = deployment["usdc"].as_str().map(String::from);
    let chain_id = deployment["chainId"]
        .as_u64()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "31337".to_string());

    info!(
        "✅ Faucet info loaded: address={:?}, chain_id={}",
        faucet_address, chain_id
    );

    Ok(Json(FaucetInfoResponse {
        available: true,
        environment: environment.clone(),
        faucet_address: faucet_address.clone(),
        usdc_address: usdc_address.clone(),
        amount_per_request: Some("1000000".to_string()), // 1M USDC
        cooldown_seconds: Some(3600),                    // 1 hour
        rpc_url: Some("http://127.0.0.1:8545".to_string()),
        chain_id: Some(chain_id.clone()),
        network_config: Some(NetworkConfig {
            network_name: if environment == "local" {
                "Local Anvil Devnet"
            } else {
                "Staging Anvil Devnet"
            }
            .to_string(),
            rpc_url: if environment == "local" {
                "http://127.0.0.1:8545"
            } else {
                "http://staging-anvil:8545" // TODO: Configure staging URL
            }
            .to_string(),
            chain_id,
            currency_symbol: "ETH".to_string(),
            block_explorer_url: None,
        }),
    }))
}

/// Check faucet status for a specific address
///
/// Returns whether the address can request funds and their current balance.
#[utoipa::path(
    get,
    path = "/api/v1/faucet/status",
    tag = "Dev",
    params(
        ("address" = String, Query, description = "Ethereum wallet address to check")
    ),
    responses(
        (status = 200, description = "Faucet status retrieved", body = FaucetStatusResponse),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn check_faucet_status(
    Query(params): Query<FaucetStatusQuery>,
) -> Result<Json<FaucetStatusResponse>, StatusCode> {
    info!("🔍 Checking faucet status for address: {}", params.address);

    // Validate address format
    if !params.address.starts_with("0x") || params.address.len() != 42 {
        warn!("Invalid address format: {}", params.address);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get environment
    let environment = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    if !matches!(environment.as_str(), "local" | "staging") {
        return Ok(Json(FaucetStatusResponse {
            can_request_now: false,
            time_until_next_request: 0,
            user_balance: None,
            faucet_balance: None,
            error: Some("Faucet not available in this environment".to_string()),
        }));
    }

    // Try to load deployment file
    let deployment_file = format!("contracts/deployments/{}-deployment.json", environment);
    let deployment_data = match std::fs::read_to_string(&deployment_file) {
        Ok(data) => data,
        Err(e) => {
            warn!("Failed to read deployment file: {}", e);
            return Ok(Json(FaucetStatusResponse {
                can_request_now: false,
                time_until_next_request: 0,
                user_balance: None,
                faucet_balance: None,
                error: Some(
                    "Deployment file not found. Run 'make deploy-contracts' first.".to_string(),
                ),
            }));
        }
    };

    let _deployment: serde_json::Value =
        serde_json::from_str(&deployment_data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // For now, return basic info
    // TODO: Integrate with ethereum-client to query actual on-chain status
    info!("✅ Faucet status check completed for {}", params.address);

    Ok(Json(FaucetStatusResponse {
        can_request_now: true, // Optimistic - user will find out when they try
        time_until_next_request: 0,
        user_balance: None,   // Would need ethereum-client integration
        faucet_balance: None, // Would need ethereum-client integration
        error: None,
    }))
}

/// Faucet configuration endpoint
///
/// Returns complete faucet configuration for frontend integration.
#[utoipa::path(
    get,
    path = "/api/v1/faucet/config",
    tag = "Dev",
    responses(
        (status = 200, description = "Faucet configuration", body = HashMap<String, serde_json::Value>)
    )
)]
pub async fn get_faucet_config() -> Json<HashMap<String, serde_json::Value>> {
    let mut config = HashMap::new();

    let environment = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    config.insert("environment".to_string(), environment.clone().into());
    config.insert(
        "faucet_enabled".to_string(),
        matches!(environment.as_str(), "local" | "staging").into(),
    );
    config.insert(
        "amount_per_request_usdc".to_string(),
        "1000000".to_string().into(),
    );
    config.insert("cooldown_hours".to_string(), 1.into());

    Json(config)
}
