//! Credit line request and management API handlers
//!
//! This module implements API handlers for credit line operations including:
//! - Credit line request submission
//! - Eligibility evaluation and pool matching
//! - Credit line approval/rejection processing
//! - Active credit line management

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::types::ApiState;
use arda_credit_db::{
    create_credit_line_request, get_all_liquidity_pools, get_credit_line_requests_by_merchant,
    get_merchant_by_user_id, get_merchant_profile_by_merchant_id, get_merchant_turnover_analytics,
};
use arda_credit_lib::{CreditEvaluator, KycStatus, MerchantProfile as LibMerchantProfile};

// ============================================================================
// API TYPES AND SCHEMAS
// ============================================================================

/// Request to submit a credit line application
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitCreditLineRequestBody {
    /// Requested credit amount in cents (e.g., 5000000 = $50,000)
    #[schema(minimum = 100_00, maximum = 1000000_00)]
    pub requested_amount: u64,
    /// Preferred maximum APR in basis points (optional, e.g., 1200 = 12%)
    #[schema(minimum = 500, maximum = 5000)]
    pub requested_rate_apr: Option<u64>,
    /// Preferred repayment tenor in days (default: 30)
    #[schema(minimum = 7, maximum = 365)]
    pub requested_tenor_days: Option<u32>,
    /// Description/justification for credit line request
    #[schema(max_length = 500)]
    pub request_description: Option<String>,
}

/// Credit line request response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreditLineRequestResponse {
    pub request_id: Uuid,
    pub merchant_id: Uuid,
    pub requested_amount: String, // Formatted amount with currency
    pub requested_rate_apr: Option<String>, // Formatted percentage
    pub requested_tenor_days: u32,
    pub status: String,
    pub ltv_ratio: Option<String>,        // Formatted percentage
    pub collateral_basis: Option<String>, // Formatted amount
    pub evaluation_score: Option<u8>,
    pub rejection_reason: Option<String>,
    pub matched_pool_name: Option<String>,
    pub approved_amount: Option<String>,
    pub approved_rate_apr: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Credit line eligibility check response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EligibilityCheckResponse {
    pub eligible: bool,
    pub overall_score: u8,
    pub checks: Vec<EligibilityCheckDetail>,
    pub required_actions: Vec<String>,
    pub estimated_collateral_basis: String,
    pub max_credit_amount: String,
}

/// Individual eligibility check detail
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EligibilityCheckDetail {
    pub check_name: String,
    pub passed: bool,
    pub description: String,
    pub weight: u8,
}

/// Available pools for merchant browsing
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AvailablePoolResponse {
    pub pool_id: Uuid,
    pub pool_name: String,
    pub max_ltv_ratio: String,       // Formatted percentage
    pub max_interest_rate: String,   // Formatted percentage
    pub available_liquidity: String, // Formatted amount
    pub repayment_period_days: u32,
    pub match_score: Option<u8>,
    pub estimated_approval_amount: Option<String>,
}

/// Pool browsing query parameters
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct PoolBrowsingQuery {
    /// Minimum required liquidity in cents
    #[param(minimum = 1000_00)]
    pub min_amount: Option<u64>,
    /// Maximum acceptable LTV ratio in basis points
    #[param(maximum = 9500)]
    pub max_ltv: Option<u64>,
    /// Maximum acceptable interest rate in basis points
    #[param(maximum = 5000)]
    pub max_rate: Option<u64>,
    /// Minimum repayment period in days
    #[param(minimum = 7)]
    pub min_repayment_days: Option<u32>,
}

/// Credit line request list query parameters
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct CreditLineRequestQuery {
    /// Filter by status
    // Filter by status
    pub status: Option<String>,
    /// Number of results per page
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Offset for pagination
    #[param(minimum = 0)]
    pub offset: Option<u32>,
}

/// Credit line requests list response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreditLineRequestListResponse {
    pub requests: Vec<CreditLineRequestResponse>,
    pub total_count: u32,
    pub has_more: bool,
}

/// Error response for credit line operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreditLineErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<String>,
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// Check credit line eligibility for merchant
#[utoipa::path(
    get,
    path = "/api/v1/credit-lines/eligibility",
    summary = "Check credit line eligibility",
    description = "Evaluate merchant eligibility for credit lines based on profile, turnover, and documents",
    tag = "Credit Lines",
    responses(
        (status = 200, description = "Eligibility evaluation completed", body = EligibilityCheckResponse),
        (status = 401, description = "Unauthorized - invalid JWT token"),
        (status = 403, description = "Forbidden - insufficient permissions"),
        (status = 404, description = "Merchant not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(state))]
pub async fn check_credit_eligibility(
    State(state): State<ApiState>,
) -> Result<Json<EligibilityCheckResponse>, (StatusCode, Json<CreditLineErrorResponse>)> {
    // For this implementation, we'll use a hardcoded user_id for testing
    // In a real implementation, this would come from authentication middleware
    let test_user_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Invalid UUID".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: Some("Invalid test user ID".to_string()),
                }),
            )
        })?;

    // Get merchant data
    let merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(CreditLineErrorResponse {
                    error: "Merchant not found".to_string(),
                    code: "MERCHANT_NOT_FOUND".to_string(),
                    details: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to get merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Get merchant profile
    let profile = match get_merchant_profile_by_merchant_id(&state.pool, merchant.id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(CreditLineErrorResponse {
                    error: "Merchant profile not found".to_string(),
                    code: "PROFILE_NOT_FOUND".to_string(),
                    details: Some(
                        "Complete your merchant profile before checking eligibility".to_string(),
                    ),
                }),
            ))
        }
        Err(e) => {
            error!("Failed to get merchant profile: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Get turnover analytics
    let _turnover_analytics = get_merchant_turnover_analytics(&state.pool, merchant.id)
        .await
        .unwrap_or_default();

    // Convert to lib types for evaluation
    let lib_profile = LibMerchantProfile {
        merchant_id: [0u8; 32], // Not used in evaluation
        industry: profile.industry.clone(),
        years_in_business: profile.years_in_business.map(|y| y as u32),
        employee_count: profile.employee_count.map(|c| c as u32),
        monthly_revenue_estimate: profile
            .monthly_revenue_estimate
            .map(|r| r.to_u64().unwrap_or(0))
            .unwrap_or(0),
        business_description: profile.business_description.clone(),
        profile_completed: profile.profile_completed,
    };

    // Mock turnover data for evaluation (in real implementation, get from database)
    let mock_turnover = vec![]; // TODO: Get actual turnover data

    // Mock document verification (in real implementation, get from database)
    let mock_documents = vec![]; // TODO: Get actual document data

    let kyc_status = match merchant.kyc_status.as_str() {
        "APPROVED" => KycStatus::Approved,
        "PENDING" => KycStatus::Pending,
        "REJECTED" => KycStatus::Rejected,
        _ => KycStatus::NotSubmitted,
    };

    // Perform eligibility evaluation
    let evaluator = CreditEvaluator::new();
    let merchant_record = arda_credit_lib::merchant::MerchantRecord {
        merchant_id: [0u8; 32], // Not used in evaluation
        mcc: merchant
            .mcc
            .as_ref()
            .map(|mcc| mcc.parse().unwrap_or(0))
            .unwrap_or(0),
        kyc_status: match kyc_status {
            KycStatus::Approved => 2,
            KycStatus::Pending => 1,
            KycStatus::Rejected => 3,
            _ => 0,
        },
        profile_completed: profile.profile_completed,
        collateral_basis: 0,
        last_turnover_update: 0,
        verified_documents: 0,
        credit_score: 0,
    };

    let eligibility = evaluator.validate_eligibility(
        &merchant_record,
        &lib_profile,
        &mock_turnover,
        &mock_documents,
        kyc_status,
    );

    // Calculate estimated collateral basis and max credit
    let collateral_basis = evaluator.calculate_collateral_basis(&mock_turnover);
    let max_credit = (collateral_basis.to_f64().unwrap_or(0.0) * 0.8) as u64; // 80% LTV max

    // Convert eligibility checks to API format
    let checks = eligibility
        .checks
        .iter()
        .map(|(name, passed)| EligibilityCheckDetail {
            check_name: name.clone(),
            passed: *passed,
            description: get_check_description(name),
            weight: get_check_weight(name),
        })
        .collect();

    // Determine required actions
    let mut required_actions = Vec::new();
    if !eligibility.eligible {
        for reason in &eligibility.reasons {
            required_actions.push(reason.clone());
        }
    }

    Ok(Json(EligibilityCheckResponse {
        eligible: eligibility.eligible,
        overall_score: eligibility.score,
        checks,
        required_actions,
        estimated_collateral_basis: format!(
            "${:.2}",
            collateral_basis.to_f64().unwrap_or(0.0) / 100.0
        ),
        max_credit_amount: format!("${:.2}", max_credit.to_f64().unwrap_or(0.0) / 100.0),
    }))
}

/// Submit credit line request
#[utoipa::path(
    post,
    path = "/api/v1/credit-lines/request",
    summary = "Submit credit line request",
    description = "Submit a credit line application with automatic evaluation and pool matching",
    tag = "Credit Lines",
    request_body = SubmitCreditLineRequestBody,
    responses(
        (status = 200, description = "Credit line request submitted successfully", body = CreditLineRequestResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized - invalid JWT token"),
        (status = 403, description = "Forbidden - insufficient permissions"),
        (status = 404, description = "Merchant not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(state, body))]
pub async fn submit_credit_line_request(
    State(state): State<ApiState>,
    Json(body): Json<SubmitCreditLineRequestBody>,
) -> Result<Json<CreditLineRequestResponse>, (StatusCode, Json<CreditLineErrorResponse>)> {
    // For this implementation, we'll use a hardcoded user_id for testing
    // In a real implementation, this would come from authentication middleware
    let test_user_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Invalid UUID".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: Some("Invalid test user ID".to_string()),
                }),
            )
        })?;

    // Validate request parameters
    if body.requested_amount < 10000 {
        // Minimum $100
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CreditLineErrorResponse {
                error: "Minimum credit line amount is $100".to_string(),
                code: "AMOUNT_TOO_LOW".to_string(),
                details: None,
            }),
        ));
    }

    if body.requested_amount > 100000000 {
        // Maximum $1M
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CreditLineErrorResponse {
                error: "Maximum credit line amount is $1,000,000".to_string(),
                code: "AMOUNT_TOO_HIGH".to_string(),
                details: None,
            }),
        ));
    }

    // Get merchant data
    let merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(CreditLineErrorResponse {
                    error: "Merchant not found".to_string(),
                    code: "MERCHANT_NOT_FOUND".to_string(),
                    details: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to get merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Create credit line request in database
    let request_data = arda_credit_db::CreateCreditLineRequestData {
        merchant_id: merchant.id,
        requested_amount: rust_decimal::Decimal::from(body.requested_amount as i64),
        requested_rate_apr: body
            .requested_rate_apr
            .map(|r| rust_decimal::Decimal::from(r as i64)),
        requested_tenor_days: body.requested_tenor_days.unwrap_or(30) as i32,
        request_description: body.request_description,
    };

    let request = match create_credit_line_request(&state.pool, request_data).await {
        Ok(request) => request,
        Err(e) => {
            error!("Failed to create credit line request: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Failed to create credit line request".to_string(),
                    code: "CREATION_FAILED".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // TODO: Trigger async evaluation and pool matching
    info!(
        "Credit line request submitted: {} for merchant: {}",
        request.id, merchant.id
    );

    // Convert to API response format
    let response = CreditLineRequestResponse {
        request_id: request.id,
        merchant_id: merchant.id,
        requested_amount: format!(
            "${:.2}",
            request.requested_amount.to_f64().unwrap_or(0.0) / 100.0
        ),
        requested_rate_apr: request
            .requested_rate_apr
            .map(|r| format!("{:.2}%", r.to_f64().unwrap_or(0.0) / 100.0)),
        requested_tenor_days: request.requested_tenor_days.unwrap_or(30) as u32,
        status: request.status.unwrap_or("UNKNOWN".to_string()),
        ltv_ratio: request
            .ltv_ratio
            .map(|l| format!("{:.2}%", l.to_f64().unwrap_or(0.0) / 100.0)),
        collateral_basis: request
            .collateral_basis
            .map(|c| format!("${:.2}", c.to_f64().unwrap_or(0.0) / 100.0)),
        evaluation_score: None, // TODO: Add evaluation score
        rejection_reason: request.rejection_reason,
        matched_pool_name: None, // TODO: Get pool name if matched
        approved_amount: request
            .approved_amount
            .map(|a| format!("${:.2}", a.to_f64().unwrap_or(0.0) / 100.0)),
        approved_rate_apr: request
            .approved_rate_apr
            .map(|r| format!("{:.2}%", r.to_f64().unwrap_or(0.0) / 100.0)),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(
            request.created_at.unwrap_or_default(),
            Utc,
        ),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(
            request.updated_at.unwrap_or_default(),
            Utc,
        ),
    };

    Ok(Json(response))
}

/// List merchant's credit line requests
#[utoipa::path(
    get,
    path = "/api/v1/credit-lines/requests",
    summary = "List credit line requests",
    description = "Get paginated list of merchant's credit line requests",
    tag = "Credit Lines",
    params(CreditLineRequestQuery),
    responses(
        (status = 200, description = "Credit line requests retrieved", body = CreditLineRequestListResponse),
        (status = 401, description = "Unauthorized - invalid JWT token"),
        (status = 403, description = "Forbidden - insufficient permissions"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(state))]
pub async fn list_credit_line_requests(
    State(state): State<ApiState>,
    Query(query): Query<CreditLineRequestQuery>,
) -> Result<Json<CreditLineRequestListResponse>, (StatusCode, Json<CreditLineErrorResponse>)> {
    // For this implementation, we'll use a hardcoded user_id for testing
    // In a real implementation, this would come from authentication middleware
    let test_user_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Invalid UUID".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: Some("Invalid test user ID".to_string()),
                }),
            )
        })?;

    // Get merchant data
    let merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(CreditLineErrorResponse {
                    error: "Merchant not found".to_string(),
                    code: "MERCHANT_NOT_FOUND".to_string(),
                    details: None,
                }),
            ))
        }
        Err(e) => {
            error!("Failed to get merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Get credit line requests from database
    let limit = query.limit.unwrap_or(20).min(100) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let requests = match get_credit_line_requests_by_merchant(
        &state.pool,
        merchant.id,
        query.status.as_deref(),
        limit,
        offset,
    )
    .await
    {
        Ok(requests) => requests,
        Err(e) => {
            error!("Failed to get credit line requests: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreditLineErrorResponse {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Convert to API response format
    let request_responses: Vec<CreditLineRequestResponse> = requests
        .into_iter()
        .map(|request| CreditLineRequestResponse {
            request_id: request.id,
            merchant_id: merchant.id,
            requested_amount: format!(
                "${:.2}",
                request.requested_amount.to_f64().unwrap_or(0.0) / 100.0
            ),
            requested_rate_apr: request
                .requested_rate_apr
                .map(|r| format!("{:.2}%", r.to_f64().unwrap_or(0.0) / 100.0)),
            requested_tenor_days: request.requested_tenor_days.unwrap_or(30) as u32,
            status: request.status.unwrap_or("UNKNOWN".to_string()),
            ltv_ratio: request
                .ltv_ratio
                .map(|l| format!("{:.2}%", l.to_f64().unwrap_or(0.0) / 100.0)),
            collateral_basis: request
                .collateral_basis
                .map(|c| format!("${:.2}", c.to_f64().unwrap_or(0.0) / 100.0)),
            evaluation_score: None, // TODO: Add evaluation score
            rejection_reason: request.rejection_reason,
            matched_pool_name: None, // TODO: Get pool name if matched
            approved_amount: request
                .approved_amount
                .map(|a| format!("${:.2}", a.to_f64().unwrap_or(0.0) / 100.0)),
            approved_rate_apr: request
                .approved_rate_apr
                .map(|r| format!("{:.2}%", r.to_f64().unwrap_or(0.0) / 100.0)),
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(
                request.created_at.unwrap_or_default(),
                Utc,
            ),
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(
                request.updated_at.unwrap_or_default(),
                Utc,
            ),
        })
        .collect();

    let response = CreditLineRequestListResponse {
        requests: request_responses.clone(),
        total_count: request_responses.len() as u32, // TODO: Get actual total count
        has_more: request_responses.len() == limit as usize,
    };

    Ok(Json(response))
}

/// Browse available liquidity pools
#[utoipa::path(
    get,
    path = "/api/v1/pools/available",
    summary = "Browse available pools",
    description = "Browse available liquidity pools for potential credit line matching",
    tag = "Liquidity Pools",
    params(PoolBrowsingQuery),
    responses(
        (status = 200, description = "Available pools retrieved", body = Vec<AvailablePoolResponse>),
        (status = 401, description = "Unauthorized - invalid JWT token"),
        (status = 403, description = "Forbidden - insufficient permissions"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(state))]
pub async fn browse_available_pools(
    State(state): State<ApiState>,
    Query(query): Query<PoolBrowsingQuery>,
) -> Result<Json<Vec<AvailablePoolResponse>>, (StatusCode, Json<CreditLineErrorResponse>)> {
    // Get all active liquidity pools
    let pools =
        match get_all_liquidity_pools(&state.pool, Some("ACTIVE"), None, None, None, None).await {
            Ok(pools) => pools,
            Err(e) => {
                error!("Failed to get liquidity pools: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CreditLineErrorResponse {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                        details: None,
                    }),
                ));
            }
        };

    // Filter pools based on query parameters
    let filtered_pools: Vec<_> = pools
        .into_iter()
        .filter(|pool| {
            // Apply filters
            if let Some(min_amount) = query.min_amount {
                if pool.available_liquidity.to_i64().unwrap_or(0) < min_amount as i64 {
                    return false;
                }
            }

            if let Some(max_ltv) = query.max_ltv {
                if pool.max_ltv_ratio.to_i32().unwrap_or(0) > max_ltv as i32 {
                    return false;
                }
            }

            if let Some(max_rate) = query.max_rate {
                if pool.max_interest_rate.to_i32().unwrap_or(0) > max_rate as i32 {
                    return false;
                }
            }

            if let Some(min_repayment_days) = query.min_repayment_days {
                if pool.repayment_period_days < min_repayment_days as i32 {
                    return false;
                }
            }

            true
        })
        .map(|pool| AvailablePoolResponse {
            pool_id: pool.id,
            pool_name: pool.name,
            max_ltv_ratio: format!("{:.1}%", pool.max_ltv_ratio.to_f64().unwrap_or(0.0) / 100.0),
            max_interest_rate: format!(
                "{:.2}%",
                pool.max_interest_rate.to_f64().unwrap_or(0.0) / 100.0
            ),
            available_liquidity: format!(
                "${:.2}",
                pool.available_liquidity.to_f64().unwrap_or(0.0) / 100.0
            ),
            repayment_period_days: pool.repayment_period_days as u32,
            match_score: None, // TODO: Calculate match score based on merchant profile
            estimated_approval_amount: None, // TODO: Calculate based on merchant eligibility
        })
        .collect();

    Ok(Json(filtered_pools))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get description for eligibility check
fn get_check_description(check_name: &str) -> String {
    match check_name {
        "kyc_approved" => "KYC verification must be approved".to_string(),
        "profile_complete" => "Merchant profile must be completed".to_string(),
        "mcc_allowed" => "Merchant Category Code must not be restricted".to_string(),
        "revenue_sufficient" => "Monthly revenue must meet minimum threshold".to_string(),
        "turnover_history_sufficient" => "Must have sufficient turnover history".to_string(),
        "verified_turnover_sufficient" => "Must have sufficient verified turnover data".to_string(),
        "bank_statement_verified" => "Bank statement must be verified".to_string(),
        _ => "Unknown eligibility check".to_string(),
    }
}

/// Get weight for eligibility check (for display purposes)
fn get_check_weight(check_name: &str) -> u8 {
    match check_name {
        "kyc_approved" => 25,
        "profile_complete" => 15,
        "mcc_allowed" => 10,
        "revenue_sufficient" => 15,
        "turnover_history_sufficient" => 15,
        "verified_turnover_sufficient" => 10,
        "bank_statement_verified" => 10,
        _ => 0,
    }
}
