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
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::rest::UserType;
use crate::types::ApiState;
use arda_credit_db::{
    get_deposits_by_user_address, get_identity_by_id, get_invite_code_info,
    get_user_balance_by_address, get_user_balance_by_id, register_or_reregister_user,
    submit_kyc_data, update_identity_profile, update_kyc_status, use_invite_code, Identity,
    RegisterResult,
};

// ============================================================================
// USER MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Request to register a new user identity
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUserRequest {
    pub user_type: UserType,
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "SecureP@ssw0rd123")]
    pub password: String,
    pub jurisdiction: Option<String>, // Optional, defaults to "US"
    #[schema(example = "ABC123DEF456")]
    pub invite_code: Option<String>, // Optional invite code for registration
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
        user_type: identity.user_type.parse().unwrap_or(UserType::User),
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

/// Convert database Identity to API RegisterUserResponse
fn identity_to_register_response(identity: Identity) -> RegisterUserResponse {
    RegisterUserResponse {
        user_id: identity.id.to_string(),
        user_type: identity.user_type.parse().unwrap_or(UserType::User),
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
    info!(
        "👤 API: Registering new user: type={}, email={}, invite_code={:?}",
        request.user_type, request.email, request.invite_code
    );

    // If invite code is provided, validate it first
    if let Some(ref invite_code) = request.invite_code {
        info!(
            "🎫 API: Validating invite code during registration: {}",
            invite_code
        );

        match get_invite_code_info(&state.pool, invite_code).await {
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

                // Check invite code restrictions: only admin codes require admin users
                if let Some(ref code_user_type) = invite_info.user_type {
                    let requested_type = request.user_type.to_string();

                    // Only restrict admin codes to admin users
                    if code_user_type == "admin" && requested_type != "admin" {
                        warn!(
                            "❌ API: Admin invite code used for non-admin registration: code={}, requested={}",
                            code_user_type, requested_type
                        );
                        return Err((
                            StatusCode::BAD_REQUEST,
                            "This is an admin invite code and can only be used for admin user registration".to_string(),
                        ));
                    }

                    info!(
                        "✅ API: Invite code type check passed: code_type={}, user_type={}",
                        code_user_type, requested_type
                    );
                }

                info!(
                    "✅ API: Invite code validated successfully: {}",
                    invite_code
                );
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
        }
    }

    // Validate email and password using AuthService
    if let Err(e) = state.auth_service.validate_email(&request.email) {
        warn!("❌ API: Invalid email format: {}", e);
        return Err((StatusCode::BAD_REQUEST, e.user_message().to_string()));
    }

    if let Err(e) = state.auth_service.validate_password(&request.password) {
        warn!("❌ API: Invalid password format: {}", e);
        return Err((StatusCode::BAD_REQUEST, e.user_message().to_string()));
    }

    // Hash the password
    let password_hash = match state.auth_service.hash_password(&request.password) {
        Ok(hash) => hash,
        Err(e) => {
            error!("❌ API: Failed to hash password: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                e.user_message().to_string(),
            ));
        }
    };

    // Create or re-register user with soft delete handling
    match register_or_reregister_user(
        &state.pool,
        &request.user_type.to_string(),
        &request.email,
        &password_hash,
    )
    .await
    {
        Ok(RegisterResult::Created(identity)) => {
            info!(
                "✅ API: New user registered successfully: id={}",
                identity.id
            );

            // If invite code was provided, mark it as used now that user is created
            if let Some(ref invite_code) = request.invite_code {
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
            }

            Ok(Json(identity_to_register_response(identity)))
        }
        Ok(RegisterResult::Reregistered(identity)) => {
            info!(
                "✅ API: Deleted user re-registered successfully: id={}",
                identity.id
            );

            // If invite code was provided, mark it as used for the re-registered user
            if let Some(ref invite_code) = request.invite_code {
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
            }

            Ok(Json(identity_to_register_response(identity)))
        }
        Ok(RegisterResult::AlreadyExists) => {
            warn!(
                "❌ API: Registration failed - user with email already exists and is active: {}",
                request.email
            );
            Err((
                StatusCode::CONFLICT,
                "User with this email already exists".to_string(),
            ))
        }
        Err(e) => {
            error!("❌ API: Failed to register user: {}", e);
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
                    user_type: row.user_type.parse().unwrap_or(UserType::User),
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
