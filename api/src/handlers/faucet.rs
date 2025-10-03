//! Faucet Information and Request API Handler
//!
//! Provides faucet contract information for local/staging environments.
//! Used by frontend to automatically trigger faucet requests during onboarding.

use axum::{extract::Query, extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use utoipa::ToSchema;

use crate::services::deployment_config::{get_deployment, DeploymentError};

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

    /// ETH amount sent per request (for gas)
    pub eth_amount_per_request: Option<String>,

    /// Cooldown period in seconds
    pub cooldown_seconds: Option<u64>,

    /// Ethereum RPC URL for local/staging
    pub rpc_url: Option<String>,

    /// Chain ID
    pub chain_id: Option<String>,

    /// Suggested MetaMask network configuration
    pub network_config: Option<NetworkConfig>,

    /// USDC token configuration for MetaMask import
    pub token_config: Option<TokenConfig>,
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

/// USDC token configuration for MetaMask import
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenConfig {
    /// Token contract address
    pub address: String,
    /// Token symbol
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Token decimals
    pub decimals: u8,
    /// MetaMask deep link for automatic token import
    pub metamask_import_url: String,
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

/// Request body for faucet fund request
#[derive(Debug, Deserialize, ToSchema)]
pub struct FaucetRequestBody {
    /// Ethereum wallet address to receive funds
    pub address: String,
}

/// Faucet request response
#[derive(Debug, Serialize, ToSchema)]
pub struct FaucetRequestResponse {
    /// Whether the request was successful
    pub success: bool,

    /// USDC transaction hash (if successful)
    pub usdc_transaction_hash: Option<String>,

    /// ETH transaction hash (if successful)
    pub eth_transaction_hash: Option<String>,

    /// USDC amount sent (in USDC)
    pub usdc_amount: Option<String>,

    /// ETH amount sent (in ETH)
    pub eth_amount: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Message for the user
    pub message: String,
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
                eth_amount_per_request: None,
                cooldown_seconds: None,
                rpc_url: None,
                chain_id: None,
                network_config: None,
                token_config: None,
            }));
    }

    // Use centralized deployment service for local/staging
    let deployment = match get_deployment() {
        Ok(deployment) => deployment,
        Err(e) => {
            match e {
                DeploymentError::FileRead { ref file, .. } => {
                    warn!("Failed to read deployment file {}: {}", file, e);
                }
                _ => {
                    warn!("Deployment service error: {}", e);
                }
            }
        return Ok(Json(FaucetInfoResponse {
            available: false,
            environment,
            faucet_address: None,
            usdc_address: None,
            amount_per_request: None,
            eth_amount_per_request: None,
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
            token_config: None,
        }));
        }
    };

    let faucet_address = Some(deployment.config.faucet.clone());
    let usdc_address = Some(deployment.config.usdc.clone());
    let chain_id = deployment.config.chain_id.to_string();

    info!(
        "✅ Faucet info loaded from deployment service: address={:?}, chain_id={}",
        faucet_address, chain_id
    );

    // Determine RPC URL based on environment and deployment context
    // Check for Kubernetes-specific env var to detect cluster environment
    let in_kubernetes = std::env::var("KUBERNETES_SERVICE_HOST").is_ok();

    info!(
        "Faucet RPC URL detection - environment: {}, in_kubernetes: {}",
        environment, in_kubernetes
    );

    // Get the Anvil RPC URL for the given environment
    // In k8s, check for custom env var first, then use defaults
    let rpc_url = if in_kubernetes {
        if environment == "staging" {
            // For staging in k8s, ANVIL_RPC_URL must be set to an externally accessible URL
            // (LoadBalancer IP, Ingress domain, or NodePort accessible from browser)
            // Service name follows pattern: {release-name}-anvil:8545
            let url = std::env::var("ANVIL_RPC_URL")
                .unwrap_or_else(|_| {
                    warn!("ANVIL_RPC_URL not set for staging k8s deployment. Using internal service DNS which won't work from browser. Set server.anvilRpcUrl in Helm values.");
                    "http://arda-credit-anvil:8545".to_string()
                });
            url
        } else {
            // Local k8s - NodePort mapped to host 8545 for browser access
            "http://localhost:8545".to_string()
        }
    } else {
        // Running directly (not in k8s) - standard Anvil port
        "http://127.0.0.1:8545".to_string()
    };

    let network_name = match environment.as_str() {
        "local" => "Local Anvil Devnet",
        "staging" => "Staging Anvil Devnet",
        _ => "Anvil Devnet",
    };

    // Create USDC token config for MetaMask import
    let token_config = if let Some(ref usdc_addr) = usdc_address {
        Some(TokenConfig {
            address: usdc_addr.clone(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            metamask_import_url: format!(
                "https://metamask.app/add-token/?address={}&symbol=USDC&decimals=6&name=USD%20Coin",
                usdc_addr
            ),
        })
    } else {
        None
    };

    Ok(Json(FaucetInfoResponse {
        available: true,
        environment: environment.clone(),
        faucet_address: faucet_address.clone(),
        usdc_address: usdc_address.clone(),
        amount_per_request: Some("1000000".to_string()), // 1M USDC
        eth_amount_per_request: Some("0.5".to_string()), // 0.5 ETH for gas
        cooldown_seconds: Some(3600),                    // 1 hour
        rpc_url: Some(rpc_url.clone()),
        chain_id: Some(chain_id.clone()),
        network_config: Some(NetworkConfig {
            network_name: network_name.to_string(),
            rpc_url,
            chain_id,
            currency_symbol: "ETH".to_string(),
            block_explorer_url: None,
        }),
        token_config,
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

    // Use centralized deployment service
    let _deployment = match get_deployment() {
        Ok(deployment) => deployment,
        Err(e) => {
            warn!("Deployment service error: {}", e);
            return Ok(Json(FaucetStatusResponse {
                can_request_now: false,
                time_until_next_request: 0,
                user_balance: None,
                faucet_balance: None,
                error: Some(
                    "Deployment not available. Run 'make deploy-contracts' first.".to_string(),
                ),
            }));
        }
    };

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

/// Request USDC from faucet (server-side signing)
///
/// Server signs the transaction with the deployer key and sends funds to the requested address.
/// This allows users to receive funds without needing ETH for gas.
#[utoipa::path(
    post,
    path = "/api/v1/faucet/request",
    tag = "Dev",
    request_body = FaucetRequestBody,
    responses(
        (status = 200, description = "Faucet request processed", body = FaucetRequestResponse),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Faucet not available in this environment"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn request_faucet_funds(
    State(_state): State<crate::config::types::ApiState>,
    axum::extract::Json(body): axum::extract::Json<FaucetRequestBody>,
) -> Result<Json<FaucetRequestResponse>, StatusCode> {
    info!("💰 Faucet request for address: {}", body.address);

    // Validate address format
    if !body.address.starts_with("0x") || body.address.len() != 42 {
        warn!("Invalid address format: {}", body.address);
        return Ok(Json(FaucetRequestResponse {
            success: false,
            usdc_transaction_hash: None,
            eth_transaction_hash: None,
            usdc_amount: None,
            eth_amount: None,
            error: Some("Invalid Ethereum address format".to_string()),
            message: "Please provide a valid Ethereum address (0x... format)".to_string(),
        }));
    }

    // Check environment
    let environment = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    if !matches!(environment.as_str(), "local" | "staging") {
        warn!("Faucet request denied - not in local/staging environment");
        return Ok(Json(FaucetRequestResponse {
            success: false,
            usdc_transaction_hash: None,
            eth_transaction_hash: None,
            usdc_amount: None,
            eth_amount: None,
            error: Some("Faucet only available in local/staging environments".to_string()),
            message: "This faucet is only available for development and testing".to_string(),
        }));
    }

    // Load deployment info
    let deployment_file = format!("contracts/deployments/{}-deployment.json", environment);
    let deployment_data = match std::fs::read_to_string(&deployment_file) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to read deployment file: {}", e);
            return Ok(Json(FaucetRequestResponse {
                success: false,
                usdc_transaction_hash: None,
                eth_transaction_hash: None,
                usdc_amount: None,
                eth_amount: None,
                error: Some("Faucet contract not deployed".to_string()),
                message: "Please contact administrator to deploy faucet contract".to_string(),
            }));
        }
    };

    let deployment: serde_json::Value = match serde_json::from_str(&deployment_data) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to parse deployment JSON: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let faucet_address = match deployment["faucet"].as_str() {
        Some(addr) => addr,
        None => {
            error!("Faucet address not found in deployment file");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get RPC URL and private key from environment
    let rpc_url =
        std::env::var("ETHEREUM_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());

    let private_key = std::env::var("ETHEREUM_PRIVATE_KEY")
        .or_else(|_| std::env::var("ETHEREUM_WALLET_PRIVATE_KEY"))
        .map_err(|_| {
            error!("ETHEREUM_PRIVATE_KEY not set in environment");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(
        "📡 Step 1: Sending 0.5 ETH to {} for gas fees",
        body.address
    );

    // Step 1: Send ETH for gas (0.5 ETH should be plenty)
    let eth_amount = "0.5ether"; // Using ether unit for clarity
    let eth_output = std::process::Command::new("cast")
        .args(&[
            "send",
            &body.address,
            "--value",
            eth_amount,
            "--rpc-url",
            &rpc_url,
            "--private-key",
            &private_key,
            "--json",
        ])
        .output()
        .map_err(|e| {
            error!("Failed to execute cast command for ETH: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !eth_output.status.success() {
        let stderr = String::from_utf8_lossy(&eth_output.stderr);
        error!("ETH transfer failed: {}", stderr);
        return Ok(Json(FaucetRequestResponse {
            success: false,
            usdc_transaction_hash: None,
            eth_transaction_hash: None,
            usdc_amount: None,
            eth_amount: None,
            error: Some("Failed to send ETH for gas fees".to_string()),
            message: format!("ETH transfer failed: {}", stderr),
        }));
    }

    let eth_stdout = String::from_utf8_lossy(&eth_output.stdout);
    let eth_tx_hash = if let Some(start_pos) = eth_stdout.rfind('{') {
        let json_str = &eth_stdout[start_pos..];
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(tx_result) => tx_result["transactionHash"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            Err(e) => {
                warn!("Failed to parse ETH transaction receipt: {}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    info!(
        "✅ Step 1 complete: Sent 0.5 ETH - tx: {}",
        if eth_tx_hash.is_empty() {
            "pending"
        } else {
            &eth_tx_hash
        }
    );

    info!(
        "📡 Step 2: Calling faucet contract at {} for USDC",
        faucet_address
    );

    // Step 2: Send USDC via faucet contract
    // Using requestFundsImmediate which bypasses cooldown (owner only)
    let usdc_output = std::process::Command::new("cast")
        .args(&[
            "send",
            faucet_address,
            "requestFundsImmediate(address)",
            &body.address,
            "--rpc-url",
            &rpc_url,
            "--private-key",
            &private_key,
            "--json",
        ])
        .output()
        .map_err(|e| {
            error!("Failed to execute cast command for USDC: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !usdc_output.status.success() {
        let stderr = String::from_utf8_lossy(&usdc_output.stderr);
        error!("USDC faucet call failed: {}", stderr);

        // Parse error message for user-friendly response
        let error_msg = if stderr.contains("CooldownNotElapsed") {
            "You must wait 1 hour between faucet requests".to_string()
        } else if stderr.contains("InsufficientFaucetBalance") {
            "Faucet is empty. Please contact administrator".to_string()
        } else if stderr.contains("FaucetDisabled") {
            "Faucet is currently disabled".to_string()
        } else {
            "USDC transaction failed. Please try again later".to_string()
        };

        // Note: User already received ETH, but USDC failed
        return Ok(Json(FaucetRequestResponse {
            success: false,
            usdc_transaction_hash: None,
            eth_transaction_hash: Some(eth_tx_hash),
            usdc_amount: None,
            eth_amount: Some("0.5".to_string()),
            error: Some(error_msg.clone()),
            message: format!("⚠️ Sent 0.5 ETH but USDC failed: {}", error_msg),
        }));
    }

    let usdc_stdout = String::from_utf8_lossy(&usdc_output.stdout);
    let usdc_tx_hash = if let Some(start_pos) = usdc_stdout.rfind('{') {
        let json_str = &usdc_stdout[start_pos..];
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(tx_result) => tx_result["transactionHash"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            Err(e) => {
                warn!("Failed to parse USDC transaction receipt: {}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    info!(
        "✅ Step 2 complete: Sent 1,000,000 USDC - tx: {}",
        if usdc_tx_hash.is_empty() {
            "pending"
        } else {
            &usdc_tx_hash
        }
    );

    Ok(Json(FaucetRequestResponse {
        success: true,
        usdc_transaction_hash: Some(usdc_tx_hash.clone()),
        eth_transaction_hash: Some(eth_tx_hash.clone()),
        usdc_amount: Some("1000000".to_string()), // 1M USDC
        eth_amount: Some("0.5".to_string()),      // 0.5 ETH
        error: None,
        message: format!(
            "✅ Successfully sent:\n• 0.5 ETH (for gas): {}\n• 1,000,000 USDC: {}",
            if eth_tx_hash.is_empty() {
                "pending"
            } else {
                &eth_tx_hash
            },
            if usdc_tx_hash.is_empty() {
                "pending"
            } else {
                &usdc_tx_hash
            }
        ),
    }))
}
