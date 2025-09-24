# 🛒 **Worker A: Marketplace API & CLI (Phase 2 Parallel)**

## 🎯 **Your Mission - Phase 2**

You are implementing the **Marketplace API and CLI** for loan trading. The database foundation is already complete from Phase 1. Your work enables users to interact with the marketplace through REST APIs and command-line tools.

**Timeline**: 2.5 days parallel development with Worker B  
**Prerequisites**: Database foundation completed (Phase 1)  
**Your Domain**: Marketplace API endpoints, CLI commands, infrastructure integration

---

## ✅ **Phase 1 Dependencies (COMPLETED ✅)**

From the Database Foundation phase, you now have:
- ✅ Database migration `060_add_loan_trading_foundation.sql` applied
- ✅ Core database functions in `db/src/marketplace.rs` implemented
- ✅ Loan status enum extended with `for_sale` and `transfer_pending`
- ✅ Schema validation complete and tested

**⚠️ IMPORTANT - Schema Reference Update:**
- **Use `loans.id`** (not `loans.loan_id` as originally planned)
- The implemented database functions already use the correct schema
- All your API implementations should follow the existing database functions

**Available Functions** (from Phase 1):
- `create_loan_listing()` - List loan for sale
- `get_marketplace_loans_basic()` - Browse marketplace with filters  
- `get_loan_listing_by_loan_id()` - Get specific listing
- `remove_loan_listing()` - Cancel sale
- `update_listing_status()` - Update listing status
- `validate_loan_for_transfer()` - Basic transfer validation

---

## 📋 **Phase 2 Task Checklist**

### **Day 1: API Implementation**
- [ ] Build marketplace API handlers
- [ ] Add marketplace routes to infrastructure
- [ ] Test API endpoints with database functions

### **Day 2: CLI Implementation**
- [ ] Implement marketplace CLI commands
- [ ] Test CLI workflow with APIs
- [ ] Coordinate infrastructure files with Worker B

### **Day 2.5: Integration Prep**
- [ ] Finalize shared infrastructure updates
- [ ] Test marketplace endpoints independently
- [ ] Prepare for Phase 3 integration

---

## 🌐 **API Implementation (Priority 1)**

### **File: `api/src/marketplace_handlers.rs`** (New File)

Implement marketplace API endpoints using the database functions from Phase 1:

```rust
//! Marketplace API handlers for loan trading
//!
//! This module contains all marketplace-related API endpoints including:
//! - Browse marketplace loans for sale
//! - List loan for sale
//! - Remove loan from sale
//! - Marketplace statistics
//!
//! All routes use OpenAPI documentation and consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use utoipa::{IntoParams, ToSchema};

use crate::types::ApiState;
use arda_credit_db::{
    create_loan_listing, list_marketplace_loans, count_marketplace_loans, 
    get_loan_listing_by_loan_id, remove_loan_listing, get_marketplace_stats,
    MarketplaceFilters, MarketplaceLoan
};

// ============================================================================
// REQUEST/RESPONSE TYPES (Following frontend Deal interface)
// ============================================================================

/// Request to list a loan for sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoanForSaleRequest {
    pub asking_price: String,           // Decimal as string (e.g., "50000.00")
    pub description: Option<String>,    // Optional listing description
}

/// Response when listing a loan for sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoanForSaleResponse {
    pub listing_id: String,             // UUID as string
    pub loan_id: String,                // UUID as string  
    pub asking_price: String,           // Decimal as string
    pub listed_at: DateTime<Utc>,       // ISO timestamp
    pub status: String,                 // "active"
}

/// Marketplace deal (aligns with frontend Deal interface)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceDeal {
    // Core identifiers (matching frontend Deal interface)
    pub id: String,                     // loan_id as string
    pub merchant_id: String,            // borrower_id as string
    pub title: String,                  // loan.title or generated
    pub description: String,            // listing.description or loan.description
    
    // Financial details (frontend expects ranges, we provide single values)
    pub loan_amount_range: AmountRange,
    pub interest_rate_range: RateRange,
    pub asking_price: String,           // listing.asking_price
    
    // Status and metadata
    pub status: String,                 // "for_sale"
    pub risk_level: Option<String>,     // "low", "medium", "high"
    pub asset_class: Option<String>,    // Asset classification
    pub region: Option<String>,         // Geographic region
    
    // Marketplace-specific fields
    pub listed_at: DateTime<Utc>,       // When listed
    pub seller_id: String,              // seller identity ID
    pub seller_email: String,           // seller contact
    pub outstanding_balance: String,    // Current balance owed
    pub originated_at: DateTime<Utc>,   // Original loan date
}

/// Amount range (frontend expects min/max, we provide single values)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AmountRange {
    pub min: String,                    // Same as max for single loan
    pub max: String,                    // principal_amount
}

/// Interest rate range (frontend expects min/max, we provide single values)  
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RateRange {
    pub min: String,                    // Same as max for single loan
    pub max: String,                    // interest_rate_bps as percentage string
}

/// Query parameters for marketplace browsing
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct MarketplaceQuery {
    pub limit: Option<i64>,             // Pagination limit (default: 20, max: 100)
    pub offset: Option<i64>,            // Pagination offset (default: 0)
    pub min_amount: Option<String>,     // Minimum loan amount filter
    pub max_amount: Option<String>,     // Maximum loan amount filter
    pub max_price: Option<String>,      // Maximum asking price filter
    pub risk_level: Option<String>,     // Risk level filter ("low", "medium", "high")
    pub asset_class: Option<String>,    // Asset class filter
    pub region: Option<String>,         // Region filter
}

/// Response for marketplace browsing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoansForSaleResponse {
    pub listings: Vec<MarketplaceDeal>, // Available marketplace deals
    pub total_count: i64,               // Total matching results
    pub has_more: bool,                 // True if more results available
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
    pub min: String,                    // Minimum asking price
    pub max: String,                    // Maximum asking price
}

/// Response when removing loan from sale
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RemoveLoanFromSaleResponse {
    pub loan_id: String,                // UUID as string
    pub removed_at: DateTime<Utc>,      // When removed
    pub status: String,                 // "unlisted"
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert MarketplaceLoan from database to frontend MarketplaceDeal format
fn marketplace_loan_to_deal(ml: MarketplaceLoan) -> MarketplaceDeal {
    // Generate title if not provided
    let title = ml.title.unwrap_or_else(|| {
        format!("${:.0} Credit Line - {} Interest", 
                ml.principal_amount, 
                format_bps_as_percentage(ml.interest_rate_bps))
    });
    
    // Use listing description, fall back to loan description, or generate
    let description = ml.listing_description
        .or(ml.description)
        .unwrap_or_else(|| format!("Credit line for ${:.2} at {}% interest", 
                                 ml.principal_amount,
                                 format_bps_as_percentage(ml.interest_rate_bps)));
    
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

/// Extract user ID from request (placeholder for Phase 2 - coordinate with Worker B on auth)
fn get_current_user_id(_state: &ApiState) -> Result<Uuid, (StatusCode, String)> {
    // TODO: Phase 3 integration will implement proper authentication
    // For Phase 2, return placeholder for testing
    
    Err((
        StatusCode::UNAUTHORIZED,
        "User authentication - coordinate with Worker B in Phase 3".to_string(),
    ))
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
    
    // Build filters using Phase 1 database functions
    let filters = MarketplaceFilters {
        min_amount,
        max_amount,
        max_price,
        risk_level: query.risk_level,
        asset_class: query.asset_class,
        region: query.region,
        limit: query.limit,
        offset: query.offset,
    };
    
    // Query marketplace loans using Phase 1 functions
    let marketplace_loans = list_marketplace_loans(&state.pool, &filters)
        .await
        .map_err(|e| {
            error!("❌ Failed to list marketplace loans: {}", e);
            metrics::counter!("marketplace_errors_total", "operation" => "browse").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve marketplace loans".to_string(),
            )
        })?;
    
    // Get total count for pagination
    let total_count = count_marketplace_loans(&state.pool, &filters)
        .await
        .map_err(|e| {
            error!("❌ Failed to count marketplace loans: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to count marketplace loans".to_string(),
            )
        })?;
    
    // Convert to frontend format
    let listings: Vec<MarketplaceDeal> = marketplace_loans
        .into_iter()
        .map(marketplace_loan_to_deal)
        .collect();
    
    // Calculate pagination info
    let limit = filters.limit.unwrap_or(20);
    let offset = filters.offset.unwrap_or(0);
    let has_more = (offset + limit) < total_count;
    
    // Build filter options (simplified for Phase 2)
    let filters_info = MarketplaceFiltersInfo {
        available_asset_classes: vec![], // TODO: Phase 3 enhancement
        available_regions: vec![],       // TODO: Phase 3 enhancement
        price_range: PriceRange {
            min: "0".to_string(),        // TODO: Phase 3 enhancement
            max: "1000000".to_string(),  // TODO: Phase 3 enhancement
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
    metrics::counter!("marketplace_listings_returned_total").increment(response.listings.len() as u64);
    
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
    
    // Get current user ID (Phase 3 will implement real auth)
    let seller_id = get_current_user_id(&state)?;
    
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
                (StatusCode::NOT_FOUND, "Loan not found or not owned by user".to_string())
            },
            sqlx::Error::Database(ref db_err) if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) => {
                metrics::counter!("marketplace_errors_total", "type" => "already_listed").increment(1);
                (StatusCode::CONFLICT, "Loan is already listed for sale".to_string())
            },
            sqlx::Error::Database(ref db_err) if db_err.code() == Some(std::borrow::Cow::Borrowed("23514")) => {
                metrics::counter!("marketplace_errors_total", "type" => "ineligible_status").increment(1);
                (StatusCode::CONFLICT, "Loan status is not eligible for listing".to_string())
            },
            _ => {
                metrics::counter!("marketplace_errors_total", "type" => "database").increment(1);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create listing".to_string())
            }
        }
    })?;
    
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
    
    // Get current user ID (Phase 3 will implement real auth)
    let seller_id = get_current_user_id(&state)?;
    
    // Remove listing using Phase 1 database function
    let removed = remove_loan_listing(&state.pool, loan_id, seller_id)
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
    
    // Use Phase 1 database function
    let stats = get_marketplace_stats(&state.pool)
        .await
        .map_err(|e| {
            error!("❌ Failed to get marketplace stats: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve marketplace statistics".to_string(),
            )
        })?;
    
    Ok(Json(stats))
}
```

---

## 💻 **CLI Implementation (Priority 2)**

### **File: `cli/src/marketplace.rs`** (New File)

Create CLI commands that use your API endpoints:

```rust
//! Marketplace CLI commands for loan trading
//!
//! This module provides command-line interface for marketplace operations including
//! listing loans for sale, browsing the marketplace, and managing listings.

use clap::{Args, Subcommand};
use reqwest::Client;
use serde_json::json;
use std::error::Error;
use uuid::Uuid;

use crate::cli::{get_api_base_url, handle_api_error};

/// Marketplace management commands
#[derive(Debug, Subcommand)]
pub enum MarketplaceCommands {
    /// Browse loans available in the marketplace
    Browse(BrowseMarketplaceArgs),
    
    /// List a loan for sale in the marketplace
    ListForSale(ListForSaleArgs),
    
    /// Remove a loan from the marketplace
    RemoveFromSale(RemoveFromSaleArgs),
    
    /// Show marketplace statistics
    Stats,
}

/// Arguments for browsing marketplace
#[derive(Debug, Args)]
pub struct BrowseMarketplaceArgs {
    /// Maximum number of results to return
    #[arg(long, default_value = "20")]
    pub limit: u32,
    
    /// Number of results to skip (for pagination)  
    #[arg(long, default_value = "0")]
    pub offset: u32,
    
    /// Minimum loan amount to filter by
    #[arg(long)]
    pub min_amount: Option<String>,
    
    /// Maximum loan amount to filter by
    #[arg(long)]
    pub max_amount: Option<String>,
    
    /// Maximum asking price to filter by
    #[arg(long)]
    pub max_price: Option<String>,
    
    /// Risk level to filter by (low, medium, high)
    #[arg(long)]
    pub risk_level: Option<String>,
    
    /// Asset class to filter by
    #[arg(long)]
    pub asset_class: Option<String>,
    
    /// Region to filter by
    #[arg(long)]
    pub region: Option<String>,
    
    /// Show verbose output with all fields
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for listing a loan for sale
#[derive(Debug, Args)]
pub struct ListForSaleArgs {
    /// UUID of the loan to list for sale
    #[arg(long)]
    pub loan_id: String,
    
    /// Asking price in USDC
    #[arg(long)]
    pub asking_price: String,
    
    /// Optional description for the listing
    #[arg(long)]
    pub description: Option<String>,
}

/// Arguments for removing loan from sale
#[derive(Debug, Args)]
pub struct RemoveFromSaleArgs {
    /// UUID of the loan to remove from sale
    #[arg(long)]
    pub loan_id: String,
}

/// Handle marketplace commands
pub async fn handle_marketplace_command(
    command: MarketplaceCommands,
) -> Result<(), Box<dyn Error>> {
    match command {
        MarketplaceCommands::Browse(args) => browse_marketplace(args).await,
        MarketplaceCommands::ListForSale(args) => list_loan_for_sale(args).await,
        MarketplaceCommands::RemoveFromSale(args) => remove_loan_from_sale(args).await,
        MarketplaceCommands::Stats => marketplace_stats().await,
    }
}

/// Browse marketplace loans for sale
async fn browse_marketplace(args: BrowseMarketplaceArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    println!("🛒 Browsing marketplace loans...");
    
    // Build query parameters
    let mut query_params = vec![
        ("limit", args.limit.to_string()),
        ("offset", args.offset.to_string()),
    ];
    
    if let Some(min_amount) = &args.min_amount {
        query_params.push(("min_amount", min_amount.clone()));
    }
    if let Some(max_amount) = &args.max_amount {
        query_params.push(("max_amount", max_amount.clone()));
    }
    if let Some(max_price) = &args.max_price {
        query_params.push(("max_price", max_price.clone()));
    }
    if let Some(risk_level) = &args.risk_level {
        query_params.push(("risk_level", risk_level.clone()));
    }
    if let Some(asset_class) = &args.asset_class {
        query_params.push(("asset_class", asset_class.clone()));
    }
    if let Some(region) = &args.region {
        query_params.push(("region", region.clone()));
    }
    
    // Make API request to your endpoint
    let response = client
        .get(&format!("{}/api/v1/loans/for-sale", base_url))
        .query(&query_params)
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    // Parse response
    let listings = result["listings"].as_array().unwrap_or(&vec![]);
    let total_count = result["total_count"].as_i64().unwrap_or(0);
    let has_more = result["has_more"].as_bool().unwrap_or(false);
    
    if listings.is_empty() {
        println!("📭 No loans currently listed for sale");
        return Ok(());
    }
    
    println!("📊 Found {} loans ({} total)", listings.len(), total_count);
    println!("");
    
    // Display results
    for (i, listing) in listings.iter().enumerate() {
        let deal_id = listing["id"].as_str().unwrap_or("unknown");
        let title = listing["title"].as_str().unwrap_or("Untitled Loan");
        let asking_price = listing["asking_price"].as_str().unwrap_or("0");
        let loan_amount = listing["loan_amount_range"]["max"].as_str().unwrap_or("0");
        let interest_rate = listing["interest_rate_range"]["max"].as_str().unwrap_or("0");
        let seller_email = listing["seller_email"].as_str().unwrap_or("unknown");
        let listed_at = listing["listed_at"].as_str().unwrap_or("unknown");
        
        println!("{}. {} ({})", i + 1, title, deal_id);
        println!("   💰 Asking Price: ${}", asking_price);
        println!("   📊 Loan Amount: ${}", loan_amount);
        println!("   📈 Interest Rate: {}%", interest_rate);
        println!("   👤 Seller: {}", seller_email);
        println!("   📅 Listed: {}", listed_at);
        
        if args.verbose {
            let description = listing["description"].as_str().unwrap_or("");
            let outstanding = listing["outstanding_balance"].as_str().unwrap_or("0");
            let risk_level = listing["risk_level"].as_str().unwrap_or("unknown");
            let asset_class = listing["asset_class"].as_str().unwrap_or("unknown");
            let region = listing["region"].as_str().unwrap_or("unknown");
            
            println!("   📝 Description: {}", description);
            println!("   💳 Outstanding Balance: ${}", outstanding);
            println!("   ⚠️ Risk Level: {}", risk_level);
            println!("   🏢 Asset Class: {}", asset_class);
            println!("   🌍 Region: {}", region);
        }
        
        println!("");
    }
    
    if has_more {
        println!("💡 Use --offset {} to see more results", args.offset + args.limit);
    }
    
    Ok(())
}

/// List a loan for sale
async fn list_loan_for_sale(args: ListForSaleArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate loan ID format
    Uuid::parse_str(&args.loan_id).map_err(|_| "Invalid loan ID format")?;
    
    // Validate asking price format  
    args.asking_price.parse::<f64>().map_err(|_| "Invalid asking price format")?;
    
    println!("🏷️ Listing loan {} for sale at ${}", args.loan_id, args.asking_price);
    
    let request_body = json!({
        "asking_price": args.asking_price,
        "description": args.description
    });
    
    // Call your API endpoint
    let response = client
        .post(&format!("{}/api/v1/loan/{}/list-for-sale", base_url, args.loan_id))
        .json(&request_body)
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    let listing_id = result["listing_id"].as_str().unwrap_or("unknown");
    let listed_at = result["listed_at"].as_str().unwrap_or("unknown");
    let status = result["status"].as_str().unwrap_or("unknown");
    
    println!("✅ Loan listed successfully!");
    println!("   📋 Listing ID: {}", listing_id);
    println!("   📅 Listed At: {}", listed_at);
    println!("   🔘 Status: {}", status);
    println!("   💰 Asking Price: ${}", args.asking_price);
    
    if let Some(desc) = &args.description {
        println!("   📝 Description: {}", desc);
    }
    
    Ok(())
}

/// Remove loan from sale  
async fn remove_loan_from_sale(args: RemoveFromSaleArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate loan ID format
    Uuid::parse_str(&args.loan_id).map_err(|_| "Invalid loan ID format")?;
    
    println!("🗑️ Removing loan {} from sale...", args.loan_id);
    
    // Call your API endpoint
    let response = client
        .delete(&format!("{}/api/v1/loan/{}/list-for-sale", base_url, args.loan_id))
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    let removed_at = result["removed_at"].as_str().unwrap_or("unknown");
    let status = result["status"].as_str().unwrap_or("unknown");
    
    println!("✅ Loan removed from sale successfully!");
    println!("   📅 Removed At: {}", removed_at);
    println!("   🔘 Status: {}", status);
    
    Ok(())
}

/// Show marketplace statistics
async fn marketplace_stats() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    println!("📊 Getting marketplace statistics...");
    
    let response = client
        .get(&format!("{}/api/v1/loans/marketplace/stats", base_url))
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let stats: serde_json::Value = response.json().await?;
    
    let active = stats["active_listings"].as_i64().unwrap_or(0);
    let sold = stats["sold_listings"].as_i64().unwrap_or(0);
    let cancelled = stats["cancelled_listings"].as_i64().unwrap_or(0);
    let avg_price = stats["avg_asking_price"].as_f64();
    let min_price = stats["min_asking_price"].as_f64();
    let max_price = stats["max_asking_price"].as_f64();
    
    println!("");
    println!("📈 Marketplace Statistics:");
    println!("   🟢 Active Listings: {}", active);
    println!("   ✅ Sold Listings: {}", sold);
    println!("   ❌ Cancelled Listings: {}", cancelled);
    
    if let Some(avg) = avg_price {
        println!("   💰 Average Price: ${:.2}", avg);
    }
    if let Some(min) = min_price {
        println!("   📉 Minimum Price: ${:.2}", min);
    }
    if let Some(max) = max_price {
        println!("   📈 Maximum Price: ${:.2}", max);
    }
    
    println!("");
    
    Ok(())
}
```

---

## 🔧 **Infrastructure Updates (Coordinate with Worker B)**

### **Your Infrastructure Files**

**Add to `api/src/paths.rs`**:
```rust
// ============================================================================
// MARKETPLACE PATHS (Worker A)
// ============================================================================

/// Browse marketplace loans for sale
pub const LOANS_FOR_SALE: &str = api_path!("/loans/for-sale");

/// List loan for sale
pub const LOAN_LIST_FOR_SALE: &str = api_path!("/loan/{loan_id}/list-for-sale");

/// Marketplace statistics
pub const MARKETPLACE_STATS: &str = api_path!("/loans/marketplace/stats");
```

**Add to `api/src/lib.rs`**:
```rust
// Add to existing mod declarations
pub mod marketplace_handlers;

// Add to existing use declarations (if any)
pub use marketplace_handlers::*;
```

### **Shared Infrastructure (Coordinate with Worker B)**

**`api/src/router.rs`** - Both workers add routes:
```rust
// Your marketplace routes (Worker A)
use crate::marketplace_handlers::{
    browse_marketplace_endpoint, list_loan_for_sale_endpoint, 
    remove_loan_from_sale_endpoint, marketplace_stats_endpoint
};

// Add your routes to the router
.route(paths::LOANS_FOR_SALE, get(browse_marketplace_endpoint))
.route(paths::LOAN_LIST_FOR_SALE, post(list_loan_for_sale_endpoint))
.route(paths::LOAN_LIST_FOR_SALE, delete(remove_loan_from_sale_endpoint))
.route(paths::MARKETPLACE_STATS, get(marketplace_stats_endpoint))
```

**`cli/src/lib.rs`** - Both workers add modules:
```rust
// Add to existing mod declarations
pub mod marketplace;  // Your module

// Add to existing use declarations
pub use marketplace::*;
```

**`cli/src/cli.rs`** - Both workers add commands:
```rust
// Add to imports
use crate::marketplace::{MarketplaceCommands, handle_marketplace_command};

// Add to main Commands enum
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// Marketplace management commands (Worker A)
    #[command(subcommand)]
    Marketplace(MarketplaceCommands),
}

// Add to main command handler
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing code ...
    
    match args.command {
        // ... existing command handlers ...
        
        Commands::Marketplace(marketplace_cmd) => {
            handle_marketplace_command(marketplace_cmd).await
        }
    }
}
```

---

## 🧪 **Phase 2 Testing Strategy**

### **API Testing**
```bash
# Test your marketplace API endpoints
cd api && cargo test marketplace_handlers

# Manual API testing (against Phase 1 database)
curl -X GET "http://localhost:8080/api/v1/loans/for-sale?limit=5"
curl -X GET "http://localhost:8080/api/v1/loans/marketplace/stats"

# Test with sample data (if available from Phase 1)
curl -X POST "http://localhost:8080/api/v1/loan/{UUID}/list-for-sale" \
  -H "Content-Type: application/json" \
  -d '{"asking_price": "50000.00", "description": "Great loan!"}'
```

### **CLI Testing**
```bash
# Test your CLI commands
make cli ARGS="marketplace browse --limit 5"
make cli ARGS="marketplace stats"

# Test with Phase 1 sample data (if available)
make cli ARGS="marketplace list-for-sale --loan-id {UUID} --asking-price 50000.00"
make cli ARGS="marketplace remove-from-sale --loan-id {UUID}"
```

### **Integration Testing with Database**
```bash
# Verify Phase 1 database functions work with your API
make server  # Start API server
# Run CLI commands to test end-to-end flow

# Check database state
psql $DATABASE_URL -c "SELECT COUNT(*) FROM loan_listings;"
psql $DATABASE_URL -c "SELECT * FROM marketplace_loans LIMIT 5;"
```

---

## 📋 **Phase 2 Coordination with Worker B**

### **What You Provide to Worker B**:
1. **Marketplace API Format**: `MarketplaceDeal` response structure
2. **Route Patterns**: Consistent REST endpoint naming
3. **Error Handling Format**: Standard HTTP error responses
4. **CLI Command Structure**: Pattern for command organization

### **What You Need from Worker B**:
1. **Authentication Pattern**: How to extract current user from requests
2. **Transfer Route Names**: Consistent naming for their endpoints
3. **Error Response Format**: Match their error handling style
4. **Infrastructure Coordination**: router.rs and cli.rs updates

### **Daily Sync Points**:
- **Day 1 End**: Share API response formats and route patterns
- **Day 2 Midday**: Coordinate infrastructure file updates
- **Day 2 End**: Finalize shared infrastructure integration

---

## ✅ **Phase 2 Definition of Done**

### **API Success Criteria**:
- [ ] All 4 marketplace endpoints implemented and tested
- [ ] API responses match `MarketplaceDeal` format for frontend
- [ ] Proper error handling with HTTP status codes
- [ ] Metrics and logging integration
- [ ] OpenAPI documentation complete

### **CLI Success Criteria**:
- [ ] All 4 CLI commands working with your APIs
- [ ] Rich output formatting with verbose options
- [ ] Input validation and error handling
- [ ] Consistent with existing CLI patterns

### **Integration Success Criteria**:
- [ ] Infrastructure files updated (paths.rs, lib.rs)
- [ ] Router integration ready for Worker B
- [ ] CLI integration ready for Worker B
- [ ] No conflicts with Worker B's parallel work

### **Ready for Phase 3**:
- [ ] All endpoints tested independently
- [ ] Documentation complete for Worker B integration
- [ ] Sample test scenarios prepared
- [ ] Performance baseline established

**You own the marketplace user experience - make it intuitive and robust! 🛒✨**

---

## 🚀 **Getting Started**

### **Day 1: API Implementation**
1. **Create `marketplace_handlers.rs`** - 4 core API endpoints
2. **Test with Phase 1 database** - Verify functions work
3. **Add infrastructure updates** - paths.rs, lib.rs

### **Day 2: CLI Implementation**
1. **Create `marketplace.rs` CLI** - 4 corresponding commands
2. **Test CLI → API → Database flow** - Full integration
3. **Coordinate with Worker B** - Share patterns and standards

### **Day 2.5: Integration Prep**
1. **Finalize shared files** - router.rs, cli.rs coordination
2. **Test independently** - Ensure your components work
3. **Document for Phase 3** - Integration requirements and test cases

**Ready to build the marketplace user interface! Let's make loan browsing and listing seamless! 🛒🚀**
