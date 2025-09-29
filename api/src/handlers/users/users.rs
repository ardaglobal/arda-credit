//! User management handlers for the Arda Credit API
//!
//! This module contains all user-related API endpoints including:
//! - User registration, login, and profile management
//! - KYC submission and status management
//! - User balance and transaction queries
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::types::{ApiState, UserType};
use arda_credit_db::{
    get_deposits_by_user_address, get_identity_by_id, get_invite_code_info,
    get_user_balance_by_address, get_user_balance_by_id, register_or_reregister_user,
    store_ads_state_commit, submit_kyc_data, update_identity_profile, update_kyc_status,
    use_invite_code, Identity, RegisterResult,
};
use sqlx::Error as SqlxError;

// ============================================================================
// USER MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Request to register a new user identity
/// User type is determined by the invite code provided
/// Registration is passwordless - authentication uses magic links
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUserRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
    pub jurisdiction: Option<String>, // Optional, defaults to "US"
    #[schema(example = "INVESTOR2025")]
    pub invite_code: String, // Required - determines user type
}

/// Response from user registration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUserResponse {
    pub user_id: String, // UUID as string
    pub user_type: UserType,
    pub kyc_status: String,
    pub created_at: DateTime<Utc>,
}

/// User details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserDetailsResponse {
    pub user_id: String,
    pub email: String,
    pub user_type: UserType,
    pub kyc_status: String,
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    #[schema(example = "John")]
    pub first_name: Option<String>,
    #[schema(example = "Doe")]
    pub last_name: Option<String>,
    #[schema(example = "US")]
    pub jurisdiction: Option<String>,
    #[schema(example = "0x742d35Cc6b5c0c25f2d0f1b5b3f5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5")]
    pub wallet_address: Option<String>,
}

/// Update user profile request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub user_type: Option<UserType>,
    pub jurisdiction: Option<String>,
    #[schema(example = "John")]
    pub first_name: Option<String>,
    #[schema(example = "Doe")]
    pub last_name: Option<String>,
    #[schema(example = "0x742d35Cc6b5c0c25f2d0f1b5b3f5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5")]
    pub wallet_address: Option<String>,
}

/// Update user response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserResponse {
    pub user_id: String,
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

/// Delete user response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteUserResponse {
    pub user_id: String,
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
}

/// List identities query parameters
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListIdentitiesQuery {
    pub user_type: Option<UserType>,
    pub kyc_status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List identities response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListIdentitiesResponse {
    pub identities: Vec<UserDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// KYC submission request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitKycRequest {
    pub kyc_data: KycData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct KycData {
    pub document_type: String,
    pub document_hash: String,
    pub jurisdiction: Option<String>,
    pub kyc_investor_type: Option<String>, // QIB, QP, AI - required for investors
}

/// KYC submission response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitKycResponse {
    pub user_id: String,
    pub kyc_status: String,
    pub submitted_at: DateTime<Utc>,
}

/// Update KYC status request (admin only)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateKycStatusRequest {
    pub status: String, // "Pending" | "Approved" | "Rejected"
    pub notes: Option<String>,
}

/// Response for user balance query
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserBalanceResponse {
    pub user_id: Option<String>,
    pub wallet_address: String,
    pub available_balance: String,
    pub currency: String,
}

/// Response for user transaction history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserTransactionsResponse {
    pub transactions: Vec<DepositTransactionInfo>,
    pub total_count: usize,
}

/// Deposit transaction information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DepositTransactionInfo {
    pub id: i32,
    pub user_address: String,
    pub amount: String,
    pub deposit_amount_usdc: Option<String>,
    pub transaction_hash: String,
    pub status: String, // pending, confirmed, processed, failed
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub block_number: i64,
    pub created_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to parse UUID from string

/// Convert database Identity to API RegisterUserResponse
fn identity_to_register_response(identity: Identity) -> RegisterUserResponse {
    RegisterUserResponse {
        user_id: identity.id.to_string(),
        user_type: identity.user_type.parse().unwrap_or(UserType::Investor),
        kyc_status: identity.kyc_status,
        created_at: identity.created_at,
    }
}

// ============================================================================
// USER REGISTRATION AND AUTHENTICATION
// ============================================================================

/// Register a new user
#[utoipa::path(
    post,
    path = "/api/v1/user",
    request_body = RegisterUserRequest,
    responses(
        (status = 201, description = "User registered successfully", body = RegisterUserResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 409, description = "User with this email already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn register_user_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<RegisterUserRequest>,
) -> Result<Json<RegisterUserResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record business logic operation
    metrics::counter!("business_logic_operations_total", "operation" => "register_user")
        .increment(1);

    info!(
        "👤 API: Registering new user: email={}, invite_code={}",
        request.email, request.invite_code
    );

    // Validate invite code (required)
    let invite_code = &request.invite_code;
    info!(
        "🎫 API: Validating invite code during registration: {}",
        invite_code
    );

    let invite_info = match get_invite_code_info(&state.pool, invite_code).await {
        Ok(invite_info) => {
            if !invite_info.is_valid {
                warn!(
                    "❌ API: Invalid invite code provided during registration: {}",
                    invite_code
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid or expired invite code".to_string(),
                ));
            }

            if invite_info.user_type.is_none() {
                warn!(
                    "❌ API: Invite code does not specify a user type: {}",
                    invite_code
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid invite code - no user type specified".to_string(),
                ));
            }

            info!(
                "✅ API: Invite code validated successfully: {} (user_type={})",
                invite_code,
                invite_info.user_type.as_ref().unwrap()
            );

            invite_info
        }
        Err(e) => {
            error!(
                "❌ API: Failed to validate invite code during registration: {}",
                e
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate invite code".to_string(),
            ));
        }
    };

    // Extract user type from invite code
    let user_type = invite_info.user_type.as_ref().unwrap().clone();

    // Validate email format (basic validation)
    if !request.email.contains('@') || !request.email.contains('.') {
        warn!("❌ API: Invalid email format: {}", request.email);
        return Err((StatusCode::BAD_REQUEST, "Invalid email format".to_string()));
    }

    // Create or re-register user with soft delete handling
    match register_or_reregister_user(&state.pool, &user_type, &request.email).await {
        Ok(RegisterResult::Created(identity)) => {
            info!(
                "✅ API: New user registered successfully: id={}",
                identity.id
            );

            // Mark invite code as used now that user is created
            match use_invite_code(&state.pool, invite_code, Some(identity.id)).await {
                Ok(was_used) => {
                    if was_used {
                        info!(
                            "✅ API: Invite code marked as used: code={}, user_id={}",
                            invite_code, identity.id
                        );
                    } else {
                        warn!("⚠️ API: Invite code could not be marked as used (may be exhausted): {}", invite_code);
                    }
                }
                Err(e) => {
                    error!("❌ API: Failed to mark invite code as used: {}", e);
                    // Note: We don't fail the registration here since the user was already created
                    // This is logged for monitoring but shouldn't block the successful registration
                }
            }

            // Record function execution time
            let duration = start_time.elapsed();
            metrics::histogram!("function_execution_duration_seconds", "function" => "register_user")
                .record(duration.as_secs_f64());

            Ok(Json(identity_to_register_response(identity)))
        }
        Ok(RegisterResult::Reregistered(identity)) => {
            info!(
                "✅ API: Deleted user re-registered successfully: id={}",
                identity.id
            );

            // Mark invite code as used for the re-registered user
            match use_invite_code(&state.pool, invite_code, Some(identity.id)).await {
                Ok(was_used) => {
                    if was_used {
                        info!(
                            "✅ API: Invite code marked as used for re-registered user: code={}, user_id={}",
                            invite_code, identity.id
                        );
                    } else {
                        warn!("⚠️ API: Invite code could not be marked as used (may be exhausted): {}", invite_code);
                    }
                }
                Err(e) => {
                    error!(
                        "❌ API: Failed to mark invite code as used for re-registered user: {}",
                        e
                    );
                    // Note: We don't fail the registration here since the user was already re-registered
                    // This is logged for monitoring but shouldn't block the successful re-registration
                }
            }

            // Record function execution time for re-registration
            let duration = start_time.elapsed();
            metrics::histogram!("function_execution_duration_seconds", "function" => "register_user")
                .record(duration.as_secs_f64());

            Ok(Json(identity_to_register_response(identity)))
        }
        Ok(RegisterResult::AlreadyExists) => {
            warn!(
                "❌ API: Registration failed - user with email already exists and is active: {}",
                request.email
            );
            metrics::counter!("business_logic_errors_total", "operation" => "register_user", "error_type" => "already_exists").increment(1);
            Err((
                StatusCode::CONFLICT,
                "User with this email already exists".to_string(),
            ))
        }
        Err(e) => {
            error!("❌ API: Failed to register user: {}", e);
            metrics::counter!("business_logic_errors_total", "operation" => "register_user", "error_type" => "database_error").increment(1);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to register user".to_string(),
            ))
        }
    }
}

/// Get user details by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}",
    params(("user_id" = String, Path, description = "User ID")),
    responses(
        (status = 200, description = "User details retrieved", body = UserDetailsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_details_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user details: user_id={}", user_id);

    let uuid = parse_user_id(&user_id)?;

    match get_identity_by_id(&state.pool, uuid).await {
        Ok(Some(identity)) => {
            info!("✅ API: User details retrieved for ID: {}", identity.id);
            Ok(Json(identity_to_user_details(identity)))
        }
        Ok(None) => {
            warn!("❌ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Database error retrieving user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// List user identities
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(ListIdentitiesQuery),
    responses(
        (status = 200, description = "List of user identities", body = ListIdentitiesResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_identities_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListIdentitiesQuery>,
) -> Result<Json<ListIdentitiesResponse>, (StatusCode, String)> {
    info!("📋 API: Listing identities with filters");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Basic identity listing using direct SQL (excludes soft deleted users)
    let identities_result = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, created_at, updated_at, first_name, last_name, jurisdiction, wallet_address
        FROM identities
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(&state.pool)
    .await;

    match identities_result {
        Ok(rows) => {
            let identities = rows
                .into_iter()
                .map(|row| UserDetailsResponse {
                    user_id: row.id.to_string(),
                    email: row.email,
                    user_type: row.user_type.parse().unwrap_or(UserType::Investor),
                    kyc_status: row.kyc_status,
                    kyc_submitted_at: row.kyc_submitted_at,
                    kyc_verified_at: row.kyc_verified_at,
                    created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
                    updated_at: Some(row.updated_at.unwrap_or_else(chrono::Utc::now)),
                    first_name: row.first_name,
                    last_name: row.last_name,
                    jurisdiction: row.jurisdiction,
                    wallet_address: row.wallet_address,
                })
                .collect::<Vec<_>>();

            let total_count = identities.len() as u64;

            let response = ListIdentitiesResponse {
                identities,
                total_count: total_count as usize,
                has_more: total_count >= limit as u64,
            };

            info!("✅ API: Listed {} identities", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list identities: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list identities: {}", e),
            ))
        }
    }
}

// ============================================================================
// USER BALANCE AND TRANSACTION TYPES
// ============================================================================
// USER ENDPOINTS FROM rest.rs
// ============================================================================

/// Update user profile
#[utoipa::path(
    put,
    path = "/api/v1/user/{user_id}",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User profile updated successfully", body = UpdateUserResponse),
        (status = 404, description = "User not found"),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UpdateUserResponse>, (StatusCode, String)> {
    info!("🔄 API: Updating user profile: user_id={}", user_id);

    let uuid = parse_user_id(&user_id)?;

    // Build list of fields being updated
    let mut updated_fields = Vec::new();
    if request.email.is_some() {
        updated_fields.push("email".to_string());
    }
    if request.user_type.is_some() {
        updated_fields.push("user_type".to_string());
    }
    if request.jurisdiction.is_some() {
        updated_fields.push("jurisdiction".to_string());
    }
    if request.first_name.is_some() {
        updated_fields.push("first_name".to_string());
    }
    if request.last_name.is_some() {
        updated_fields.push("last_name".to_string());
    }
    if request.wallet_address.is_some() {
        updated_fields.push("wallet_address".to_string());
    }

    match update_identity_profile(
        &state.pool,
        uuid,
        request.email.as_deref(),
        request
            .user_type
            .as_ref()
            .map(std::string::ToString::to_string)
            .as_deref(),
        request.jurisdiction.as_deref(),
        request.first_name.as_deref(),
        request.last_name.as_deref(),
        request.wallet_address.as_deref(),
    )
    .await
    {
        Ok(identity) => {
            info!(
                "✅ API: User profile updated successfully for ID: {}",
                identity.id
            );
            let response = UpdateUserResponse {
                user_id: identity.id.to_string(),
                updated_fields,
                updated_at: identity.updated_at,
            };
            Ok(Json(response))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error updating user: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update user".to_string(),
                ))
            }
        }
    }
}

/// Delete user
#[utoipa::path(
    delete,
    path = "/api/v1/user/{user_id}",
    responses(
        (status = 200, description = "User deleted successfully", body = DeleteUserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn delete_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<DeleteUserResponse>, (StatusCode, String)> {
    info!("🗑️ API: Soft deleting user: user_id={}", user_id);

    let user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Check if user exists and is not already deleted
    let existing_user = sqlx::query!(
        "SELECT id, deleted_at FROM identities WHERE id = $1",
        user_uuid
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to check user existence: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to check user existence".to_string(),
        )
    })?;

    let _user = match existing_user {
        Some(user) if user.deleted_at.is_some() => {
            return Err((StatusCode::CONFLICT, "User is already deleted".to_string()));
        }
        Some(user) => user,
        None => {
            return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
        }
    };

    // Perform soft delete by setting deleted_at timestamp
    let deleted_at = Utc::now();
    let update_result = sqlx::query!(
        "UPDATE identities SET deleted_at = $1, updated_at = $1 WHERE id = $2",
        deleted_at,
        user_uuid
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to soft delete user: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to delete user".to_string(),
        )
    })?;

    if update_result.rows_affected() == 0 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update user deletion status".to_string(),
        ));
    }

    let response = DeleteUserResponse {
        user_id,
        deleted: true,
        deleted_at,
    };

    info!("✅ API: User soft deleted successfully");
    Ok(Json(response))
}

/// Get user balance by address
#[utoipa::path(
    get,
    path = "/api/v1/user/balance/{address}",
    responses(
        (status = 200, description = "User balance retrieved successfully", body = UserBalanceResponse),
        (status =404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("address" = String, Path, description = "Wallet address")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_balance_by_address_endpoint(
    Path(address): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalanceResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user balance by address: {}", address);

    match get_user_balance_by_address(&state.pool, &address).await {
        Ok(Some(balance)) => {
            let response = UserBalanceResponse {
                user_id: None, // We don't have user_id when querying by address directly
                wallet_address: address,
                available_balance: balance.to_string(),
                currency: "USDC".to_string(),
            };
            info!("✅ API: User balance retrieved successfully");
            Ok(Json(response))
        }
        Ok(None) => {
            warn!("⚠️ API: User not found for address: {}", address);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user balance: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user balance".to_string(),
            ))
        }
    }
}

/// Get user balance by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/balance",
    responses(
        (status = 200, description = "User balance retrieved successfully", body = UserBalanceResponse),
        (status =404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_balance_by_id_endpoint(
    Path(user_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalanceResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user balance by ID: {}", user_id);

    let user_uuid = parse_user_id(&user_id)?;

    // First get user details to get wallet address
    match get_identity_by_id(&state.pool, user_uuid).await {
        Ok(Some(identity)) => match get_user_balance_by_id(&state.pool, user_uuid).await {
            Ok(Some(balance)) => {
                let response = UserBalanceResponse {
                    user_id: Some(user_id),
                    wallet_address: identity
                        .wallet_address
                        .unwrap_or_else(|| "Not set".to_string()),
                    available_balance: balance.to_string(),
                    currency: "USDC".to_string(),
                };
                info!("✅ API: User balance retrieved successfully");
                Ok(Json(response))
            }
            Ok(None) => {
                warn!("⚠️ API: Balance not found for user ID: {}", user_id);
                Err((StatusCode::NOT_FOUND, "Balance not found".to_string()))
            }
            Err(e) => {
                error!("❌ API: Failed to get user balance: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to retrieve user balance".to_string(),
                ))
            }
        },
        Ok(None) => {
            warn!("⚠️ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user identity: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// Get user transactions by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/transactions",
    responses(
        (status = 200, description = "User transactions retrieved successfully", body = UserTransactionsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_transactions_endpoint(
    Path(user_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserTransactionsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user transactions by ID: {}", user_id);

    let user_uuid = parse_user_id(&user_id)?;

    // First get user details to get wallet address
    match get_identity_by_id(&state.pool, user_uuid).await {
        Ok(Some(identity)) => {
            if let Some(wallet_address) = &identity.wallet_address {
                match get_deposits_by_user_address(&state.pool, wallet_address).await {
                    Ok(deposits) => {
                        let transaction_infos: Vec<DepositTransactionInfo> = deposits
                            .into_iter()
                            .map(|deposit| DepositTransactionInfo {
                                id: deposit.id,
                                user_address: deposit.user_address,
                                amount: deposit.amount.to_string(),
                                deposit_amount_usdc: deposit
                                    .deposit_amount_usdc
                                    .map(|d| d.to_string()),
                                transaction_hash: deposit.transaction_hash,
                                status: deposit.status,
                                confirmation_count: deposit.confirmation_count,
                                required_confirmations: deposit.required_confirmations,
                                block_number: deposit.block_number,
                                created_at: deposit.created_at,
                                processed_at: deposit.processed_at,
                            })
                            .collect();

                        let response = UserTransactionsResponse {
                            total_count: transaction_infos.len(),
                            transactions: transaction_infos,
                        };

                        info!("✅ API: User transactions retrieved successfully");
                        Ok(Json(response))
                    }
                    Err(e) => {
                        error!("❌ API: Failed to get user transactions: {}", e);
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to retrieve user transactions".to_string(),
                        ))
                    }
                }
            } else {
                warn!("⚠️ API: User has no wallet address set: {}", user_id);
                Err((
                    StatusCode::BAD_REQUEST,
                    "User has no wallet address set".to_string(),
                ))
            }
        }
        Ok(None) => {
            warn!("⚠️ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user identity: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// Parse user ID string to UUID
fn parse_user_id(user_id: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    uuid::Uuid::parse_str(user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid user ID format: {}", user_id),
        )
    })
}

/// Convert database Identity to API UserDetailsResponse
fn identity_to_user_details(identity: Identity) -> UserDetailsResponse {
    UserDetailsResponse {
        user_id: identity.id.to_string(),
        email: identity.email,
        user_type: identity.user_type.parse().unwrap_or(UserType::Investor),
        kyc_status: identity.kyc_status,
        kyc_submitted_at: identity.kyc_submitted_at,
        kyc_verified_at: identity.kyc_verified_at,
        created_at: identity.created_at,
        updated_at: Some(identity.updated_at),
        first_name: identity.first_name,
        last_name: identity.last_name,
        jurisdiction: identity.jurisdiction,
        wallet_address: identity.wallet_address,
    }
}

/// Submit KYC information
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc",
    request_body = SubmitKycRequest,
    responses(
        (status = 200, description = "KYC submitted successfully", body = SubmitKycResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn submit_kyc_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<SubmitKycRequest>,
) -> Result<Json<SubmitKycResponse>, (StatusCode, String)> {
    info!(
        "📄 API: KYC submission: user_id={}, doc_type={}",
        user_id, request.kyc_data.document_type
    );

    let uuid = parse_user_id(&user_id)?;

    match submit_kyc_data(
        &state.pool,
        uuid,
        &request.kyc_data.document_type,
        &request.kyc_data.document_hash,
        request.kyc_data.jurisdiction.as_deref(),
        request.kyc_data.kyc_investor_type.as_deref(),
    )
    .await
    {
        Ok(identity) => {
            info!(
                "✅ API: KYC submitted successfully for user: {}",
                identity.id
            );
            let response = SubmitKycResponse {
                user_id: identity.id.to_string(),
                kyc_status: identity.kyc_status,
                submitted_at: identity.kyc_submitted_at.unwrap_or_else(Utc::now),
            };
            Ok(Json(response))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error submitting KYC: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to submit KYC".to_string(),
                ))
            }
        }
    }
}

/// Update KYC status (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc-status",
    request_body = UpdateKycStatusRequest,
    responses(
        (status = 200, description = "KYC status updated successfully", body = UserDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_kyc_status_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateKycStatusRequest>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!(
        "⚖️ API: Updating KYC status: user_id={}, new_status={}",
        user_id, request.status
    );

    let uuid = parse_user_id(&user_id)?;

    match update_kyc_status(&state.pool, uuid, &request.status, request.notes.as_deref()).await {
        Ok(identity) => {
            info!(
                "✅ API: KYC status updated successfully for user: {}",
                identity.id
            );
            Ok(Json(identity_to_user_details(identity)))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error updating KYC status: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update KYC status".to_string(),
                ))
            }
        }
    }
}
