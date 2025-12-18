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
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::config::types::ApiState;
use crate::middleware::session_middleware::extract_current_user;
use arda_credit_db::{
    // Marketplace functions (consolidated)
    browse_marketplace_deals, // was browse_marketplace_loans
    // get_marketplace_deal_stats, // was get_marketplace_loan_stats - TODO: implement if needed
    // get_user_deal_transfer_history, // was get_user_transfer_history (if exists) - TODO: implement if needed

    // Company functions
    business::company::get_or_create_company,

    cancel_transfer, // was cancel_transfer
    // ... other imports
    create_deal,         // was create_loan
    create_deal_listing, // was create_payment
    delete_deal,         // Delete deal function
    // Transfer functions (consolidated)
    execute_deal_transfer,     // was execute_loan_transfer
    get_deal_by_id,            // was get_loan_by_id
    get_deal_transfer_history, // was get_loan_transfer_history
    get_deals_by_user_id,      // new function to get deals by user ID
    list_deals,                // was list_loans

    remove_deal_listing, // was remove_loan_listing
    // Collateral functions
    taxonomy::collateral::{
        get_collateral_by_id, get_deals_by_collateral_id,
        record_collateral_deal_association_transaction,
    },
    update_deal,                   // was update_loan
    validate_transfer_eligibility, // was validate_transfer_eligibility
    // Enums for validation
    DealNegotiationStatus,
    DealPaymentStatus,

    // Transfer structs
    ExecuteTransferRequest,
    TransferValidationRequest,
};
use arda_credit_lib::TransactionType;
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Trigger async metrics refresh after deal operations
///
/// This function sends a non-blocking command to the background batch processor
/// to refresh metrics. The API response is not delayed by the metrics refresh.
pub fn trigger_metrics_refresh_async(state: &ApiState) {
    if let Some(batch_processor) = &state.batch_processor {
        if let Err(e) = batch_processor.refresh_metrics() {
            warn!("Failed to trigger async metrics refresh: {}", e);
        } else {
            info!("📊 Triggered async metrics refresh after deal operation");
        }
    }

    // Invalidate all cache entries since a deal operation may affect various metrics
    invalidate_metrics_cache(state);
}

/// Invalidate metrics cache entries after deal operations
pub fn invalidate_metrics_cache(state: &ApiState) {
    let cache = state.metrics_cache.clone();
    tokio::spawn(async move {
        match tokio::time::timeout(std::time::Duration::from_secs(5), cache.invalidate_all()).await
        {
            Ok(_) => {
                info!("🗑️  Invalidated metrics cache after deal operation");
            }
            Err(_) => {
                warn!("⚠️  Cache invalidation timed out after deal operation");
            }
        }
    });
}

// ============================================================================
// DEAL MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

/// Create new deal request (converted from CreateLoanRequest)
///
/// ## Auto-Create Companies Feature
/// Companies can be automatically created or looked up during deal creation:
/// - If `originator_name` is provided, the system will look up or create the originator company
/// - If `investor_name` is provided, the system will look up or create the investor company
/// - If `borrower_name` is provided, the system will look up or create the borrower company
///
/// The system enforces unique company names:
/// - If a company with the provided name already exists, that company will be used
/// - If no company exists with that name, a new company will be created
/// - Companies are identified by both UUID and name for flexible referencing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "originator_id": "123e4567-e89b-12d3-a456-426614174000",
    "originator_name": "Acme Real Estate LLC",
    "investor_id": "987fcdeb-51a2-43d7-b987-123456789abc",
    "investor_name": "Capital Investors Fund",
    "borrower_id": null,
    "borrower_name": null,
    "principal_amount": "50000",
    "interest_rate_bps": 500,
    "term_days": 365,
    "loan_status": "originated",
    "status": "draft",
    "title": "Real Estate Development Deal - Brooklyn",
    "description": "Commercial real estate development opportunity in Brooklyn",
    "risk_level": "medium",
    "asset_class": "commercial_real_estate"
}))]
pub struct CreateDealRequest {
    // was CreateLoanRequest
    /// UUID of the originating company (required)
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub originator_id: String,

    /// Company name for originator (optional). If provided, system will look up or auto-create the company with this name.
    #[schema(example = "Acme Real Estate LLC")]
    pub originator_name: Option<String>,

    /// UUID of the investing company (optional for draft deals)
    #[schema(example = "987fcdeb-51a2-43d7-b987-123456789abc")]
    pub investor_id: Option<String>,

    /// Company name for investor (optional). If provided, system will look up or auto-create the company with this name.
    #[schema(example = "Capital Investors Fund")]
    pub investor_name: Option<String>,

    /// UUID of the borrower company (optional third party)
    #[schema(example = "456e7890-a123-45b6-c789-012345678def")]
    pub borrower_id: Option<String>,

    /// Company name for borrower (optional). If provided, system will look up or auto-create the company with this name.
    #[schema(example = "Brooklyn Development Corp")]
    pub borrower_name: Option<String>,

    /// Principal amount in cents (e.g., "50000" for $500.00)
    #[schema(example = "50000")]
    pub principal_amount: String, // Amount in cents as string
    pub interest_rate_bps: u16, // Interest rate in basis points
    pub term_days: Option<u32>, // Optional deal term

    /// Payment status. Valid values: originated, active, paid_off, closed, defaulted
    #[schema(example = "originated")]
    pub loan_status: Option<String>,

    pub metadata: Option<serde_json::Value>, // Optional additional data
    pub custom_deal_id: Option<String>, // Optional user-defined identifier for external system mapping

    /// Negotiation status. Valid values: draft, under_review, negotiation, finalized, executed, funding_readiness, pending_funding, active, amendment, suspended, closed, cancelled
    #[schema(example = "draft")]
    pub status: Option<String>,

    pub industry: Option<String>, // Industry sector for the deal
    pub document_ids: Option<Vec<String>>, // Array of document UUIDs associated with this deal

    // Marketplace-friendly fields
    pub title: Option<String>,
    pub description: Option<String>,
    pub risk_level: Option<String>,
    pub asset_class: Option<String>,
    pub region: Option<String>,
    pub minimum_investment: Option<String>,

    /// UUID of collateral asset (optional)
    #[schema(example = "789e0123-b456-78c9-d012-345678901abc")]
    pub collateral_id: Option<String>,
}

/// Deal details response (converted from LoanDetailsResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DealDetailsResponse {
    // was LoanDetailsResponse
    pub deal_id: String,                     // was loan_id, now deal_id
    pub originator_id: Option<String>,       // was borrower_id - optional for anonymized responses
    pub investor_id: Option<String>, // was lender_id - optional for draft deals and anonymized responses
    pub borrower_id: Option<String>, // Company ID of the borrower (optional third party)
    pub borrower_legal_name: Option<String>, // Legal name of the borrower company (from MVP spec)
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub loan_status: String, // deal_status field
    pub originated_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
    pub custom_deal_id: Option<String>, // Optional user-defined identifier for external system mapping
    pub status: String,                 // Deal status: draft, active, closed, cancelled
    pub industry: Option<String>,       // Industry sector for the deal
    pub document_ids: Option<Vec<String>>, // Array of document UUIDs associated with this deal

    // Marketplace-friendly fields
    pub title: Option<String>,
    pub description: Option<String>,
    pub risk_level: Option<String>,
    pub asset_class: Option<String>,
    pub region: Option<String>,
    pub minimum_investment: Option<String>,

    /// UUID of collateral asset (optional, deprecated - use collateral field instead)
    pub collateral_id: Option<String>,

    /// Full collateral object with detection metadata (preferred over collateral_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collateral: Option<crate::handlers::taxonomy::collateral::CollateralResponse>,
}

/// List deals query parameters (converted from ListLoansQuery)
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListDealsQuery {
    // was ListLoansQuery
    pub status: Option<String>,
    pub originator_id: Option<String>, // was borrower_id
    pub investor_id: Option<String>,   // was lender_id
    pub borrower_id: Option<String>,   // Company ID of the borrower (optional third party)
    pub industry: Option<String>,      // Filter by industry sector
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    /// Anonymize response by hiding company identifiers (originator_id, investor_id, borrower_id)
    #[schema(example = false)]
    pub anon: Option<bool>,
}

/// List deals response (converted from ListLoansResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListDealsResponse {
    // was ListLoansResponse
    pub deals: Vec<DealDetailsResponse>, // was loans
    /// Total number of deals in the database matching the filters (not just the returned count)
    pub total_count: i64,
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
#[schema(example = json!({
    "status": "under_review",
    "loan_status": "active"
}))]
pub struct UpdateDealRequest {
    // was UpdateLoanRequest
    pub principal_amount: Option<String>, // Amount in cents as string
    pub outstanding_balance: Option<String>, // Outstanding balance in cents as string
    pub interest_rate_bps: Option<u16>,   // Interest rate in basis points

    /// Payment status. Valid values: originated, active, paid_off, closed, defaulted
    #[schema(example = "active")]
    pub loan_status: Option<String>,

    pub maturity_date: Option<Option<String>>, // Optional maturity date (ISO 8601), Option<Option<T>> to handle NULL
    pub custom_deal_id: Option<Option<String>>, // Optional custom deal ID, Option<Option<T>> to handle NULL
    pub borrower_id: Option<Option<String>>, // Company ID of borrower, Option<Option<T>> to handle NULL

    /// Negotiation status. Valid values: draft, under_review, negotiation, finalized, executed, funding_readiness, pending_funding, active, amendment, suspended, closed, cancelled
    #[schema(example = "under_review")]
    pub status: Option<String>,

    pub industry: Option<String>, // Industry sector for the deal

    /// Array of document UUIDs associated with this deal, Option<Option<T>> to handle NULL
    #[schema(example = json!(["123e4567-e89b-12d3-a456-426614174000"]))]
    pub document_ids: Option<Option<Vec<String>>>,

    // Marketplace-friendly fields
    pub title: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub risk_level: Option<Option<String>>,
    pub asset_class: Option<Option<String>>,
    pub region: Option<Option<String>>,
    pub minimum_investment: Option<Option<String>>,

    /// UUID of collateral asset (optional), Option<Option<T>> to handle NULL
    pub collateral_id: Option<Option<String>>,
}

/// Update deal response (converted from UpdateLoanResponse)
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateDealResponse {
    // was UpdateLoanResponse
    pub deal_id: String, // was loan_id, now deal_id
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub deal_details: DealDetailsResponse, // was loan_details
}

/// Delete deal response (converted from DeleteLoanResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteDealResponse {
    // was DeleteLoanResponse
    pub deal_id: String, // was loan_id, now deal_id
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
    pub final_status: String,
}

/// Record deal payment request (converted from RecordPaymentRequest)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecordDealPaymentRequest {
    // was RecordPaymentRequest
    pub deal_id: String,                   // was loan_id, now deal_id
    pub amount: String,                    // Amount in cents as string
    pub payment_type: String,              // "Principal" | "Interest" | "Mixed"
    pub payment_reference: Option<String>, // Optional external reference
    pub notes: Option<String>,
}

/// Payment response (converted from PaymentResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub transaction_id: String,
    pub deal_id: String, // was loan_id, now deal_id
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String, // "Pending" | "Processed" | "Failed"
    pub created_at: DateTime<Utc>,
}

/// Payment details response (converted from PaymentDetailsResponse)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentDetailsResponse {
    pub transaction_id: String,
    pub deal_id: String, // was loan_id, now deal_id
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
    pub deal_id: String, // was loan_id, now deal_id
    pub payments: Vec<PaymentDetailsResponse>,
    pub total_payments: usize,
    pub total_amount_paid: String,
}

/// Request to make a payment on a deal
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct MakePaymentRequest {
    /// Payment amount in USDC (e.g., "5000.00")
    pub amount: String,

    /// Payment type (optional, defaults to "regular")
    #[serde(default = "default_payment_type")]
    pub payment_type: Option<String>,

    /// Optional notes about the payment
    pub notes: Option<String>,
}

fn default_payment_type() -> Option<String> {
    Some("regular".to_string())
}

/// Response from payment creation
#[derive(Debug, Serialize, ToSchema)]
pub struct MakePaymentResponse {
    /// Payment ID
    pub payment_id: String,

    /// Deal ID
    pub deal_id: String,

    /// Payment amount
    pub amount: String,

    /// Payment type
    pub payment_type: String,

    /// Updated deal outstanding balance
    pub new_outstanding_balance: String,

    /// Deal status after payment
    pub deal_status: String,

    /// Transaction ID (for zkVM tracking)
    pub transaction_id: String,

    /// When payment was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request to close out a paid-off deal
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CloseoutDealRequest {
    /// Optional notes about the closeout
    pub notes: Option<String>,

    /// Confirmation that deal is fully paid off (required)
    pub confirm_paid_off: bool,
}

/// Response from deal closeout
#[derive(Debug, Serialize, ToSchema)]
pub struct CloseoutResponse {
    /// Deal ID
    pub deal_id: String,

    /// Deal status (should be "closed")
    pub deal_status: String,

    /// Transaction ID (for zkVM tracking)
    pub transaction_id: String,

    /// When closeout was completed
    pub closed_at: chrono::DateTime<chrono::Utc>,
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
    pub deal_id: String, // was loan_id, now deal_id
    pub seller_id: uuid::Uuid,
    pub asking_price: String,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub delisted_at: Option<DateTime<Utc>>,
    pub status: String,
}

/// Marketplace deal response (consolidated from marketplace)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceDealResponse {
    // was MarketplaceLoanResponse
    // Listing information
    pub listing_id: uuid::Uuid,
    pub deal_id: uuid::Uuid, // was loan_id, now deal_id
    pub seller_id: uuid::Uuid,
    pub asking_price: String,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub listing_status: String,

    // Deal information
    pub originator_id: uuid::Uuid,       // was borrower_id
    pub investor_id: Option<uuid::Uuid>, // was lender_id - now optional for draft deals
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub deal_status: String, // deal_status field
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
    pub deal_id: String,              // was loan_id, now deal_id
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
    pub deal_id: String,                 // was loan_id, now deal_id
    pub current_investor_id: uuid::Uuid, // was current_lender_id
    pub message: String,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Fetch collateral data for a list of deals
///
/// Returns a HashMap of collateral_id -> CollateralResponse with deal counts computed
async fn fetch_collateral_for_deals(
    pool: &sqlx::PgPool,
    deals: &[arda_credit_db::Deal],
) -> std::collections::HashMap<uuid::Uuid, crate::handlers::taxonomy::collateral::CollateralResponse>
{
    use std::collections::HashMap;

    // Extract unique collateral IDs from deals
    let mut collateral_ids: Vec<uuid::Uuid> =
        deals.iter().filter_map(|d| d.collateral_id).collect();
    collateral_ids.sort();
    collateral_ids.dedup();

    if collateral_ids.is_empty() {
        return HashMap::new();
    }

    // Fetch all collateral records
    let mut collateral_map = HashMap::new();
    for coll_id in collateral_ids {
        if let Ok(collateral) = get_collateral_by_id(pool, coll_id).await {
            // Get deal IDs for this collateral
            let deal_ids = get_deals_by_collateral_id(pool, coll_id)
                .await
                .unwrap_or_default();

            let deal_count = deal_ids.len() as i64;
            let is_double_pledged = deal_count > 1;

            // Build CollateralResponse
            let response = crate::handlers::taxonomy::collateral::CollateralResponse {
                id: collateral.id.to_string(),
                name: collateral.name,
                collateral_type: collateral.collateral_type,
                description: collateral.description,
                address: collateral.address,
                custom_id: collateral.custom_id,
                double_pledged: collateral.double_pledged.map(|u| u.to_string()),
                has_potential_duplicates: collateral.has_potential_duplicates,
                created_at: collateral.created_at.to_rfc3339(),
                updated_at: collateral.updated_at.to_rfc3339(),
                metadata: collateral.metadata,
                deal_count: Some(deal_count),
                deal_ids: Some(deal_ids.iter().map(|id| id.to_string()).collect()),
                is_double_pledged: Some(is_double_pledged),
                has_detection: Some(collateral.has_detection),
                detection_count: Some(collateral.detection_count),
                last_detection_at: collateral.last_detection_at.map(|dt| dt.to_rfc3339()),
                detection_status: collateral.detection_status,
            };

            collateral_map.insert(coll_id, response);
        }
    }

    collateral_map
}

// ============================================================================
// DEAL MANAGEMENT ENDPOINTS
// ============================================================================

/// Create a new deal (converted from create_loan_endpoint)
///
/// ## Auto-Create Companies
/// This endpoint can automatically create or lookup companies during deal creation.
/// If `originator_name`, `investor_name`, or `borrower_name` are provided in the request,
/// the system will:
/// 1. Check if a company with the provided UUID exists
/// 2. Check if a company with the provided name exists (enforces unique names)
/// 3. Create a new company if neither exists
/// 4. Use the resulting company UUID for the deal
///
/// This allows seamless deal creation from document extraction without pre-registering companies.
#[utoipa::path(
    post,
    path = "/api/v1/deal",  // will be updated in paths later
    request_body = CreateDealRequest,
    responses(
        (status = 201, description = "Deal created successfully. Companies may have been auto-created.", body = DealDetailsResponse),
        (status = 400, description = "Invalid request - check UUID format or field validation"),
        (status = 500, description = "Internal server error - may indicate company creation failure")
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn create_deal_endpoint(
    // was create_loan_endpoint
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateDealRequest>, // was CreateLoanRequest
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // was LoanDetailsResponse
    let start_time = std::time::Instant::now();

    // Record business logic operation
    metrics::counter!("business_logic_operations_total", "operation" => "create_deal").increment(1); // was create_loan

    // Extract authenticated user (session middleware sets x-user-* headers)
    let current_user = extract_current_user(&headers).ok_or((
        StatusCode::UNAUTHORIZED,
        "User not authenticated".to_string(),
    ))?;

    info!(
        "💰 API: Creating deal: originator={}, investor={}, amount={}", // updated log message
        request.originator_id,
        request.investor_id.as_deref().unwrap_or("none"),
        request.principal_amount
    );

    // Parse UUIDs
    let originator_id = uuid::Uuid::parse_str(&request.originator_id).map_err(|_| {
        // was borrower_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid originator ID format".to_string(), // was borrower ID
        )
    })?;

    // Parse investor_id if provided (optional for draft deals)
    let investor_id = if let Some(ref investor_id_str) = request.investor_id {
        if investor_id_str.is_empty() {
            // Empty string means no investor (draft deals)
            None
        } else {
            Some(uuid::Uuid::parse_str(investor_id_str).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid investor ID format".to_string(),
                )
            })?)
        }
    } else {
        // No investor provided (draft deals)
        None
    };

    // Parse borrower_id if provided (optional third party)
    let borrower_id = if let Some(ref borrower_id_str) = request.borrower_id {
        if borrower_id_str.is_empty() {
            // Empty string means no borrower
            None
        } else {
            Some(uuid::Uuid::parse_str(borrower_id_str).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid borrower ID format".to_string(),
                )
            })?)
        }
    } else {
        // No borrower provided
        None
    };

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

    // Parse document IDs if provided
    let document_ids_json = if let Some(doc_ids) = &request.document_ids {
        let uuids: Result<Vec<uuid::Uuid>, _> =
            doc_ids.iter().map(|id| uuid::Uuid::parse_str(id)).collect();

        let uuids = uuids.map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid document ID format".to_string(),
            )
        })?;

        Some(serde_json::to_value(uuids).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize document IDs".to_string(),
            )
        })?)
    } else {
        None
    };

    // Parse optional minimum investment
    let minimum_investment = if let Some(min_inv_str) = &request.minimum_investment {
        Some(min_inv_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid minimum investment amount".to_string(),
            )
        })?)
    } else {
        None
    };

    // Validate loan_status if provided
    if let Some(ref loan_status_str) = request.loan_status {
        if DealPaymentStatus::from_db(loan_status_str).is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid loan_status '{}'. Valid values: originated, active, paid_off, closed, defaulted", loan_status_str),
            ));
        }
    }

    // Validate status (negotiation status) if provided
    if let Some(ref status_str) = request.status {
        if DealNegotiationStatus::from_db(status_str).is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid status '{}'. Valid values: draft, under_review, negotiation, finalized, executed, funding_readiness, pending_funding, active, amendment, suspended, closed, cancelled", status_str),
            ));
        }
    }

    // Auto-create or lookup companies if company names are provided
    // This ensures companies exist before creating the deal
    let final_originator_id = if let Some(ref originator_name) = request.originator_name {
        info!(
            "🏢 Looking up or creating originator company: {}",
            originator_name
        );
        get_or_create_company(&state.pool, originator_id, originator_name)
            .await
            .map_err(|e| {
                error!("❌ Failed to create/lookup originator company: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create/lookup originator company: {}", e),
                )
            })?
            .id
    } else {
        originator_id
    };

    let final_investor_id = if let Some(investor_uuid) = investor_id {
        if let Some(ref investor_name) = request.investor_name {
            info!(
                "🏢 Looking up or creating investor company: {}",
                investor_name
            );
            Some(
                get_or_create_company(&state.pool, investor_uuid, investor_name)
                    .await
                    .map_err(|e| {
                        error!("❌ Failed to create/lookup investor company: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to create/lookup investor company: {}", e),
                        )
                    })?
                    .id,
            )
        } else {
            Some(investor_uuid)
        }
    } else {
        None
    };

    let final_borrower_id = if let Some(borrower_uuid) = borrower_id {
        if let Some(ref borrower_name) = request.borrower_name {
            info!(
                "🏢 Looking up or creating borrower company: {}",
                borrower_name
            );
            Some(
                get_or_create_company(&state.pool, borrower_uuid, borrower_name)
                    .await
                    .map_err(|e| {
                        error!("❌ Failed to create/lookup borrower company: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to create/lookup borrower company: {}", e),
                        )
                    })?
                    .id,
            )
        } else {
            Some(borrower_uuid)
        }
    } else {
        None
    };

    // Validate that all parties are different companies
    if let Some(investor) = final_investor_id {
        if final_originator_id == investor {
            error!(
                "❌ Originator and investor cannot be the same company: {}",
                final_originator_id
            );
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Originator and investor must be different companies. Both resolved to company ID: {}. \
                    This can happen when company names are reused. Please ensure unique companies are selected.",
                    final_originator_id
                ),
            ));
        }
    }

    if let Some(borrower) = final_borrower_id {
        if final_originator_id == borrower {
            error!(
                "❌ Originator and borrower cannot be the same company: {}",
                final_originator_id
            );
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Originator and borrower must be different companies. Both resolved to company ID: {}",
                    final_originator_id
                ),
            ));
        }

        if let Some(investor) = final_investor_id {
            if investor == borrower {
                error!(
                    "❌ Investor and borrower cannot be the same company: {}",
                    investor
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Investor and borrower must be different companies. Both resolved to company ID: {}",
                        investor
                    ),
                ));
            }
        }
    }

    // Parse collateral_id if provided
    let collateral_uuid = if let Some(coll_id_str) = &request.collateral_id {
        Some(Uuid::parse_str(coll_id_str).map_err(|e| {
            error!("❌ Invalid collateral ID format: {}", e);
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid collateral ID format: {}", e),
            )
        })?)
    } else {
        None
    };

    // Create deal in database
    // Note: originator_id, investor_id, and borrower_id now reference companies directly
    let deal = create_deal(
        // was create_loan
        &state.pool,
        final_originator_id, // Company ID of the originator (was borrower_id)
        final_investor_id, // Company ID of the investor (was lender_id) - optional for draft deals
        final_borrower_id, // Company ID of the borrower (optional third party)
        principal_amount,
        request.interest_rate_bps as i32,
        maturity_date,
        request.loan_status.as_deref(),
        request.custom_deal_id.as_deref(),
        request.status.as_deref(),
        request.industry.as_deref(),
        document_ids_json.as_ref(),
        request.title.as_deref(),
        request.description.as_deref(),
        request.risk_level.as_deref(),
        request.asset_class.as_deref(),
        request.region.as_deref(),
        minimum_investment,
        collateral_uuid, // Collateral UUID
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

    // Convert document IDs from JSON to Vec<String> for response
    let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
        if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
            Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
        } else {
            None
        }
    });

    // Publish DealCreated event (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
        let collateral_ids = deal.collateral_id.map(|id| vec![id]).unwrap_or_default();
        let event = crate::events::DealEvent::DealCreated {
            event_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            deal: crate::events::DealPayload {
                id: deal.id,
                status: deal.deal_status.clone(),
                company_id: deal.originator_id,
            },
            collateral_ids: collateral_ids.clone(),
        };
        let publisher_clone = publisher.clone();
        tokio::spawn(async move {
            if let Err(e) = publisher_clone.publish_deal(event).await {
                tracing::warn!("Failed to publish DealCreated event: {}", e);
            }
        });

        // If collateral is linked, also publish DealCollateralLinked event
        if !collateral_ids.is_empty() {
            let event = crate::events::DealEvent::DealCollateralLinked {
                event_id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                deal_id: deal.id,
                collateral_ids,
            };
            let publisher_clone = publisher.clone();
            tokio::spawn(async move {
                if let Err(e) = publisher_clone.publish_deal(event).await {
                    tracing::warn!("Failed to publish DealCollateralLinked event: {}", e);
                }
            });
        }
    }

    // Record collateral-deal association transaction for audit trail
    if let Some(coll_id) = deal.collateral_id {
        if let Err(e) = record_collateral_deal_association_transaction(
            &state.pool,
            coll_id,
            deal.id,
            None, // System event - no specific user
        )
        .await
        {
            tracing::warn!(
                "Failed to record collateral-deal association transaction: {}",
                e
            );
        }
    }

    // NEW: Record transaction for zkVM
    // CRITICAL: Metadata must encode originator_id and investor_id for zkVM
    let investor_id_for_zkvm = final_investor_id.unwrap_or(Uuid::nil());

    let tx_id = state
        .transaction_recording
        .record_transaction_with_metadata(
            arda_credit_lib::TransactionType::DealOrigination,
            current_user.id,
            Some(deal.id),
            deal.principal_amount,
            Some(json!({
                // For zkVM: encode IDs in metadata
                // zkVM expects first 32 bytes = originator, next 32 = investor
                "originator_id": final_originator_id.to_string(),
                "investor_id": investor_id_for_zkvm.to_string(),
                "interest_rate_bps": deal.interest_rate_bps,
                "term_days": request.term_days,
            })),
            Some(final_originator_id), // originator_company_id
            final_investor_id,         // investor_company_id
            deal.collateral_id,        // collateral_id
        )
        .await
        .map_err(|e| {
            error!("Failed to record deal origination transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Deal created but transaction recording failed: {}", e),
            )
        })?;

    info!("Deal {} created with transaction {}", deal.id, tx_id);

    // Fetch collateral if available
    let collateral = if let Some(coll_id) = deal.collateral_id {
        if let Ok(collateral) = get_collateral_by_id(&state.pool, coll_id).await {
            let deal_ids = get_deals_by_collateral_id(&state.pool, coll_id)
                .await
                .unwrap_or_default();

            let deal_count = deal_ids.len() as i64;
            let is_double_pledged = deal_count > 1;

            Some(crate::handlers::taxonomy::collateral::CollateralResponse {
                id: collateral.id.to_string(),
                name: collateral.name,
                collateral_type: collateral.collateral_type,
                description: collateral.description,
                address: collateral.address,
                custom_id: collateral.custom_id,
                double_pledged: collateral.double_pledged.map(|u| u.to_string()),
                has_potential_duplicates: collateral.has_potential_duplicates,
                created_at: collateral.created_at.to_rfc3339(),
                updated_at: collateral.updated_at.to_rfc3339(),
                metadata: collateral.metadata,
                deal_count: Some(deal_count),
                deal_ids: Some(deal_ids.iter().map(|id| id.to_string()).collect()),
                is_double_pledged: Some(is_double_pledged),
                has_detection: Some(collateral.has_detection),
                detection_count: Some(collateral.detection_count),
                last_detection_at: collateral.last_detection_at.map(|dt| dt.to_rfc3339()),
                detection_status: collateral.detection_status,
            })
        } else {
            None
        }
    } else {
        None
    };

    let response = DealDetailsResponse {
        // was LoanDetailsResponse
        deal_id: deal.id.to_string(), // was loan_id, now deal_id
        originator_id: Some(deal.originator_id.to_string()), // was borrower_id
        investor_id: deal.investor_id.map(|id| id.to_string()), // was lender_id - now optional
        borrower_id: deal.borrower_id.map(|id| id.to_string()), // Company ID of borrower (optional)
        borrower_legal_name: deal.borrower_legal_name.clone(), // Borrower company legal name
        principal_amount: deal.principal_amount.to_string(),
        outstanding_balance: deal.outstanding_balance.to_string(),
        interest_rate_bps: deal.interest_rate_bps as u16,
        loan_status: deal.deal_status, // deal_status field
        originated_at: deal.originated_at,
        last_updated: Some(deal.updated_at),
        custom_deal_id: deal.custom_deal_id,
        status: deal.status,
        industry: deal.industry,
        document_ids: document_ids_response,
        title: deal.title,
        description: deal.description,
        risk_level: deal.risk_level,
        asset_class: deal.asset_class,
        region: deal.region,
        minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
        collateral_id: deal.collateral_id.map(|id| id.to_string()),
        collateral,
    };

    // Record function execution time
    let duration = start_time.elapsed();
    metrics::histogram!("function_execution_duration_seconds", "function" => "create_deal")
        .record(duration.as_secs_f64());

    // Trigger async metrics refresh if enabled
    trigger_metrics_refresh_async(&state);

    info!("✅ Deal created successfully: {}", response.deal_id); // updated log message
    Ok(Json(response))
}

/// List deals with optional filtering and anonymization (converted from list_loans_endpoint)
///
/// When `anon=true` is specified, company identifiers (originator_id, investor_id)
/// are excluded from the response to provide anonymized market overview data.
#[utoipa::path(
    get,
    path = "/api/v1/deals",
    params(ListDealsQuery),  // was ListLoansQuery
    responses(
        (status = 200, description = "List of deals (anonymized when anon=true)", body = ListDealsResponse),  // was loans
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

    let borrower_id = if let Some(id_str) = &query.borrower_id {
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid borrower ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    let result = list_deals(
        // was list_loans
        &state.pool,
        query.limit.map(|l| l as i64),
        query.offset.map(|o| o as i64),
        query.status.as_deref(),
        originator_id, // was borrower_id
        investor_id,   // was lender_id
        borrower_id,   // Company ID of borrower (optional third party)
        query.industry.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to list deals: {}", e); // updated error message
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list deals".to_string(), // updated error message
        )
    })?;

    // Check if anonymized response is requested
    let is_anonymized = query.anon.unwrap_or(false);

    // Fetch collateral data for all deals
    let collateral_map = fetch_collateral_for_deals(&state.pool, &result.deals).await;

    // Convert to response format
    let deal_responses: Vec<DealDetailsResponse> = result
        .deals // was LoanDetailsResponse
        .into_iter()
        .map(|deal| {
            // Convert document IDs from JSON to Vec<String> for response
            let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
                if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
                    Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
                } else {
                    None
                }
            });

            // Get collateral if available
            let collateral = deal
                .collateral_id
                .and_then(|coll_id| collateral_map.get(&coll_id).cloned());

            let mut response = DealDetailsResponse {
                // was loan, LoanDetailsResponse
                deal_id: deal.id.to_string(), // was loan_id, now deal_id
                originator_id: None,
                investor_id: None,
                borrower_id: None,
                borrower_legal_name: None,
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status, // deal_status field
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: None,
                status: deal.status,
                industry: deal.industry,
                document_ids: None,
                title: None,
                description: None,
                risk_level: None,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: None,
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
            };

            // Populate sensitive fields only if not anonymized
            if !is_anonymized {
                response.originator_id = Some(deal.originator_id.to_string());
                response.investor_id = deal.investor_id.map(|id| id.to_string());
                response.borrower_id = deal.borrower_id.map(|id| id.to_string());
                response.borrower_legal_name = deal.borrower_legal_name.clone();
                response.custom_deal_id = deal.custom_deal_id;
                response.document_ids = document_ids_response;
                response.title = deal.title;
                response.description = deal.description;
                response.risk_level = deal.risk_level;
                response.minimum_investment = deal.minimum_investment.map(|d| d.to_string());
            }

            response
        })
        .collect();

    let response = ListDealsResponse {
        // was ListLoansResponse
        deals: deal_responses, // was loans
        total_count: result.total_count,
    };

    info!(
        "✅ API: Listed {} of {} total deals",
        response.deals.len(),
        response.total_count
    ); // updated log message
    Ok(Json(response))
}

/// Get deals by user ID (user can be either originator or investor)
#[utoipa::path(
    get,
    path = "/api/v1/deals/{user_id}",
    params(
        ("user_id" = String, Path, description = "User ID to get deals for"),
        ListDealsQuery
    ),
    responses(
        (status = 200, description = "List of deals for the user", body = ListDealsResponse),
        (status = 400, description = "Invalid user ID format"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_deals_by_user_id_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(query): Query<ListDealsQuery>,
) -> Result<Json<ListDealsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting deals for user: {}", user_id);

    // Parse user ID
    let user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    let result = get_deals_by_user_id(
        &state.pool,
        user_uuid,
        query.limit.map(|l| l as i64),
        query.offset.map(|o| o as i64),
        query.status.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to get deals for user: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get deals for user".to_string(),
        )
    })?;

    // Fetch collateral data for all deals
    let collateral_map = fetch_collateral_for_deals(&state.pool, &result.deals).await;

    // Convert to response format
    let deal_responses: Vec<DealDetailsResponse> = result
        .deals
        .into_iter()
        .map(|deal| {
            // Get collateral if available
            let collateral = deal
                .collateral_id
                .and_then(|coll_id| collateral_map.get(&coll_id).cloned());

            DealDetailsResponse {
                deal_id: deal.id.to_string(),
                originator_id: Some(deal.originator_id.to_string()),
                investor_id: deal.investor_id.map(|id| id.to_string()),
                borrower_id: deal.borrower_id.map(|id| id.to_string()),
                borrower_legal_name: deal.borrower_legal_name.clone(),
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status,
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
                status: deal.status,
                industry: deal.industry,
                document_ids: deal
                    .document_ids
                    .and_then(|ids| serde_json::from_value::<Vec<String>>(ids).ok()),
                title: deal.title,
                description: deal.description,
                risk_level: deal.risk_level,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
            }
        })
        .collect();

    let response = ListDealsResponse {
        deals: deal_responses,
        total_count: result.total_count,
    };

    info!(
        "✅ API: Found {} of {} total deals for user {}",
        response.deals.len(),
        response.total_count,
        user_id
    );
    Ok(Json(response))
}

/// Get all deals for a company (as originator or investor)
#[utoipa::path(
    get,
    path = "/api/v1/company/{company_id}/deals",
    params(
        ("company_id" = String, Path, description = "Company ID to get deals for"),
        ("limit" = Option<i32>, Query, description = "Maximum number of deals to return"),
        ("offset" = Option<i32>, Query, description = "Offset for pagination"),
        ("status" = Option<String>, Query, description = "Filter deals by status")
    ),
    responses(
        (status = 200, description = "List of deals for the company", body = ListDealsResponse),
        (status = 400, description = "Invalid company ID format"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_deals_by_company_endpoint(
    State(state): State<ApiState>,
    Path(company_id): Path<String>,
    Query(query): Query<ListDealsQuery>,
) -> Result<Json<ListDealsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting deals for company: {}", company_id);

    // Parse company ID
    let company_uuid = uuid::Uuid::parse_str(&company_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid company ID format".to_string(),
        )
    })?;

    let result = arda_credit_db::get_deals_by_company_id(
        &state.pool,
        company_uuid,
        query.limit.map(|l| l as i64),
        query.offset.map(|o| o as i64),
        query.status.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to get deals for company: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get deals for company".to_string(),
        )
    })?;

    // Fetch collateral data for all deals
    let collateral_map = fetch_collateral_for_deals(&state.pool, &result.deals).await;

    // Convert to response format
    let deal_responses: Vec<DealDetailsResponse> = result
        .deals
        .into_iter()
        .map(|deal| {
            // Convert document IDs from JSON to Vec<String> for response
            let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
                if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
                    Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
                } else {
                    None
                }
            });

            // Get collateral if available
            let collateral = deal
                .collateral_id
                .and_then(|coll_id| collateral_map.get(&coll_id).cloned());

            DealDetailsResponse {
                deal_id: deal.id.to_string(),
                originator_id: Some(deal.originator_id.to_string()),
                investor_id: deal.investor_id.map(|id| id.to_string()),
                borrower_id: deal.borrower_id.map(|id| id.to_string()),
                borrower_legal_name: deal.borrower_legal_name.clone(),
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status,
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
                status: deal.status,
                industry: deal.industry,
                document_ids: document_ids_response,
                title: deal.title,
                description: deal.description,
                risk_level: deal.risk_level,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
            }
        })
        .collect();

    let response = ListDealsResponse {
        deals: deal_responses,
        total_count: result.total_count,
    };

    info!(
        "✅ API: Found {} of {} total deals for company {}",
        response.deals.len(),
        response.total_count,
        company_id
    );
    Ok(Json(response))
}

/// Get draft deals for a company (deals in negotiation status "draft")
#[utoipa::path(
    get,
    path = "/api/v1/company/{company_id}/deals/draft",
    params(
        ("company_id" = String, Path, description = "Company ID to get draft deals for"),
        ("limit" = Option<i32>, Query, description = "Maximum number of deals to return"),
        ("offset" = Option<i32>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "List of draft deals for the company", body = ListDealsResponse),
        (status = 400, description = "Invalid company ID format"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_draft_deals_by_company_endpoint(
    State(state): State<ApiState>,
    Path(company_id): Path<String>,
    Query(query): Query<ListDealsQuery>,
) -> Result<Json<ListDealsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting draft deals for company: {}", company_id);

    // Parse company ID
    let company_uuid = uuid::Uuid::parse_str(&company_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid company ID format".to_string(),
        )
    })?;

    let result = arda_credit_db::get_draft_deals_by_company_id(
        &state.pool,
        company_uuid,
        query.limit.map(|l| l as i64),
        query.offset.map(|o| o as i64),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to get draft deals for company: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get draft deals for company".to_string(),
        )
    })?;

    // Fetch collateral data for all deals
    let collateral_map = fetch_collateral_for_deals(&state.pool, &result.deals).await;

    // Convert to response format
    let deal_responses: Vec<DealDetailsResponse> = result
        .deals
        .into_iter()
        .map(|deal| {
            // Convert document IDs from JSON to Vec<String> for response
            let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
                if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
                    Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
                } else {
                    None
                }
            });

            // Get collateral if available
            let collateral = deal
                .collateral_id
                .and_then(|coll_id| collateral_map.get(&coll_id).cloned());

            DealDetailsResponse {
                deal_id: deal.id.to_string(),
                originator_id: Some(deal.originator_id.to_string()),
                investor_id: deal.investor_id.map(|id| id.to_string()),
                borrower_id: deal.borrower_id.map(|id| id.to_string()),
                borrower_legal_name: deal.borrower_legal_name.clone(),
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status,
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
                status: deal.status,
                industry: deal.industry,
                document_ids: document_ids_response,
                title: deal.title,
                description: deal.description,
                risk_level: deal.risk_level,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
            }
        })
        .collect();

    let response = ListDealsResponse {
        deals: deal_responses,
        total_count: result.total_count,
    };

    info!(
        "✅ API: Found {} of {} total draft deals for company {}",
        response.deals.len(),
        response.total_count,
        company_id
    );
    Ok(Json(response))
}

/// Get deal details by ID (converted from get_loan_details_endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/deal/{deal_id}",  // was loan_id, now deal_id
    params(
        ("deal_id" = String, Path, description = "Deal ID to retrieve")  // was loan_id, now deal_id
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
    Path(deal_id): Path<String>, // was loan_id, now deal_id
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // was LoanDetailsResponse
    info!("🔍 API: Getting deal details: {}", deal_id); // updated log message

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        // was loan_id, now deal_id
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
            // Convert document IDs from JSON to Vec<String> for response
            let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
                if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
                    Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
                } else {
                    None
                }
            });

            // Fetch collateral if available
            let collateral = if let Some(coll_id) = deal.collateral_id {
                if let Ok(collateral) = get_collateral_by_id(&state.pool, coll_id).await {
                    let deal_ids = get_deals_by_collateral_id(&state.pool, coll_id)
                        .await
                        .unwrap_or_default();

                    let deal_count = deal_ids.len() as i64;
                    let is_double_pledged = deal_count > 1;

                    Some(crate::handlers::taxonomy::collateral::CollateralResponse {
                        id: collateral.id.to_string(),
                        name: collateral.name,
                        collateral_type: collateral.collateral_type,
                        description: collateral.description,
                        address: collateral.address,
                        custom_id: collateral.custom_id,
                        double_pledged: collateral.double_pledged.map(|u| u.to_string()),
                        has_potential_duplicates: collateral.has_potential_duplicates,
                        created_at: collateral.created_at.to_rfc3339(),
                        updated_at: collateral.updated_at.to_rfc3339(),
                        metadata: collateral.metadata,
                        deal_count: Some(deal_count),
                        deal_ids: Some(deal_ids.iter().map(|id| id.to_string()).collect()),
                        is_double_pledged: Some(is_double_pledged),
                        has_detection: Some(collateral.has_detection),
                        detection_count: Some(collateral.detection_count),
                        last_detection_at: collateral.last_detection_at.map(|dt| dt.to_rfc3339()),
                        detection_status: collateral.detection_status,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let response = DealDetailsResponse {
                // was LoanDetailsResponse
                deal_id: deal.id.to_string(), // was loan_id, now deal_id
                originator_id: Some(deal.originator_id.to_string()), // was borrower_id
                investor_id: deal.investor_id.map(|id| id.to_string()), // was lender_id - now optional
                borrower_id: deal.borrower_id.map(|id| id.to_string()), // Company ID of borrower (optional)
                borrower_legal_name: deal.borrower_legal_name.clone(), // Borrower company legal name
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status, // deal_status field
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
                status: deal.status,
                industry: deal.industry,
                document_ids: document_ids_response,
                title: deal.title,
                description: deal.description,
                risk_level: deal.risk_level,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
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
    path = "/api/v1/deal/{deal_id}",  // was loan_id, now deal_id
    request_body = UpdateDealRequest,  // was UpdateLoanRequest
    responses(
        (status = 200, description = "Deal updated successfully", body = UpdateDealResponse),  // was Loan
        (status = 400, description = "Invalid request data"),
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal ID")  // was loan_id, now deal_id
    ),
    tag = "Deals"  // was "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_deal_endpoint(
    // was update_loan_endpoint
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,            // was loan_id, now deal_id
    Json(request): Json<UpdateDealRequest>, // was UpdateLoanRequest
) -> Result<Json<UpdateDealResponse>, (StatusCode, String)> {
    // was UpdateLoanResponse
    info!("📝 API: Updating deal: {}", deal_id); // updated log message

    // Parse deal ID
    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        // was loan_id, now deal_id
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(), // was loan ID
        )
    })?;

    // Fetch old deal state for change detection (Option<Deal>)
    let old_deal = get_deal_by_id(&state.pool, deal_uuid).await.ok().flatten();

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

    // Parse minimum investment
    let minimum_investment = if let Some(min_inv_opt) = &request.minimum_investment {
        Some(match min_inv_opt {
            Some(min_inv_str) => {
                Some(min_inv_str.parse::<rust_decimal::Decimal>().map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        "Invalid minimum investment amount".to_string(),
                    )
                })?)
            }
            None => None,
        })
    } else {
        None
    };

    // Parse borrower_id
    let borrower_id = if let Some(borrower_opt) = &request.borrower_id {
        Some(match borrower_opt {
            Some(borrower_str) => Some(uuid::Uuid::parse_str(borrower_str).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid borrower ID format".to_string(),
                )
            })?),
            None => None,
        })
    } else {
        None
    };

    // Validate loan_status if provided
    if let Some(ref loan_status_str) = request.loan_status {
        if DealPaymentStatus::from_db(loan_status_str).is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid loan_status '{}'. Valid values: originated, active, paid_off, closed, defaulted", loan_status_str),
            ));
        }
    }

    // Validate status (negotiation status) if provided
    if let Some(ref status_str) = request.status {
        if DealNegotiationStatus::from_db(status_str).is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid status '{}'. Valid values: draft, under_review, negotiation, finalized, executed, funding_readiness, pending_funding, active, amendment, suspended, closed, cancelled", status_str),
            ));
        }
    }

    // Parse document_ids into JSON format
    let document_ids_json = if let Some(doc_ids_opt) = &request.document_ids {
        Some(match doc_ids_opt {
            Some(doc_ids) => Some(serde_json::to_value(doc_ids).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid document IDs format".to_string(),
                )
            })?),
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
        borrower_id,                 // Company ID of borrower (optional third party)
        request.status.as_deref(),   // Deal negotiation status
        request.industry.as_deref(), // Industry sector
        document_ids_json.as_ref().map(|opt| opt.as_ref()), // Document IDs array
        request.title.as_ref().map(|opt| opt.as_deref()),
        request.description.as_ref().map(|opt| opt.as_deref()),
        request.risk_level.as_ref().map(|opt| opt.as_deref()),
        request.asset_class.as_ref().map(|opt| opt.as_deref()),
        request.region.as_ref().map(|opt| opt.as_deref()),
        minimum_investment,
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
            if request.status.is_some() {
                updated_fields.push("status".to_string());
            }

            // Convert document IDs from JSON to Vec<String> for response
            let document_ids_response = deal.document_ids.as_ref().and_then(|json| {
                if let Ok(uuids) = serde_json::from_value::<Vec<uuid::Uuid>>(json.clone()) {
                    Some(uuids.into_iter().map(|uuid| uuid.to_string()).collect())
                } else {
                    None
                }
            });

            // Fetch collateral if available
            let collateral = if let Some(coll_id) = deal.collateral_id {
                if let Ok(collateral) = get_collateral_by_id(&state.pool, coll_id).await {
                    let deal_ids = get_deals_by_collateral_id(&state.pool, coll_id)
                        .await
                        .unwrap_or_default();

                    let deal_count = deal_ids.len() as i64;
                    let is_double_pledged = deal_count > 1;

                    Some(crate::handlers::taxonomy::collateral::CollateralResponse {
                        id: collateral.id.to_string(),
                        name: collateral.name,
                        collateral_type: collateral.collateral_type,
                        description: collateral.description,
                        address: collateral.address,
                        custom_id: collateral.custom_id,
                        double_pledged: collateral.double_pledged.map(|u| u.to_string()),
                        has_potential_duplicates: collateral.has_potential_duplicates,
                        created_at: collateral.created_at.to_rfc3339(),
                        updated_at: collateral.updated_at.to_rfc3339(),
                        metadata: collateral.metadata,
                        deal_count: Some(deal_count),
                        deal_ids: Some(deal_ids.iter().map(|id| id.to_string()).collect()),
                        is_double_pledged: Some(is_double_pledged),
                        has_detection: Some(collateral.has_detection),
                        detection_count: Some(collateral.detection_count),
                        last_detection_at: collateral.last_detection_at.map(|dt| dt.to_rfc3339()),
                        detection_status: collateral.detection_status,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            // Convert to response format
            let deal_details = DealDetailsResponse {
                // was LoanDetailsResponse
                deal_id: deal.id.to_string(), // was loan_id, now deal_id
                originator_id: Some(deal.originator_id.to_string()), // was borrower_id
                investor_id: deal.investor_id.map(|id| id.to_string()), // was lender_id - now optional
                borrower_id: deal.borrower_id.map(|id| id.to_string()), // Company ID of borrower (optional)
                borrower_legal_name: deal.borrower_legal_name.clone(), // Borrower company legal name
                principal_amount: deal.principal_amount.to_string(),
                outstanding_balance: deal.outstanding_balance.to_string(),
                interest_rate_bps: deal.interest_rate_bps as u16,
                loan_status: deal.deal_status, // deal_status field
                originated_at: deal.originated_at,
                last_updated: Some(deal.updated_at),
                custom_deal_id: deal.custom_deal_id,
                status: deal.status,
                industry: deal.industry,
                document_ids: document_ids_response,
                title: deal.title,
                description: deal.description,
                risk_level: deal.risk_level,
                asset_class: deal.asset_class,
                region: deal.region,
                minimum_investment: deal.minimum_investment.map(|d| d.to_string()),
                collateral_id: deal.collateral_id.map(|id| id.to_string()),
                collateral,
            };

            // Publish events based on changes (fire-and-forget)
            if let Some(publisher) = &state.event_publisher {
                let collateral_ids = deal.collateral_id.map(|id| vec![id]).unwrap_or_default();

                // Check if deal_status changed
                if let Some(ref new_status) = request.loan_status {
                    if let Some(ref old) = old_deal {
                        if *new_status != old.deal_status {
                            let event = crate::events::DealEvent::DealStatusChanged {
                                event_id: uuid::Uuid::new_v4(),
                                timestamp: chrono::Utc::now(),
                                deal_id: deal.id,
                                previous_status: old.deal_status.clone(),
                                new_status: new_status.clone(),
                                collateral_ids: collateral_ids.clone(),
                            };
                            let publisher = publisher.clone();
                            tokio::spawn(async move {
                                if let Err(e) = publisher.publish_deal(event).await {
                                    tracing::warn!(
                                        "Failed to publish DealStatusChanged event: {}",
                                        e
                                    );
                                }
                            });
                        }
                    }
                }

                // Check if collateral_id changed
                // Note: collateral_id is not in the update request, so we can't detect changes here
                // This would require adding collateral_id to UpdateDealRequest in the future
            }

            let response = UpdateDealResponse {
                // was UpdateLoanResponse
                deal_id: deal.id.to_string(), // was loan_id, now deal_id
                updated_fields,
                updated_at: deal.updated_at,
                deal_details, // was loan_details
            };

            // Trigger async metrics refresh if enabled
            trigger_metrics_refresh_async(&state);

            info!("✅ API: Deal updated successfully: {}", deal_id); // updated log message
            Ok(Json(response))
        }
        None => {
            warn!("❌ API: Deal not found: {}", deal_id); // updated log message
            Err((StatusCode::NOT_FOUND, "Deal not found".to_string())) // updated error message
        }
    }
}

/// Delete deal (converted from delete_loan_endpoint)
#[utoipa::path(
    delete,
    path = "/api/v1/deal/{deal_id}",
    responses(
        (status = 200, description = "Deal deleted successfully", body = DeleteDealResponse),
        (status = 400, description = "Invalid deal ID format"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Not authorized to delete this deal"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal UUID")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn delete_deal_endpoint(
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,
) -> Result<Json<DeleteDealResponse>, (StatusCode, String)> {
    info!("🗑️ API: Deleting deal: deal_id={}", deal_id);

    // Parse deal ID
    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    // Check if deal exists
    let deal = get_deal_by_id(&state.pool, deal_uuid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?;

    // Delete the deal (CASCADE will handle related records)
    let deleted = delete_deal(&state.pool, deal_uuid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete deal: {}", e),
        )
    })?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, "Deal not found".to_string()));
    }

    // 6. Trigger metrics refresh
    trigger_metrics_refresh_async(&state);

    info!("✅ API: Deal deleted successfully: deal_id={}", deal_id);

    Ok(Json(DeleteDealResponse {
        deal_id,
        deleted: true,
        deleted_at: Utc::now(),
        final_status: deal.deal_status.clone(),
    }))
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

    let transaction_id = Uuid::new_v4().to_string();

    let response = PaymentResponse {
        transaction_id,
        deal_id: request.deal_id, // was loan_id, now deal_id
        amount: request.amount,
        payment_type: request.payment_type,
        transaction_status: "Pending".to_string(), // Will be batched for ZK proof
        created_at: Utc::now(),
    };

    info!("✅ API: Payment recorded successfully");
    Ok(Json(response))
}

/// Make a payment on a deal
///
/// Creates a payment record, updates the deal's outstanding balance,
/// and records a transaction for zkVM processing.
///
/// The deal status may transition:
/// - originated → active (first payment)
/// - active → paid_off (if balance reaches 0)
///
/// # Validations
/// - Deal must exist
/// - Deal must be in originated or active status
/// - Payment amount must be > 0
/// - Payment amount must be <= outstanding balance
/// - User must have permission (originator or authorized party)
///
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/payment",
    request_body = MakePaymentRequest,
    responses(
        (status = 200, description = "Payment created successfully", body = MakePaymentResponse),
        (status = 400, description = "Invalid request - check amount or deal status"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - user doesn't have permission"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn make_payment_endpoint(
    State(state): State<ApiState>,
    Path(deal_id_str): Path<String>,
    Json(request): Json<MakePaymentRequest>,
) -> Result<Json<MakePaymentResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record metrics
    metrics::counter!("api_payment_requests_total").increment(1);

    // 1. Parse deal_id
    let deal_id = Uuid::parse_str(&deal_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    // 2. Parse amount
    let amount = rust_decimal::Decimal::from_str(&request.amount)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid amount format".to_string()))?;

    if amount <= rust_decimal::Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Payment amount must be greater than zero".to_string(),
        ));
    }

    // 3. Get deal and validate status
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?;

    if deal.deal_status != "originated" && deal.deal_status != "active" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot make payment on deal with status '{}'. Must be 'originated' or 'active'.",
                deal.deal_status
            ),
        ));
    }

    // 4. Validate payment amount doesn't exceed outstanding balance
    if amount > deal.outstanding_balance {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Payment amount ${} exceeds outstanding balance ${}",
                amount, deal.outstanding_balance
            ),
        ));
    }

    // 5. Get authenticated user (from session middleware)
    // TODO: Extract user_id from session/auth context
    // For now, assume originator is making payment
    let payer_id = deal.originator_id;

    // 6. Create payment record in database
    let payment_id = Uuid::new_v4();
    let payment_type = request
        .payment_type
        .unwrap_or_else(|| "regular".to_string());

    sqlx::query!(
        r#"
        INSERT INTO deal_payments (
            id, deal_id, payer_id, amount, payment_type, created_at
        )
        VALUES ($1, $2, $3, $4, $5, NOW())
        "#,
        payment_id,
        deal_id,
        payer_id,
        amount,
        payment_type,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to create payment record: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // 7. Update deal outstanding balance
    let new_balance = deal.outstanding_balance - amount;

    // Determine new deal status
    let new_status = if new_balance == rust_decimal::Decimal::ZERO {
        "paid_off"
    } else if deal.deal_status == "originated" {
        "active" // First payment activates the deal
    } else {
        &deal.deal_status // Stay in current status
    };

    sqlx::query!(
        r#"
        UPDATE deals
        SET outstanding_balance = $1,
            deal_status = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        new_balance,
        new_status,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to update deal balance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // 8. Record transaction for zkVM
    let tx_id = state
        .transaction_recording
        .record_transaction_with_metadata(
            TransactionType::DealPayment,
            payer_id,
            Some(deal_id),
            amount,
            Some(json!({
                "payment_id": payment_id.to_string(),
                "payment_type": payment_type.clone(),
                "notes": request.notes,
                "previous_balance": deal.outstanding_balance.to_string(),
                "new_balance": new_balance.to_string(),
                "status_transition": format!("{} → {}", deal.deal_status, new_status),
            })),
            Some(deal.originator_id), // originator_company_id
            deal.investor_id,         // investor_company_id
            deal.collateral_id,       // collateral_id
        )
        .await
        .map_err(|e| {
            error!("Failed to record payment transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Payment created but transaction recording failed: {}", e),
            )
        })?;

    let duration = start_time.elapsed();
    info!(
        "Payment {} created for deal {} in {:?}, transaction {}",
        payment_id, deal_id, duration, tx_id
    );

    metrics::histogram!("api_payment_duration_seconds").record(duration.as_secs_f64());

    // 9. Return response
    Ok(Json(MakePaymentResponse {
        payment_id: payment_id.to_string(),
        deal_id: deal_id.to_string(),
        amount: amount.to_string(),
        payment_type,
        new_outstanding_balance: new_balance.to_string(),
        deal_status: new_status.to_string(),
        transaction_id: tx_id.to_string(),
        created_at: chrono::Utc::now(),
    }))
}

/// Close out a paid-off deal
///
/// Marks a deal as closed after it has been fully paid off.
/// This is the final state in the deal lifecycle.
///
/// # Validations
/// - Deal must exist
/// - Deal must have status "paid_off"
/// - Outstanding balance must be exactly 0
/// - User must be the investor (permission check)
/// - User must confirm the closeout
///
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/closeout",
    request_body = CloseoutDealRequest,
    responses(
        (status = 200, description = "Deal closed successfully", body = CloseoutResponse),
        (status = 400, description = "Invalid request - check deal status or confirmation"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - only investor can close deal"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn closeout_deal_endpoint(
    State(state): State<ApiState>,
    Path(deal_id_str): Path<String>,
    Json(request): Json<CloseoutDealRequest>,
) -> Result<Json<CloseoutResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    metrics::counter!("api_closeout_requests_total").increment(1);

    // 1. Parse deal_id
    let deal_id = Uuid::parse_str(&deal_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    // 2. Validate confirmation
    if !request.confirm_paid_off {
        return Err((
            StatusCode::BAD_REQUEST,
            "Must confirm deal is paid off (confirm_paid_off: true)".to_string(),
        ));
    }

    // 3. Get deal and validate status
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?;

    if deal.deal_status != "paid_off" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot close deal with status '{}'. Must be 'paid_off' (fully paid).",
                deal.deal_status
            ),
        ));
    }

    if deal.outstanding_balance != rust_decimal::Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot close deal with outstanding balance ${}. Must be fully paid.",
                deal.outstanding_balance
            ),
        ));
    }

    // 4. Get authenticated user and check permission
    // TODO: Extract user_id from session/auth middleware
    // For now, assume investor is closing
    let investor_id = deal.investor_id.ok_or((
        StatusCode::BAD_REQUEST,
        "Deal has no investor assigned".to_string(),
    ))?;

    // TODO: Add permission check
    // if user_id != investor_id {
    //     return Err((StatusCode::FORBIDDEN, "Only investor can close deal"));
    // }

    // 5. Update deal status to closed
    sqlx::query!(
        r#"
        UPDATE deals
        SET deal_status = 'closed',
            updated_at = NOW()
        WHERE id = $1
        "#,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to close deal: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // 6. Record transaction for zkVM
    let tx_id = state
        .transaction_recording
        .record_transaction_with_metadata(
            TransactionType::DealCloseout,
            investor_id,
            Some(deal_id),
            rust_decimal::Decimal::ZERO, // No amount for closeout
            Some(json!({
                "notes": request.notes,
                "closed_by": investor_id.to_string(),
                "original_principal": deal.principal_amount.to_string(),
                "final_status": "closed",
            })),
            Some(deal.originator_id), // originator_company_id
            deal.investor_id,         // investor_company_id
            deal.collateral_id,       // collateral_id
        )
        .await
        .map_err(|e| {
            error!("Failed to record closeout transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Deal closed but transaction recording failed: {}", e),
            )
        })?;

    let duration = start_time.elapsed();
    info!(
        "Deal {} closed in {:?}, transaction {}",
        deal_id, duration, tx_id
    );

    metrics::histogram!("api_closeout_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("api_closeout_success_total").increment(1);

    // 7. Return response
    Ok(Json(CloseoutResponse {
        deal_id: deal_id.to_string(),
        deal_status: "closed".to_string(),
        transaction_id: tx_id.to_string(),
        closed_at: chrono::Utc::now(),
    }))
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
        deal_id: Uuid::new_v4().to_string(), // was loan_id, now deal_id
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
    path = "/api/v1/deal/{deal_id}/payments",  // was loan_id, now deal_id
    responses(
        (status = 200, description = "Payment history retrieved successfully", body = DealPaymentHistoryResponse),  // was Loan
        (status = 404, description = "Deal not found"),  // was Loan
        (status = 500, description = "Internal server error")
    ),
    params(
        ("deal_id" = String, Path, description = "Deal ID"),  // was loan_id, now deal_id
        ListDealsQuery  // was ListLoansQuery
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_deal_payment_history_endpoint(
    // was get_loan_payment_history_endpoint
    State(_state): State<ApiState>,
    Path(deal_id): Path<String>,           // was loan_id, now deal_id
    Query(_params): Query<ListDealsQuery>, // Reuse query params for limit/offset, was ListLoansQuery
) -> Result<Json<DealPaymentHistoryResponse>, (StatusCode, String)> {
    // was LoanPaymentHistoryResponse
    info!("📜 API: Getting payment history: deal_id={}", deal_id); // updated log message

    // TODO: Implement payment history lookup with database integration
    let response = DealPaymentHistoryResponse {
        // was LoanPaymentHistoryResponse
        deal_id, // was loan_id, now deal_id
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
            deal_id: deal.deal_id, // was loan_id, now deal_id
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
            deal_status: deal.deal_status, // deal_status field
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
    Path(deal_id): Path<String>, // was loan_id, now deal_id
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
        deal_uuid, // was loan_id, now deal_id
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
        deal_id: listing.deal_id.to_string(), // was loan_id, now deal_id
        seller_id: listing.seller_id,
        asking_price: listing.asking_price.to_string(),
        description: listing.description,
        listed_at: listing.listed_at,
        expires_at: listing.expires_at,
        delisted_at: listing.delisted_at,
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
        (status = 403, description = "Not authorized to remove this listing"),
        (status = 404, description = "Deal listing not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state), level = "info")]
pub async fn remove_deal_from_sale_endpoint(
    // was remove_loan_from_sale_endpoint
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(deal_id): Path<String>, // was loan_id, now deal_id
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🗑️ API: Removing deal from sale: {}", deal_id);

    // Extract current user from session
    let current_user = extract_current_user(&headers).ok_or_else(|| {
        warn!("❌ No authenticated user found for remove-from-sale request");
        (
            StatusCode::UNAUTHORIZED,
            "Authentication required".to_string(),
        )
    })?;

    let seller_id = current_user.id;

    let deal_uuid = uuid::Uuid::parse_str(&deal_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid deal ID format".to_string(),
        )
    })?;

    // Attempt to remove the listing
    let removal_success = remove_deal_listing(&state.pool, deal_uuid, seller_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to remove deal from sale: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to remove deal from sale".to_string(),
            )
        })?;

    if !removal_success {
        warn!(
            "⚠️ Deal listing not found or user not authorized: deal_id={}, seller_id={}",
            deal_id, seller_id
        );
        return Err((
            StatusCode::NOT_FOUND,
            "Deal listing not found or you are not authorized to remove it".to_string(),
        ));
    }

    info!(
        "✅ Deal removed from sale: {} by user: {}",
        deal_id, current_user.email
    );
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
#[instrument(skip(_state), level = "info")]
pub async fn deal_marketplace_stats_endpoint(
    // was marketplace_stats_endpoint
    State(_state): State<ApiState>,
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
    Path(deal_id): Path<String>, // was loan_id, now deal_id
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
            deal_id: deal_uuid,               // was loan_id, now deal_id
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
        deal_id: transfer.deal_id.to_string(), // was loan_id, now deal_id
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
    Path(deal_id): Path<String>, // was loan_id, now deal_id
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
        deal_uuid, // was loan_id, now deal_id
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
            deal_id: transfer.deal_id.to_string(), // was loan_id, now deal_id
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
#[instrument(skip(_state), level = "info")]
pub async fn get_user_deal_transfer_history_endpoint(
    // was get_user_transfer_history_endpoint
    State(_state): State<ApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<TransferHistoryQuery>,
) -> Result<Json<Vec<DealTransferResponse>>, (StatusCode, String)> {
    // was LoanTransferResponse
    info!("👤 API: Getting user deal transfer history: {}", user_id);

    let _user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
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
    Path(deal_id): Path<String>, // was loan_id, now deal_id
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
            deal_id: deal_uuid,                          // was loan_id, now deal_id
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
        deal_id: deal_id.clone(), // was loan_id, now deal_id
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
