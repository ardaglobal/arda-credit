use alloy_primitives::FixedBytes;
use ethereum_client::{Config, EthereumClient, Result};
use std::str::FromStr;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Load configuration from environment
    let config = Config::from_env()?;

    // Create ethereum client
    let client = EthereumClient::new_without_validation(config).await?;

    // Example state ID (you would use a real state ID in practice)
    let state_id =
        FixedBytes::from_str("0x1234567890123456789012345678901234567890123456789012345678901234")
            .unwrap_or_default();

    info!(
        "Querying batch public values for state ID: 0x{}",
        hex::encode(state_id.as_slice())
    );

    // Query previous total balance
    match client.get_previous_total_balance(state_id).await {
        Ok(previous_balance) => {
            if previous_balance == 0 {
                info!("No previous total balance found (returns 0)");
            } else {
                info!("Previous total balance: {}", previous_balance);
            }
        }
        Err(e) => {
            error!("Error getting previous total balance: {e}");
        }
    }

    // Query current total balance
    match client.get_current_total_balance(state_id).await {
        Ok(current_balance) => {
            if current_balance == 0 {
                info!("No current total balance found (returns 0)");
            } else {
                info!("Current total balance: {}", current_balance);
            }
        }
        Err(e) => {
            error!("Error getting current total balance: {e}");
        }
    }

    // Query previous state root from batch
    match client.get_previous_state_root_from_batch(state_id).await {
        Ok(previous_root) => {
            if previous_root == FixedBytes::ZERO {
                info!("No previous state root from batch found (returns zero)");
            } else {
                info!(
                    "Previous state root from batch: 0x{}",
                    hex::encode(previous_root.as_slice())
                );
            }
        }
        Err(e) => {
            error!("Error getting previous state root from batch: {e}");
        }
    }

    // Query new state root from batch
    match client.get_new_state_root_from_batch(state_id).await {
        Ok(new_root) => {
            if new_root == FixedBytes::ZERO {
                info!("No new state root from batch found (returns zero)");
            } else {
                info!(
                    "New state root from batch: 0x{}",
                    hex::encode(new_root.as_slice())
                );
            }
        }
        Err(e) => {
            error!("Error getting new state root from batch: {e}");
        }
    }

    // Compare with existing methods for contrast
    info!("Comparing with existing state root methods...");

    // Query previous state root (from history)
    match client.get_previous_state_root(state_id).await {
        Ok(previous_root_history) => {
            if previous_root_history == FixedBytes::ZERO {
                info!("No previous state root from history found (returns zero)");
            } else {
                info!(
                    "Previous state root from history: 0x{}",
                    hex::encode(previous_root_history.as_slice())
                );
            }
        }
        Err(e) => {
            error!("Error getting previous state root from history: {e}");
        }
    }

    // Query current state
    match client.get_current_state(state_id).await {
        Ok(Some(current_state)) => {
            info!(
                "Current state root: 0x{}",
                hex::encode(current_state.state_root.as_slice())
            );
        }
        Ok(None) => {
            info!("No current state found");
        }
        Err(e) => {
            error!("Error getting current state: {e}");
        }
    }

    info!("Batch public values query demo completed.");
    Ok(())
}
