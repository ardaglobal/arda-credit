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
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::config::types::{ApiState, UserType};
use arda_credit_db::{
    create_investor_profile, get_deposits_by_user_address, get_identity_by_id,
    get_investor_profile_by_user_id, get_invite_code_info, register_or_reregister_user,
    submit_kyc_data, update_identity_profile, update_kyc_status, use_invite_code,
    CreateInvestorProfileRequest, InvestorProfile, RegisterResult, User,
};
use arda_credit_lib::TransactionType;
use serde_json::json;

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

/// Investor profile data for nested response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InvestorProfileData {
    pub accredited_investor: bool,
    #[schema(value_type = Option<String>, example = "1000000.000000")]
    pub investment_capacity: Option<rust_decimal::Decimal>,
    #[schema(example = "US")]
    pub regulatory_jurisdiction: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Originator profile data for nested response (placeholder for future implementation)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OriginatorProfileData {
    // TODO: Define originator profile fields when originator profiles table is created
    #[schema(example = "Placeholder for future originator profile fields")]
    pub placeholder: String,
}

/// Company membership information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompanyMembershipData {
    pub company_id: String,
    pub company_name: String,
    pub role: String,
    pub status: String,
    pub profile_type: Option<String>, // investor or originator
    pub joined_at: Option<DateTime<Utc>>,
}

/// User details response with nested profile information
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
    /// Nested investor profile data (only present for investor user types)
    pub investor_profile: Option<InvestorProfileData>,
    /// Nested originator profile data (placeholder for future implementation)
    pub originator_profile: Option<OriginatorProfileData>,
    /// List of companies the user is a member of
    pub companies: Vec<CompanyMembershipData>,
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

/// List users query parameters
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListIdentitiesQuery {
    pub user_type: Option<UserType>,
    pub kyc_status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List users response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListIdentitiesResponse {
    pub users: Vec<UserDetailsResponse>,
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

// UserBalanceResponse removed - now using consolidated balance response types from balance.rs

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

/// Convert database User to API RegisterUserResponse
fn identity_to_register_response(identity: User) -> RegisterUserResponse {
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
    tag = "Users"
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

            // Create profile based on user type
            let user_type_parsed = identity.user_type.parse().unwrap_or(UserType::Investor);
            if let Err(e) = ensure_user_profiles_exist(
                &state.pool,
                identity.id,
                &user_type_parsed,
                request.jurisdiction.as_deref(),
            )
            .await
            {
                error!(
                    "❌ Failed to create profile for newly registered user {}: {}",
                    identity.id, e
                );
                // Log error but don't fail registration - profile can be created later
            }

            // NEW: Record transaction for zkVM
            let tx_id = match state
                .transaction_recording
                .record_transaction_with_metadata(
                    TransactionType::UserRegistration,
                    identity.id,
                    None,                        // No deal_id for user registration
                    rust_decimal::Decimal::ZERO, // No amount for user registration
                    Some(json!({
                        "email": request.email.clone(),
                        "user_type": user_type_parsed.to_string(),
                    })),
                    None, // originator_company_id
                    None, // investor_company_id
                    None, // collateral_id
                )
                .await
            {
                Ok(tx_id) => {
                    info!("✅ Recorded UserRegistration transaction: {}", tx_id);
                    tx_id
                }
                Err(e) => {
                    error!("❌ Failed to record user registration transaction: {}", e);
                    // For user registration, we fail the entire request if transaction recording fails
                    // This ensures consistency between user creation and zkVM state
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("User created but transaction recording failed: {}", e),
                    ));
                }
            };

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

            // Create profile based on user type for re-registered user
            let user_type_parsed = identity.user_type.parse().unwrap_or(UserType::Investor);
            if let Err(e) = ensure_user_profiles_exist(
                &state.pool,
                identity.id,
                &user_type_parsed,
                request.jurisdiction.as_deref(),
            )
            .await
            {
                error!(
                    "❌ Failed to create profile for re-registered user {}: {}",
                    identity.id, e
                );
                // Log error but don't fail registration - profile can be created later
            }

            // NEW: Record transaction for zkVM (also for re-registered users)
            let tx_id = match state
                .transaction_recording
                .record_transaction_with_metadata(
                    TransactionType::UserRegistration,
                    identity.id,
                    None,                        // No deal_id for user registration
                    rust_decimal::Decimal::ZERO, // No amount for user registration
                    Some(json!({
                        "email": request.email.clone(),
                        "user_type": user_type_parsed.to_string(),
                        "re_registered": true,
                    })),
                    None, // originator_company_id
                    None, // investor_company_id
                    None, // collateral_id
                )
                .await
            {
                Ok(tx_id) => {
                    info!(
                        "✅ Recorded UserRegistration transaction for re-registered user: {}",
                        tx_id
                    );
                    tx_id
                }
                Err(e) => {
                    error!(
                        "❌ Failed to record user re-registration transaction: {}",
                        e
                    );
                    // For user registration, we fail the entire request if transaction recording fails
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("User re-registered but transaction recording failed: {}", e),
                    ));
                }
            };

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
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_details_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Getting user details with profiles: user_id={}",
        user_id
    );

    let uuid = parse_user_id(&user_id)?;

    match get_user_details_with_profiles(&state.pool, uuid).await {
        Ok(user_details) => {
            info!(
                "✅ API: User details with profiles retrieved for ID: {}",
                uuid
            );
            Ok(Json(user_details))
        }
        Err(sqlx::Error::RowNotFound) => {
            warn!("❌ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Database error retrieving user details: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user details".to_string(),
            ))
        }
    }
}

/// List user users
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(ListIdentitiesQuery),
    responses(
        (status = 200, description = "List of user users", body = ListIdentitiesResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_identities_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListIdentitiesQuery>,
) -> Result<Json<ListIdentitiesResponse>, (StatusCode, String)> {
    info!("📋 API: Listing users with filters");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // For better performance and to avoid complex LEFT JOIN type handling,
    // fetch user users first, then fetch profiles separately
    let identities_result = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, 
               created_at, updated_at, first_name, last_name, jurisdiction, wallet_address
        FROM users
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
            let mut users = Vec::new();

            for row in rows {
                let user_type_parsed = row.user_type.parse().unwrap_or(UserType::Investor);

                // Fetch profile information based on user type
                let (investor_profile, originator_profile) = match user_type_parsed {
                    UserType::Investor => {
                        // Fetch investor profile if exists
                        match get_investor_profile_by_user_id(&state.pool, row.id).await {
                            Ok(Some(profile)) => {
                                let investor_data = InvestorProfileData {
                                    accredited_investor: profile.accredited_investor,
                                    investment_capacity: profile.investment_capacity,
                                    regulatory_jurisdiction: profile.regulatory_jurisdiction,
                                    created_at: profile.created_at,
                                    updated_at: profile.updated_at,
                                };
                                (Some(investor_data), None)
                            }
                            Ok(None) => {
                                // No investor profile exists yet
                                (None, None)
                            }
                            Err(e) => {
                                warn!(
                                    "⚠️ Failed to fetch investor profile for user {}: {}",
                                    row.id, e
                                );
                                (None, None)
                            }
                        }
                    }
                    UserType::Originator => {
                        // TODO: Implement originator profile fetching when originator profiles table is created
                        info!(
                            "🏗️ TODO: Originator profile should be included for user: {}",
                            row.id
                        );
                        (None, None) // Will be: (None, Some(originator_data)) when implemented
                    }
                    UserType::Admin => {
                        // Admin users don't have extended profiles
                        (None, None)
                    }
                };

                // Fetch company memberships for this user
                let company_memberships =
                    match fetch_user_company_memberships(&state.pool, row.id).await {
                        Ok(companies) => companies,
                        Err(e) => {
                            warn!(
                                "⚠️ Failed to fetch company memberships for user {}: {}",
                                row.id, e
                            );
                            Vec::new() // Return empty vec on error
                        }
                    };

                users.push(UserDetailsResponse {
                    user_id: row.id.to_string(),
                    email: row.email,
                    user_type: user_type_parsed,
                    kyc_status: row.kyc_status,
                    kyc_submitted_at: row.kyc_submitted_at,
                    kyc_verified_at: row.kyc_verified_at,
                    created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
                    updated_at: Some(row.updated_at.unwrap_or_else(chrono::Utc::now)),
                    first_name: row.first_name,
                    last_name: row.last_name,
                    jurisdiction: row.jurisdiction,
                    wallet_address: row.wallet_address,
                    investor_profile,
                    originator_profile,
                    companies: company_memberships,
                });
            }

            let total_count = users.len() as u64;

            let response = ListIdentitiesResponse {
                users,
                total_count: total_count as usize,
                has_more: total_count >= limit as u64,
            };

            info!("✅ API: Listed {} users", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list users: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list users: {}", e),
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

            // If user type was updated, ensure appropriate profile exists
            if let Some(ref new_user_type) = request.user_type {
                if let Err(e) = ensure_user_profiles_exist(
                    &state.pool,
                    identity.id,
                    new_user_type,
                    request.jurisdiction.as_deref(),
                )
                .await
                {
                    error!(
                        "❌ Failed to create profile for updated user {}: {}",
                        identity.id, e
                    );
                    // Log error but don't fail the update - profile can be created later
                }
            }

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
    let existing_user = sqlx::query!("SELECT id, deleted_at FROM users WHERE id = $1", user_uuid)
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
        "UPDATE users SET deleted_at = $1, updated_at = $1 WHERE id = $2",
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

// DEPRECATED: Old balance endpoints removed - now using consolidated balance endpoints from balance.rs
// New endpoints:
// - GET /api/v1/user/{address}/balance -> balance::get_balance_by_address_endpoint
// - GET /api/v1/user/{user_id}/balance -> balance::get_balance_by_user_id_endpoint

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
    tag = "Users"
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

/// Convert database InvestorProfile to API InvestorProfileData
fn investor_profile_to_data(profile: InvestorProfile) -> InvestorProfileData {
    InvestorProfileData {
        accredited_investor: profile.accredited_investor,
        investment_capacity: profile.investment_capacity,
        regulatory_jurisdiction: profile.regulatory_jurisdiction,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    }
}

/// Get user details with profile information based on user type
async fn get_user_details_with_profiles(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> Result<UserDetailsResponse, sqlx::Error> {
    // Get basic user identity
    let identity = get_identity_by_id(pool, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    let user_type_parsed = identity.user_type.parse().unwrap_or(UserType::Investor);

    // Fetch profile information based on user type
    let (investor_profile, originator_profile) = match user_type_parsed {
        UserType::Investor => {
            // Fetch investor profile
            let profile = get_investor_profile_by_user_id(pool, user_id).await?;
            let investor_data = profile.map(investor_profile_to_data);
            (investor_data, None)
        }
        UserType::Originator => {
            // TODO: Implement originator profile fetching when originator profiles table is created
            info!("🏗️ Originator profile requested but originator profiles not yet implemented for user: {}", user_id);
            (None, None) // Will be: (None, Some(originator_data)) when implemented
        }
        UserType::Admin => {
            // Admin users don't have extended profiles
            (None, None)
        }
    };

    // Fetch company memberships
    let company_memberships = fetch_user_company_memberships(pool, user_id).await?;

    Ok(UserDetailsResponse {
        user_id: identity.id.to_string(),
        email: identity.email,
        user_type: user_type_parsed,
        kyc_status: identity.kyc_status,
        kyc_submitted_at: identity.kyc_submitted_at,
        kyc_verified_at: identity.kyc_verified_at,
        created_at: identity.created_at,
        updated_at: Some(identity.updated_at),
        first_name: identity.first_name,
        last_name: identity.last_name,
        jurisdiction: identity.jurisdiction,
        wallet_address: identity.wallet_address,
        investor_profile,
        originator_profile,
        companies: company_memberships,
    })
}

/// Fetch all company memberships for a user
async fn fetch_user_company_memberships(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> Result<Vec<CompanyMembershipData>, sqlx::Error> {
    let memberships = sqlx::query!(
        r#"
        SELECT
            cm.company_id,
            c.name as company_name,
            cm.role,
            cm.status,
            cm.profile_type,
            cm.accepted_at as joined_at
        FROM company_memberships cm
        JOIN companies c ON cm.company_id = c.id
        WHERE cm.user_id = $1
        ORDER BY cm.created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(memberships
        .into_iter()
        .map(|row| CompanyMembershipData {
            company_id: row.company_id.to_string(),
            company_name: row.company_name,
            role: row.role,
            status: row.status,
            profile_type: Some(row.profile_type),
            joined_at: row.joined_at,
        })
        .collect())
}

/// Create user type-specific profiles after user creation/update
async fn ensure_user_profiles_exist(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    user_type: &UserType,
    jurisdiction: Option<&str>,
) -> Result<(), sqlx::Error> {
    match user_type {
        UserType::Investor => {
            // Check if investor profile already exists
            let existing_profile = get_investor_profile_by_user_id(pool, user_id).await?;
            if existing_profile.is_none() {
                info!("🏗️ Creating investor profile for user: {}", user_id);
                let profile_request = CreateInvestorProfileRequest {
                    accredited_investor: Some(false), // Default to false, can be updated later
                    investment_capacity: None,        // Can be set later through profile updates
                    regulatory_jurisdiction: Some(jurisdiction.unwrap_or("US").to_string()),
                };

                match create_investor_profile(pool, user_id, &profile_request).await {
                    Ok(profile) => {
                        info!(
                            "✅ Created investor profile for user: {} with profile ID: {}",
                            user_id, profile.id
                        );
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to create investor profile for user {}: {}",
                            user_id, e
                        );
                        return Err(e);
                    }
                }
            }
        }
        UserType::Originator => {
            // TODO: Implement originator profile creation when originator profiles table is created
            info!(
                "🏗️ TODO: Originator profile creation needed but not yet implemented for user: {}",
                user_id
            );
        }
        UserType::Admin => {
            // Admin users don't need extended profiles
            info!(
                "👑 Admin user registered, no extended profile needed: {}",
                user_id
            );
        }
    }

    Ok(())
}

/// Convert database User to API UserDetailsResponse (legacy function for backward compatibility)
/// This function now delegates to get_user_details_with_profiles for consistency
pub fn identity_to_user_details(identity: User) -> UserDetailsResponse {
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
        // Profile fields will be None in legacy conversion
        // Use get_user_details_with_profiles for full profile data
        investor_profile: None,
        originator_profile: None,
        companies: Vec::new(), // Empty companies in legacy conversion
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

            // If KYC was approved, ensure user profiles exist
            if request.status.to_lowercase() == "approved" {
                let user_type_parsed = identity.user_type.parse().unwrap_or(UserType::Investor);
                if let Err(e) = ensure_user_profiles_exist(
                    &state.pool,
                    identity.id,
                    &user_type_parsed,
                    identity.jurisdiction.as_deref(),
                )
                .await
                {
                    error!(
                        "❌ Failed to create profile for KYC-approved user {}: {}",
                        identity.id, e
                    );
                    // Log error but don't fail the KYC update - profile can be created later
                }
            }

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
