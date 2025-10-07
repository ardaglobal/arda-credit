///! Wallet Address DTOs
///!
///! This module provides DTOs for company wallet address operations.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to create a new wallet address
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateWalletAddressRequest {
    #[validate(length(equal = 42))]
    #[schema(example = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb")]
    pub wallet_address: String,

    #[validate(length(max = 100))]
    #[schema(example = "Treasury Wallet")]
    pub address_label: Option<String>,

    #[schema(example = "hot_wallet")]
    pub address_type: String, // hot_wallet, cold_storage, multisig, smart_contract

    #[schema(example = true)]
    pub is_primary: bool,

    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to update a wallet address
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateWalletAddressRequest {
    #[validate(length(max = 100))]
    #[schema(example = "Treasury Wallet")]
    pub address_label: Option<String>,

    #[schema(example = "hot_wallet")]
    pub address_type: Option<String>,

    #[schema(example = "active")]
    pub status: Option<String>, // active, suspended, archived

    pub is_primary: Option<bool>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Wallet address response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WalletAddressResponse {
    pub id: Uuid,
    pub company_id: Uuid,

    #[schema(example = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb")]
    pub wallet_address: String,

    #[schema(example = "Treasury Wallet")]
    pub address_label: Option<String>,

    #[schema(example = "hot_wallet")]
    pub address_type: String,

    #[schema(example = "active")]
    pub status: String,

    pub is_primary: bool,

    #[schema(example = "1000000000")]
    pub available_balance: String, // wei

    #[schema(example = "0")]
    pub pending_balance: String, // wei

    #[schema(example = "5000000000")]
    pub lifetime_deposits: String, // wei

    #[schema(example = "500000000")]
    pub lifetime_withdrawals: String, // wei

    pub notes: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User's accessible wallet address (with permissions)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UserWalletAddressResponse {
    pub wallet_address_id: Uuid,

    #[schema(example = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb")]
    pub wallet_address: String,

    #[schema(example = "Treasury Wallet")]
    pub address_label: Option<String>,

    #[schema(example = "1000000000")]
    pub available_balance: String, // wei

    pub permissions: Vec<String>, // Array of permission types
}

/// Request to grant wallet permission
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct GrantWalletPermissionRequest {
    pub wallet_address_id: Option<Uuid>, // None = all company wallets

    pub membership_id: Uuid,

    #[schema(example = "view_balance")]
    pub permission_type: String, // view_balance, initiate_withdrawal, etc.

    pub daily_limit: Option<String>, // wei, for withdrawal limits

    #[schema(example = false)]
    pub requires_approval: bool,

    pub expires_at: Option<DateTime<Utc>>,
}

/// Wallet permission response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WalletPermissionResponse {
    pub id: Uuid,
    pub wallet_address_id: Option<Uuid>,
    pub company_id: Uuid,
    pub membership_id: Uuid,

    #[schema(example = "view_balance")]
    pub permission_type: String,

    pub daily_limit: Option<String>, // wei
    pub requires_approval: bool,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Conversion helpers
impl From<arda_credit_db::business::wallet::CompanyWalletAddress> for WalletAddressResponse {
    fn from(wallet: arda_credit_db::business::wallet::CompanyWalletAddress) -> Self {
        Self {
            id: wallet.id,
            company_id: wallet.company_id,
            wallet_address: wallet.wallet_address,
            address_label: wallet.address_label,
            address_type: wallet.address_type.to_string(),
            status: wallet.status.to_string(),
            is_primary: wallet.is_primary,
            available_balance: wallet.available_balance.to_string(),
            pending_balance: wallet.pending_balance.to_string(),
            lifetime_deposits: wallet.lifetime_deposits.to_string(),
            lifetime_withdrawals: wallet.lifetime_withdrawals.to_string(),
            notes: wallet.notes,
            metadata: wallet.metadata,
            created_by: wallet.created_by,
            created_at: wallet.created_at,
            updated_at: wallet.updated_at,
        }
    }
}

impl From<arda_credit_db::business::wallet::UserWalletAddress> for UserWalletAddressResponse {
    fn from(wallet: arda_credit_db::business::wallet::UserWalletAddress) -> Self {
        Self {
            wallet_address_id: wallet.wallet_address_id,
            wallet_address: wallet.wallet_address,
            address_label: wallet.address_label,
            available_balance: wallet.available_balance.to_string(),
            permissions: wallet.permissions,
        }
    }
}

impl From<arda_credit_db::business::wallet::WalletAddressPermission> for WalletPermissionResponse {
    fn from(perm: arda_credit_db::business::wallet::WalletAddressPermission) -> Self {
        Self {
            id: perm.id,
            wallet_address_id: perm.wallet_address_id,
            company_id: perm.company_id,
            membership_id: perm.membership_id,
            permission_type: perm.permission_type.to_string(),
            daily_limit: perm.daily_limit.map(|d| d.to_string()),
            requires_approval: perm.requires_approval,
            granted_by: perm.granted_by,
            granted_at: perm.granted_at,
            expires_at: perm.expires_at,
            created_at: perm.created_at,
        }
    }
}
