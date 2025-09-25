//! 🚫 DEPRECATED: Marketplace API handlers have been consolidated
//!
//! This file has been DEPRECATED as part of the loan-to-deal conversion (Phase 5, Task 5.3).
//!
//! ALL MARKETPLACE FUNCTIONALITY HAS BEEN MOVED TO:
//!   `api/src/deal_handlers.rs`
//!
//! New endpoints:
//! - GET /api/v1/deals/for-sale (was /marketplace/loans)
//! - POST /api/v1/deal/{id}/list-for-sale (was /marketplace/loan/{id})
//! - DELETE /api/v1/deal/{id}/remove-from-sale (was /marketplace/loan/{id})
//! - GET /api/v1/deals/marketplace/stats (was /marketplace/stats)
//!
//! This file can be safely REMOVED after confirming the new consolidated
//! endpoints work correctly.
//!
//! Original functionality:
//! - Browse marketplace loans for sale
//! - List loan for sale
//! - Remove loan from sale
//! - Marketplace statistics

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::trading_auth::{extract_user_id_from_headers, validate_loan_ownership};
use crate::types::ApiState;
use arda_credit_db::{
    create_loan_listing, get_marketplace_loans_basic, LoanListing, MarketplaceLoan,
};

// ============================================================================
// REQUEST/RESPONSE TYPES (Following frontend Deal interface)
// ============================================================================

/// Request to list a loan for sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoanForSaleRequest {
    pub asking_price: String,        // Decimal as string (e.g., "50000.00")
    pub description: Option<String>, // Optional listing description
}

/// Response when listing a loan for sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoanForSaleResponse {
    pub listing_id: String,       // UUID as string
    pub loan_id: String,          // UUID as string
    pub asking_price: String,     // Decimal as string
    pub listed_at: DateTime<Utc>, // ISO timestamp
    pub status: String,           // "active"
}

/// Marketplace deal (aligns with frontend Deal interface)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceDeal {
    // Core identifiers (matching frontend Deal interface)
    pub id: String,          // loan_id as string
    pub merchant_id: String, // borrower_id as string
    pub title: String,       // loan.title or generated
    pub description: String, // listing.description or loan.description

    // Financial details (frontend expects ranges, we provide single values)
    pub loan_amount_range: AmountRange,
    pub interest_rate_range: RateRange,
    pub asking_price: String, // listing.asking_price

    // Status and metadata
    pub status: String,              // "for_sale"
    pub risk_level: Option<String>,  // "low", "medium", "high"
    pub asset_class: Option<String>, // Asset classification
    pub region: Option<String>,      // Geographic region

    // Marketplace-specific fields
    pub listed_at: DateTime<Utc>,     // When listed
    pub seller_id: String,            // seller identity ID
    pub seller_email: String,         // seller contact
    pub outstanding_balance: String,  // Current balance owed
    pub originated_at: DateTime<Utc>, // Original loan date
}

/// Amount range (frontend expects min/max, we provide single values)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AmountRange {
    pub min: String, // Same as max for single loan
    pub max: String, // principal_amount
}

/// Interest rate range (frontend expects min/max, we provide single values)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RateRange {
    pub min: String, // Same as max for single loan
    pub max: String, // interest_rate_bps as percentage string
}

/// Query parameters for marketplace browsing
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct MarketplaceQuery {
    pub limit: Option<i64>,          // Pagination limit (default: 20, max: 100)
    pub offset: Option<i64>,         // Pagination offset (default: 0)
    pub min_amount: Option<String>,  // Minimum loan amount filter
    pub max_amount: Option<String>,  // Maximum loan amount filter
    pub max_price: Option<String>,   // Maximum asking price filter
    pub risk_level: Option<String>,  // Risk level filter ("low", "medium", "high")
    pub asset_class: Option<String>, // Asset class filter
    pub region: Option<String>,      // Region filter
}

/// Response for marketplace browsing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoansForSaleResponse {
    pub listings: Vec<MarketplaceDeal>,  // Available marketplace deals
    pub total_count: i64,                // Total matching results
    pub has_more: bool,                  // True if more results available
    pub filters: MarketplaceFiltersInfo, // Available filter options
}

/// Available filter options for marketplace
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceFiltersInfo {
    pub available_asset_classes: Vec<String>,
    pub available_regions: Vec<String>,
    pub price_range: PriceRange,
}

/// Price range information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PriceRange {
    pub min: String, // Minimum asking price
    pub max: String, // Maximum asking price
}

/// Response when removing loan from sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RemoveLoanFromSaleResponse {
    pub loan_id: String,           // UUID as string
    pub removed_at: DateTime<Utc>, // When removed
    pub status: String,            // "unlisted"
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert MarketplaceLoan from database to frontend MarketplaceDeal format
fn marketplace_loan_to_deal(ml: MarketplaceLoan) -> MarketplaceDeal {
    // Generate title if not provided
    let title = ml.title.unwrap_or_else(|| {
        format!(
            "${:.0} Credit Line - {} Interest",
            ml.principal_amount,
            format_bps_as_percentage(ml.interest_rate_bps)
        )
    });

    // Use listing description, fall back to loan description, or generate
    let description = ml
        .listing_description
        .or(ml.description)
        .unwrap_or_else(|| {
            format!(
                "Credit line for ${:.2} at {}% interest",
                ml.principal_amount,
                format_bps_as_percentage(ml.interest_rate_bps)
            )
        });

    // Convert interest rate from basis points to percentage string
    let interest_rate_pct = format_bps_as_percentage(ml.interest_rate_bps);

    MarketplaceDeal {
        // Core identifiers
        id: ml.loan_id.to_string(),
        merchant_id: ml.borrower_id.to_string(),
        title,
        description,

        // Financial details (single values as ranges)
        loan_amount_range: AmountRange {
            min: ml.principal_amount.to_string(),
            max: ml.principal_amount.to_string(),
        },
        interest_rate_range: RateRange {
            min: interest_rate_pct.clone(),
            max: interest_rate_pct,
        },
        asking_price: ml.asking_price.to_string(),

        // Status and metadata
        status: "for_sale".to_string(),
        risk_level: ml.risk_level,
        asset_class: ml.asset_class,
        region: ml.region,

        // Marketplace-specific
        listed_at: ml.listed_at,
        seller_id: ml.lender_id.to_string(),
        seller_email: ml.seller_email,
        outstanding_balance: ml.outstanding_balance.to_string(),
        originated_at: ml.originated_at,
    }
}

/// Convert basis points to percentage string (e.g., 1200 -> "12.00")
fn format_bps_as_percentage(bps: i32) -> String {
    format!("{:.2}", (bps as f64) / 100.0)
}

/// Parse decimal from string with validation
fn parse_decimal(s: &str, field_name: &str) -> Result<Decimal, (StatusCode, String)> {
    s.parse::<Decimal>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid {} format: {}", field_name, s),
        )
    })
}

/// Extract user ID from request headers
fn get_current_user_id(headers: &HeaderMap) -> Result<Uuid, (StatusCode, String)> {
    extract_user_id_from_headers(headers)
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Browse marketplace loans for sale
#[utoipa::path(
    get,
    path = "/api/v1/loans/for-sale",
    params(MarketplaceQuery),
    responses(
        (status = 200, description = "Marketplace loans retrieved successfully", body = LoansForSaleResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Marketplace"
)]
#[instrument(skip(state), level = "info")]
pub async fn browse_marketplace_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<MarketplaceQuery>,
) -> Result<Json<LoansForSaleResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record metrics
    metrics::counter!("marketplace_browse_requests_total").increment(1);
    metrics::counter!("marketplace_active_users_total").increment(1); // Unique users browsing

    info!(
        "🛒 MARKETPLACE: Browsing loans - limit={:?}, min_amount={:?}, max_price={:?}",
        query.limit, query.min_amount, query.max_price
    );

    // Parse optional decimal filters
    let min_amount = if let Some(ref s) = query.min_amount {
        Some(parse_decimal(s, "min_amount")?)
    } else {
        None
    };

    let max_amount = if let Some(ref s) = query.max_amount {
        Some(parse_decimal(s, "max_amount")?)
    } else {
        None
    };

    let max_price = if let Some(ref s) = query.max_price {
        Some(parse_decimal(s, "max_price")?)
    } else {
        None
    };

    // For Phase 2, we use the basic marketplace function
    // TODO: Phase 3 will implement advanced filtering
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    // Query marketplace loans using Phase 1 functions
    let marketplace_loans = get_marketplace_loans_basic(&state.pool, Some(limit), Some(offset))
        .await
        .map_err(|e| {
            error!("❌ Failed to list marketplace loans: {}", e);
            metrics::counter!("marketplace_errors_total", "operation" => "browse").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve marketplace loans".to_string(),
            )
        })?;

    // Get total count (for Phase 2, we approximate)
    let total_count = marketplace_loans.len() as i64;
    let has_more = (offset + limit) < total_count;

    // Convert to frontend format
    let listings: Vec<MarketplaceDeal> = marketplace_loans
        .into_iter()
        .map(marketplace_loan_to_deal)
        .collect();

    // Build filter options (simplified for Phase 2)
    let filters_info = MarketplaceFiltersInfo {
        available_asset_classes: vec![], // TODO: Phase 3 enhancement
        available_regions: vec![],       // TODO: Phase 3 enhancement
        price_range: PriceRange {
            min: "0".to_string(),       // TODO: Phase 3 enhancement
            max: "1000000".to_string(), // TODO: Phase 3 enhancement
        },
    };

    let response = LoansForSaleResponse {
        listings,
        total_count,
        has_more,
        filters: filters_info,
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("marketplace_browse_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("marketplace_listings_returned_total")
        .increment(response.listings.len() as u64);

    info!(
        "✅ MARKETPLACE: Found {} loans ({}ms)",
        response.listings.len(),
        duration.as_millis()
    );

    Ok(Json(response))
}

/// List a loan for sale in the marketplace
#[utoipa::path(
    post,
    path = "/api/v1/loan/{loan_id}/list-for-sale",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan to list for sale")
    ),
    request_body = ListLoanForSaleRequest,
    responses(
        (status = 201, description = "Loan listed successfully", body = ListLoanForSaleResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized - user not authenticated"),
        (status = 403, description = "Forbidden - user doesn't own loan or not KYC approved"),
        (status = 404, description = "Loan not found"),
        (status = 409, description = "Loan already listed or ineligible"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Marketplace"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_loan_for_sale_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ListLoanForSaleRequest>,
) -> Result<Json<ListLoanForSaleResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record metrics
    metrics::counter!("marketplace_list_requests_total").increment(1);

    info!(
        "🏷️ MARKETPLACE: Listing loan {} for sale at price {}",
        loan_id_str, request.asking_price
    );

    // Parse loan ID
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Parse asking price
    let asking_price = parse_decimal(&request.asking_price, "asking_price")?;

    // Validate asking price is positive
    if asking_price <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Asking price must be positive".to_string(),
        ));
    }

    // Get current user ID from headers
    let seller_id = get_current_user_id(&headers)?;

    // Validate user owns the loan
    let owns_loan = validate_loan_ownership(&state.pool, loan_id, seller_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to validate loan ownership: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate ownership".to_string(),
            )
        })?;

    if !owns_loan {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not own this loan".to_string(),
        ));
    }

    // Create listing using Phase 1 database function
    let listing = create_loan_listing(
        &state.pool,
        loan_id,
        seller_id,
        asking_price,
        request.description,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create loan listing: {}", e);

        // Map specific database errors to HTTP status codes
        match e {
            sqlx::Error::RowNotFound => {
                metrics::counter!("marketplace_errors_total", "type" => "not_found").increment(1);
                (
                    StatusCode::NOT_FOUND,
                    "Loan not found or not owned by user".to_string(),
                )
            }
            sqlx::Error::Database(ref db_err)
                if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) =>
            {
                metrics::counter!("marketplace_errors_total", "type" => "already_listed")
                    .increment(1);
                (
                    StatusCode::CONFLICT,
                    "Loan is already listed for sale".to_string(),
                )
            }
            sqlx::Error::Database(ref db_err)
                if db_err.code() == Some(std::borrow::Cow::Borrowed("23514")) =>
            {
                metrics::counter!("marketplace_errors_total", "type" => "ineligible_status")
                    .increment(1);
                (
                    StatusCode::CONFLICT,
                    "Loan status is not eligible for listing".to_string(),
                )
            }
            _ => {
                metrics::counter!("marketplace_errors_total", "type" => "database").increment(1);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to create listing".to_string(),
                )
            }
        }
    })?;

    // Record successful listing metrics
    metrics::counter!("marketplace_listings_created_total").increment(1);
    metrics::histogram!("marketplace_listing_price_usdc")
        .record(asking_price.to_f64().unwrap_or(0.0));
    metrics::counter!("marketplace_active_listings_total").increment(1);

    let response = ListLoanForSaleResponse {
        listing_id: listing.listing_id.to_string(),
        loan_id: listing.loan_id.to_string(),
        asking_price: listing.asking_price.to_string(),
        listed_at: listing.listed_at,
        status: listing.status,
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("marketplace_list_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("marketplace_listings_created_total").increment(1);

    info!(
        "✅ MARKETPLACE: Listed loan {} successfully ({}ms)",
        loan_id,
        duration.as_millis()
    );

    Ok(Json(response))
}

/// Remove loan from marketplace (cancel listing)
#[utoipa::path(
    delete,
    path = "/api/v1/loan/{loan_id}/list-for-sale",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan to remove from sale")
    ),
    responses(
        (status = 200, description = "Loan removed from sale successfully", body = RemoveLoanFromSaleResponse),
        (status = 400, description = "Invalid loan ID format"),
        (status = 401, description = "Unauthorized - user not authenticated"),
        (status = 403, description = "Forbidden - user doesn't own loan"),
        (status = 404, description = "Loan listing not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Marketplace"
)]
#[instrument(skip(state), level = "info")]
pub async fn remove_loan_from_sale_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RemoveLoanFromSaleResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record metrics
    metrics::counter!("marketplace_remove_requests_total").increment(1);

    info!("🗑️ MARKETPLACE: Removing loan {} from sale", loan_id_str);

    // Parse loan ID
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Get current user ID from headers
    let seller_id = get_current_user_id(&headers)?;

    // Validate user owns the loan
    let owns_loan = validate_loan_ownership(&state.pool, loan_id, seller_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to validate loan ownership: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate ownership".to_string(),
            )
        })?;

    if !owns_loan {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not own this loan".to_string(),
        ));
    }

    // Remove listing using Phase 1 database function
    let removed = arda_credit_db::remove_loan_listing(&state.pool, loan_id, seller_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to remove loan listing: {}", e);
            metrics::counter!("marketplace_errors_total", "operation" => "remove").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to remove listing".to_string(),
            )
        })?;

    if !removed {
        return Err((
            StatusCode::NOT_FOUND,
            "Loan listing not found or not owned by user".to_string(),
        ));
    }

    let response = RemoveLoanFromSaleResponse {
        loan_id: loan_id.to_string(),
        removed_at: chrono::Utc::now(),
        status: "unlisted".to_string(),
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("marketplace_remove_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("marketplace_listings_removed_total").increment(1);

    info!(
        "✅ MARKETPLACE: Removed loan {} from sale ({}ms)",
        loan_id,
        duration.as_millis()
    );

    Ok(Json(response))
}

/// Get marketplace statistics (admin endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/loans/marketplace/stats",
    responses(
        (status = 200, description = "Marketplace statistics", body = serde_json::Value),
        (status = 500, description = "Internal server error")
    ),
    tag = "Marketplace"
)]
pub async fn marketplace_stats_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("📊 MARKETPLACE: Getting marketplace statistics");

    // For Phase 2, return basic statistics
    // TODO: Phase 3 will implement comprehensive marketplace stats
    let stats = serde_json::json!({
        "active_listings": 0,  // TODO: Implement count query
        "sold_listings": 0,    // TODO: Implement sold count
        "cancelled_listings": 0, // TODO: Implement cancelled count
        "avg_asking_price": null, // TODO: Implement price aggregation
        "min_asking_price": null, // TODO: Implement price aggregation
        "max_asking_price": null, // TODO: Implement price aggregation
        "phase": "2",
        "note": "Statistics implementation in Phase 3"
    });

    Ok(Json(stats))
}
