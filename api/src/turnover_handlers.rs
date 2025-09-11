//! Turnover & Analytics handlers for the Arda Credit API
//!
//! This module contains all merchant turnover-related API endpoints including:
//! - Turnover data submission and retrieval
//! - Turnover analytics and insights
//! - Merchant profile completion tracking
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::types::ApiState;
use arda_credit_db::{
    create_or_update_turnover, get_merchant_by_user_id, get_merchant_profile_completion,
    get_merchant_turnover, get_merchant_turnover_analytics, update_turnover,
    CreateTurnoverRequest, MerchantTurnoverDaily, MerchantTurnoverAnalytics,
    MerchantProfileCompletion, UpdateTurnoverRequest,
};
use uuid::Uuid;

// ============================================================================
// TURNOVER MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Create new turnover record request
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct TurnoverCreateRequest {
    pub date: chrono::NaiveDate,
    pub gross_receipts: String,  // Decimal as string for API
    pub refunds: Option<String>, // Optional decimal as string
    pub source: Option<String>,
    pub verification_method: Option<String>,
}

impl From<TurnoverCreateRequest> for CreateTurnoverRequest {
    fn from(req: TurnoverCreateRequest) -> Self {
        Self {
            date: req.date,
            gross_receipts: req.gross_receipts.parse().unwrap_or_default(),
            refunds: req.refunds.and_then(|s| s.parse().ok()),
            source: req.source,
            verification_method: req.verification_method,
        }
    }
}

/// Request to update existing turnover data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct TurnoverUpdateRequest {
    pub gross_receipts: Option<String>, // Optional decimal as string
    pub refunds: Option<String>,
    pub verification_method: Option<String>,
}

impl From<TurnoverUpdateRequest> for UpdateTurnoverRequest {
    fn from(req: TurnoverUpdateRequest) -> Self {
        Self {
            gross_receipts: req.gross_receipts.and_then(|s| s.parse().ok()),
            refunds: req.refunds.and_then(|s| s.parse().ok()),
            verification_method: req.verification_method,
        }
    }
}

// ============================================================================
// MERCHANT TURNOVER RESPONSE TYPES (for OpenAPI)
// ============================================================================

/// Turnover record response for API
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct TurnoverResponse {
    pub id: String,
    pub merchant_id: String,
    pub date: chrono::NaiveDate,
    pub gross_receipts: String,
    pub refunds: String,
    pub net_receipts: String,
    pub source: String,
    pub verified: bool,
    pub verification_method: Option<String>,
    pub currency: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<MerchantTurnoverDaily> for TurnoverResponse {
    fn from(turnover: MerchantTurnoverDaily) -> Self {
        Self {
            id: turnover.id.to_string(),
            merchant_id: turnover.merchant_id.to_string(),
            date: turnover.date,
            gross_receipts: turnover.gross_receipts.to_string(),
            refunds: turnover.refunds.to_string(),
            net_receipts: turnover.net_receipts.to_string(),
            source: turnover.source,
            verified: turnover.verified,
            verification_method: turnover.verification_method,
            currency: turnover.currency,
            created_at: turnover.created_at,
            updated_at: turnover.updated_at,
        }
    }
}

/// Turnover analytics response for API
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct TurnoverAnalyticsResponse {
    pub merchant_id: String,
    pub legal_name: String,
    pub total_days_recorded: i64,
    pub verified_days: i64,
    pub total_net_receipts: Option<String>,
    pub avg_daily_receipts: Option<String>,
    pub last_turnover_date: Option<chrono::NaiveDate>,
    pub first_turnover_date: Option<chrono::NaiveDate>,
    pub monthly_estimate: Option<String>,
    pub data_quality_score: i32,
}

impl From<MerchantTurnoverAnalytics> for TurnoverAnalyticsResponse {
    fn from(analytics: MerchantTurnoverAnalytics) -> Self {
        Self {
            merchant_id: analytics.merchant_id.to_string(),
            legal_name: analytics.legal_name,
            total_days_recorded: analytics.total_days_recorded,
            verified_days: analytics.verified_days,
            total_net_receipts: analytics.total_net_receipts.map(|d| d.to_string()),
            avg_daily_receipts: analytics.avg_daily_receipts.map(|d| d.to_string()),
            last_turnover_date: analytics.last_turnover_date,
            first_turnover_date: analytics.first_turnover_date,
            monthly_estimate: analytics.monthly_estimate.map(|d| d.to_string()),
            data_quality_score: analytics.data_quality_score,
        }
    }
}

/// Profile completion response for API
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ProfileCompletionResponse {
    pub merchant_id: String,
    pub legal_name: String,
    pub kyc_status: String,
    pub has_legal_name: bool,
    pub has_tax_id: bool,
    pub has_mcc: bool,
    pub has_industry: bool,
    pub has_years_in_business: bool,
    pub has_revenue_estimate: bool,
    pub kyc_approved: bool,
    pub has_sufficient_turnover_history: bool,
    pub has_verified_turnover_data: bool,
    pub has_bank_statement: bool,
    pub completion_score: i32,
}

impl From<MerchantProfileCompletion> for ProfileCompletionResponse {
    fn from(completion: MerchantProfileCompletion) -> Self {
        Self {
            merchant_id: completion.merchant_id.to_string(),
            legal_name: completion.legal_name,
            kyc_status: completion.kyc_status,
            has_legal_name: completion.has_legal_name,
            has_tax_id: completion.has_tax_id,
            has_mcc: completion.has_mcc,
            has_industry: completion.has_industry,
            has_years_in_business: completion.has_years_in_business,
            has_revenue_estimate: completion.has_revenue_estimate,
            kyc_approved: completion.kyc_approved,
            has_sufficient_turnover_history: completion.has_sufficient_turnover_history,
            has_verified_turnover_data: completion.has_verified_turnover_data,
            has_bank_statement: completion.has_bank_statement,
            completion_score: completion.completion_score,
        }
    }
}

// ============================================================================
// TURNOVER MANAGEMENT ENDPOINTS
// ============================================================================

/// Create new turnover record
#[utoipa::path(
    post,
    path = "/api/merchant/turnover",
    request_body = TurnoverCreateRequest,
    responses(
        (status = 201, description = "Turnover data submitted successfully", body = TurnoverResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only merchants can submit turnover data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant Turnover"
)]
#[instrument(skip(state), level = "info")]
pub async fn create_turnover_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<TurnoverCreateRequest>,
) -> Result<Json<TurnoverResponse>, (StatusCode, String)> {
    info!(
        "💰 Creating turnover record for merchant: date={}",
        request.date
    );

    // Extract user ID from X-User-ID header
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

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Get merchant record
    let merchant = match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("Merchant profile not found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found. Please complete merchant registration first."
                    .to_string(),
            ));
        }
        Err(e) => {
            error!("Database error fetching merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch merchant profile: {}", e),
            ));
        }
    };

    // Parse and validate turnover data
    let gross_receipts: rust_decimal::Decimal = match request.gross_receipts.parse() {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid gross_receipts format".to_string(),
            ));
        }
    };

    if gross_receipts < rust_decimal::Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Gross receipts cannot be negative".to_string(),
        ));
    }

    if let Some(ref refunds_str) = request.refunds {
        let refunds: rust_decimal::Decimal = match refunds_str.parse() {
            Ok(val) => val,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid refunds format".to_string(),
                ));
            }
        };

        if refunds < rust_decimal::Decimal::ZERO {
            return Err((
                StatusCode::BAD_REQUEST,
                "Refunds cannot be negative".to_string(),
            ));
        }
        if refunds > gross_receipts {
            return Err((
                StatusCode::BAD_REQUEST,
                "Refunds cannot exceed gross receipts".to_string(),
            ));
        }
    }

    // Convert to database request type
    let db_request = request.into();

    // Create or update turnover record
    let turnover = match create_or_update_turnover(&state.pool, merchant.id, &db_request).await {
        Ok(turnover) => turnover,
        Err(e) => {
            error!("Failed to create/update turnover: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save turnover data: {}", e),
            ));
        }
    };

    info!(
        "✅ Turnover record created/updated for merchant: {}",
        merchant.legal_name
    );
    Ok(Json(turnover.into()))
}

/// Get turnover data for a merchant with optional date filtering
#[utoipa::path(
    get,
    path = "/api/merchant/turnover",
    params(
        ("start_date" = Option<String>, Query, description = "Start date filter (YYYY-MM-DD format)"),
        ("end_date" = Option<String>, Query, description = "End date filter (YYYY-MM-DD format)"),
        ("limit" = Option<i32>, Query, description = "Maximum number of records to return"),
    ),
    responses(
        (status = 200, description = "Turnover data retrieved successfully", body = Vec<TurnoverResponse>),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only merchants can access turnover data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant Turnover"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_turnover_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<TurnoverResponse>>, (StatusCode, String)> {
    // Extract user ID from X-User-ID header
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

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    info!(
        "📊 Fetching turnover data for merchant: user_id={}",
        user_id
    );

    // Get merchant record
    let merchant = match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("Merchant profile not found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found".to_string(),
            ));
        }
        Err(e) => {
            error!("Database error fetching merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch merchant profile: {}", e),
            ));
        }
    };

    // Parse query parameters
    let start_date = params
        .get("start_date")
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let end_date = params
        .get("end_date")
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let limit = params.get("limit").and_then(|s| s.parse::<i32>().ok());

    // Fetch turnover data
    let turnover_data = match get_merchant_turnover(
        &state.pool,
        merchant.id,
        start_date,
        end_date,
        None,
        limit.map(|l| l as i64),
        None,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to fetch turnover data: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve turnover data: {}", e),
            ));
        }
    };

    info!("✅ Retrieved {} turnover records", turnover_data.len());
    Ok(Json(turnover_data.into_iter().map(|t| t.into()).collect()))
}

/// Update specific turnover record by date
#[utoipa::path(
    put,
    path = "/api/merchant/turnover/{date}",
    params(
        ("date" = String, Path, description = "Date of turnover record to update (YYYY-MM-DD format)")
    ),
    request_body = TurnoverUpdateRequest,
    responses(
        (status = 200, description = "Turnover data updated successfully", body = TurnoverResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only merchants can update turnover data"),
        (status = 404, description = "Turnover record not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant Turnover"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_turnover_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(date_str): axum::extract::Path<String>,
    Json(request): Json<TurnoverUpdateRequest>,
) -> Result<Json<TurnoverResponse>, (StatusCode, String)> {
    info!("📝 Updating turnover record: date={}", date_str);

    // Extract user ID from X-User-ID header
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

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Parse date parameter
    let date = match chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid date format. Use YYYY-MM-DD".to_string(),
            ));
        }
    };

    // Get merchant record
    let merchant = match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("Merchant profile not found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found".to_string(),
            ));
        }
        Err(e) => {
            error!("Database error fetching merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch merchant profile: {}", e),
            ));
        }
    };

    // Parse and validate update request
    if let Some(ref gross_receipts_str) = request.gross_receipts {
        let gross_receipts: rust_decimal::Decimal = match gross_receipts_str.parse() {
            Ok(val) => val,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid gross_receipts format".to_string(),
                ));
            }
        };

        if gross_receipts < rust_decimal::Decimal::ZERO {
            return Err((
                StatusCode::BAD_REQUEST,
                "Gross receipts cannot be negative".to_string(),
            ));
        }
    }

    if let Some(ref refunds_str) = request.refunds {
        let refunds: rust_decimal::Decimal = match refunds_str.parse() {
            Ok(val) => val,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid refunds format".to_string(),
                ));
            }
        };

        if refunds < rust_decimal::Decimal::ZERO {
            return Err((
                StatusCode::BAD_REQUEST,
                "Refunds cannot be negative".to_string(),
            ));
        }
    }

    // Convert to database request type
    let db_request = request.into();

    // Update turnover record
    let turnover = match update_turnover(&state.pool, merchant.id, date, &db_request).await {
        Ok(Some(turnover)) => turnover,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("No turnover record found for date: {}", date_str),
            ));
        }
        Err(e) => {
            error!("Failed to update turnover: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update turnover data: {}", e),
            ));
        }
    };

    info!("✅ Turnover record updated for date: {}", date_str);
    Ok(Json(turnover.into()))
}

/// Get merchant turnover analytics and insights
#[utoipa::path(
    get,
    path = "/api/merchant/turnover/analytics",
    responses(
        (status = 200, description = "Analytics retrieved successfully", body = TurnoverAnalyticsResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only merchants can access analytics"),
        (status = 404, description = "Analytics not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant Analytics"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_turnover_analytics_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<TurnoverAnalyticsResponse>, (StatusCode, String)> {
    // Extract user ID from X-User-ID header
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

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    info!(
        "📈 Fetching turnover analytics for merchant: user_id={}",
        user_id
    );

    // Get merchant record
    let merchant = match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("Merchant profile not found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found".to_string(),
            ));
        }
        Err(e) => {
            error!("Database error fetching merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch merchant profile: {}", e),
            ));
        }
    };

    // Get analytics
    let analytics = match get_merchant_turnover_analytics(&state.pool, merchant.id).await {
        Ok(Some(analytics)) => analytics,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                "No turnover data available for analytics".to_string(),
            ));
        }
        Err(e) => {
            error!("Failed to fetch analytics: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve analytics: {}", e),
            ));
        }
    };

    info!(
        "✅ Analytics retrieved: quality_score={}%",
        analytics.data_quality_score
    );
    Ok(Json(analytics.into()))
}

/// Get merchant profile completion status
#[utoipa::path(
    get,
    path = "/api/merchant/profile/completion",
    responses(
        (status = 200, description = "Profile completion status retrieved", body = ProfileCompletionResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only merchants can access profile completion"),
        (status = 404, description = "Profile completion data not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchant Profile"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_profile_completion_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ProfileCompletionResponse>, (StatusCode, String)> {
    // Extract user ID from X-User-ID header
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

    let user_id = Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    info!(
        "🎯 Fetching profile completion for merchant: user_id={}",
        user_id
    );

    // Get merchant record
    let merchant = match get_merchant_by_user_id(&state.pool, user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("Merchant profile not found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                "Merchant profile not found".to_string(),
            ));
        }
        Err(e) => {
            error!("Database error fetching merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch merchant profile: {}", e),
            ));
        }
    };

    // Get profile completion data
    let completion = match get_merchant_profile_completion(&state.pool, merchant.id).await {
        Ok(Some(completion)) => completion,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                "Profile completion data not available".to_string(),
            ));
        }
        Err(e) => {
            error!("Failed to fetch profile completion: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve profile completion: {}", e),
            ));
        }
    };

    info!(
        "✅ Profile completion retrieved: score={}%",
        completion.completion_score
    );
    Ok(Json(completion.into()))
}
