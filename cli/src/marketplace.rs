//! Marketplace CLI commands for loan trading
//!
//! This module provides command-line interface for marketplace operations including
//! listing loans for sale, browsing the marketplace, and managing listings.

use clap::{Args, Subcommand};
use eyre::Result;
use reqwest::Client;
use serde_json::json;
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
pub async fn handle_marketplace_command(command: MarketplaceCommands) -> Result<()> {
    match command {
        MarketplaceCommands::Browse(args) => browse_marketplace(args).await,
        MarketplaceCommands::ListForSale(args) => list_loan_for_sale(args).await,
        MarketplaceCommands::RemoveFromSale(args) => remove_loan_from_sale(args).await,
        MarketplaceCommands::Stats => marketplace_stats().await,
    }
}

/// Browse marketplace loans for sale
async fn browse_marketplace(args: BrowseMarketplaceArgs) -> Result<()> {
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
        handle_api_error(response).await?;
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    // Parse response
    let empty_vec = vec![];
    let listings = result["listings"].as_array().unwrap_or(&empty_vec);
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
        let interest_rate = listing["interest_rate_range"]["max"]
            .as_str()
            .unwrap_or("0");
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
        println!(
            "💡 Use --offset {} to see more results",
            args.offset + args.limit
        );
    }

    Ok(())
}

/// List a loan for sale
async fn list_loan_for_sale(args: ListForSaleArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate loan ID format
    Uuid::parse_str(&args.loan_id).map_err(|_| eyre::eyre!("Invalid loan ID format"))?;

    // Validate asking price format
    args.asking_price
        .parse::<f64>()
        .map_err(|_| eyre::eyre!("Invalid asking price format"))?;

    println!(
        "🏷️ Listing loan {} for sale at ${}",
        args.loan_id, args.asking_price
    );

    let request_body = json!({
        "asking_price": args.asking_price,
        "description": args.description
    });

    // Call your API endpoint
    let response = client
        .post(&format!(
            "{}/api/v1/loan/{}/list-for-sale",
            base_url, args.loan_id
        ))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
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
async fn remove_loan_from_sale(args: RemoveFromSaleArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate loan ID format
    Uuid::parse_str(&args.loan_id).map_err(|_| eyre::eyre!("Invalid loan ID format"))?;

    println!("🗑️ Removing loan {} from sale...", args.loan_id);

    // Call your API endpoint
    let response = client
        .delete(&format!(
            "{}/api/v1/loan/{}/list-for-sale",
            base_url, args.loan_id
        ))
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
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
async fn marketplace_stats() -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    println!("📊 Getting marketplace statistics...");

    let response = client
        .get(&format!("{}/api/v1/loans/marketplace/stats", base_url))
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
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
