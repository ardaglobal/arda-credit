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
use crate::rest::UserType;

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
    info!("🏪 API: Registering new merchant: email={}", request.email);
    
    // TODO: Implement full merchant registration from rest.rs
    Err((StatusCode::NOT_IMPLEMENTED, "Merchant registration not yet implemented".to_string()))
}

/// Register a new investor
#[utoipa::path(
    post,
    path = "/api/v1/auth/register/investor",
    request_body = RegisterInvestorRequest,
    responses(
        (status = 201, description = "Investor registered successfully", body = RegisterInvestorResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 409, description = "Investor with this email already exists"),
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
    
    // TODO: Implement full investor registration from rest.rs
    Err((StatusCode::NOT_IMPLEMENTED, "Investor registration not yet implemented".to_string()))
}
