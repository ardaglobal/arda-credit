//! Consolidated balance handlers for the Arda Credit API
//!
//! This module provides unified balance endpoints that return comprehensive
//! balance information including available balance, total deposited, and
//! future support for pending deposits/withdrawals and locked funds.
//!
//! Endpoints:
//! - GET /api/v1/user/{user_id}/balance - Get balances for all addresses associated with user
//! - GET /api/v1/user/{address}/balance - Get balance for specific address

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::config::types::{ApiState, ErrorResponse};
use arda_credit_db::{get_identity_by_id};

// ============================================================================
// RESPONSE STRUCTURES
// ============================================================================

/// Comprehensive balance information for a single address
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddressBalance {
    pub address: String,
    #[schema(value_type = String, example = "5000000")]
    pub available_balance: Decimal,
    #[schema(value_type = String, example = "5.000000")]
    pub available_balance_usdc: Decimal,
    #[schema(value_type = String, example = "5000000")]
    pub total_deposited: Decimal,
    #[schema(value_type = String, example = "5.000000")]
    pub total_deposited_usdc: Decimal,
    pub deposit_count: i32,
    // Future fields for comprehensive balance info
    #[schema(value_type = String, example = "0")]
    pub pending_deposits: Option<Decimal>,
    #[schema(value_type = String, example = "0")]
    pub pending_withdrawals: Option<Decimal>,
    #[schema(value_type = String, example = "0")]
    pub locked_funds: Option<Decimal>,
}

/// Balance response for user ID endpoint (supports multiple addresses)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserBalanceResponse {
    pub user_id: String,
    pub balances: Vec<AddressBalance>,
    /// Total balance across all addresses
    #[schema(value_type = String, example = "10000000")]
    pub total_available_balance: Decimal,
    #[schema(value_type = String, example = "10.000000")]
    pub total_available_balance_usdc: Decimal,
    pub currency: String,
}

/// Balance response for address endpoint (single address)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddressBalanceResponse {
    pub balance: AddressBalance,
    pub currency: String,
}

// ============================================================================
// SERVICE FUNCTIONS
// ============================================================================

/// Get comprehensive balance for a specific address
pub async fn get_balance_for_address(
    pool: &sqlx::PgPool,
    address: &str,
) -> Result<Option<AddressBalance>, Box<dyn std::error::Error + Send + Sync>> {
    // Use the existing balance service logic but return our new structure
    let balance_result = arda_credit_db::get_user_available_balance(pool, address).await;

    match balance_result {
        Ok(Some(balance)) => {
            // Get the original deposit balance to show total deposited
            let original_deposit = arda_credit_db::get_user_deposit_balance(pool, address).await?;
            let total_deposited = original_deposit
                .as_ref()
                .map(|d| d.total_deposited)
                .unwrap_or(Decimal::ZERO);
            let total_deposited_usdc = original_deposit
                .as_ref()
                .and_then(|d| d.total_deposited_usdc)
                .unwrap_or(Decimal::ZERO);

            Ok(Some(AddressBalance {
                address: balance.user_address.clone(),
                available_balance: balance.total_deposited, // Available after subtracting pending withdrawals
                available_balance_usdc: balance.total_deposited_usdc.unwrap_or(Decimal::ZERO),
                total_deposited,
                total_deposited_usdc,
                deposit_count: balance.deposit_count,
                // Future fields - currently None but ready for implementation
                pending_deposits: Some(Decimal::ZERO),
                pending_withdrawals: Some(Decimal::ZERO),
                locked_funds: Some(Decimal::ZERO),
            }))
        }
        Ok(None) => {
            // Return zero balance for valid address with no deposit history
            Ok(Some(AddressBalance {
                address: address.to_string(),
                available_balance: Decimal::ZERO,
                available_balance_usdc: Decimal::ZERO,
                total_deposited: Decimal::ZERO,
                total_deposited_usdc: Decimal::ZERO,
                deposit_count: 0,
                pending_deposits: Some(Decimal::ZERO),
                pending_withdrawals: Some(Decimal::ZERO),
                locked_funds: Some(Decimal::ZERO),
            }))
        }
        Err(e) => Err(e.into()),
    }
}

/// Get balances for all addresses associated with a user ID
pub async fn get_balances_for_user_id(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Vec<AddressBalance>, Box<dyn std::error::Error + Send + Sync>> {
    // Get user's identity to find associated addresses
    let user_opt = get_identity_by_id(pool, user_id).await?;

    match user_opt {
        Some(user) => {
            if let Some(wallet_address) = user.wallet_address {
                // Currently users have single address, but this structure supports multiple
                match get_balance_for_address(pool, &wallet_address).await? {
                    Some(balance) => Ok(vec![balance]),
                    None => Ok(vec![]),
                }
            } else {
                // User has no linked wallet - return empty array
                Ok(vec![])
            }
        }
        None => Ok(vec![]), // User not found - return empty array
    }
}

// ============================================================================
// ENDPOINT HANDLERS
// ============================================================================

/// Get balance for specific address
#[utoipa::path(
    get,
    path = "/api/v1/address/{address}/balance",
    params(
        ("address" = String, Path, description = "Wallet address")
    ),
    responses(
        (status = 200, description = "Balance retrieved successfully", body = AddressBalanceResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Balance"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_balance_by_address_endpoint(
    Path(address): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<AddressBalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔍 API: Getting balance for address: {}", address);

    match get_balance_for_address(&state.pool, &address).await {
        Ok(Some(balance)) => {
            info!("✅ API: Balance retrieved successfully for address: {}", address);
            Ok(Json(AddressBalanceResponse {
                balance,
                currency: "USDC".to_string(),
            }))
        }
        Ok(None) => {
            // This shouldn't happen with our current implementation, but handle gracefully
            error!("❌ API: Unexpected None result for address: {}", address);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "balance_error",
                    "Failed to retrieve balance",
                )),
            ))
        }
        Err(e) => {
            error!("❌ API: Failed to get balance for address {}: {:?}", address, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "balance_error",
                    "Failed to retrieve balance",
                )),
            ))
        }
    }
}

/// Get balances for all addresses associated with user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/balance",
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    responses(
        (status = 200, description = "User balances retrieved successfully", body = UserBalanceResponse),
        (status = 400, description = "Invalid user ID format", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Balance"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_balance_by_user_id_endpoint(
    Path(user_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔍 API: Getting balances for user ID: {}", user_id);

    // Parse user ID
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            warn!("⚠️ API: Invalid user ID format: {}", user_id);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_user_id",
                    "Invalid user ID format",
                )),
            ));
        }
    };

    // Check if user exists
    match get_identity_by_id(&state.pool, user_uuid).await {
        Ok(Some(_)) => {
            // User exists, get balances
            match get_balances_for_user_id(&state.pool, user_uuid).await {
                Ok(balances) => {
                    // Calculate totals
                    let total_available_balance: Decimal = balances
                        .iter()
                        .map(|b| b.available_balance)
                        .sum();
                    let total_available_balance_usdc: Decimal = balances
                        .iter()
                        .map(|b| b.available_balance_usdc)
                        .sum();

                    info!("✅ API: Balances retrieved successfully for user: {} ({} addresses)", 
                          user_id, balances.len());
                    
                    Ok(Json(UserBalanceResponse {
                        user_id,
                        balances,
                        total_available_balance,
                        total_available_balance_usdc,
                        currency: "USDC".to_string(),
                    }))
                }
                Err(e) => {
                    error!("❌ API: Failed to get balances for user {}: {:?}", user_id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::new(
                            "balance_error",
                            "Failed to retrieve user balances",
                        )),
                    ))
                }
            }
        }
        Ok(None) => {
            warn!("⚠️ API: User not found: {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("user_not_found", "User not found")),
            ))
        }
        Err(e) => {
            error!("❌ API: Failed to check user existence: {}: {:?}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "database_error",
                    "Failed to retrieve user",
                )),
            ))
        }
    }
}

/// Helper function for parsing user ID (legacy compatibility)
#[allow(dead_code)]
fn parse_user_id(user_id: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    uuid::Uuid::parse_str(user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid user ID format: {}", user_id),
        )
    })
}
