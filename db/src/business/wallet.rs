///! Company Wallet Address Module
///!
///! This module provides types and functions for company-owned wallet addresses
///! with permission-based access control.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{DbError, DbResult};

// ============================================================================
// CORE TYPES
// ============================================================================

/// Company wallet address
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyWalletAddress {
    pub id: Uuid,
    pub company_id: Uuid,
    pub wallet_address: String, // 0x... Ethereum address
    pub address_label: Option<String>,
    pub address_type: WalletAddressType,
    pub status: WalletAddressStatus,
    pub is_primary: bool,
    pub available_balance: rust_decimal::Decimal,
    pub pending_balance: rust_decimal::Decimal,
    pub lifetime_deposits: rust_decimal::Decimal,
    pub lifetime_withdrawals: rust_decimal::Decimal,
    pub notes: Option<String>,
    #[sqlx(default)]
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Wallet address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "snake_case")]
pub enum WalletAddressType {
    HotWallet,
    ColdStorage,
    Multisig,
    SmartContract,
}

impl std::fmt::Display for WalletAddressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HotWallet => write!(f, "hot_wallet"),
            Self::ColdStorage => write!(f, "cold_storage"),
            Self::Multisig => write!(f, "multisig"),
            Self::SmartContract => write!(f, "smart_contract"),
        }
    }
}

impl std::str::FromStr for WalletAddressType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hot_wallet" => Ok(Self::HotWallet),
            "cold_storage" => Ok(Self::ColdStorage),
            "multisig" => Ok(Self::Multisig),
            "smart_contract" => Ok(Self::SmartContract),
            _ => Err(format!("Invalid wallet address type: {}", s)),
        }
    }
}

/// Wallet address status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum WalletAddressStatus {
    Active,
    Suspended,
    Archived,
}

impl std::fmt::Display for WalletAddressStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for WalletAddressStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("Invalid wallet address status: {}", s)),
        }
    }
}

/// Wallet permission type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "wallet_permission_type", rename_all = "snake_case")]
pub enum WalletPermissionType {
    ViewBalance,
    ViewTransactions,
    InitiateWithdrawal,
    ApproveWithdrawal,
    ManageAddress,
    AddAddress,
    ArchiveAddress,
}

impl std::fmt::Display for WalletPermissionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ViewBalance => write!(f, "view_balance"),
            Self::ViewTransactions => write!(f, "view_transactions"),
            Self::InitiateWithdrawal => write!(f, "initiate_withdrawal"),
            Self::ApproveWithdrawal => write!(f, "approve_withdrawal"),
            Self::ManageAddress => write!(f, "manage_address"),
            Self::AddAddress => write!(f, "add_address"),
            Self::ArchiveAddress => write!(f, "archive_address"),
        }
    }
}

impl std::str::FromStr for WalletPermissionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "view_balance" => Ok(Self::ViewBalance),
            "view_transactions" => Ok(Self::ViewTransactions),
            "initiate_withdrawal" => Ok(Self::InitiateWithdrawal),
            "approve_withdrawal" => Ok(Self::ApproveWithdrawal),
            "manage_address" => Ok(Self::ManageAddress),
            "add_address" => Ok(Self::AddAddress),
            "archive_address" => Ok(Self::ArchiveAddress),
            _ => Err(format!("Invalid wallet permission type: {}", s)),
        }
    }
}

/// Wallet address permission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletAddressPermission {
    pub id: Uuid,
    pub wallet_address_id: Option<Uuid>, // None means permission applies to all company wallets
    pub company_id: Uuid,
    pub membership_id: Uuid,
    pub permission_type: WalletPermissionType,
    pub daily_limit: Option<rust_decimal::Decimal>,
    pub requires_approval: bool,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// User's accessible wallet address (with permissions)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserWalletAddress {
    pub wallet_address_id: Uuid,
    pub wallet_address: String,
    pub address_label: Option<String>,
    pub available_balance: rust_decimal::Decimal,
    pub permissions: Vec<String>, // Array of permission types
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to create a new wallet address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWalletAddressRequest {
    pub company_id: Uuid,
    pub wallet_address: String,
    pub address_label: Option<String>,
    pub address_type: WalletAddressType,
    pub is_primary: bool,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to update a wallet address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWalletAddressRequest {
    pub address_label: Option<String>,
    pub address_type: Option<WalletAddressType>,
    pub status: Option<WalletAddressStatus>,
    pub is_primary: Option<bool>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to grant wallet permission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantWalletPermissionRequest {
    pub wallet_address_id: Option<Uuid>, // None = all company wallets
    pub membership_id: Uuid,
    pub permission_type: WalletPermissionType,
    pub daily_limit: Option<rust_decimal::Decimal>,
    pub requires_approval: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

// ============================================================================
// DATABASE OPERATIONS - WALLET ADDRESSES
// ============================================================================

/// Create a new company wallet address
pub async fn create_wallet_address(
    pool: &PgPool,
    request: &CreateWalletAddressRequest,
    created_by: Uuid,
) -> DbResult<CompanyWalletAddress> {
    let metadata = request
        .metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    // If this is primary, unset other primary addresses first
    if request.is_primary {
        sqlx::query(
            r#"
            UPDATE company_wallet_addresses
            SET is_primary = FALSE
            WHERE company_id = $1
            "#,
        )
        .bind(request.company_id)
        .execute(pool)
        .await
        .map_err(|e| {
            DbError::TransactionFailed(format!("Failed to unset primary addresses: {}", e))
        })?;
    }

    let wallet = sqlx::query_as::<_, CompanyWalletAddress>(
        r#"
        INSERT INTO company_wallet_addresses (
            company_id, wallet_address, address_label, address_type,
            is_primary, notes, metadata, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            id, company_id, wallet_address, address_label, address_type,
            status, is_primary, available_balance, pending_balance,
            lifetime_deposits, lifetime_withdrawals, notes, metadata,
            created_by, created_at, updated_at
        "#,
    )
    .bind(request.company_id)
    .bind(&request.wallet_address)
    .bind(&request.address_label)
    .bind(request.address_type.to_string())
    .bind(request.is_primary)
    .bind(&request.notes)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to create wallet address: {}", e)))?;

    Ok(wallet)
}

/// Get wallet address by ID
pub async fn get_wallet_address_by_id(
    pool: &PgPool,
    wallet_address_id: Uuid,
) -> DbResult<CompanyWalletAddress> {
    let wallet = sqlx::query_as::<_, CompanyWalletAddress>(
        r#"
        SELECT
            id, company_id, wallet_address, address_label, address_type,
            status, is_primary, available_balance, pending_balance,
            lifetime_deposits, lifetime_withdrawals, notes, metadata,
            created_by, created_at, updated_at
        FROM company_wallet_addresses
        WHERE id = $1
        "#,
    )
    .bind(wallet_address_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Wallet address not found: {}", e)))?;

    Ok(wallet)
}

/// Get wallet address by Ethereum address
pub async fn get_wallet_address_by_address(
    pool: &PgPool,
    address: &str,
) -> DbResult<CompanyWalletAddress> {
    let wallet = sqlx::query_as::<_, CompanyWalletAddress>(
        r#"
        SELECT
            id, company_id, wallet_address, address_label, address_type,
            status, is_primary, available_balance, pending_balance,
            lifetime_deposits, lifetime_withdrawals, notes, metadata,
            created_by, created_at, updated_at
        FROM company_wallet_addresses
        WHERE wallet_address = $1
        "#,
    )
    .bind(address)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Wallet address not found: {}", e)))?;

    Ok(wallet)
}

/// List wallet addresses for a company
pub async fn list_company_wallet_addresses(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<WalletAddressStatus>,
) -> DbResult<Vec<CompanyWalletAddress>> {
    let wallets = match status {
        Some(status) => {
            sqlx::query_as::<_, CompanyWalletAddress>(
                r#"
                SELECT
                    id, company_id, wallet_address, address_label, address_type,
                    status, is_primary, available_balance, pending_balance,
                    lifetime_deposits, lifetime_withdrawals, notes, metadata,
                    created_by, created_at, updated_at
                FROM company_wallet_addresses
                WHERE company_id = $1 AND status = $2
                ORDER BY is_primary DESC, created_at DESC
                "#,
            )
            .bind(company_id)
            .bind(status.to_string())
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, CompanyWalletAddress>(
                r#"
                SELECT
                    id, company_id, wallet_address, address_label, address_type,
                    status, is_primary, available_balance, pending_balance,
                    lifetime_deposits, lifetime_withdrawals, notes, metadata,
                    created_by, created_at, updated_at
                FROM company_wallet_addresses
                WHERE company_id = $1
                ORDER BY is_primary DESC, created_at DESC
                "#,
            )
            .bind(company_id)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| DbError::TransactionFailed(format!("Failed to list wallet addresses: {}", e)))?;

    Ok(wallets)
}

/// Get user's accessible wallet addresses in a company
pub async fn get_user_wallet_addresses(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
) -> DbResult<Vec<UserWalletAddress>> {
    let wallets = sqlx::query_as::<_, UserWalletAddress>(
        r#"
        SELECT * FROM get_user_wallet_addresses($1, $2)
        "#,
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        DbError::TransactionFailed(format!("Failed to get user wallet addresses: {}", e))
    })?;

    Ok(wallets)
}

/// Update a wallet address
pub async fn update_wallet_address(
    pool: &PgPool,
    wallet_address_id: Uuid,
    request: &UpdateWalletAddressRequest,
) -> DbResult<CompanyWalletAddress> {
    // Build dynamic update query
    let mut updates = Vec::new();
    let mut param_count = 2; // Starting from $2 since $1 is wallet_address_id

    if request.address_label.is_some() {
        updates.push(format!("address_label = ${}", param_count));
        param_count += 1;
    }
    if request.address_type.is_some() {
        updates.push(format!("address_type = ${}", param_count));
        param_count += 1;
    }
    if request.status.is_some() {
        updates.push(format!("status = ${}", param_count));
        param_count += 1;
    }
    if request.is_primary.is_some() {
        updates.push(format!("is_primary = ${}", param_count));
        param_count += 1;
    }
    if request.notes.is_some() {
        updates.push(format!("notes = ${}", param_count));
        param_count += 1;
    }
    if request.metadata.is_some() {
        updates.push(format!("metadata = ${}", param_count));
    }

    if updates.is_empty() {
        return get_wallet_address_by_id(pool, wallet_address_id).await;
    }

    let query_str = format!(
        r#"
        UPDATE company_wallet_addresses
        SET {}
        WHERE id = $1
        RETURNING
            id, company_id, wallet_address, address_label, address_type,
            status, is_primary, available_balance, pending_balance,
            lifetime_deposits, lifetime_withdrawals, notes, metadata,
            created_by, created_at, updated_at
        "#,
        updates.join(", ")
    );

    let mut query = sqlx::query_as::<_, CompanyWalletAddress>(&query_str).bind(wallet_address_id);

    if let Some(label) = &request.address_label {
        query = query.bind(label);
    }
    if let Some(addr_type) = &request.address_type {
        query = query.bind(addr_type.to_string());
    }
    if let Some(status) = &request.status {
        query = query.bind(status.to_string());
    }
    if let Some(is_primary) = &request.is_primary {
        query = query.bind(is_primary);
    }
    if let Some(notes) = &request.notes {
        query = query.bind(notes);
    }
    if let Some(metadata) = &request.metadata {
        query = query.bind(metadata);
    }

    let wallet = query.fetch_one(pool).await.map_err(|e| {
        DbError::TransactionFailed(format!("Failed to update wallet address: {}", e))
    })?;

    Ok(wallet)
}

// ============================================================================
// DATABASE OPERATIONS - PERMISSIONS
// ============================================================================

/// Grant wallet permission to a user
pub async fn grant_wallet_permission(
    pool: &PgPool,
    request: &GrantWalletPermissionRequest,
    granted_by: Uuid,
) -> DbResult<WalletAddressPermission> {
    // Get company_id from membership
    let company_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT company_id FROM company_memberships WHERE id = $1
        "#,
    )
    .bind(request.membership_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Membership not found: {}", e)))?;

    let permission = sqlx::query_as::<_, WalletAddressPermission>(
        r#"
        INSERT INTO wallet_address_permissions (
            wallet_address_id, company_id, membership_id, permission_type,
            daily_limit, requires_approval, granted_by, expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (wallet_address_id, membership_id, permission_type) DO UPDATE
        SET daily_limit = EXCLUDED.daily_limit,
            requires_approval = EXCLUDED.requires_approval,
            expires_at = EXCLUDED.expires_at
        RETURNING
            id, wallet_address_id, company_id, membership_id, permission_type,
            daily_limit, requires_approval, granted_by, granted_at, expires_at, created_at
        "#,
    )
    .bind(request.wallet_address_id)
    .bind(company_id)
    .bind(request.membership_id)
    .bind(request.permission_type.to_string())
    .bind(&request.daily_limit)
    .bind(request.requires_approval)
    .bind(granted_by)
    .bind(request.expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to grant permission: {}", e)))?;

    Ok(permission)
}

/// Check if user has specific wallet permission
pub async fn has_wallet_permission(
    pool: &PgPool,
    user_id: Uuid,
    wallet_address_id: Uuid,
    permission_type: WalletPermissionType,
) -> DbResult<bool> {
    let has_perm = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT has_wallet_permission($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(wallet_address_id)
    .bind(permission_type.to_string())
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to check permission: {}", e)))?;

    Ok(has_perm)
}

/// Check if user can authorize transactions (must be admin with approve_withdrawal permission)
pub async fn can_authorize_transaction(
    pool: &PgPool,
    user_id: Uuid,
    wallet_address_id: Uuid,
) -> DbResult<bool> {
    let can_authorize = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT can_authorize_transaction($1, $2)
        "#,
    )
    .bind(user_id)
    .bind(wallet_address_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to check authorization: {}", e)))?;

    Ok(can_authorize)
}

/// List permissions for a wallet address
pub async fn list_wallet_permissions(
    pool: &PgPool,
    wallet_address_id: Uuid,
) -> DbResult<Vec<WalletAddressPermission>> {
    let permissions = sqlx::query_as::<_, WalletAddressPermission>(
        r#"
        SELECT
            id, wallet_address_id, company_id, membership_id, permission_type,
            daily_limit, requires_approval, granted_by, granted_at, expires_at, created_at
        FROM wallet_address_permissions
        WHERE wallet_address_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(wallet_address_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to list permissions: {}", e)))?;

    Ok(permissions)
}

/// Revoke wallet permission
pub async fn revoke_wallet_permission(pool: &PgPool, permission_id: Uuid) -> DbResult<()> {
    sqlx::query(
        r#"
        DELETE FROM wallet_address_permissions
        WHERE id = $1
        "#,
    )
    .bind(permission_id)
    .execute(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to revoke permission: {}", e)))?;

    Ok(())
}
