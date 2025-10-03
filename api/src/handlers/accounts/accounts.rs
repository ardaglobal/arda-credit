//! Account management API handlers
//!
//! This module provides account-centric endpoints for the new balance tracking system.
//! All financial operations are now keyed by account_id rather than raw addresses.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use arda_credit_db::{
    get_account_by_address, get_account_by_id, get_accounts_by_org_id, get_accounts_by_user_id,
    get_balance, get_balances_by_org_id, get_balances_by_user_id, get_balances_for_account,
    get_deposits_by_account_id, get_withdrawals_by_account_id, Account, Balance, Deposit,
    Withdrawal,
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AccountLookupQuery {
    /// Lookup account by Ethereum address
    pub address: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountLookupResponse {
    pub accounts: Vec<Account>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserAccountsResponse {
    pub user_id: Uuid,
    pub accounts: Vec<Account>,
    pub total_count: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountBalanceResponse {
    pub account_id: Uuid,
    pub account_label: String,
    pub account_number: String,
    pub account_status: String,
    pub balance: BalanceInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceInfo {
    pub currency: String,
    pub available_balance: String,
    pub available_balance_usdc: String,
    pub locked_balance: String,
    pub locked_balance_usdc: String,
    pub total_deposited: String,
    pub total_deposited_usdc: String,
    pub total_withdrawn: String,
    pub total_withdrawn_usdc: String,
    pub deposit_count: i32,
    pub withdrawal_count: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserBalancesResponse {
    pub user_id: Uuid,
    pub accounts: Vec<AccountWithBalances>,
    pub total_available_usdc: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountWithBalances {
    pub account_id: Uuid,
    pub account_label: String,
    pub account_number: String,
    pub account_status: String,
    pub balances: Vec<BalanceInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountDepositsResponse {
    pub account_id: Uuid,
    pub deposits: Vec<DepositInfo>,
    pub total_count: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DepositInfo {
    pub id: i32,
    pub amount_usdc: String,
    pub currency: String,
    pub transaction_hash: String,
    pub block_number: i64,
    pub status: String,
    pub confirmation_count: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountWithdrawalsResponse {
    pub account_id: Uuid,
    pub withdrawals: Vec<WithdrawalInfo>,
    pub total_count: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WithdrawalInfo {
    pub id: i32,
    pub recipient_address: String,
    pub amount_usdc: String,
    pub currency: String,
    pub state: String,
    pub transaction_hash: Option<String>,
    pub created_at: String,
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Lookup account by Ethereum address
#[utoipa::path(
    get,
    path = "/api/v1/accounts",
    params(AccountLookupQuery),
    responses(
        (status = 200, description = "Account found", body = AccountLookupResponse),
        (status = 400, description = "Missing address parameter"),
        (status = 404, description = "No account found for address")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn lookup_account(
    Query(params): Query<AccountLookupQuery>,
    State(state): State<ApiState>,
) -> Result<Json<AccountLookupResponse>, StatusCode> {
    if let Some(address) = params.address {
        info!("🔍 Looking up account by address: {}", address);

        let account = get_account_by_address(&state.pool, &address)
            .await
            .map_err(|e| {
                error!("Failed to lookup account: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        Ok(Json(AccountLookupResponse {
            accounts: account.into_iter().collect(),
        }))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// Get account details by ID
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account details", body = Account),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<Account>, StatusCode> {
    info!("Getting account: {}", account_id);

    let account = get_account_by_id(&state.pool, account_id)
        .await
        .map_err(|e| {
            error!("Failed to get account: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(account))
}

/// Get all accounts for a user
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/accounts",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User accounts", body = UserAccountsResponse),
        (status = 404, description = "User not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_user_accounts(
    Path(user_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<UserAccountsResponse>, StatusCode> {
    info!("Getting accounts for user: {}", user_id);

    let accounts = get_accounts_by_user_id(&state.pool, user_id)
        .await
        .map_err(|e| {
            error!("Failed to get user accounts: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total_count = accounts.len();

    Ok(Json(UserAccountsResponse {
        user_id,
        accounts,
        total_count,
    }))
}

/// Get account balance
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/balance",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account balance", body = AccountBalanceResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_balance_endpoint(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<AccountBalanceResponse>, StatusCode> {
    info!("💰 Getting balance for account: {}", account_id);

    // Get account details
    let account = get_account_by_id(&state.pool, account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get balance (default currency from account)
    let balance = get_balance(&state.pool, account_id, &account.currency)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_else(|| {
            // Return zero balance if not found
            Balance {
                account_id,
                currency: account.currency.clone(),
                available_balance: Decimal::ZERO,
                locked_balance: Decimal::ZERO,
                total_deposited: Decimal::ZERO,
                total_withdrawn: Decimal::ZERO,
                deposit_count: 0,
                withdrawal_count: 0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                first_deposit_at: None,
                last_deposit_at: None,
                first_withdrawal_at: None,
                last_withdrawal_at: None,
            }
        });

    // Convert to human-readable (USDC has 6 decimals)
    let divisor = Decimal::new(1_000_000, 0);

    Ok(Json(AccountBalanceResponse {
        account_id: account.account_id,
        account_label: account.account_label,
        account_number: account.account_number,
        account_status: account.status,
        balance: BalanceInfo {
            currency: balance.currency.clone(),
            available_balance: balance.available_balance.to_string(),
            available_balance_usdc: (balance.available_balance / divisor).to_string(),
            locked_balance: balance.locked_balance.to_string(),
            locked_balance_usdc: (balance.locked_balance / divisor).to_string(),
            total_deposited: balance.total_deposited.to_string(),
            total_deposited_usdc: (balance.total_deposited / divisor).to_string(),
            total_withdrawn: balance.total_withdrawn.to_string(),
            total_withdrawn_usdc: (balance.total_withdrawn / divisor).to_string(),
            deposit_count: balance.deposit_count,
            withdrawal_count: balance.withdrawal_count,
        },
    }))
}

/// Get balances for all user accounts
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/balances",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User balances across all accounts", body = UserBalancesResponse),
        (status = 404, description = "User not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_user_balances(
    Path(user_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalancesResponse>, StatusCode> {
    info!("Getting balances for user: {}", user_id);

    // Get all user accounts
    let accounts = get_accounts_by_user_id(&state.pool, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let divisor = Decimal::new(1_000_000, 0);
    let mut total_available = Decimal::ZERO;
    let mut account_balances = Vec::new();

    for account in accounts {
        let balances = get_balances_for_account(&state.pool, account.account_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let balance_infos: Vec<BalanceInfo> = balances
            .iter()
            .map(|b| {
                total_available += b.available_balance;
                BalanceInfo {
                    currency: b.currency.clone(),
                    available_balance: b.available_balance.to_string(),
                    available_balance_usdc: (b.available_balance / divisor).to_string(),
                    locked_balance: b.locked_balance.to_string(),
                    locked_balance_usdc: (b.locked_balance / divisor).to_string(),
                    total_deposited: b.total_deposited.to_string(),
                    total_deposited_usdc: (b.total_deposited / divisor).to_string(),
                    total_withdrawn: b.total_withdrawn.to_string(),
                    total_withdrawn_usdc: (b.total_withdrawn / divisor).to_string(),
                    deposit_count: b.deposit_count,
                    withdrawal_count: b.withdrawal_count,
                }
            })
            .collect();

        account_balances.push(AccountWithBalances {
            account_id: account.account_id,
            account_label: account.account_label,
            account_number: account.account_number,
            account_status: account.status,
            balances: balance_infos,
        });
    }

    Ok(Json(UserBalancesResponse {
        user_id,
        accounts: account_balances,
        total_available_usdc: (total_available / divisor).to_string(),
    }))
}

/// Get deposits for account
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/deposits",
    params(
        ("account_id" = Uuid, Path, description = "Account ID"),
        ("limit" = Option<i32>, Query, description = "Max results"),
        ("offset" = Option<i32>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Account deposits", body = AccountDepositsResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_deposits_endpoint(
    Path(account_id): Path<Uuid>,
    Query(params): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<AccountDepositsResponse>, StatusCode> {
    info!("Getting deposits for account: {}", account_id);

    let deposits = get_deposits_by_account_id(&state.pool, account_id, None)
        .await
        .map_err(|e| {
            error!("Failed to get deposits: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let divisor = Decimal::new(1_000_000, 0);
    let deposit_infos: Vec<DepositInfo> = deposits
        .iter()
        .map(|d| DepositInfo {
            id: d.id,
            amount_usdc: (d.amount / divisor).to_string(),
            currency: d.currency.clone(),
            transaction_hash: d.transaction_hash.clone(),
            block_number: d.block_number,
            status: d.status.clone(),
            confirmation_count: d.confirmation_count,
            created_at: d.created_at.to_rfc3339(),
        })
        .collect();

    let total_count = deposit_infos.len();
    let has_more = false; // TODO: Implement pagination

    Ok(Json(AccountDepositsResponse {
        account_id,
        deposits: deposit_infos,
        total_count,
        has_more,
    }))
}

/// Get withdrawals for account
#[utoipa::path(
    get,
    path = "/api/v1/account/{account_id}/withdrawals",
    params(
        ("account_id" = Uuid, Path, description = "Account ID")
    ),
    responses(
        (status = 200, description = "Account withdrawals", body = AccountWithdrawalsResponse),
        (status = 404, description = "Account not found")
    ),
    tag = "Accounts"
)]
#[instrument(skip(state))]
pub async fn get_account_withdrawals_endpoint(
    Path(account_id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<AccountWithdrawalsResponse>, StatusCode> {
    info!("Getting withdrawals for account: {}", account_id);

    let withdrawals = get_withdrawals_by_account_id(&state.pool, account_id, None)
        .await
        .map_err(|e| {
            error!("Failed to get withdrawals: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let divisor = Decimal::new(1_000_000, 0);
    let withdrawal_infos: Vec<WithdrawalInfo> = withdrawals
        .iter()
        .map(|w| WithdrawalInfo {
            id: w.id,
            recipient_address: w.recipient_address.clone(),
            amount_usdc: (w.amount / divisor).to_string(),
            currency: w.currency.clone(),
            state: w.state.clone(),
            transaction_hash: w.transaction_hash.clone(),
            created_at: w.created_at.to_rfc3339(),
        })
        .collect();

    let total_count = withdrawal_infos.len();
    let has_more = false; // TODO: Implement pagination

    Ok(Json(AccountWithdrawalsResponse {
        account_id,
        withdrawals: withdrawal_infos,
        total_count,
        has_more,
    }))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PaginationQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
