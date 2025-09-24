//! Merchant and Investor handlers for the Arda Credit API
//!
//! This module contains all merchant and investor-related API endpoints including:
//! - Merchant registration and profile management
//! - Investor registration and profile management
//! - Turnover data submission and analytics
//! - Profile completion tracking
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
use crate::types::ApiState;
use crate::types::UserType;
use crate::user_handlers::UpdateUserRequest;
use arda_credit_db::update_identity_profile;
use arda_credit_db::{
    create_investor_profile, create_merchant, get_identity_by_email, get_identity_by_id,
    get_investor_profile_by_user_id, get_investor_profile_by_user_id_or_email,
    get_investor_profile_with_identity, get_merchant_by_user_id,
    get_merchant_profile_by_merchant_id, register_or_reregister_user, update_investor_profile,
    update_merchant_profile, CreateInvestorProfileRequest, CreateMerchantRequest, RegisterResult,
    UpdateInvestorProfileRequest as DbUpdateInvestorProfileRequest,
    UpdateMerchantProfileRequest as DbUpdateMerchantProfileRequest,
};

// ============================================================================
// MERCHANT & INVESTOR TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Request to register a new merchant with business information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterMerchantRequest {
    #[schema(example = "merchant@example.com")]
    pub email: String,
    #[schema(example = "MyBusiness LLC")]
    pub legal_name: String,
    #[schema(example = "MyBusiness Store")]
    pub dba: Option<String>, // Doing Business As name
    #[schema(example = "12-3456789")]
    pub tax_id: Option<String>,
    #[schema(example = "5999")]
    pub mcc: Option<String>, // Merchant Category Code
    #[schema(example = "USA")]
    pub country: String,
    #[schema(example = "LLC")]
    pub business_type: Option<String>, // LLC, CORP, SOLE_PROP, PARTNERSHIP, OTHER
    pub jurisdiction: Option<String>, // Optional, defaults to "US"
    #[schema(example = "ABC123DEF456")]
    pub invite_code: Option<String>, // Optional invite code for registration
}

/// Response from merchant registration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterMerchantResponse {
    pub user_id: String,     // UUID as string
    pub merchant_id: String, // UUID as string
    pub user_type: UserType,
    pub legal_name: String,
    pub kyc_status: String,
    pub created_at: DateTime<Utc>,
}

/// Request to register a new investor with profile
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterInvestorRequest {
    #[schema(example = "investor@example.com")]
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub jurisdiction: Option<String>, // Optional, defaults to "US"
    pub accredited_investor: Option<bool>, // Optional, defaults to false
    #[schema(value_type = Option<String>, example = "100000.50")]
    pub investment_capacity: Option<rust_decimal::Decimal>, // Optional max investment
    pub regulatory_jurisdiction: Option<String>, // Optional, defaults to "US"
    #[schema(example = "ABC123DEF456")]
    pub invite_code: Option<String>, // Optional invite code for registration
}

/// Response from investor registration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterInvestorResponse {
    pub user_id: String, // UUID as string
    pub user_type: UserType,
    pub kyc_status: String,
    pub created_at: DateTime<Utc>,
    pub profile_id: String, // UUID as string of investor profile
    pub accredited_investor: bool,
    pub regulatory_jurisdiction: String,
}

/// Query parameters for getting investor profile
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct InvestorProfileQuery {
    /// User ID (UUID)
    pub user_id: Option<String>,
    /// User email
    pub email: Option<String>,
}

/// Query parameters for merchant profile endpoint
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct MerchantProfileQuery {
    /// User ID (UUID) to query merchant profile
    pub user_id: Option<String>,
    /// User email to query merchant profile
    pub email: Option<String>,
}

/// Investor profile response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InvestorProfileResponse {
    pub profile_id: String,
    pub user_id: String,
    pub email: String,
    pub user_type: UserType,
    pub kyc_status: String,
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub kyc_investor_type: Option<String>,
    pub accredited_investor: bool,
    #[schema(value_type = Option<String>, example = "100000.50")]
    pub investment_capacity: Option<rust_decimal::Decimal>,
    pub regulatory_jurisdiction: String,
    pub profile_created_at: DateTime<Utc>,
    pub profile_updated_at: DateTime<Utc>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub jurisdiction: Option<String>,
    pub wallet_address: Option<String>,
}

/// Merchant profile response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MerchantProfileResponse {
    pub profile_id: String,
    pub merchant_id: String,
    pub user_id: String,
    pub email: String,
    pub user_type: UserType,
    pub kyc_status: String,
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub legal_name: String,
    pub dba: Option<String>,
    pub tax_id: Option<String>,
    pub mcc: Option<String>,
    pub country: String,
    pub business_type: Option<String>,
    pub industry: Option<String>,
    pub years_in_business: Option<i32>,
    pub employee_count: Option<i32>,
    #[schema(value_type = Option<String>, example = "50000.00")]
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    pub business_description: Option<String>,
    pub profile_completed: bool,
    pub merchant_created_at: DateTime<Utc>,
    pub merchant_updated_at: DateTime<Utc>,
    pub profile_created_at: Option<DateTime<Utc>>,
    pub profile_updated_at: Option<DateTime<Utc>>,
    pub jurisdiction: Option<String>,
}

/// Request to update merchant profile information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateMerchantProfileRequest {
    #[schema(example = "Updated Business LLC")]
    pub legal_name: Option<String>,
    #[schema(example = "Updated Store")]
    pub dba: Option<String>,
    #[schema(example = "5912")]
    pub mcc: Option<String>,
    #[schema(example = "CAN")]
    pub country: Option<String>,
    #[schema(example = "CORP")]
    pub business_type: Option<String>,
    #[schema(example = "Retail")]
    pub industry: Option<String>,
    #[schema(example = 5)]
    pub years_in_business: Option<i32>,
    #[schema(example = 25)]
    pub employee_count: Option<i32>,
    #[schema(example = "150000.00", value_type = Option<String>)]
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    #[schema(example = "We provide high-quality retail services to customers.")]
    pub business_description: Option<String>,
}

/// Request to update merchant profile
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateMerchantRequest {
    pub legal_name: Option<String>,
    pub dba: Option<String>,
    pub mcc: Option<String>,
    pub country: Option<String>,
    pub business_type: Option<String>,
    pub industry: Option<String>,
    pub years_in_business: Option<i32>,
    pub employee_count: Option<i32>,
    #[schema(value_type = Option<String>, example = "60000.00")]
    pub monthly_revenue_estimate: Option<rust_decimal::Decimal>,
    pub business_description: Option<String>,
}

/// Request to update investor profile
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateInvestorProfileRequest {
    pub accredited_investor: Option<bool>,
    #[schema(value_type = Option<String>, example = "100000.50")]
    pub investment_capacity: Option<rust_decimal::Decimal>,
    pub regulatory_jurisdiction: Option<String>,
}

// ============================================================================
// PLACEHOLDER ENDPOINTS (TO BE IMPLEMENTED)
// ============================================================================

/// Register a new merchant
#[utoipa::path(
    post,
    path = "/api/v1/auth/register/merchant",
    request_body = RegisterMerchantRequest,
    responses(
        (status = 201, description = "Merchant registered successfully", body = RegisterMerchantResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 409, description = "Merchant with this email already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant"
)]
#[instrument(skip(state), level = "info")]
pub async fn register_merchant_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<RegisterMerchantRequest>,
) -> Result<Json<RegisterMerchantResponse>, (StatusCode, String)> {
    info!("🏪 API: Registering merchant: {}", request.email);

    // Validate email format (basic validation)
    if !request.email.contains('@') || !request.email.contains('.') {
        warn!(
            "❌ API: Invalid email format for merchant: {}",
            request.email
        );
        return Err((StatusCode::BAD_REQUEST, "Invalid email format".to_string()));
    }

    // For merchants, use empty password (no authentication required)
    let password_hash = String::new();

    // Register the user first as merchant
    match register_or_reregister_user(&state.pool, "originator", &request.email).await {
        Ok(result) => {
            let identity = match result {
                RegisterResult::Created(identity) => identity,
                RegisterResult::Reregistered(identity) => identity,
                RegisterResult::AlreadyExists => {
                    // Email exists - get the existing identity and check if merchant record exists
                    let existing_identity =
                        match get_identity_by_email(&state.pool, &request.email).await {
                            Ok(Some(identity)) => identity,
                            Ok(None) => {
                                return Err((
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Identity not found after registration check".to_string(),
                                ));
                            }
                            Err(e) => {
                                error!("❌ API: Failed to get existing identity: {}", e);
                                return Err((
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to retrieve existing identity".to_string(),
                                ));
                            }
                        };

                    // Check if merchant already exists for this user
                    match get_merchant_by_user_id(&state.pool, existing_identity.id).await {
                        Ok(Some(merchant)) => {
                            // Merchant already exists - return success
                            info!(
                                "✅ API: Merchant already exists for user: user_id={}, merchant_id={}",
                                existing_identity.id, merchant.id
                            );
                            return Ok(Json(RegisterMerchantResponse {
                                user_id: existing_identity.id.to_string(),
                                merchant_id: merchant.id.to_string(),
                                user_type: UserType::Originator,
                                legal_name: merchant.legal_name,
                                kyc_status: existing_identity.kyc_status,
                                created_at: existing_identity.created_at,
                            }));
                        }
                        Ok(None) => {
                            // Identity exists but merchant doesn't - create merchant record
                            info!(
                                "🔄 API: Identity exists but merchant missing - creating merchant record for user: {}",
                                existing_identity.id
                            );
                            existing_identity
                        }
                        Err(e) => {
                            error!("❌ API: Failed to check existing merchant: {}", e);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Failed to check existing merchant".to_string(),
                            ));
                        }
                    }
                }
            };

            // Update additional profile fields if provided
            if request.jurisdiction.is_some() {
                let update_request = UpdateUserRequest {
                    email: None,
                    user_type: None,
                    first_name: None,
                    last_name: None,
                    jurisdiction: request.jurisdiction.clone(),
                    wallet_address: None,
                };

                let user_type_str = update_request.user_type.as_ref().map(|t| t.to_string());
                if let Err(e) = update_identity_profile(
                    &state.pool,
                    identity.id,
                    update_request.email.as_deref(),
                    user_type_str.as_deref(),
                    update_request.jurisdiction.as_deref(),
                    update_request.first_name.as_deref(),
                    update_request.last_name.as_deref(),
                    update_request.wallet_address.as_deref(),
                )
                .await
                {
                    warn!("Failed to update merchant profile fields: {}", e);
                }
            }

            // Create merchant account with business information
            let merchant_request = CreateMerchantRequest {
                legal_name: request.legal_name.clone(),
                dba: request.dba.clone(),
                tax_id: request.tax_id.clone(),
                mcc: request.mcc.clone(),
                country: request.country.clone(),
                business_type: request.business_type.clone(),
            };

            match create_merchant(&state.pool, identity.id, &merchant_request).await {
                Ok(merchant) => {
                    info!(
                        "✅ API: Merchant registered successfully: user_id={}, merchant_id={}, legal_name={}",
                        identity.id, merchant.id, merchant.legal_name
                    );

                    Ok(Json(RegisterMerchantResponse {
                        user_id: identity.id.to_string(),
                        merchant_id: merchant.id.to_string(),
                        user_type: UserType::Originator,
                        legal_name: merchant.legal_name,
                        kyc_status: identity.kyc_status,
                        created_at: identity.created_at,
                    }))
                }
                Err(e) => {
                    error!("❌ API: Failed to create merchant account: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to create merchant account".to_string(),
                    ))
                }
            }
        }
        Err(e) => {
            error!("❌ API: Failed to register merchant user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to register merchant".to_string(),
            ))
        }
    }
}

/// Get merchant profile
#[utoipa::path(
    get,
    path = "/api/v1/merchant/profile",
    params(MerchantProfileQuery),
    responses(
        (status = 200, description = "Merchant profile retrieved successfully", body = MerchantProfileResponse),
        (status = 400, description = "Bad request - invalid parameters"),
        (status = 404, description = "Merchant profile not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchants"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_merchant_profile_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<MerchantProfileQuery>,
) -> Result<Json<MerchantProfileResponse>, (StatusCode, String)> {
    // Validate query parameters - exactly one of user_id or email must be provided
    let (user_id, search_type) = match (query.user_id, query.email) {
        (Some(user_id_str), None) => {
            // Parse user_id as UUID
            let user_id = uuid::Uuid::parse_str(&user_id_str).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid user_id format - must be a valid UUID".to_string(),
                )
            })?;
            (user_id, format!("user_id: {}", user_id_str))
        }
        (None, Some(email)) => {
            // For email, we need to get the user_id from the identity first
            let identity = match get_identity_by_email(&state.pool, &email).await {
                Ok(Some(identity)) => identity,
                Ok(None) => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        format!("No user found with email: {}", email),
                    ));
                }
                Err(e) => {
                    error!("❌ API: Failed to get identity by email: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to lookup user by email".to_string(),
                    ));
                }
            };
            (identity.id, format!("email: {}", email))
        }
        (Some(_), Some(_)) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Provide either user_id OR email, not both".to_string(),
            ));
        }
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Either user_id or email parameter is required".to_string(),
            ));
        }
    };

    info!(
        "🔍 API: Getting merchant profile for {} ({})",
        search_type, user_id
    );

    // Get the merchant by user ID
    match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => {
            // Get the user identity for additional info
            let identity = match get_identity_by_id(&state.pool, merchant.user_id).await {
                Ok(Some(identity)) => identity,
                Ok(None) => {
                    warn!(
                        "❌ API: Identity not found for merchant user: {}",
                        merchant.user_id
                    );
                    return Err((StatusCode::NOT_FOUND, "User identity not found".to_string()));
                }
                Err(e) => {
                    error!("❌ API: Failed to get identity for merchant: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get user identity".to_string(),
                    ));
                }
            };

            // Try to get the merchant profile (optional)
            let profile = match get_merchant_profile_by_merchant_id(&state.pool, merchant.id).await
            {
                Ok(profile_opt) => profile_opt,
                Err(e) => {
                    warn!(
                        "⚠️ API: Failed to get merchant profile (continuing without): {}",
                        e
                    );
                    None
                }
            };

            let response = MerchantProfileResponse {
                profile_id: profile
                    .as_ref()
                    .map(|p| p.id.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                merchant_id: merchant.id.to_string(),
                user_id: merchant.user_id.to_string(),
                email: identity.email,
                user_type: UserType::Originator,
                kyc_status: identity.kyc_status,
                kyc_submitted_at: identity.kyc_submitted_at,
                kyc_verified_at: identity.kyc_verified_at,
                legal_name: merchant.legal_name,
                dba: merchant.dba,
                tax_id: merchant.tax_id,
                mcc: merchant.mcc,
                country: merchant.country,
                business_type: merchant.business_type,
                industry: profile.as_ref().and_then(|p| p.industry.clone()),
                years_in_business: profile.as_ref().and_then(|p| p.years_in_business),
                employee_count: profile.as_ref().and_then(|p| p.employee_count),
                monthly_revenue_estimate: profile.as_ref().and_then(|p| p.monthly_revenue_estimate),
                business_description: profile
                    .as_ref()
                    .and_then(|p| p.business_description.clone()),
                profile_completed: profile
                    .as_ref()
                    .map(|p| p.profile_completed)
                    .unwrap_or(false),
                merchant_created_at: merchant.created_at,
                merchant_updated_at: merchant.updated_at,
                profile_created_at: profile.as_ref().map(|p| p.created_at),
                profile_updated_at: profile.as_ref().map(|p| p.updated_at),
                jurisdiction: None, // This would come from identity if needed
            };

            info!(
                "✅ API: Merchant profile retrieved successfully: {}",
                merchant.id
            );
            Ok(Json(response))
        }
        Ok(None) => {
            warn!("❌ API: Merchant not found for user: {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found".to_string(),
            ))
        }
        Err(e) => {
            error!("❌ API: Failed to get merchant: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve merchant profile".to_string(),
            ))
        }
    }
}

/// Update merchant profile
#[utoipa::path(
    put,
    path = "/api/v1/merchant/profile",
    request_body = UpdateMerchantProfileRequest,
    responses(
        (status = 200, description = "Merchant profile updated successfully", body = MerchantProfileResponse),
        (status = 404, description = "Merchant profile not found"),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchants"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_merchant_profile_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<UpdateMerchantProfileRequest>,
) -> Result<Json<MerchantProfileResponse>, (StatusCode, String)> {
    // For this implementation, we'll use a hardcoded user_id for testing
    // In a real implementation, this would come from authentication middleware
    let test_user_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid UUID".to_string(),
            )
        })?;

    info!(
        "🔄 API: Updating merchant profile for test user: {}",
        test_user_id
    );

    // Get the merchant by user ID first to ensure it exists
    let merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("❌ API: Merchant not found for user: {}", test_user_id);
            return Err((StatusCode::NOT_FOUND, "Merchant not found".to_string()));
        }
        Err(e) => {
            error!("❌ API: Failed to get merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get merchant".to_string(),
            ));
        }
    };

    // Convert API request to database request
    let db_request = DbUpdateMerchantProfileRequest {
        legal_name: request.legal_name,
        dba: request.dba,
        mcc: request.mcc,
        country: request.country,
        business_type: request.business_type,
        industry: request.industry,
        years_in_business: request.years_in_business,
        employee_count: request.employee_count,
        monthly_revenue_estimate: request.monthly_revenue_estimate,
        business_description: request.business_description,
    };

    // Update the merchant profile
    match update_merchant_profile(&state.pool, merchant.id, &db_request).await {
        Ok(_) => {
            info!(
                "✅ API: Merchant profile updated successfully: {}",
                merchant.id
            );

            // Return the updated profile by calling the get endpoint
            // Get the user identity for additional info
            let identity = match get_identity_by_id(&state.pool, merchant.user_id).await {
                Ok(Some(identity)) => identity,
                Ok(None) => {
                    warn!(
                        "❌ API: Identity not found for merchant user: {}",
                        merchant.user_id
                    );
                    return Err((StatusCode::NOT_FOUND, "User identity not found".to_string()));
                }
                Err(e) => {
                    error!("❌ API: Failed to get identity for merchant: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get user identity".to_string(),
                    ));
                }
            };

            // Get the updated merchant (refetch from database)
            let updated_merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
                Ok(Some(merchant)) => merchant,
                Ok(None) => {
                    warn!(
                        "❌ API: Updated merchant not found for user: {}",
                        test_user_id
                    );
                    return Err((
                        StatusCode::NOT_FOUND,
                        "Updated merchant not found".to_string(),
                    ));
                }
                Err(e) => {
                    error!("❌ API: Failed to get updated merchant: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get updated merchant".to_string(),
                    ));
                }
            };

            // Get the updated merchant profile (optional)
            let profile =
                match get_merchant_profile_by_merchant_id(&state.pool, updated_merchant.id).await {
                    Ok(profile_opt) => profile_opt,
                    Err(e) => {
                        warn!(
                        "⚠️ API: Failed to get updated merchant profile (continuing without): {}",
                        e
                    );
                        None
                    }
                };

            let response = MerchantProfileResponse {
                profile_id: profile
                    .as_ref()
                    .map(|p| p.id.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                merchant_id: updated_merchant.id.to_string(),
                user_id: updated_merchant.user_id.to_string(),
                email: identity.email,
                user_type: UserType::Originator,
                kyc_status: identity.kyc_status,
                kyc_submitted_at: identity.kyc_submitted_at,
                kyc_verified_at: identity.kyc_verified_at,
                legal_name: updated_merchant.legal_name,
                dba: updated_merchant.dba,
                tax_id: updated_merchant.tax_id,
                mcc: updated_merchant.mcc,
                country: updated_merchant.country,
                business_type: updated_merchant.business_type,
                industry: profile.as_ref().and_then(|p| p.industry.clone()),
                years_in_business: profile.as_ref().and_then(|p| p.years_in_business),
                employee_count: profile.as_ref().and_then(|p| p.employee_count),
                monthly_revenue_estimate: profile.as_ref().and_then(|p| p.monthly_revenue_estimate),
                business_description: profile
                    .as_ref()
                    .and_then(|p| p.business_description.clone()),
                profile_completed: profile
                    .as_ref()
                    .map(|p| p.profile_completed)
                    .unwrap_or(false),
                merchant_created_at: updated_merchant.created_at,
                merchant_updated_at: updated_merchant.updated_at,
                profile_created_at: profile.as_ref().map(|p| p.created_at),
                profile_updated_at: profile.as_ref().map(|p| p.updated_at),
                jurisdiction: None,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to update merchant profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update merchant profile".to_string(),
            ))
        }
    }
}

/// Get investor profile
#[utoipa::path(
    get,
    path = "/api/v1/investor/profile",
    params(InvestorProfileQuery),
    responses(
        (status = 200, description = "Investor profile retrieved successfully", body = InvestorProfileResponse),
        (status = 400, description = "Invalid request parameters - exactly one of user_id or email must be provided"),
        (status = 404, description = "Investor profile not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Investors",
    security(
        ("jwt_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn get_investor_profile_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<InvestorProfileQuery>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    // Validate that exactly one of user_id or email is provided
    match (&query.user_id, &query.email) {
        (Some(_), Some(_)) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Exactly one of user_id or email must be provided, not both".to_string(),
            ))
        }
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Exactly one of user_id or email must be provided".to_string(),
            ))
        }
        _ => {}
    }

    // Parse user_id if provided
    let user_id_option = if let Some(user_id_str) = &query.user_id {
        Some(uuid::Uuid::parse_str(user_id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid user ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    let email_option = query.email.as_deref();

    info!(
        "🔍 API: Getting investor profile for user_id: {:?}, email: {:?}",
        user_id_option, email_option
    );

    match get_investor_profile_by_user_id_or_email(&state.pool, user_id_option, email_option).await
    {
        Ok(Some((profile, identity))) => Ok(Json(InvestorProfileResponse {
            profile_id: profile.id.to_string(),
            user_id: profile.user_id.to_string(),
            email: identity.email,
            user_type: UserType::Investor,
            kyc_status: identity.kyc_status,
            kyc_submitted_at: identity.kyc_submitted_at,
            kyc_verified_at: identity.kyc_verified_at,
            kyc_investor_type: identity.kyc_investor_type.map(|t| t.to_string()),
            accredited_investor: profile.accredited_investor,
            investment_capacity: profile.investment_capacity,
            regulatory_jurisdiction: profile.regulatory_jurisdiction,
            profile_created_at: profile.created_at,
            profile_updated_at: profile.updated_at,
            first_name: identity.first_name,
            last_name: identity.last_name,
            jurisdiction: identity.jurisdiction,
            wallet_address: identity.wallet_address,
        })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            "Investor profile not found".to_string(),
        )),
        Err(e) => {
            error!("❌ API: Failed to get investor profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get investor profile".to_string(),
            ))
        }
    }
}

/// Update investor profile
#[utoipa::path(
    put,
    path = "/api/v1/investor/profile",
    request_body = UpdateInvestorProfileRequest,
    responses(
        (status = 200, description = "Investor profile updated successfully", body = InvestorProfileResponse),
        (status = 404, description = "Investor profile not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Investors",
    security(
        ("jwt_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn update_investor_profile_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<UpdateInvestorProfileRequest>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    // Extract user ID from X-User-ID header (temporary until JWT middleware is implemented)
    let user_id_str = headers
        .get("X-User-ID")
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing user ID header".to_string(),
        ))?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid user ID header".to_string(),
            )
        })?;

    let user_id = uuid::Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    info!("📝 API: Updating investor profile for user: {}", user_id);

    // Convert to database update request
    let db_request = DbUpdateInvestorProfileRequest {
        accredited_investor: request.accredited_investor,
        investment_capacity: request.investment_capacity,
        regulatory_jurisdiction: request.regulatory_jurisdiction,
    };

    match update_investor_profile(&state.pool, user_id, &db_request).await {
        Ok(profile) => {
            // Get the updated profile with identity info for full response
            match get_investor_profile_with_identity(&state.pool, user_id).await {
                Ok(Some((_, identity))) => Ok(Json(InvestorProfileResponse {
                    profile_id: profile.id.to_string(),
                    user_id: profile.user_id.to_string(),
                    email: identity.email,
                    user_type: UserType::Investor,
                    kyc_status: identity.kyc_status,
                    kyc_submitted_at: identity.kyc_submitted_at,
                    kyc_verified_at: identity.kyc_verified_at,
                    kyc_investor_type: identity.kyc_investor_type.map(|t| t.to_string()),
                    accredited_investor: profile.accredited_investor,
                    investment_capacity: profile.investment_capacity,
                    regulatory_jurisdiction: profile.regulatory_jurisdiction,
                    profile_created_at: profile.created_at,
                    profile_updated_at: profile.updated_at,
                    first_name: identity.first_name,
                    last_name: identity.last_name,
                    jurisdiction: identity.jurisdiction,
                    wallet_address: identity.wallet_address,
                })),
                Ok(None) => Err((
                    StatusCode::NOT_FOUND,
                    "Investor profile not found".to_string(),
                )),
                Err(e) => {
                    error!("❌ API: Failed to get updated investor profile: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get updated investor profile".to_string(),
                    ))
                }
            }
        }
        Err(e) => {
            if e.to_string().contains("not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    "Investor profile not found".to_string(),
                ))
            } else {
                error!("❌ API: Failed to update investor profile: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update investor profile".to_string(),
                ))
            }
        }
    }
}

/// Register a new investor
#[utoipa::path(
    post,
    path = "/api/v1/auth/register/investor",
    request_body = RegisterInvestorRequest,
    responses(
        (status = 201, description = "Investor registered successfully", body = RegisterInvestorResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 409, description = "Investor profile already exists for this email"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Investor"
)]
#[instrument(skip(state), level = "info")]
pub async fn register_investor_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<RegisterInvestorRequest>,
) -> Result<Json<RegisterInvestorResponse>, (StatusCode, String)> {
    info!("💼 API: Registering new investor: email={}", request.email);

    // Validate email format (basic validation)
    if !request.email.contains('@') || !request.email.contains('.') {
        warn!(
            "❌ API: Invalid email format for investor: {}",
            request.email
        );
        return Err((StatusCode::BAD_REQUEST, "Invalid email format".to_string()));
    }

    // First, check if email already exists
    let existing_identity = match get_identity_by_email(&state.pool, &request.email).await {
        Ok(Some(identity)) => {
            // Email exists, check if user already has an investor profile
            match get_investor_profile_by_user_id(&state.pool, identity.id).await {
                Ok(Some(_existing_profile)) => {
                    // User already has an investor profile - return conflict
                    warn!(
                        "❌ API: Investor profile already exists for email: {}",
                        request.email
                    );
                    return Err((
                        StatusCode::CONFLICT,
                        "Investor profile already exists for this email".to_string(),
                    ));
                }
                Ok(None) => {
                    // User exists but no investor profile - use existing identity
                    info!(
                        "🔄 API: User exists but no investor profile - creating investor profile for existing user: {}",
                        identity.id
                    );
                    identity
                }
                Err(e) => {
                    error!("❌ API: Failed to check existing investor profile: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to check existing investor profile".to_string(),
                    ));
                }
            }
        }
        Ok(None) => {
            // Email doesn't exist - create new user
            match register_or_reregister_user(&state.pool, "investor", &request.email).await {
                Ok(result) => {
                    match result {
                        RegisterResult::Created(identity) => identity,
                        RegisterResult::Reregistered(identity) => identity,
                        RegisterResult::AlreadyExists => {
                            // This shouldn't happen since we already checked email doesn't exist
                            error!("❌ API: Unexpected AlreadyExists result when email should not exist");
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Unexpected registration conflict".to_string(),
                            ));
                        }
                    }
                }
                Err(e) => {
                    error!("❌ API: Failed to register new investor user: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to register investor".to_string(),
                    ));
                }
            }
        }
        Err(e) => {
            error!("❌ API: Failed to check existing identity: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to check existing user".to_string(),
            ));
        }
    };

    // At this point we have an identity (either existing or newly created)
    let identity = existing_identity;

    // Update additional profile fields if provided
    if request.first_name.is_some() || request.last_name.is_some() || request.jurisdiction.is_some()
    {
        let update_request = UpdateUserRequest {
            email: None,
            user_type: None,
            first_name: request.first_name.clone(),
            last_name: request.last_name.clone(),
            jurisdiction: request.jurisdiction.clone(),
            wallet_address: None,
        };

        let user_type_str = update_request.user_type.as_ref().map(|t| t.to_string());
        if let Err(e) = update_identity_profile(
            &state.pool,
            identity.id,
            update_request.email.as_deref(),
            user_type_str.as_deref(),
            update_request.jurisdiction.as_deref(),
            update_request.first_name.as_deref(),
            update_request.last_name.as_deref(),
            update_request.wallet_address.as_deref(),
        )
        .await
        {
            warn!("Failed to update investor profile fields: {}", e);
        }
    }

    // Create investor profile
    let profile_request = CreateInvestorProfileRequest {
        accredited_investor: request.accredited_investor,
        investment_capacity: request.investment_capacity,
        regulatory_jurisdiction: request.regulatory_jurisdiction,
    };

    match create_investor_profile(&state.pool, identity.id, &profile_request).await {
        Ok(profile) => {
            info!(
                "✅ API: Investor profile {} created for user {}",
                profile.id, identity.id
            );

            Ok(Json(RegisterInvestorResponse {
                user_id: identity.id.to_string(),
                user_type: UserType::Investor,
                kyc_status: identity.kyc_status,
                created_at: identity.created_at,
                profile_id: profile.id.to_string(),
                accredited_investor: profile.accredited_investor,
                regulatory_jurisdiction: profile.regulatory_jurisdiction,
            }))
        }
        Err(e) => {
            error!("❌ API: Failed to create investor profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create investor profile".to_string(),
            ))
        }
    }
}
