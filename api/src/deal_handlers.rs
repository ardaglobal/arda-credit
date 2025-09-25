//! Deal management handlers for the Arda Credit API
//!
//! This module contains all deal-related API endpoints including:
//! - Deal creation and management
//! - Deal payment processing
//! - Deal statistics and reporting
//! - Deal lifecycle tracking
//!
//! All routes are documented with OpenAPI and use consistent error handling.
//!
//! ## Recent Changes
//! Converted from loan-based terminology to deal-based terminology:
//! - Loan → Deal
//! - Borrower → Originator
//! - Lender → Investor
//! - All API endpoints and types updated accordingly

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::types::ApiState;
use arda_credit_db::{
    // Marketplace functions (consolidated)
    browse_marketplace_deals, // was browse_marketplace_loans
    cancel_transfer,          // was cancel_transfer
    // ... other imports
    create_deal,         // was create_loan
    create_deal_listing, // was create_loan_listing
    create_deal_payment, // was create_payment
    // Transfer functions (consolidated)
    execute_deal_transfer,         // was execute_loan_transfer
    get_deal_by_id,                // was get_loan_by_id
    get_deal_transfer_history,     // was get_loan_transfer_history
    list_deals,                    // was list_loans
    remove_deal_listing,           // was remove_loan_listing
    update_deal,                   // was update_loan
    validate_transfer_eligibility, // was validate_transfer_eligibility
    // get_marketplace_deal_stats, // was get_marketplace_loan_stats - TODO: implement if needed
    // get_user_deal_transfer_history, // was get_user_transfer_history (if exists) - TODO: implement if needed

    // Transfer structs
    ExecuteTransferRequest,
    TransferValidationRequest,
};
use uuid::Uuid;

// ============================================================================
// DEAL MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Create new deal request (converted from CreateLoanRequest)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateDealRequest {
    // was CreateLoanRequest
    pub originator_id: String,               // was borrower_id
    pub investor_id: String,                 // was lender_id
    pub principal_amount: String,            // Amount in cents as string
    pub interest_rate_bps: u16,              // Interest rate in basis points
    pub term_days: Option<u32>,              // Optional deal term
    pub metadata: Option<serde_json::Value>, // Optional additional data
    pub custom_deal_id: Option<String>, // Optional user-defined identifier for external system mapping
}

/// Deal details response (converted from LoanDetailsResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealDetailsResponse {
    // was LoanDetailsResponse
    pub deal_id: String,       // was loan_id
    pub originator_id: String, // was borrower_id
    pub investor_id: String,   // was lender_id
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub loan_status: String, // keeping same field name
    pub originated_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
    pub custom_deal_id: Option<String>, // Optional user-defined identifier for external system mapping
}

/// List deals query parameters (converted from ListLoansQuery)
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListDealsQuery {
    // was ListLoansQuery
    pub status: Option<String>,
    pub originator_id: Option<String>, // was borrower_id
    pub investor_id: Option<String>,   // was lender_id
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List deals response (converted from ListLoansResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListDealsResponse {
    // was ListLoansResponse
    pub deals: Vec<DealDetailsResponse>, // was loans
    pub total_count: usize,
    pub has_more: bool,
}

/// Deal statistics response (converted from LoanStatsResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealStatsResponse {
    // was LoanStatsResponse
    pub total_deals: u64,          // was total_loans
    pub total_principal: String,   // Total principal amount as string
    pub total_outstanding: String, // Total outstanding balance as string
    pub average_interest_rate_bps: u32,
    pub status_breakdown: std::collections::HashMap<String, u64>,
}

/// Update deal request (converted from UpdateLoanRequest)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateDealRequest {
    // was UpdateLoanRequest
    pub principal_amount: Option<String>, // Amount in cents as string
    pub outstanding_balance: Option<String>, // Outstanding balance in cents as string
    pub interest_rate_bps: Option<u16>,   // Interest rate in basis points
    pub loan_status: Option<String>, // "originated", "active", "paid_off", "closed", "defaulted"
    pub maturity_date: Option<Option<String>>, // Optional maturity date (ISO 8601), Option<Option<T>> to handle NULL
    pub custom_deal_id: Option<Option<String>>, // Optional custom deal ID, Option<Option<T>> to handle NULL
}

/// Update deal response (converted from UpdateLoanResponse)
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateDealResponse {
    // was UpdateLoanResponse
    pub deal_id: String, // was loan_id
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub deal_details: DealDetailsResponse, // was loan_details
}

/// Delete deal response (converted from DeleteLoanResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteDealResponse {
    // was DeleteLoanResponse
    pub deal_id: String, // was loan_id
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
    pub final_status: String,
}

/// Record deal payment request (converted from RecordPaymentRequest)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecordDealPaymentRequest {
    // was RecordPaymentRequest
    pub deal_id: String,                   // was loan_id
    pub amount: String,                    // Amount in cents as string
    pub payment_type: String,              // "Principal" | "Interest" | "Mixed"
    pub payment_reference: Option<String>, // Optional external reference
    pub notes: Option<String>,
}

/// Payment response (converted from PaymentResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub transaction_id: String,
    pub deal_id: String, // was loan_id
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String, // "Pending" | "Processed" | "Failed"
    pub created_at: DateTime<Utc>,
}

/// Payment details response (converted from PaymentDetailsResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentDetailsResponse {
    pub transaction_id: String,
    pub deal_id: String, // was loan_id
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String,
    pub batch_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Deal payment history response (converted from LoanPaymentHistoryResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealPaymentHistoryResponse {
    // was LoanPaymentHistoryResponse
    pub deal_id: String, // was loan_id
    pub payments: Vec<PaymentDetailsResponse>,
    pub total_payments: usize,
    pub total_amount_paid: String,
}

// ============================================================================
// CONSOLIDATED MARKETPLACE TYPES (moved from marketplace_handlers)
// ============================================================================

/// Browse deals query parameters (consolidated from marketplace)
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct BrowseDealsQuery {
    // was BrowseMarketplaceQuery
    pub limit: Option<i32>,
    pub status: Option<String>,
}

/// List deal for sale request (consolidated from marketplace)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListDealForSaleRequest {
    // was ListLoanForSaleRequest
    pub seller_id: uuid::Uuid,
    pub asking_price: String, // Amount in cents as string
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Deal listing response (consolidated from marketplace)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealListingResponse {
    // was LoanListingResponse
    pub listing_id: uuid::Uuid,
    pub deal_id: String, // was loan_id
    pub seller_id: uuid::Uuid,
    pub asking_price: String,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: String,
}

/// Marketplace deal response (consolidated from marketplace)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceDealResponse {
    // was MarketplaceLoanResponse
    // Listing information
    pub listing_id: uuid::Uuid,
    pub deal_id: uuid::Uuid, // was loan_id
    pub seller_id: uuid::Uuid,
    pub asking_price: String,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub listing_status: String,

    // Deal information
    pub originator_id: uuid::Uuid, // was borrower_id
    pub investor_id: uuid::Uuid,   // was lender_id
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub deal_status: String, // keeping same field name
    pub originated_at: DateTime<Utc>,
}

/// Deal marketplace statistics (consolidated from marketplace)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealMarketplaceStats {
    // was MarketplaceStats
    pub total_listings: u64,
    pub active_listings: u64,
    pub total_volume: String, // Total volume of deals listed
    pub average_asking_price: String,
    pub deals_sold_this_month: u64, // was loans_sold_this_month
}

// ============================================================================
// CONSOLIDATED TRANSFER TYPES (moved from transfer_handlers)
// ============================================================================

/// Transfer deal ownership request (consolidated from transfer)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransferDealOwnershipRequest {
    // was TransferLoanOwnershipRequest
    pub from_investor_id: uuid::Uuid, // was from_lender_id
    pub to_investor_id: uuid::Uuid,   // was to_lender_id
    pub transfer_price: String,       // Amount in cents as string
}

/// Deal transfer response (consolidated from transfer)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealTransferResponse {
    // was LoanTransferResponse
    pub transfer_id: uuid::Uuid,
    pub deal_id: String,              // was loan_id
    pub from_investor_id: uuid::Uuid, // was from_lender_id
    pub to_investor_id: uuid::Uuid,   // was to_lender_id
    pub transfer_price: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Transfer history query parameters (consolidated from transfer)
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TransferHistoryQuery {
    pub limit: Option<i32>,
}

/// Validate deal transfer request (consolidated from transfer)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ValidateDealTransferRequest {
    // was ValidateTransferRequest
    pub current_investor_id: uuid::Uuid, // was current_lender_id
}

/// Deal transfer validation response (consolidated from transfer)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealTransferValidationResponse {
    // was TransferValidationResponse
    pub is_valid: bool,
    pub deal_id: String,                 // was loan_id
    pub current_investor_id: uuid::Uuid, // was current_lender_id
    pub message: String,
}

// ============================================================================
// DEAL MANAGEMENT ENDPOINTS
// ============================================================================

/// Create a new deal (converted from create_loan_endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/deal",  // will be updated in paths later
    request_body = CreateDealRequest,
    responses(
        (status = 201, description = "Deal created successfully", body = DealDetailsResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn create_deal_endpoint(
    // was create_loan_endpoint
    State(state): State<ApiState>,
    Json(request): Json<CreateDealRequest>, // was CreateLoanRequest
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // was LoanDetailsResponse
    let start_time = std::time::Instant::now();

    // Record business logic operation
    metrics::counter!("business_logic_operations_total", "operation" => "create_deal").increment(1); // was create_loan

    info!(
        "💰 API: Creating deal: originator={}, investor={}, amount={}", // updated log message
        request.originator_id, request.investor_id, request.principal_amount
    );

    // Parse UUIDs
    let originator_id = uuid::Uuid::parse_str(&request.originator_id).map_err(|_| {
        // was borrower_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid originator ID format".to_string(), // was borrower ID
        )
    })?;
    let investor_id = uuid::Uuid::parse_str(&request.investor_id).map_err(|_| {
        // was lender_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid investor ID format".to_string(), // was lender ID
        )
    })?;

    // Parse principal amount
    let principal_amount = request
        .principal_amount
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?;

    // Parse optional maturity date
    let maturity_date = if let Some(term_days) = request.term_days {
        Some(chrono::Utc::now() + chrono::Duration::days(term_days as i64))
    } else {
        None
    };

    // Create deal in database
    let deal = create_deal(
        // was create_loan
        &state.pool,
        originator_id, // was borrower_id
        investor_id,   // was lender_id
        principal_amount,
        request.interest_rate_bps as i32,
        maturity_date,
        request.custom_deal_id.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create deal: {}", e); // updated error message
        metrics::counter!("business_logic_errors_total", "operation" => "create_deal").increment(1); // was create_loan
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create deal".to_string(), // updated error message
        )
    })?;

    let response = DealDetailsResponse {
        // was LoanDetailsResponse
        deal_id: deal.id.to_string(),                  // was loan_id
        originator_id: deal.originator_id.to_string(), // was borrower_id
        investor_id: deal.investor_id.to_string(),     // was lender_id
        principal_amount: deal.principal_amount.to_string(),
        outstanding_balance: deal.outstanding_balance.to_string(),
        interest_rate_bps: deal.interest_rate_bps as u16,
        loan_status: deal.loan_status, // keeping same field name
        originated_at: deal.originated_at,
        last_updated: Some(deal.updated_at),
        custom_deal_id: deal.custom_deal_id,
    };

    // Record function execution time
    let duration = start_time.elapsed();
    metrics::histogram!("function_execution_duration_seconds", "function" => "create_deal")
        .record(duration.as_secs_f64());

    info!("✅ Deal created successfully: {}", response.deal_id); // updated log message
    Ok(Json(response))
}

/// List deals with optional filtering (converted from list_loans_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deals",
    params(ListDealsQuery),  // was ListLoansQuery
    responses(
        (status = 200, description = "List of deals", body = ListDealsResponse),  // was loans
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_deals_endpoint(
    // was list_loans_endpoint
    State(state): State<ApiState>,
    Query(query): Query<ListDealsQuery>, // was ListLoansQuery
) -> Result<Json<ListDealsResponse>, (StatusCode, String)> {
    // was ListLoansResponse
    info!("📋 API: Listing deals with filters: {:?}", query); // updated log message

    let originator_id = if let Some(id_str) = &query.originator_id {
        // was borrower_id
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid originator ID format".to_string(), // was borrower ID
            )
        })?)
    } else {
        None
    };

    let investor_id = if let Some(id_str) = &query.investor_id {
        // was lender_id
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid investor ID format".to_string(), // was lender ID
            )
        })?)
    } else {
        None
    };

    let deals = list_deals(
        // was list_loans
        &state.pool,
        query.limit.map(|l| l as i64),
        query.offset.map(|o| o as i64),
        query.status.as_deref(),
        originator_id, // was borrower_id
        investor_id,   // was lender_id
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to list deals: {}", e); // updated error message
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list deals".to_string(), // updated error message
        )
    })?;

    // Convert to response format
    let deal_responses: Vec<DealDetailsResponse> = deals // was LoanDetailsResponse
        .into_iter()
        .map(|deal| DealDetailsResponse {
            // was loan, LoanDetailsResponse
            deal_id: deal.id.to_string(),                  // was loan_id
            originator_id: deal.originator_id.to_string(), // was borrower_id
            investor_id: deal.investor_id.to_string(),     // was lender_id
            principal_amount: deal.principal_amount.to_string(),
            outstanding_balance: deal.outstanding_balance.to_string(),
            interest_rate_bps: deal.interest_rate_bps as u16,
            loan_status: deal.loan_status, // keeping same field name
            originated_at: deal.originated_at,
            last_updated: Some(deal.updated_at),
            custom_deal_id: deal.custom_deal_id,
        })
        .collect();

    let total_count = deal_responses.len();
    let limit = query.limit.unwrap_or(100) as usize;
    let has_more = total_count >= limit;

    let response = ListDealsResponse {
        // was ListLoansResponse
        deals: deal_responses, // was loans
        total_count,
        has_more,
    };

    info!("✅ API: Listed {} deals", response.total_count); // updated log message
    Ok(Json(response))
}

/// Get deal details by ID (converted from get_loan_details_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deal/{deal_id}",  // was loan_id
    params(
        ("deal_id" = String, Path, description = "Deal ID to retrieve")  // was loan_id
    ),
    responses(
        (status = 200, description = "Deal details", body = DealDetailsResponse),  // was Loan
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_deal_details_endpoint(
    // was get_loan_details_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>, // was loan_id
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // was LoanDetailsResponse
    info!("🔍 API: Getting deal details: {}", deal_id); // updated log message

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        // was loan_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(), // was loan ID
        )
    })?;

    let deal = get_deal_by_id(&state.pool, deal_uuid) // was get_loan_by_id
        .await
        .map_err(|e| {
            error!("❌ Database error getting deal: {}", e); // updated error message
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve deal".to_string(), // updated error message
            )
        })?;

    match deal {
        Some(deal) => {
            let response = DealDetailsResponse {
                // was LoanDetailsResponse
                deal_id: deal.id.to_string(), // was loan_id
                originator_id: deal.originator_id.to_string(), // was borrower_id
                investor_id: deal.investor_id.to_string(), // was lender_id
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.loan_status, // keeping same field name
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
            };

            info!("✅ Deal details retrieved: {}", deal_id); // updated log message
            Ok(Json(response))
        }
        None => {
            warn!("⚠️ Deal not found: {}", deal_id); // updated log message
            Err((
                StatusCode::NOT_FOUND,
                "Deal not found".to_string(), // updated error message
            ))
        }
    }
}

/// Get deal statistics (converted from get_loan_stats_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deals/stats",
    responses(
        (status = 200, description = "Deal statistics retrieved successfully", body = DealStatsResponse),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(ListDealsQuery),  // was ListLoansQuery
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_deal_stats_endpoint(
    // was get_loan_stats_endpoint
    State(_state): State<ApiState>,
    Query(_params): Query<ListDealsQuery>, // Reuse query params for filtering, was ListLoansQuery
) -> Result<Json<DealStatsResponse>, (StatusCode, String)> {
    // was LoanStatsResponse
    info!("📊 API: Getting deal statistics"); // updated log message

    // TODO: Implement deal statistics with database integration
    let mut status_breakdown = std::collections::HashMap::new();
    status_breakdown.insert("Originated".to_string(), 10);
    status_breakdown.insert("Active".to_string(), 120);
    status_breakdown.insert("PaidOff".to_string(), 15);
    status_breakdown.insert("Closed".to_string(), 5);

    let response = DealStatsResponse {
        // was LoanStatsResponse
        total_deals: 150,                          // was total_loans
        total_principal: "50000000".to_string(),   // $500,000.00
        total_outstanding: "48500000".to_string(), // $485,000.00
        average_interest_rate_bps: 475,
        status_breakdown,
    };

    info!("✅ API: Deal statistics retrieved"); // updated log message
    Ok(Json(response))
}

/// Update deal (converted from update_loan_endpoint)
#[utoipa::path(
    put,
    path = "/api/v1/deal/{deal_id}",  // was loan_id
    request_body = UpdateDealRequest,  // was UpdateLoanRequest
    responses(
        (status = 200, description = "Deal updated successfully", body = UpdateDealResponse),  // was Loan
        (status = 400, description = "Invalid request data"),
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal ID")  // was loan_id
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_deal_endpoint(
    // was update_loan_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,            // was loan_id
    Json(request): Json<UpdateDealRequest>, // was UpdateLoanRequest
) -> Result<Json<UpdateDealResponse>, (StatusCode, String)> {
    // was UpdateLoanResponse
    info!("📝 API: Updating deal: {}", deal_id); // updated log message

    // Parse deal ID
    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        // was loan_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(), // was loan ID
        )
    })?;

    // Convert request fields to database types
    let principal_amount = if let Some(amount_str) = &request.principal_amount {
        Some(amount_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?)
    } else {
        None
    };

    let outstanding_balance = if let Some(balance_str) = &request.outstanding_balance {
        Some(balance_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid outstanding balance".to_string(),
            )
        })?)
    } else {
        None
    };

    let interest_rate_bps = request.interest_rate_bps.map(|rate| rate as i32);

    let maturity_date = if let Some(date_opt) = &request.maturity_date {
        Some(match date_opt {
            Some(date_str) => Some(
                chrono::DateTime::parse_from_rfc3339(date_str)
                    .map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            "Invalid maturity date format".to_string(),
                        )
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        })
    } else {
        None
    };

    // Update the deal in the database
    let updated_deal = update_deal(
        // was update_loan
        &state.pool,
        deal_uuid,
        principal_amount,
        outstanding_balance,
        interest_rate_bps,
        request.loan_status.as_deref(),
        maturity_date,
        request.custom_deal_id.as_ref().map(|opt| opt.as_deref()),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to update deal: {}", e); // updated error message
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update deal".to_string(), // updated error message
        )
    })?;

    match updated_deal {
        Some(deal) => {
            // Track which fields were updated
            let mut updated_fields = Vec::new();
            if request.principal_amount.is_some() {
                updated_fields.push("principal_amount".to_string());
            }
            if request.outstanding_balance.is_some() {
                updated_fields.push("outstanding_balance".to_string());
            }
            if request.interest_rate_bps.is_some() {
                updated_fields.push("interest_rate_bps".to_string());
            }
            if request.loan_status.is_some() {
                updated_fields.push("loan_status".to_string());
            }
            if request.maturity_date.is_some() {
                updated_fields.push("maturity_date".to_string());
            }
            if request.custom_deal_id.is_some() {
                updated_fields.push("custom_deal_id".to_string());
            }

            // Convert to response format
            let deal_details = DealDetailsResponse {
                // was LoanDetailsResponse
                deal_id: deal.id.to_string(), // was loan_id
                originator_id: deal.originator_id.to_string(), // was borrower_id
                investor_id: deal.investor_id.to_string(), // was lender_id
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.loan_status, // keeping same field name
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
            };

            let response = UpdateDealResponse {
                // was UpdateLoanResponse
                deal_id: deal.id.to_string(), // was loan_id
                updated_fields,
                updated_at: deal.updated_at,
                deal_details, // was loan_details
            };

            info!("✅ API: Deal updated successfully: {}", deal_id); // updated log message
            Ok(Json(response))
        }
        None => {
            warn!("❌ API: Deal not found: {}", deal_id); // updated log message
            Err((StatusCode::NOT_FOUND, "Deal not found".to_string())) // updated error message
        }
    }
}

/// Delete/close deal (converted from delete_loan_endpoint)
#[utoipa::path(
    delete,
    path = "/api/v1/deal/{deal_id}",  // was loan_id
    responses(
        (status = 200, description = "Deal closed successfully", body = DeleteDealResponse),  // was Loan
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal ID")  // was loan_id
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(_state), level = "info")]
pub async fn delete_deal_endpoint(
    // was delete_loan_endpoint
    State(_state): State<ApiState>,
    Path(deal_id): Path<String>, // was loan_id
) -> Result<Json<DeleteDealResponse>, (StatusCode, String)> {
    // was DeleteLoanResponse
    info!("🗑️ API: Closing deal: deal_id={}", deal_id); // updated log message

    let _deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        // was loan_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(), // was loan ID
        )
    })?;

    // TODO: Implement deal closure with database integration
    // For now, simulate successful closure
    let _updated_rows = 1; // Simulate one row updated

    let response = DeleteDealResponse {
        // was DeleteLoanResponse
        deal_id, // was loan_id
        deleted: true,
        deleted_at: Utc::now(),
        final_status: "closed".to_string(),
    };

    info!("✅ API: Deal closed successfully"); // updated log message
    Ok(Json(response))
}

/// Record a deal payment (converted from record_payment_endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/payments",
    request_body = RecordDealPaymentRequest,  // was RecordPaymentRequest
    responses(
        (status = 200, description = "Payment recorded successfully", body = PaymentResponse),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn record_deal_payment_endpoint(
    // was record_payment_endpoint
    State(_state): State<ApiState>,
    Json(request): Json<RecordDealPaymentRequest>, // was RecordPaymentRequest
) -> Result<Json<PaymentResponse>, (StatusCode, String)> {
    info!(
        "💳 API: Recording payment: deal_id={}, amount={}, type={}", // updated log message
        request.deal_id, request.amount, request.payment_type
    );

    // TODO: Implement payment recording with database integration
    // TODO: When implementing payment recording, also decrease borough allocation:
    // if let Some((pool_id, merchant_borough)) = get_deal_pool_and_borough(&request.deal_id).await {
    //     let payment_amount = Decimal::from_str(&request.amount).unwrap_or(Decimal::ZERO);
    //     match arda_credit_db::update_borough_allocation(
    //         &state.pool,
    //         pool_id,
    //         &merchant_borough,
    //         -payment_amount,  // Negative for payment (reduces allocation)
    //     ).await {
    //         Ok(()) => info!("Reduced borough allocation by payment amount"),
    //         Err(e) => error!("Failed to update borough allocation for payment: {}", e),
    //     }
    // }

    let transaction_id = Uuid::new_v4().to_string();

    let response = PaymentResponse {
        transaction_id,
        deal_id: request.deal_id, // was loan_id
        amount: request.amount,
        payment_type: request.payment_type,
        transaction_status: "Pending".to_string(), // Will be batched for ZK proof
        created_at: Utc::now(),
    };

    info!("✅ API: Payment recorded successfully");
    Ok(Json(response))
}

/// Get payment details (converted from get_payment_details_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/payment/{transaction_id}",
    responses(
        (status = 200, description = "Payment details retrieved successfully", body = PaymentDetailsResponse),
        (status = 404, description = "Payment not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("transaction_id" = String, Path, description = "Transaction ID")
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_payment_details_endpoint(
    State(_state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> Result<Json<PaymentDetailsResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Getting payment details: transaction_id={}",
        transaction_id
    );

    // TODO: Implement payment lookup with database integration
    let response = PaymentDetailsResponse {
        transaction_id,
        deal_id: Uuid::new_v4().to_string(), // was loan_id
        amount: "25000".to_string(),
        payment_type: "Principal".to_string(),
        transaction_status: "Processed".to_string(),
        batch_id: Some(Uuid::new_v4().to_string()),
        created_at: Utc::now(),
        processed_at: Some(Utc::now()),
    };

    info!("✅ API: Payment details retrieved");
    Ok(Json(response))
}

/// Get deal payment history (converted from get_loan_payment_history_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deal/{deal_id}/payments",  // was loan_id
    responses(
        (status = 200, description = "Payment history retrieved successfully", body = DealPaymentHistoryResponse),  // was Loan
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal ID"),  // was loan_id
        ListDealsQuery  // was ListLoansQuery
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_deal_payment_history_endpoint(
    // was get_loan_payment_history_endpoint
    State(_state): State<ApiState>,
    Path(deal_id): Path<String>,           // was loan_id
    Query(_params): Query<ListDealsQuery>, // Reuse query params for limit/offset, was ListLoansQuery
) -> Result<Json<DealPaymentHistoryResponse>, (StatusCode, String)> {
    // was LoanPaymentHistoryResponse
    info!("📜 API: Getting payment history: deal_id={}", deal_id); // updated log message

    // TODO: Implement payment history lookup with database integration
    let response = DealPaymentHistoryResponse {
        // was LoanPaymentHistoryResponse
        deal_id, // was loan_id
        payments: vec![],
        total_payments: 0,
        total_amount_paid: "0".to_string(),
    };

    info!("✅ API: Payment history retrieved");
    Ok(Json(response))
}

// ============================================================================
// CONSOLIDATED MARKETPLACE ENDPOINTS (moved from marketplace_handlers)
// ============================================================================
//
// The following functions were consolidated from api/src/marketplace_handlers.rs
// into this file as part of the loan-to-deal conversion. All marketplace
// functionality is now available through deal endpoints:
//
// - /deals/for-sale (browse marketplace)
// - /deal/{id}/list-for-sale (list deal for sale)
// - /deal/{id}/remove-from-sale (remove deal from sale)
// - /deals/marketplace/stats (marketplace statistics)
//
// This consolidation provides a unified API experience where all deal-related
// operations (CRUD, payments, marketplace, transfers) are accessible through
// the /deal/* and /deals/* endpoints.
//
// ============================================================================

// ============================================================================
// CONSOLIDATED TRANSFER ENDPOINTS (moved from transfer_handlers)
// ============================================================================
//
// The following functions were consolidated from api/src/transfer_handlers.rs
// into this file as part of the loan-to-deal conversion. All transfer
// functionality is now available through deal endpoints:
//
// - /deal/{id}/transfer (initiate deal ownership transfer)
// - /deal/{id}/transfers (get deal transfer history)
// - /user/{id}/deal-transfers (get user's deal transfer history)
// - /deal/{id}/transfer/validate (validate deal transfer)
//
// This consolidation provides a unified API experience where all deal-related
// operations (CRUD, payments, marketplace, transfers) are accessible through
// the /deal/* and /deals/* endpoints.
//
// Transfer functionality maintains the same business logic as before:
// - Ownership validation before transfer
// - Transfer price negotiation
// - Transfer history tracking
// - Multi-step transfer process with status tracking
//
// ============================================================================

/// Browse deals available for sale (consolidated from browse_marketplace_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deals/for-sale",
    params(
        ("limit" = Option<i32>, Query, description = "Maximum number of deals to return"),
        ("status" = Option<String>, Query, description = "Filter deals by listing status")
    ),
    responses(
        (status = 200, description = "List of deals for sale", body = Vec<MarketplaceDealResponse>),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn browse_deals_for_sale_endpoint(
    // was browse_marketplace_endpoint
    State(state): State<ApiState>,
    Query(params): Query<BrowseDealsQuery>, // was BrowseMarketplaceQuery
) -> Result<Json<Vec<MarketplaceDealResponse>>, (StatusCode, String)> {
    // was MarketplaceLoanResponse
    info!(
        "🏪 API: Browsing deals for sale with limit: {:?}",
        params.limit
    );

    let deals = browse_marketplace_deals(
        // was browse_marketplace_loans
        &state.pool,
        params.limit.map(|l| l as i64),
        params.status,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to browse marketplace deals: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to browse marketplace deals".to_string(),
        )
    })?;

    let response: Vec<MarketplaceDealResponse> = deals // was MarketplaceLoanResponse
        .into_iter()
        .map(|deal| MarketplaceDealResponse {
            // was loan, MarketplaceLoanResponse
            listing_id: deal.listing_id,
            deal_id: deal.deal_id, // was loan_id
            seller_id: deal.seller_id,
            asking_price: deal.asking_price.to_string(),
            description: deal.description,
            listed_at: deal.listed_at,
            expires_at: deal.expires_at,
            listing_status: deal.status,
            // Deal details
            originator_id: deal.originator_id, // was borrower_id
            investor_id: deal.investor_id,     // was lender_id
            principal_amount: deal.principal_amount.to_string(),
            outstanding_balance: deal.outstanding_balance.to_string(),
            interest_rate_bps: deal.interest_rate_bps as u16,
            deal_status: deal.loan_status, // keeping same field name
            originated_at: deal.originated_at,
        })
        .collect();

    info!("✅ Found {} deals for sale", response.len());
    Ok(Json(response))
}

/// List a deal for sale (consolidated from list_loan_for_sale_endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/list-for-sale",
    params(
        ("deal_id" = String, Path, description = "Deal ID to list for sale")
    ),
    request_body = ListDealForSaleRequest,  // was ListLoanForSaleRequest
    responses(
        (status = 201, description = "Deal listed for sale successfully", body = DealListingResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_deal_for_sale_endpoint(
    // was list_loan_for_sale_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,                 // was loan_id
    Json(request): Json<ListDealForSaleRequest>, // was ListLoanForSaleRequest
) -> Result<Json<DealListingResponse>, (StatusCode, String)> {
    // was LoanListingResponse
    info!("🏷️ API: Listing deal for sale: {}", deal_id);

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    let asking_price = request
        .asking_price
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid asking price format".to_string(),
            )
        })?;

    let listing = create_deal_listing(
        // was create_loan_listing
        &state.pool,
        deal_uuid, // was loan_id
        request.seller_id,
        asking_price,
        request.description,
        request.expires_at,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to list deal for sale: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list deal for sale".to_string(),
        )
    })?;

    let response = DealListingResponse {
        // was LoanListingResponse
        listing_id: listing.listing_id,
        deal_id: listing.deal_id.to_string(), // was loan_id
        seller_id: listing.seller_id,
        asking_price: listing.asking_price.to_string(),
        description: listing.description,
        listed_at: listing.listed_at,
        expires_at: listing.expires_at,
        status: listing.status,
    };

    info!("✅ Deal listed for sale: {}", deal_id);
    Ok(Json(response))
}

/// Remove deal from sale (consolidated from remove_loan_from_sale_endpoint)
#[utoipa::path(
    delete,
    path = "/api/v1/deal/{deal_id}/remove-from-sale",
    params(
        ("deal_id" = String, Path, description = "Deal ID to remove from sale")
    ),
    responses(
        (status = 200, description = "Deal removed from sale successfully"),
        (status = 404, description = "Deal listing not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn remove_deal_from_sale_endpoint(
    // was remove_loan_from_sale_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>, // was loan_id
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🗑️ API: Removing deal from sale: {}", deal_id);

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    remove_deal_listing(&state.pool, deal_uuid, uuid::Uuid::nil()) // TODO: Add authentication to get current user
        .await
        .map_err(|e| {
            error!("❌ Failed to remove deal from sale: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to remove deal from sale".to_string(),
            )
        })?;

    info!("✅ Deal removed from sale: {}", deal_id);
    Ok(Json(serde_json::json!({
        "message": "Deal removed from sale successfully",
        "deal_id": deal_id
    })))
}

/// Get deal marketplace statistics (consolidated from marketplace_stats_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deals/marketplace/stats",
    responses(
        (status = 200, description = "Deal marketplace statistics", body = DealMarketplaceStats),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn deal_marketplace_stats_endpoint(
    // was marketplace_stats_endpoint
    State(state): State<ApiState>,
) -> Result<Json<DealMarketplaceStats>, (StatusCode, String)> {
    // was MarketplaceStats
    info!("📊 API: Getting deal marketplace statistics");

    // TODO: Implement get_marketplace_deal_stats function
    // let stats = get_marketplace_deal_stats(&state.pool).await
    let stats = DealMarketplaceStats {
        total_listings: 0,
        active_listings: 0,
        total_volume: "0".to_string(),
        average_asking_price: "0".to_string(),
        deals_sold_this_month: 0,
    };

    Ok(Json(stats))
}

/// Transfer deal ownership (consolidated from transfer_loan_ownership_endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/transfer",
    params(
        ("deal_id" = String, Path, description = "Deal ID to transfer")
    ),
    request_body = TransferDealOwnershipRequest,  // was TransferLoanOwnershipRequest
    responses(
        (status = 201, description = "Deal transfer initiated successfully", body = DealTransferResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn transfer_deal_ownership_endpoint(
    // was transfer_loan_ownership_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,                       // was loan_id
    Json(request): Json<TransferDealOwnershipRequest>, // was TransferLoanOwnershipRequest
) -> Result<Json<DealTransferResponse>, (StatusCode, String)> {
    // was LoanTransferResponse
    info!("🔄 API: Transferring deal ownership: {}", deal_id);

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    let transfer_price = request
        .transfer_price
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid transfer price format".to_string(),
            )
        })?;

    // Create the transfer record
    let transfer = execute_deal_transfer(
        // was execute_loan_transfer
        &state.pool,
        &ExecuteTransferRequest {
            deal_id: deal_uuid,               // was loan_id
            buyer_id: request.to_investor_id, // was to_lender_id
            purchase_price: transfer_price,
            notes: None,
        },
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create deal transfer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create deal transfer".to_string(),
        )
    })?;

    let response = DealTransferResponse {
        // was LoanTransferResponse
        transfer_id: transfer.transfer_id,
        deal_id: transfer.deal_id.to_string(), // was loan_id
        from_investor_id: transfer.from_investor_id, // was from_lender_id
        to_investor_id: transfer.to_investor_id, // was to_lender_id
        transfer_price: transfer.purchase_price.to_string(),
        status: "completed".to_string(), // assuming immediate completion
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(transfer.transfer_date, Utc),
    };

    info!("✅ Deal transfer initiated: {}", deal_id);
    Ok(Json(response))
}

/// Get deal transfer history (consolidated from get_loan_transfer_history_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deal/{deal_id}/transfers",
    params(
        ("deal_id" = String, Path, description = "Deal ID to get transfer history for"),
        ("limit" = Option<i32>, Query, description = "Maximum number of transfers to return")
    ),
    responses(
        (status = 200, description = "Deal transfer history", body = Vec<DealTransferResponse>),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_deal_transfer_history_endpoint(
    // was get_loan_transfer_history_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>, // was loan_id
    Query(params): Query<TransferHistoryQuery>,
) -> Result<Json<Vec<DealTransferResponse>>, (StatusCode, String)> {
    // was LoanTransferResponse
    info!("📜 API: Getting deal transfer history: {}", deal_id);

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    let transfers = get_deal_transfer_history(
        // was get_loan_transfer_history
        &state.pool,
        deal_uuid, // was loan_id
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to get deal transfer history: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get deal transfer history".to_string(),
        )
    })?;

    let response: Vec<DealTransferResponse> = transfers // was LoanTransferResponse
        .into_iter()
        .map(|transfer| DealTransferResponse {
            // was LoanTransferResponse
            transfer_id: transfer.transfer_id,
            deal_id: transfer.deal_id.to_string(), // was loan_id
            from_investor_id: transfer.from_investor_id, // was from_lender_id
            to_investor_id: transfer.to_investor_id, // was to_lender_id
            transfer_price: transfer.purchase_price.to_string(),
            status: transfer.transfer_status,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(transfer.transfer_date, Utc),
        })
        .collect();

    info!("✅ Retrieved {} deal transfers", response.len());
    Ok(Json(response))
}

/// Get user's deal transfer history (consolidated from get_user_transfer_history_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/deal-transfers",
    params(
        ("user_id" = String, Path, description = "User ID to get transfer history for"),
        ("limit" = Option<i32>, Query, description = "Maximum number of transfers to return")
    ),
    responses(
        (status = 200, description = "User's deal transfer history", body = Vec<DealTransferResponse>),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_deal_transfer_history_endpoint(
    // was get_user_transfer_history_endpoint
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<TransferHistoryQuery>,
) -> Result<Json<Vec<DealTransferResponse>>, (StatusCode, String)> {
    // was LoanTransferResponse
    info!("👤 API: Getting user deal transfer history: {}", user_id);

    let user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // TODO: Implement get_user_deal_transfer_history function
    // let transfers = get_user_deal_transfer_history(&state.pool, user_uuid, params.limit.map(|l| l as i64)).await
    let transfers = Vec::new(); // Placeholder

    Ok(Json(transfers))
}

/// Validate deal transfer (consolidated from validate_transfer_endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/transfer/validate",
    params(
        ("deal_id" = String, Path, description = "Deal ID to validate transfer for")
    ),
    request_body = ValidateDealTransferRequest,  // was ValidateTransferRequest
    responses(
        (status = 200, description = "Transfer validation result", body = DealTransferValidationResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn validate_deal_transfer_endpoint(
    // was validate_transfer_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,                      // was loan_id
    Json(request): Json<ValidateDealTransferRequest>, // was ValidateTransferRequest
) -> Result<Json<DealTransferValidationResponse>, (StatusCode, String)> {
    // was TransferValidationResponse
    info!("✔️ API: Validating deal transfer: {}", deal_id);

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    let validation = validate_transfer_eligibility(
        &state.pool,
        &TransferValidationRequest {
            deal_id: deal_uuid,                          // was loan_id
            buyer_id: request.current_investor_id,       // was current_lender_id
            purchase_price: rust_decimal::Decimal::ZERO, // placeholder
        },
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to validate deal transfer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to validate deal transfer".to_string(),
        )
    })?;

    let response = DealTransferValidationResponse {
        // was TransferValidationResponse
        is_valid: validation.is_valid,
        deal_id: deal_id.clone(),                         // was loan_id
        current_investor_id: request.current_investor_id, // was current_lender_id
        message: if validation.is_valid {
            "Deal transfer is valid".to_string()
        } else {
            "Deal transfer is not valid".to_string()
        },
    };

    info!(
        "✅ Deal transfer validation completed: {} - valid: {}",
        deal_id, validation.is_valid
    );
    Ok(Json(response))
}

/// Cancel deal transfer (consolidated from cancel_transfer functionality)
#[utoipa::path(
    delete,
    path = "/api/v1/deal/transfer/{transfer_id}/cancel",
    params(
        ("transfer_id" = String, Path, description = "Transfer ID to cancel")
    ),
    responses(
        (status = 200, description = "Transfer cancelled successfully"),
        (status = 404, description = "Transfer not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn cancel_deal_transfer_endpoint(
    State(state): State<ApiState>,
    Path(transfer_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("❌ API: Cancelling deal transfer: {}", transfer_id);

    let transfer_uuid = uuid::Uuid::parse_str(&transfer_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid transfer ID format".to_string(),
        )
    })?;

    cancel_transfer(&state.pool, transfer_uuid, uuid::Uuid::nil()) // TODO: Add authentication to get current user
        .await
        .map_err(|e| {
            error!("❌ Failed to cancel deal transfer: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to cancel deal transfer".to_string(),
            )
        })?;

    info!("✅ Deal transfer cancelled: {}", transfer_id);
    Ok(Json(serde_json::json!({
        "message": "Deal transfer cancelled successfully",
        "transfer_id": transfer_id
    })))
}
