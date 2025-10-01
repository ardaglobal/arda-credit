//! Contract configuration and real-time transaction update handlers
//!
//! This module provides endpoints for:
//! - Fetching Arda contract configuration dynamically
//! - Real-time transaction updates via Server-Sent Events (SSE)

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::api::router::ApiState;

// ============================================================================
// CONTRACT CONFIGURATION TYPES
// ============================================================================

/// Arda contract configuration response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContractConfigResponse {
    /// The Ethereum address of the Arda contract
    pub contract_address: String,
    /// The Ethereum network/chain ID (1 for mainnet, 11155111 for sepolia, etc.)
    pub chain_id: u64,
    /// RPC URL for the Ethereum network
    pub rpc_url: String,
    /// Block explorer base URL for transaction viewing
    pub block_explorer_url: String,
    /// Contract deployment block number (for filtering events)
    pub deployment_block: u64,
    /// Current API version
    pub api_version: String,
}

/// Query parameters for contract configuration
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ContractConfigQuery {
    /// Optional network parameter (defaults to configured network)
    pub network: Option<String>,
}

// ============================================================================
// REAL-TIME TRANSACTION UPDATE TYPES
// ============================================================================

/// Real-time transaction update event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionUpdateEvent {
    /// Event type (deposit_confirmed, withdrawal_processed, etc.)
    pub event_type: String,
    /// Transaction hash
    pub transaction_hash: String,
    /// User's Ethereum address
    pub user_address: String,
    /// Transaction amount in USDC (human readable)
    pub amount: String,
    /// Current confirmation count
    pub confirmations: u32,
    /// Required confirmations for finality
    pub required_confirmations: u32,
    /// Transaction status
    pub status: String,
    /// Block number where transaction was mined
    pub block_number: u64,
    /// Unix timestamp when event occurred
    pub timestamp: u64,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Query parameters for transaction updates SSE
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TransactionUpdatesQuery {
    /// Filter updates for specific user address
    pub user_address: Option<String>,
    /// Filter updates for specific event types (comma-separated)
    pub event_types: Option<String>,
    /// Only include events after this timestamp
    pub since: Option<u64>,
}

// ============================================================================
// CONTRACT CONFIGURATION HANDLER
// ============================================================================

/// Get Arda contract configuration
///
/// Returns the current Arda contract address, network information, and other
/// configuration details needed for frontend wallet integration.
#[utoipa::path(
    get,
    path = "/api/v1/contract/config",
    params(ContractConfigQuery),
    responses(
        (status = 200, description = "Contract configuration retrieved successfully", body = ContractConfigResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(_state))]
pub async fn get_contract_config(
    State(_state): State<ApiState>,
    Query(params): Query<ContractConfigQuery>,
) -> Result<Json<ContractConfigResponse>, StatusCode> {
    info!(
        "Getting contract configuration for network: {:?}",
        params.network
    );

    // Get configuration from environment variables
    let contract_address = std::env::var("ETHEREUM_CONTRACT_ADDRESS").map_err(|_| {
        error!("ETHEREUM_CONTRACT_ADDRESS not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rpc_url = std::env::var("ETHEREUM_RPC_URL").map_err(|_| {
        error!("ETHEREUM_RPC_URL not configured");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Determine chain ID and block explorer from RPC URL
    let (chain_id, block_explorer_url, deployment_block) = if rpc_url.contains("mainnet") {
        (1u64, "https://etherscan.io".to_string(), 18000000u64) // Mainnet
    } else if rpc_url.contains("sepolia") {
        (
            11155111u64,
            "https://sepolia.etherscan.io".to_string(),
            4000000u64,
        ) // Sepolia
    } else if rpc_url.contains("goerli") {
        (5u64, "https://goerli.etherscan.io".to_string(), 7500000u64) // Goerli
    } else {
        // Default to local development network
        (31337u64, "http://localhost:8545".to_string(), 0u64) // Anvil/Hardhat
    };

    let config = ContractConfigResponse {
        contract_address,
        chain_id,
        rpc_url,
        block_explorer_url,
        deployment_block,
        api_version: crate::config::paths::API_VERSION.to_string(),
    };

    info!(
        "Returning contract config: chain_id={}, address={}",
        config.chain_id, config.contract_address
    );

    Ok(Json(config))
}

// ============================================================================
// REAL-TIME TRANSACTION UPDATES HANDLER
// ============================================================================

/// Get latest transaction updates
///
/// Provides the latest transaction updates including deposit confirmations,
/// withdrawal processing, and other blockchain events. The frontend can poll this
/// endpoint to get updates about transaction status.
#[utoipa::path(
    get,
    path = "/api/v1/transactions/updates",
    params(TransactionUpdatesQuery),
    responses(
        (status = 200, description = "Latest transaction updates", body = Vec<TransactionUpdateEvent>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(_state))]
pub async fn get_transaction_updates(
    State(_state): State<ApiState>,
    Query(params): Query<TransactionUpdatesQuery>,
) -> Result<Json<Vec<TransactionUpdateEvent>>, StatusCode> {
    info!(
        "Getting transaction updates for user_address={:?}, event_types={:?}",
        params.user_address, params.event_types
    );

    // Parse event type filters
    let event_type_filters: Option<Vec<String>> = params
        .event_types
        .map(|types| types.split(',').map(|s| s.trim().to_string()).collect());

    // In a real implementation, this would:
    // 1. Query the database for recent transaction updates
    // 2. Check blockchain events from smart contract
    // 3. Filter based on user_address and event_types
    // 4. Return the latest updates since the provided timestamp

    // For now, return mock data
    let mock_updates =
        generate_mock_transaction_updates(&params.user_address, &event_type_filters, params.since);

    Ok(Json(mock_updates))
}

/// Generate mock transaction update events
fn generate_mock_transaction_updates(
    user_address: &Option<String>,
    event_type_filters: &Option<Vec<String>>,
    since: Option<u64>,
) -> Vec<TransactionUpdateEvent> {
    let mut events = Vec::new();
    let current_timestamp = chrono::Utc::now().timestamp() as u64;

    // Generate a few mock events
    for i in 1..=5 {
        let event_types = vec![
            "deposit_confirmed",
            "withdrawal_processed",
            "deposit_pending",
        ];
        let event_type = event_types[i % event_types.len()].to_string();

        // Apply event type filter
        if let Some(ref filters) = event_type_filters {
            if !filters.contains(&event_type) {
                continue;
            }
        }

        let timestamp = current_timestamp - (i as u64 * 300); // 5 minutes apart

        // Apply timestamp filter
        if let Some(since_timestamp) = since {
            if timestamp <= since_timestamp {
                continue;
            }
        }

        let mock_event = TransactionUpdateEvent {
            event_type,
            transaction_hash: format!("0x{:064x}", i),
            user_address: user_address
                .clone()
                .unwrap_or_else(|| "0x742E4C1D5B5B8Ac5dC12345678901234567890aB".to_string()),
            amount: format!("{}.{:02}", 100 + i, i * 25),
            confirmations: if i <= 2 { 12 } else { i as u32 * 2 },
            required_confirmations: 12,
            status: if i <= 2 {
                "confirmed".to_string()
            } else {
                "pending".to_string()
            },
            block_number: 18000000 + i as u64,
            timestamp,
            metadata: {
                let mut map = HashMap::new();
                map.insert(
                    "gas_used".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(21000)),
                );
                map.insert(
                    "gas_price".to_string(),
                    serde_json::Value::String("20 gwei".to_string()),
                );
                map.insert(
                    "block_confirmations".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(i)),
                );
                map
            },
        };

        events.push(mock_event);
    }

    events
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get network name from chain ID
pub fn get_network_name(chain_id: u64) -> &'static str {
    match chain_id {
        1 => "mainnet",
        5 => "goerli",
        11155111 => "sepolia",
        31337 => "localhost",
        _ => "unknown",
    }
}

/// Validate Ethereum address format
pub fn is_valid_ethereum_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("0x")
        && address.chars().skip(2).all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethereum_address_validation() {
        assert!(is_valid_ethereum_address(
            "0x742E4C1D5B5B8Ac5dC12345678901234567890aB"
        ));
        assert!(!is_valid_ethereum_address(
            "742E4C1D5B5B8Ac5dC12345678901234567890aB"
        )); // Missing 0x
        assert!(!is_valid_ethereum_address(
            "0x742E4C1D5B5B8Ac5dC12345678901234567890a"
        )); // Too short
        assert!(!is_valid_ethereum_address(
            "0x742E4C1D5B5B8Ac5dC12345678901234567890aBC"
        )); // Too long
    }

    #[test]
    fn test_network_names() {
        assert_eq!(get_network_name(1), "mainnet");
        assert_eq!(get_network_name(5), "goerli");
        assert_eq!(get_network_name(11155111), "sepolia");
        assert_eq!(get_network_name(31337), "localhost");
        assert_eq!(get_network_name(999999), "unknown");
    }
}
