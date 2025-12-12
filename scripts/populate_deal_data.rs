#!/usr/bin/env cargo run --bin populate_deal_data --
//! Deal data population script for the Arda Credit database
//!
//! This script creates realistic deal data for testing the deals API endpoint,
//! zkVM batch processing, and automatically refreshes the metrics cache.
//!
//! ## Transaction Coverage
//!
//! The script creates the following transaction types for zkVM testing:
//! - UserRegistration (for new users)
//! - DealOrigination (for all deals)
//! - DealPayment (1-5 per deal based on status)
//! - DealCloseout (for 'closed' status deals)
//! - CollateralCreation (~70% of deals)
//! - CollateralDealAssociation (~70% of deals)
//!
//! This provides 60% coverage of zkVM transaction types.
//!
//! ## Modes
//!
//! - Random mode: Generate random deals with random companies (no --email)
//! - User mode: Generate deals for a specific user's companies (with --email)
//! - CSV mode: Export to CSV instead of database (with --csv)
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage (random deals)
//! cargo run --bin populate_deal_data -- --count 25
//!
//! # User-specific deals
//! cargo run --bin populate_deal_data -- --email user@example.com --count 10
//!
//! # With all zkVM transaction types
//! cargo run --bin populate_deal_data -- --count 50
//!
//! # Clear and regenerate
//! cargo run --bin populate_deal_data -- --clear --count 100
//! ```

use chrono::NaiveDate;
use chrono::{Duration, Utc};
use clap::{Arg, Command};
use db::{
    business::company::{
        create_company, create_membership, Company, CompanyRole, CreateCompanyRequest, ProfileType,
    },
    business::metrics::{
        batch_upsert_market_value_metrics, batch_upsert_pie_chart_metrics, clear_all_metrics,
        CreateMarketValueMetricRequest, CreatePieChartMetricRequest,
    },
    create_identity, get_identity_by_email, init_db_with_url, list_deals, Collateral, Deal, User,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, env, str::FromStr};
use uuid::Uuid;

// ============================================================================
// CSV EXPORT STRUCTURES
// ============================================================================

/// CSV row structure for exporting deal data
#[derive(Debug, Clone, Serialize)]
struct DealCsvRow {
    originator_name: String,
    investor_name: Option<String>,
    borrower_name: Option<String>,
    principal_amount: String,
    outstanding_balance: String,
    interest_rate_bps: i32,
    deal_status: String,
    originated_at: String,
    maturity_date: Option<String>,
    status: String,
    industry: Option<String>,
    title: Option<String>,
    description: Option<String>,
    risk_level: Option<String>,
    asset_class: Option<String>,
    region: Option<String>,
    minimum_investment: Option<String>,
    // Collateral fields for testing double pledge detection
    collateral_id: Option<String>,
    collateral_name: Option<String>,
    collateral_type: Option<String>,
    collateral_address: Option<String>,
    collateral_custom_id: Option<String>,
}

// ============================================================================
// BATCH UPLOAD STRUCTURES (for arda-collateral-intel service)
// ============================================================================

/// Batch upload request for collateral intel service
#[derive(Debug, Serialize)]
struct BatchUploadRequest {
    deals: Vec<BatchDealPayload>,
}

/// Deal payload for batch upload
#[derive(Debug, Serialize)]
struct BatchDealPayload {
    deal: BatchDealInfo,
    collaterals: Vec<BatchCollateralInfo>,
}

/// Deal info for batch upload
#[derive(Debug, Serialize)]
struct BatchDealInfo {
    id: String,
    status: String,
    company_id: String,
}

/// Collateral info for batch upload
#[derive(Debug, Serialize)]
struct BatchCollateralInfo {
    id: String,
    name: String,
    collateral_type: String,
    address: Option<String>,
    custom_id: Option<String>,
    description: Option<String>,
}

/// Response from batch upload
#[derive(Debug, Deserialize)]
struct BatchUploadResponse {
    total_deals: i32,
    total_collaterals: i32,
    tasks_queued: i32,
    message: String,
}

// ============================================================================
// CONFIGURATION CONSTANTS
// ============================================================================

// Default values
const DEFAULT_DEAL_COUNT_STR: &str = "25";
const DEFAULT_DATABASE_URL: &str = "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db";

// Deal generation parameters
const MIN_PRINCIPAL_AMOUNT: u64 = 50_000_000;
const MAX_PRINCIPAL_AMOUNT: u64 = 5_000_000_000;
const MIN_INTEREST_RATE_BPS: i32 = 400; // 4% APR
const MAX_INTEREST_RATE_BPS: i32 = 1200; // 12% APR
const MIN_OUTSTANDING_BALANCE_PCT: u32 = 60; // 60% of principal
const MAX_OUTSTANDING_BALANCE_PCT: u32 = 95; // 95% of principal
const MIN_MATURITY_MONTHS: i64 = 12; // 1 year
const MAX_MATURITY_MONTHS: i64 = 60; // 5 years
const MAX_DAYS_AGO_ORIGINATED: i64 = 365; // 0-1 year ago

// Company generation
const MIN_RANDOM_COMPANIES: usize = 6;
const COMPANY_COUNT_DIVISOR: u32 = 3; // count / 3 companies

// Deal status distribution weights (must sum to 100)
const DEAL_STATUS_ACTIVE_WEIGHT: u32 = 60;
const DEAL_STATUS_ORIGINATED_WEIGHT: u32 = 20;
const DEAL_STATUS_PAID_OFF_WEIGHT: u32 = 15;
const DEAL_STATUS_CLOSED_WEIGHT: u32 = 5;

// Risk level distribution weights (must sum to 100)
const RISK_LEVEL_LOW_WEIGHT: u32 = 30;
const RISK_LEVEL_MEDIUM_WEIGHT: u32 = 50;
const RISK_LEVEL_HIGH_WEIGHT: u32 = 20;

// Data arrays
const INDUSTRIES: &[&str] = &[
    "Technology",
    "Healthcare",
    "Real Estate",
    "Manufacturing",
    "Financial Services",
    "Energy",
    "Retail",
    "Construction",
    "Transportation",
    "Agriculture",
    "Media",
    "Education",
    "Biotechnology",
    "Automotive",
    "Aerospace",
];

const REGIONS: &[&str] = &[
    "North America",
    "Europe",
    "Asia-Pacific",
    "Latin America",
    "Middle East",
    "Africa",
    "Global",
    "United States",
    "Canada",
    "United Kingdom",
];

const ASSET_CLASSES: &[&str] = &[
    "Equipment Finance",
    "Working Capital",
    "Real Estate",
    "Invoice Finance",
    "Supply Chain Finance",
    "Trade Finance",
    "Project Finance",
    "Asset-Based Lending",
    "Revenue-Based Finance",
    "Bridge Loans",
    "Term Loans",
    "Lines of Credit",
];

const COMPANY_TYPES: &[&str] = &[
    "Tech Corp",
    "Healthcare Inc",
    "Real Estate Partners",
    "Manufacturing Co",
    "Finance Group",
    "Energy Solutions",
    "Logistics Ltd",
    "Retail Ventures",
    "Construction Co",
    "Consulting Partners",
    "Media Group",
    "Education Corp",
    "Biotech Inc",
    "Automotive Co",
    "Aerospace Ltd",
    "Telecom Solutions",
];

const COMPANY_PREFIXES: &[&str] = &[
    "Acme",
    "Global",
    "Premier",
    "Advanced",
    "Superior",
    "Dynamic",
    "Innovative",
    "Strategic",
    "Unified",
    "Integrated",
    "Elite",
    "Professional",
    "Efficient",
    "Reliable",
    "Secure",
    "Rapid",
    "Smart",
    "Modern",
    "Digital",
    "Future",
];

// Collateral data for testing duplicate detection
const COLLATERAL_NAMES: &[&str] = &[
    "123 Main Street Property",
    "456 Oak Avenue Building",
    "789 Elm Street Commercial Space",
    "101 Pine Road Industrial Complex",
    "202 Maple Drive Warehouse",
    "CAT D8T Bulldozer Fleet",
    "John Deere 9RX Series Tractors",
    "Caterpillar 797F Mining Trucks",
    "Boeing 737-800 Aircraft",
    "Gulfstream G650 Private Jet",
    "Inventory - Consumer Electronics",
    "Inventory - Automotive Parts",
    "Accounts Receivable Portfolio Q4-2024",
    "Equipment - CNC Machining Center",
    "Equipment - Industrial 3D Printers",
    "Patent Portfolio - AI Technology",
    "Trademark - Brand Assets Collection",
    "Server Infrastructure - Data Center A",
    "Medical Equipment - MRI Scanner",
    "Oil & Gas Rights - Block 47",
];

const COLLATERAL_TYPES: &[&str] = &[
    "real_estate",
    "commercial_property",
    "industrial_property",
    "warehouse",
    "construction_equipment",
    "agricultural_equipment",
    "mining_equipment",
    "aircraft",
    "private_aircraft",
    "inventory",
    "electronics_inventory",
    "automotive_inventory",
    "accounts_receivable",
    "manufacturing_equipment",
    "technology_equipment",
    "intellectual_property",
    "trademark",
    "it_infrastructure",
    "medical_equipment",
    "mineral_rights",
];

const COLLATERAL_ADDRESSES: &[&str] = &[
    "123 Main St, New York, NY 10001",
    "456 Oak Ave, Los Angeles, CA 90012",
    "789 Elm St, Chicago, IL 60601",
    "101 Pine Rd, Houston, TX 77002",
    "202 Maple Dr, Phoenix, AZ 85003",
    "303 Cedar Ln, Philadelphia, PA 19102",
    "404 Birch Way, San Antonio, TX 78201",
    "505 Spruce Ct, San Diego, CA 92101",
    "606 Willow St, Dallas, TX 75201",
    "707 Ash Blvd, San Jose, CA 95110",
    "808 Broadway, Seattle, WA 98101",
    "909 Market St, San Francisco, CA 94103",
    "1010 Congress Ave, Austin, TX 78701",
    "1111 Peachtree St, Atlanta, GA 30309",
    "1212 Beacon St, Boston, MA 02215",
    "GPS: 40.7128° N, 74.0060° W",
    "GPS: 34.0522° N, 118.2437° W",
    "GPS: 41.8781° N, 87.6298° W",
    "Parcel ID: APN-2024-00451-A",
    "Serial: CAT-2024-D8T-00789",
];

/// Refresh all metrics data after deal population
async fn refresh_metrics_data(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = pool.begin().await?;

    // Clear existing metrics
    clear_all_metrics(&mut tx).await?;

    // Get ALL deals for calculation (set high limit to avoid pagination issues)
    let deals_result =
        list_deals(pool, Some(1_000_000), None, None, None, None, None, None).await?;
    let deals = &deals_result.deals;

    if deals.is_empty() {
        println!("ℹ️ No deals found, skipping metrics calculation");
        tx.commit().await?;
        return Ok(());
    }

    println!("📊 Calculating metrics for {} deals", deals.len());

    // Calculate market value metrics
    let market_value_metrics = calculate_market_value_metrics(deals)?;
    let inserted_market = batch_upsert_market_value_metrics(&mut tx, market_value_metrics).await?;

    // Calculate pie chart metrics
    let pie_chart_metrics = calculate_pie_chart_metrics(deals)?;
    let inserted_pie = batch_upsert_pie_chart_metrics(&mut tx, pie_chart_metrics).await?;

    tx.commit().await?;

    println!(
        "✅ Refreshed metrics: {} market value points, {} pie chart points",
        inserted_market, inserted_pie
    );

    Ok(())
}

/// Calculate market value time series metrics for all deals
fn calculate_market_value_metrics(
    deals: &[Deal],
) -> Result<Vec<CreateMarketValueMetricRequest>, Box<dyn std::error::Error>> {
    let mut metrics = Vec::new();

    // Calculate for both growth types
    for growth_type in ["market_growth", "arda_growth"] {
        // Group deals by date
        let mut daily_totals: HashMap<NaiveDate, (Decimal, i32)> = HashMap::new();

        for deal in deals {
            let date = match growth_type {
                "market_growth" => deal.originated_at.date_naive(),
                "arda_growth" => deal.created_at.date_naive(),
                _ => continue,
            };

            let entry = daily_totals.entry(date).or_insert((Decimal::ZERO, 0));
            entry.0 += deal.principal_amount;
            entry.1 += 1;
        }

        // Convert to cumulative time series
        let mut sorted_dates: Vec<_> = daily_totals.keys().cloned().collect();
        sorted_dates.sort();

        let mut cumulative_value = Decimal::ZERO;
        let mut cumulative_count = 0;

        for date in sorted_dates {
            let (daily_value, daily_count) = daily_totals[&date];
            cumulative_value += daily_value;
            cumulative_count += daily_count;

            metrics.push(CreateMarketValueMetricRequest {
                date_value: date,
                growth_type: growth_type.to_string(),
                filter_type: "all".to_string(),
                entity_id: None,
                cumulative_value,
                deal_count: cumulative_count,
            });
        }
    }

    Ok(metrics)
}

/// Calculate pie chart distribution metrics for all deals
fn calculate_pie_chart_metrics(
    deals: &[Deal],
) -> Result<Vec<CreatePieChartMetricRequest>, Box<dyn std::error::Error>> {
    let mut metrics = Vec::new();

    let chart_types = ["industry", "region", "asset_class"];
    let time_ranges = ["all_time", "last_year"];

    for chart_type in chart_types {
        for time_range in time_ranges {
            // Filter deals for time range
            let filtered_deals: Vec<_> = if time_range == "last_year" {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(365);
                deals
                    .iter()
                    .filter(|d| d.originated_at >= cutoff)
                    .cloned()
                    .collect()
            } else {
                deals.to_vec()
            };

            // Group deals by category
            let mut category_totals: HashMap<String, (Decimal, i32)> = HashMap::new();
            let mut total_value = Decimal::ZERO;

            for deal in &filtered_deals {
                let category = match chart_type {
                    "industry" => deal.industry.as_deref().unwrap_or("Unknown").to_string(),
                    "region" => deal.region.as_deref().unwrap_or("Unknown").to_string(),
                    "asset_class" => deal.asset_class.as_deref().unwrap_or("Unknown").to_string(),
                    _ => continue,
                };

                let entry = category_totals
                    .entry(category)
                    .or_insert((Decimal::ZERO, 0));
                entry.0 += deal.principal_amount;
                entry.1 += 1;
                total_value += deal.principal_amount;
            }

            // Calculate percentages and create metrics
            for (category, (value, count)) in category_totals {
                let percentage = if total_value > Decimal::ZERO {
                    (value / total_value * Decimal::from(100)).round_dp(2)
                } else {
                    Decimal::ZERO
                };

                metrics.push(CreatePieChartMetricRequest {
                    chart_type: chart_type.to_string(),
                    filter_type: "all".to_string(),
                    entity_id: None,
                    time_range: time_range.to_string(),
                    category,
                    value,
                    percentage,
                    deal_count: count,
                });
            }
        }
    }

    Ok(metrics)
}

/// Create a deal with custom originated_at and created_at timestamps
async fn create_deal_with_custom_dates(
    pool: &sqlx::PgPool,
    originator_id: Uuid,
    investor_id: Option<Uuid>,
    principal_amount: Decimal,
    interest_rate_bps: i32,
    maturity_date: Option<chrono::DateTime<Utc>>,
    deal_status: Option<&str>,
    custom_deal_id: Option<&str>,
    status: Option<&str>,
    industry: Option<&str>,
    document_ids: Option<&serde_json::Value>,
    title: Option<&str>,
    description: Option<&str>,
    risk_level: Option<&str>,
    asset_class: Option<&str>,
    region: Option<&str>,
    minimum_investment: Option<Decimal>,
    originated_at: chrono::DateTime<Utc>,
    created_at: chrono::DateTime<Utc>,
    collateral_id: Option<Uuid>,
) -> Result<Deal, sqlx::Error> {
    let id = Uuid::new_v4();
    let default_json = serde_json::Value::Array(vec![]);
    let doc_ids = document_ids.unwrap_or(&default_json);

    sqlx::query!(
        r#"
        INSERT INTO deals (
            id, originator_id, investor_id, principal_amount, outstanding_balance, interest_rate_bps, deal_status, maturity_date, originated_at, created_at, updated_at,
            custom_deal_id, status, industry, document_ids,
            title, description, risk_level, asset_class, region, minimum_investment, collateral_id
        )
        VALUES (
            $1, $2, $3, $4, $4, $5, $6, $7, $8, $9, $9,
            $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19, $20
        )
        "#,
        id,
        originator_id,
        investor_id,
        principal_amount,
        interest_rate_bps,
        deal_status.unwrap_or("originated"),
        maturity_date,
        originated_at,
        created_at,
        custom_deal_id,
        status.unwrap_or("draft"),
        industry,
        doc_ids,
        title,
        description,
        risk_level,
        asset_class,
        region,
        minimum_investment,
        collateral_id
    )
    .execute(pool)
    .await?;

    let deal = Deal {
        id,
        originator_id,
        investor_id,
        borrower_id: None,         // Optional third-party borrower
        borrower_legal_name: None, // Borrower company legal name
        principal_amount,
        outstanding_balance: principal_amount,
        interest_rate_bps,
        deal_status: deal_status.unwrap_or("originated").to_string(),
        originated_at,
        maturity_date,
        created_at,
        updated_at: created_at,
        custom_deal_id: custom_deal_id.map(|s| s.to_string()),
        status: status.unwrap_or("draft").to_string(),
        industry: industry.map(|s| s.to_string()),
        document_ids: Some(doc_ids.clone()),
        title: title.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        risk_level: risk_level.map(|s| s.to_string()),
        asset_class: asset_class.map(|s| s.to_string()),
        region: region.map(|s| s.to_string()),
        minimum_investment,
        payment_frequency: None,
        collateral_type: None,
        collateral_value: None,
        ltv_ratio: None,
        credit_score: None,
        dscr: None,
        state_transition_hash: None,
        proof_included: false,
        collateral_id,
    };

    Ok(deal)
}

// ============================================================================
// DEAL TRANSACTION HISTORY FUNCTIONS
// ============================================================================

/// Generate SHA256 hash of a UUID for deal_transactions table
fn hash_uuid(uuid: &Uuid) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(uuid.as_bytes());
    hasher.finalize().to_vec()
}

/// Create a deal_transactions record to track state progression events
async fn create_deal_transaction_record(
    pool: &sqlx::PgPool,
    transaction_type: &str, // "deal_origination", "deal_payment", "deal_closeout"
    deal_id: Uuid,
    user_id: Option<Uuid>, // Optional for script-generated deals
    amount: Decimal,
    created_at: chrono::DateTime<Utc>,
    payment_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    let transaction_id = Uuid::new_v4();
    let transaction_id_hash = hash_uuid(&transaction_id);
    // Use a dummy UUID for hash if user_id is None
    let dummy_uuid = Uuid::nil();
    let user_id_hash = hash_uuid(&user_id.unwrap_or(dummy_uuid));
    let deal_id_hash = hash_uuid(&deal_id);

    sqlx::query(
        r#"
        INSERT INTO deal_transactions (
            id, transaction_id_hash, user_id_hash, deal_id_hash,
            transaction_type, amount, user_id, deal_id, payment_id,
            metadata, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(transaction_id)
    .bind(transaction_id_hash)
    .bind(user_id_hash)
    .bind(deal_id_hash)
    .bind(transaction_type)
    .bind(amount)
    .bind(user_id)
    .bind(deal_id)
    .bind(payment_id)
    .bind(json!({
        "created_by_script": true,
        "transaction_type": transaction_type
    }))
    .bind(created_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Create realistic state progression history for a deal based on its target status
async fn create_deal_state_progression(
    pool: &sqlx::PgPool,
    deal: &Deal,
    target_status: &str,
    rng: &mut impl Rng,
) -> Result<(), Box<dyn std::error::Error>> {
    let originated_at = deal.originated_at;
    let maturity_date = deal
        .maturity_date
        .unwrap_or(originated_at + Duration::days(365));

    // Always create deal_origination transaction
    // Note: user_id is None for script-generated deals without specific users
    create_deal_transaction_record(
        pool,
        "deal_origination",
        deal.id,
        None, // No specific user for script-generated deals
        deal.principal_amount,
        originated_at,
        None,
    )
    .await?;

    match target_status {
        "originated" => {
            // Just the origination event, no payments yet
        }
        "active" => {
            // Create 1-2 partial payment transactions
            let num_payments = rng.gen_range(1..=2);
            let total_days = (maturity_date - originated_at).num_days();

            for _i in 0..num_payments {
                // Spread payments across 20-60% of deal lifetime
                let days_offset = rng.gen_range(30..=(total_days * 6 / 10).max(30));
                let payment_date = originated_at + Duration::days(days_offset);

                // Partial payment (10-30% of principal each)
                let payment_pct = rng.gen_range(10..=30);
                let payment_amount =
                    deal.principal_amount * Decimal::from(payment_pct) / Decimal::from(100);

                create_deal_transaction_record(
                    pool,
                    "deal_payment",
                    deal.id,
                    None, // No specific user for script-generated deals
                    payment_amount,
                    payment_date,
                    None, // No actual payment record created
                )
                .await?;
            }
        }
        "paid_off" => {
            // Create 3-5 payment transactions that sum to principal amount
            let num_payments = rng.gen_range(3..=5);
            let total_days = (maturity_date - originated_at).num_days();

            // Calculate payment amounts that sum to principal
            let mut remaining = deal.principal_amount;
            let mut payment_amounts = Vec::new();

            for _i in 0..(num_payments - 1) {
                // Each payment is 15-35% of remaining balance
                let pct = rng.gen_range(15..=35);
                let amount = (remaining * Decimal::from(pct) / Decimal::from(100)).round_dp(2);
                payment_amounts.push(amount);
                remaining -= amount;
            }
            // Last payment is exactly the remaining balance
            payment_amounts.push(remaining);

            // Create evenly spaced payment transactions
            for (i, amount) in payment_amounts.iter().enumerate() {
                let days_offset = ((i + 1) as i64 * total_days) / (num_payments as i64);
                let payment_date = originated_at + Duration::days(days_offset.min(total_days - 1));

                create_deal_transaction_record(
                    pool,
                    "deal_payment",
                    deal.id,
                    None, // No specific user for script-generated deals
                    *amount,
                    payment_date,
                    None, // No actual payment record created
                )
                .await?;
            }
        }
        "closed" => {
            // Create full payment history + closeout transaction
            let num_payments = rng.gen_range(3..=5);
            let total_days = (maturity_date - originated_at).num_days();

            // Calculate payment amounts that sum to principal
            let mut remaining = deal.principal_amount;
            let mut payment_amounts = Vec::new();

            for _i in 0..(num_payments - 1) {
                let pct = rng.gen_range(15..=35);
                let amount = (remaining * Decimal::from(pct) / Decimal::from(100)).round_dp(2);
                payment_amounts.push(amount);
                remaining -= amount;
            }
            payment_amounts.push(remaining);

            // Create payment transactions
            let mut last_payment_date = originated_at;
            for (i, amount) in payment_amounts.iter().enumerate() {
                let days_offset = ((i + 1) as i64 * total_days) / (num_payments as i64);
                let payment_date = originated_at + Duration::days(days_offset.min(total_days - 1));
                last_payment_date = payment_date;

                create_deal_transaction_record(
                    pool,
                    "deal_payment",
                    deal.id,
                    None, // No specific user for script-generated deals
                    *amount,
                    payment_date,
                    None, // No actual payment record created
                )
                .await?;
            }

            // Create closeout transaction a few days after final payment
            let closeout_date = last_payment_date + Duration::days(rng.gen_range(3..=14));
            create_deal_transaction_record(
                pool,
                "deal_closeout",
                deal.id,
                None, // No specific user for script-generated deals
                Decimal::ZERO,
                closeout_date,
                None,
            )
            .await?;
        }
        _ => {
            // Unknown status, just create origination
        }
    }

    Ok(())
}

// ============================================================================
// CSV EXPORT FUNCTIONS
// ============================================================================

/// Write deal data to a CSV file
fn write_deals_to_csv(deals: Vec<DealCsvRow>) -> Result<String, Box<dyn std::error::Error>> {
    // Generate timestamped filename
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("loan_tape_{}.csv", timestamp);

    println!("📝 Writing {} deals to {}", deals.len(), filename);

    // Create CSV writer
    let mut wtr = csv::Writer::from_path(&filename)?;

    // Write all deals
    for deal in deals {
        wtr.serialize(deal)?;
    }

    wtr.flush()?;

    println!("✅ CSV file created: {}", filename);
    Ok(filename)
}

/// Generate deal data for a specific user (CSV mode - no database operations)
/// Creates a single-party portfolio with user as originator and random investors
fn generate_user_deals_csv(
    email: &str,
    count: u32,
) -> Result<Vec<DealCsvRow>, Box<dyn std::error::Error>> {
    let mut deals = Vec::new();
    let mut rng = thread_rng();

    // Generate synthetic company name from email for the ORIGINATOR (consistent party)
    let user_name = email.split('@').next().unwrap_or("User");
    let user_name_formatted = user_name
        .split('.')
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let originator_company_name = format!("{} Originator Corp", user_name_formatted);

    // Create random investor company names (need enough variety)
    let investor_count = (count / 3).max(5) as usize; // At least 5 investors
    let mut investor_company_names = Vec::new();
    for i in 0..investor_count {
        let prefix = COMPANY_PREFIXES.choose(&mut rng).unwrap();
        let suffix = COMPANY_TYPES.choose(&mut rng).unwrap();
        investor_company_names.push(format!("{} {} #{}", prefix, suffix, i + 1));
    }

    println!(
        "🏢 Creating single-party portfolio: {} as originator with {} random investors",
        originator_company_name,
        investor_company_names.len()
    );

    // Generate ALL deals with same originator, different investors
    for i in 0..count {
        let investor_name = investor_company_names.choose(&mut rng).unwrap().clone();
        let deal = generate_csv_deal_row(
            originator_company_name.clone(),
            Some(investor_name),
            &format!("Portfolio Deal #{}", i + 1),
            &mut rng,
        )?;
        deals.push(deal);
    }

    println!(
        "✅ Generated {} deals for single-party portfolio (originator: {})",
        deals.len(),
        originator_company_name
    );

    Ok(deals)
}

/// Generate random deal data (CSV mode - no database operations)
/// Creates a single-party portfolio with one consistent originator and random investors
fn generate_random_deals_csv(count: u32) -> Result<Vec<DealCsvRow>, Box<dyn std::error::Error>> {
    let mut deals = Vec::new();
    let mut rng = thread_rng();

    // Generate synthetic company names
    let company_count = (count / COMPANY_COUNT_DIVISOR).max(MIN_RANDOM_COMPANIES as u32) as usize;
    let mut company_names = Vec::new();

    for i in 0..company_count {
        let prefix = COMPANY_PREFIXES.choose(&mut rng).unwrap();
        let suffix = COMPANY_TYPES.choose(&mut rng).unwrap();
        company_names.push(format!("{} {} #{}", prefix, suffix, i + 1));
    }

    // Use first company as consistent originator
    let originator_name = company_names[0].clone();

    // Remaining companies are the investor pool
    let investor_pool: Vec<String> = company_names.iter().skip(1).cloned().collect();

    println!(
        "🏢 Creating single-party portfolio: {} as originator with {} potential investors",
        originator_name,
        investor_pool.len()
    );

    // Generate ALL deals with same originator, random investors
    for i in 0..count {
        let investor_name = investor_pool.choose(&mut rng).unwrap().clone();

        let deal = generate_csv_deal_row(
            originator_name.clone(),
            Some(investor_name),
            &format!("Portfolio Deal #{}", i + 1),
            &mut rng,
        )?;
        deals.push(deal);
    }

    println!(
        "✅ Generated {} deals for single-party portfolio (originator: {})",
        deals.len(),
        originator_name
    );

    Ok(deals)
}

/// Generate a single CSV deal row with realistic randomized data
fn generate_csv_deal_row(
    originator_name: String,
    investor_name: Option<String>,
    title_prefix: &str,
    rng: &mut impl Rng,
) -> Result<DealCsvRow, Box<dyn std::error::Error>> {
    // Realistic principal amounts
    let principal_amount =
        Decimal::from(rng.gen_range(MIN_PRINCIPAL_AMOUNT..=MAX_PRINCIPAL_AMOUNT));

    // Interest rates
    let interest_rate_bps = rng.gen_range(MIN_INTEREST_RATE_BPS..=MAX_INTEREST_RATE_BPS);

    // Deal statuses with realistic distribution
    let deal_statuses = [
        ("active", DEAL_STATUS_ACTIVE_WEIGHT),
        ("originated", DEAL_STATUS_ORIGINATED_WEIGHT),
        ("paid_off", DEAL_STATUS_PAID_OFF_WEIGHT),
        ("closed", DEAL_STATUS_CLOSED_WEIGHT),
    ];

    let total_weight: u32 = deal_statuses.iter().map(|(_, weight)| weight).sum();
    let random_weight = rng.gen_range(0..total_weight);
    let mut cumulative_weight = 0;
    let mut selected_status = "active";

    for (status, weight) in &deal_statuses {
        cumulative_weight += weight;
        if random_weight < cumulative_weight {
            selected_status = status;
            break;
        }
    }

    // Risk levels distribution
    let risk_levels = [
        ("low", RISK_LEVEL_LOW_WEIGHT),
        ("medium", RISK_LEVEL_MEDIUM_WEIGHT),
        ("high", RISK_LEVEL_HIGH_WEIGHT),
    ];
    let risk_total: u32 = risk_levels.iter().map(|(_, w)| w).sum();
    let risk_rand = rng.gen_range(0..risk_total);
    let mut risk_cumulative = 0;
    let mut selected_risk = "medium";

    for (risk, weight) in &risk_levels {
        risk_cumulative += weight;
        if risk_rand < risk_cumulative {
            selected_risk = risk;
            break;
        }
    }

    // Calculate dates
    let maturity_months = rng.gen_range(MIN_MATURITY_MONTHS..=MAX_MATURITY_MONTHS);
    let originated_at = Utc::now() - Duration::days(rng.gen_range(0..=MAX_DAYS_AGO_ORIGINATED));
    let maturity_date = originated_at + Duration::days(maturity_months * 30);

    // Outstanding balance (percentage of principal for active deals, 0 for paid_off/closed)
    let outstanding_balance = if selected_status == "paid_off" || selected_status == "closed" {
        Decimal::ZERO
    } else {
        let percentage = rng.gen_range(MIN_OUTSTANDING_BALANCE_PCT..=MAX_OUTSTANDING_BALANCE_PCT);
        principal_amount * Decimal::from_str(&format!("0.{}", percentage)).unwrap()
    };

    let industry = INDUSTRIES.choose(rng).unwrap();
    let asset_class = ASSET_CLASSES.choose(rng).unwrap();
    let region = REGIONS.choose(rng).unwrap();

    let description = format!(
        "{} deal in {} sector with {} risk profile. Principal: ${}, Rate: {}% APR",
        asset_class,
        industry,
        selected_risk,
        principal_amount,
        (interest_rate_bps as f64) / 100.0
    );

    // Generate collateral data (70% probability, matching database mode behavior)
    let (collateral_id, collateral_name, collateral_type, collateral_address, collateral_custom_id) =
        if rng.gen_bool(0.7) {
            // Generate collateral UUID and data
            let coll_id = Uuid::new_v4();
            let coll_name = COLLATERAL_NAMES.choose(rng).unwrap().to_string();
            let coll_type = COLLATERAL_TYPES.choose(rng).unwrap().to_string();
            let coll_address = COLLATERAL_ADDRESSES.choose(rng).unwrap().to_string();

            // Generate custom_id 50% of the time
            let coll_custom_id = if rng.gen_bool(0.5) {
                Some(format!(
                    "COLL-{}-{:04}",
                    chrono::Utc::now().format("%Y%m"),
                    rng.gen_range(1000..9999)
                ))
            } else {
                None
            };

            (
                Some(coll_id.to_string()),
                Some(coll_name),
                Some(coll_type),
                Some(coll_address),
                coll_custom_id,
            )
        } else {
            (None, None, None, None, None)
        };

    Ok(DealCsvRow {
        originator_name,
        investor_name,
        borrower_name: None,
        principal_amount: principal_amount.to_string(),
        outstanding_balance: outstanding_balance.to_string(),
        interest_rate_bps,
        deal_status: selected_status.to_string(),
        originated_at: originated_at.to_rfc3339(),
        maturity_date: Some(maturity_date.to_rfc3339()),
        status: "active".to_string(),
        industry: Some(industry.to_string()),
        title: Some(title_prefix.to_string()),
        description: Some(description),
        risk_level: Some(selected_risk.to_string()),
        asset_class: Some(asset_class.to_string()),
        region: Some(region.to_string()),
        minimum_investment: None,
        collateral_id,
        collateral_name,
        collateral_type,
        collateral_address,
        collateral_custom_id,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = Command::new("Deal Data Populator")
        .about("Generate realistic deal data for testing and refresh metrics cache")
        .arg(
            Arg::new("email")
                .long("email")
                .value_name("EMAIL")
                .help("User email for user-specific mode. If not provided, generates random deals")
                .required(false),
        )
        .arg(
            Arg::new("count")
                .long("count")
                .value_name("NUMBER")
                .help("Number of deals to generate")
                .default_value(DEFAULT_DEAL_COUNT_STR)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("clear")
                .long("clear")
                .help("Clear existing deal data before generating new data")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("csv")
                .long("csv")
                .help("Export to CSV instead of writing to database")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dp")
                .long("dp")
                .value_name("NUMBER")
                .help(
                    "Number of double-pledged deals to create (uses same collateral intentionally)",
                )
                .value_parser(clap::value_parser!(u32)),
        )
        .get_matches();

    let email = matches.get_one::<String>("email");
    let count = *matches.get_one::<u32>("count").unwrap();
    let clear_data = matches.get_flag("clear");
    let csv_mode = matches.get_flag("csv");
    let double_pledged_count = matches.get_one::<u32>("dp").copied();

    println!("🚀 Starting deal data population...");

    // CSV mode: Generate and export to CSV without database operations
    if csv_mode {
        println!("📊 CSV Export Mode: No database operations will be performed");

        let deals = match email {
            Some(user_email) => {
                println!("👤 User mode: Generating deals for {}", user_email);
                generate_user_deals_csv(user_email, count)?
            }
            None => {
                println!("🎲 Random mode: Generating {} random deals", count);
                generate_random_deals_csv(count)?
            }
        };

        let filename = write_deals_to_csv(deals)?;

        println!("🎉 CSV export completed successfully!");
        println!("\n📁 Output file: {}", filename);
        println!("\n💡 Next steps:");
        println!("   1. Upload this CSV to your frontend application");
        println!("   2. The frontend will map the data to your database");
        println!("   3. View the portfolio information in your profile");

        return Ok(());
    }

    // Database mode: Traditional behavior (create deals in database)
    // Get database URL from environment
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());

    // Initialize database connection
    let pool = init_db_with_url(&database_url).await?;
    println!("✅ Connected to database");

    // Clear existing data if requested
    if clear_data {
        println!("🧹 Clearing existing deal data...");
        clear_existing_data(&pool).await?;
    }

    // Determine mode and generate deals
    match email {
        Some(user_email) => {
            println!("👤 User mode: Generating deals for {}", user_email);
            if let Some(dp_count) = double_pledged_count {
                println!(
                    "🔗 Will create {} double-pledged deals for duplicate testing",
                    dp_count
                );
            }
            generate_user_deals(&pool, user_email, count, double_pledged_count).await?;
        }
        None => {
            println!("🎲 Random mode: Generating {} random deals", count);
            if let Some(dp_count) = double_pledged_count {
                println!(
                    "🔗 Will create {} double-pledged deals for duplicate testing",
                    dp_count
                );
            }
            generate_random_deals(&pool, count, double_pledged_count).await?;
        }
    }

    println!("🎉 Deal data population completed successfully!");
    println!("\n📊 Test your data:");
    println!("GET http://localhost:8080/api/v1/deals");
    println!("\n📈 Test your metrics:");
    println!("GET http://localhost:8080/api/v1/metrics/market-value?growth_type=market_growth&filter_by=all&time_range=all_time");
    println!("GET http://localhost:8080/api/v1/metrics/pie-charts?chart_type=industry&filter_by=all&time_range=all_time");
    if let Some(user_email) = email {
        println!("\n👤 User-specific deals:");
        println!(
            "GET http://localhost:8080/api/v1/deals?email={}",
            user_email
        );
    }

    Ok(())
}

/// Clear existing deal data from the database
async fn clear_existing_data(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // Delete in dependency order
    sqlx::query!("DELETE FROM deal_payments")
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM deals").execute(pool).await?;

    // Clear collateral data
    // Note: collateral_matches, collateral_check_queue, and collateral_checks tables
    // were removed in migration 060 - functionality moved to arda-collateral-intel service
    sqlx::query!("DELETE FROM collateral").execute(pool).await?;

    // Clear metrics data (must be done before deleting companies due to FK constraints)
    sqlx::query!("DELETE FROM metrics_market_value_daily")
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM metrics_pie_chart_data")
        .execute(pool)
        .await?;

    // Optionally clear test companies (keep real user companies)
    sqlx::query!("DELETE FROM company_memberships WHERE company_id IN (SELECT id FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%')")
        .execute(pool)
        .await?;
    sqlx::query!(
        "DELETE FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%'"
    )
    .execute(pool)
    .await?;

    println!("✅ Cleared existing deal, metrics, collateral, and test company data");
    Ok(())
}

/// Report on duplicate matches found
async fn report_duplicate_matches(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let result = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) as total_collateral,
            COUNT(DISTINCT double_pledged) FILTER (WHERE double_pledged IS NOT NULL) as unique_duplicates,
            COUNT(*) FILTER (WHERE double_pledged IS NOT NULL) as collateral_with_duplicates
        FROM collateral
        "#
    )
    .fetch_one(pool)
    .await?;

    println!("  📊 Duplicate Detection Summary:");
    println!(
        "     Total collateral: {}",
        result.total_collateral.unwrap_or(0)
    );
    println!(
        "     Collateral with duplicates: {}",
        result.collateral_with_duplicates.unwrap_or(0)
    );
    println!(
        "     Unique duplicate groups: {}",
        result.unique_duplicates.unwrap_or(0)
    );

    // Show some examples
    let examples = sqlx::query!(
        r#"
        SELECT 
            c1.name as original_name,
            c1.id as original_id,
            c2.name as duplicate_name,
            c2.id as duplicate_id
        FROM collateral c1
        INNER JOIN collateral c2 ON c1.double_pledged = c2.id
        LIMIT 5
        "#
    )
    .fetch_all(pool)
    .await?;

    if !examples.is_empty() {
        println!("  🔗 Example duplicate matches:");
        for ex in examples {
            println!(
                "     '{}' matched with '{}'",
                ex.original_name, ex.duplicate_name
            );
        }
    }

    Ok(())
}

/// Submit batch upload request to collateral intel service
async fn submit_batch_to_intel_service(
    deals_with_collateral: Vec<(Deal, Vec<Collateral>)>,
    intel_service_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if deals_with_collateral.is_empty() {
        println!("ℹ️  No deals with collateral to submit to intel service");
        return Ok(());
    }

    println!(
        "🚀 Submitting {} deals with collateral to intel service at {}",
        deals_with_collateral.len(),
        intel_service_url
    );

    // Build batch upload request
    let batch_request = BatchUploadRequest {
        deals: deals_with_collateral
            .into_iter()
            .map(|(deal, collaterals)| BatchDealPayload {
                deal: BatchDealInfo {
                    id: deal.id.to_string(),
                    status: deal.deal_status.clone(),
                    company_id: deal.originator_id.to_string(),
                },
                collaterals: collaterals
                    .into_iter()
                    .map(|c| BatchCollateralInfo {
                        id: c.id.to_string(),
                        name: c.name.clone(),
                        collateral_type: c.collateral_type.clone().unwrap_or_default(),
                        address: c.address.clone(),
                        custom_id: c.custom_id.clone(),
                        description: c.description.clone(),
                    })
                    .collect(),
            })
            .collect(),
    };

    // Submit to intel service
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!("{}/api/v1/batch/upload", intel_service_url);
    let response = client.post(&url).json(&batch_request).send().await?;

    if response.status().is_success() {
        let batch_response: BatchUploadResponse = response.json().await?;
        println!(
            "✅ Batch upload successful: {} deals, {} collaterals, {} tasks queued",
            batch_response.total_deals,
            batch_response.total_collaterals,
            batch_response.tasks_queued
        );
        println!("   {}", batch_response.message);
    } else {
        let status = response.status();
        let body = response.text().await?;
        eprintln!("❌ Batch upload failed with status {}: {}", status, body);
        return Err(format!("Batch upload failed: {} - {}", status, body).into());
    }

    Ok(())
}

/// Generate deals for a specific user
async fn generate_user_deals(
    pool: &sqlx::PgPool,
    email: &str,
    count: u32,
    double_pledged_count: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Look up user by email, create if doesn't exist
    let user = match get_identity_by_email(pool, email).await? {
        Some(user) => {
            println!("👤 Found existing user: {} ({})", user.email, user.id);
            user
        }
        None => {
            println!("👤 User {} not found, creating new user...", email);
            let user = create_identity(pool, "investor", email).await?;
            println!("✅ Created new user: {} ({})", user.email, user.id);

            // NEW: Record user registration transaction
            println!("  📝 Recording UserRegistration transaction");
            create_deal_transaction_record(
                pool,
                "user_registration",
                Uuid::nil(),
                Some(user.id),
                Decimal::ZERO,
                user.created_at,
                None,
            )
            .await?;

            user
        }
    };

    // Ensure user has companies
    let (investor_company, originator_company) = ensure_user_companies(pool, &user).await?;

    // Get or create additional random companies for pairing
    let random_companies = ensure_random_companies(pool, 5).await?;

    // Track if we should create double-pledged deals
    let mut rng = thread_rng();
    let mut dp_remaining = double_pledged_count.unwrap_or(0);
    let mut shared_collateral: Option<Collateral> = None;
    let dp_collateral_index = rng.gen_range(0..COLLATERAL_NAMES.len());

    // Track all deals and collateral for batch submission
    let mut all_deals_with_collateral: Vec<(Deal, Vec<Collateral>)> = Vec::new();

    // Generate deals with 50/50 split
    let investor_deals = count / 2;
    let originator_deals = count - investor_deals;

    println!("💰 Creating {} deals as investor", investor_deals);
    for i in 0..investor_deals {
        let originator = random_companies
            .choose(&mut rng)
            .unwrap_or(&originator_company);

        // Create collateral for this deal
        let collateral = if dp_remaining > 0 {
            // Create intentional duplicate
            if shared_collateral.is_none() {
                // First double-pledged deal - create the shared collateral
                let coll =
                    create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                        .await?;
                println!(
                    "  🔗 Created shared collateral {} for {} double-pledged deals",
                    coll.id,
                    double_pledged_count.unwrap()
                );
                shared_collateral = Some(coll.clone());
                Some(coll)
            } else {
                dp_remaining -= 1;

                if dp_remaining > 0 {
                    // Create another collateral with same attributes (should be detected as duplicate)
                    Some(
                        create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                            .await?,
                    )
                } else {
                    // Last double-pledged deal uses the original
                    shared_collateral.clone()
                }
            }
        } else {
            // Random collateral (70% of deals have collateral)
            if rng.gen_bool(0.7) {
                Some(create_random_collateral(pool, &mut rng, false, None).await?)
            } else {
                None
            }
        };

        let (deal, collateral_opt) = create_realistic_deal(
            pool,
            originator.id,
            Some(investor_company.id),
            &format!("User Investment Deal #{}", i + 1),
            collateral.clone(),
        )
        .await?;

        // Track deal with its collateral for batch submission
        if let Some(coll) = collateral_opt {
            all_deals_with_collateral.push((deal, vec![coll]));
        }
    }

    println!("🏢 Creating {} deals as originator", originator_deals);
    for i in 0..originator_deals {
        let investor = random_companies
            .choose(&mut rng)
            .unwrap_or(&investor_company);

        // Create collateral for this deal
        let collateral = if dp_remaining > 0 {
            // Create intentional duplicate
            if shared_collateral.is_none() {
                // First double-pledged deal - create the shared collateral
                let coll =
                    create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                        .await?;
                println!(
                    "  🔗 Created shared collateral {} for {} double-pledged deals",
                    coll.id,
                    double_pledged_count.unwrap()
                );
                shared_collateral = Some(coll.clone());
                Some(coll)
            } else {
                dp_remaining -= 1;

                if dp_remaining > 0 {
                    // Create another collateral with same attributes (should be detected as duplicate)
                    Some(
                        create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                            .await?,
                    )
                } else {
                    // Last double-pledged deal uses the original
                    shared_collateral.clone()
                }
            }
        } else {
            // Random collateral (70% of deals have collateral)
            if rng.gen_bool(0.7) {
                Some(create_random_collateral(pool, &mut rng, false, None).await?)
            } else {
                None
            }
        };

        let (deal, collateral_opt) = create_realistic_deal(
            pool,
            originator_company.id,
            Some(investor.id),
            &format!("User Origination Deal #{}", i + 1),
            collateral.clone(),
        )
        .await?;

        // Track deal with its collateral for batch submission
        if let Some(coll) = collateral_opt {
            all_deals_with_collateral.push((deal, vec![coll]));
        }
    }

    println!("✅ Created {} deals for user {}", count, email);
    println!("   📊 Collateral breakdown:");
    println!(
        "      • {} deals WITH collateral ({}%)",
        all_deals_with_collateral.len(),
        (all_deals_with_collateral.len() as f64 / count as f64 * 100.0).round()
    );
    println!(
        "      • {} deals WITHOUT collateral ({}%)",
        count as usize - all_deals_with_collateral.len(),
        ((count as usize - all_deals_with_collateral.len()) as f64 / count as f64 * 100.0).round()
    );

    // Submit batch to collateral intel service if configured
    if let Ok(intel_url) = env::var("COLLATERAL_INTEL_SERVICE_URL") {
        if !all_deals_with_collateral.is_empty() {
            println!(
                "ℹ️  Note: Only deals with collateral are submitted to intel service for analysis"
            );
            println!(
                "🔗 Submitting {} of {} deals to intel service (only deals with collateral)...",
                all_deals_with_collateral.len(),
                count
            );
            if let Err(e) =
                submit_batch_to_intel_service(all_deals_with_collateral, &intel_url).await
            {
                eprintln!(
                    "⚠️  Warning: Failed to submit batch to intel service: {}",
                    e
                );
                eprintln!("   Continuing without intel service submission...");
            }
        } else {
            println!("ℹ️  No deals with collateral - skipping intel service submission");
        }
    } else {
        println!("ℹ️  COLLATERAL_INTEL_SERVICE_URL not set, skipping intel service submission");
    }

    if double_pledged_count.is_some() && double_pledged_count.unwrap() > 0 {
        println!("🔍 Waiting 5 seconds for duplicate detection to process...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        // Report on duplicate detection
        println!("📊 Checking duplicate detection results...");
        report_duplicate_matches(pool).await?;
    }

    // Refresh metrics after creating deals
    println!("📊 Refreshing metrics data...");
    refresh_metrics_data(pool).await?;
    println!("✅ Metrics data refreshed successfully");

    Ok(())
}

/// Generate random deals with random companies
async fn generate_random_deals(
    pool: &sqlx::PgPool,
    count: u32,
    double_pledged_count: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = thread_rng();

    // Create random companies for originators and investors
    let companies = ensure_random_companies(
        pool,
        (count / COMPANY_COUNT_DIVISOR).max(MIN_RANDOM_COMPANIES as u32) as usize,
    )
    .await?;

    println!("💰 Generating {} random deals with collateral", count);

    // Track if we should create double-pledged deals
    let mut dp_remaining = double_pledged_count.unwrap_or(0);
    let mut shared_collateral: Option<Collateral> = None;
    let dp_collateral_index = rng.gen_range(0..COLLATERAL_NAMES.len());

    // Track all deals and collateral for batch submission
    let mut all_deals_with_collateral: Vec<(Deal, Vec<Collateral>)> = Vec::new();

    for i in 0..count {
        let originator = companies.choose(&mut rng).unwrap();
        let investor_candidates: Vec<&Company> =
            companies.iter().filter(|c| c.id != originator.id).collect();
        let investor = investor_candidates.choose(&mut rng).unwrap();

        // Create collateral for this deal
        let collateral = if dp_remaining > 0 {
            // Create intentional duplicate
            if shared_collateral.is_none() {
                // First double-pledged deal - create the shared collateral
                let coll =
                    create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                        .await?;
                println!(
                    "  🔗 Created shared collateral {} for {} double-pledged deals",
                    coll.id,
                    double_pledged_count.unwrap()
                );
                shared_collateral = Some(coll.clone());
                Some(coll)
            } else {
                dp_remaining -= 1;

                if dp_remaining > 0 {
                    // Create another collateral with same attributes (should be detected as duplicate)
                    Some(
                        create_random_collateral(pool, &mut rng, true, Some(dp_collateral_index))
                            .await?,
                    )
                } else {
                    // Last double-pledged deal uses the original
                    shared_collateral.clone()
                }
            }
        } else {
            // Random collateral (70% of deals have collateral)
            if rng.gen_bool(0.7) {
                Some(create_random_collateral(pool, &mut rng, false, None).await?)
            } else {
                None
            }
        };

        let (deal, collateral_opt) = create_realistic_deal(
            pool,
            originator.id,
            Some(investor.id),
            &format!("Random Deal #{}", i + 1),
            collateral.clone(),
        )
        .await?;

        // Track deal with its collateral for batch submission
        if let Some(coll) = collateral_opt {
            all_deals_with_collateral.push((deal, vec![coll]));
        }
    }

    println!("✅ Generated {} random deals", count);
    println!("   📊 Collateral breakdown:");
    println!(
        "      • {} deals WITH collateral ({}%)",
        all_deals_with_collateral.len(),
        (all_deals_with_collateral.len() as f64 / count as f64 * 100.0).round()
    );
    println!(
        "      • {} deals WITHOUT collateral ({}%)",
        count as usize - all_deals_with_collateral.len(),
        ((count as usize - all_deals_with_collateral.len()) as f64 / count as f64 * 100.0).round()
    );

    // Submit batch to collateral intel service if configured
    if let Ok(intel_url) = env::var("COLLATERAL_INTEL_SERVICE_URL") {
        if !all_deals_with_collateral.is_empty() {
            println!(
                "ℹ️  Note: Only deals with collateral are submitted to intel service for analysis"
            );
            println!(
                "🔗 Submitting {} of {} deals to intel service (only deals with collateral)...",
                all_deals_with_collateral.len(),
                count
            );
            if let Err(e) =
                submit_batch_to_intel_service(all_deals_with_collateral, &intel_url).await
            {
                eprintln!(
                    "⚠️  Warning: Failed to submit batch to intel service: {}",
                    e
                );
                eprintln!("   Continuing without intel service submission...");
            }
        } else {
            println!("ℹ️  No deals with collateral - skipping intel service submission");
        }
    } else {
        println!("ℹ️  COLLATERAL_INTEL_SERVICE_URL not set, skipping intel service submission");
    }

    if double_pledged_count.is_some() && double_pledged_count.unwrap() > 0 {
        println!("🔍 Waiting 5 seconds for duplicate detection to process...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        // Report on duplicate detection
        println!("📊 Checking duplicate detection results...");
        report_duplicate_matches(pool).await?;
    }

    // Refresh metrics after creating deals
    println!("📊 Refreshing metrics data...");
    refresh_metrics_data(pool).await?;
    println!("✅ Metrics data refreshed successfully");

    Ok(())
}

/// Ensure user has both investor and originator companies
async fn ensure_user_companies(
    pool: &sqlx::PgPool,
    user: &User,
) -> Result<(Company, Company), Box<dyn std::error::Error>> {
    let user_name = user.email.split('@').next().unwrap_or("User");

    // Create investor company
    let investor_company_name = format!("{} Investor Corp", user_name);
    let investor_request = CreateCompanyRequest {
        name: investor_company_name.clone(),
        description: Some(format!("Investment company for {}", user.email)),
        metadata: Some(json!({"created_by_script": true, "user_email": user.email})),
    };

    let investor_company = create_company(pool, &investor_request).await?;

    // Create company membership for investor
    create_membership(
        pool,
        investor_company.id,
        user.id,
        CompanyRole::Admin,
        ProfileType::Investor,
        None, // invited_by
        None, // permissions (will use defaults)
    )
    .await?;

    // NEW: Record user registration transaction for zkVM
    // Check if user was recently created (within last minute) to avoid duplicates
    let user_age = (Utc::now() - user.created_at).num_seconds();
    if user_age < 60 {
        println!(
            "  📝 Recording UserRegistration transaction for {}",
            user.email
        );
        create_deal_transaction_record(
            pool,
            "user_registration",
            Uuid::nil(), // No deal_id for user registration
            Some(user.id),
            Decimal::ZERO,
            user.created_at, // Use user's actual creation time
            None,
        )
        .await?;
    }

    // Create originator company
    let originator_company_name = format!("{} Originator Corp", user_name);
    let originator_request = CreateCompanyRequest {
        name: originator_company_name.clone(),
        description: Some(format!("Origination company for {}", user.email)),
        metadata: Some(json!({"created_by_script": true, "user_email": user.email})),
    };

    let originator_company = create_company(pool, &originator_request).await?;

    // Create company membership for originator
    create_membership(
        pool,
        originator_company.id,
        user.id,
        CompanyRole::Admin,
        ProfileType::Originator,
        None, // invited_by
        None, // permissions (will use defaults)
    )
    .await?;

    println!(
        "🏢 Created companies: {} and {}",
        investor_company_name, originator_company_name
    );

    Ok((investor_company, originator_company))
}

/// Ensure we have enough random companies for deal generation
async fn ensure_random_companies(
    pool: &sqlx::PgPool,
    min_count: usize,
) -> Result<Vec<Company>, Box<dyn std::error::Error>> {
    let mut companies = Vec::new();

    // Use predefined company types and prefixes

    for i in 0..min_count {
        let prefix = COMPANY_PREFIXES.choose(&mut thread_rng()).unwrap();
        let suffix = COMPANY_TYPES.choose(&mut thread_rng()).unwrap();
        let company_name = format!("{} {} #{}", prefix, suffix, i + 1);

        let request = CreateCompanyRequest {
            name: company_name.clone(),
            description: Some(format!("Test company created for deal generation")),
            metadata: Some(json!({"created_by_script": true, "type": "random"})),
        };

        let company = create_company(pool, &request).await?;
        companies.push(company);
    }

    println!("🏢 Created {} random companies", companies.len());
    Ok(companies)
}

/// Create a collateral asset with randomized data
async fn create_random_collateral(
    pool: &sqlx::PgPool,
    rng: &mut impl Rng,
    force_duplicate: bool,
    duplicate_index: Option<usize>,
) -> Result<Collateral, Box<dyn std::error::Error>> {
    use db::{create_collateral, CreateCollateralRequest};

    let (name, collateral_type, address) = if force_duplicate && duplicate_index.is_some() {
        // Use fixed index for intentional duplicates
        let idx = duplicate_index.unwrap();
        (
            COLLATERAL_NAMES[idx].to_string(),
            Some(COLLATERAL_TYPES[idx].to_string()),
            Some(COLLATERAL_ADDRESSES[idx].to_string()),
        )
    } else {
        // Random selection
        (
            COLLATERAL_NAMES.choose(rng).unwrap().to_string(),
            Some(COLLATERAL_TYPES.choose(rng).unwrap().to_string()),
            Some(COLLATERAL_ADDRESSES.choose(rng).unwrap().to_string()),
        )
    };

    let description = Some(format!(
        "Collateral asset of type {} located at {}",
        collateral_type.as_ref().unwrap(),
        address.as_ref().unwrap()
    ));

    // Generate custom_id 50% of the time
    let custom_id = if rng.gen_bool(0.5) {
        Some(format!(
            "COLL-{}-{:04}",
            chrono::Utc::now().format("%Y%m"),
            rng.gen_range(1000..9999)
        ))
    } else {
        None
    };

    let request = CreateCollateralRequest {
        name,
        collateral_type,
        description,
        address,
        custom_id,
        metadata: Some(json!({
            "created_by_script": true,
            "is_duplicate_test": force_duplicate
        })),
        created_by: None,
    };

    let collateral = create_collateral(pool, request).await?;

    // NEW: Record collateral creation transaction for zkVM
    println!(
        "  📝 Recording CollateralCreation transaction for {}",
        collateral.id
    );
    db::record_collateral_transaction(
        pool,
        collateral.id,
        "collateral_creation",
        None, // No specific user for script-generated collateral
        None, // No deal association yet
        Some(serde_json::json!({"created_by_script": true})),
    )
    .await?;

    Ok(collateral)
}

/// Create a realistic deal with randomized but reasonable data
/// Returns the deal and optionally the collateral if one was provided
async fn create_realistic_deal(
    pool: &sqlx::PgPool,
    originator_id: Uuid,
    investor_id: Option<Uuid>,
    title_prefix: &str,
    collateral: Option<Collateral>,
) -> Result<(Deal, Option<Collateral>), Box<dyn std::error::Error>> {
    let mut rng = thread_rng();

    // Realistic principal amounts
    let principal_amount =
        Decimal::from(rng.gen_range(MIN_PRINCIPAL_AMOUNT..=MAX_PRINCIPAL_AMOUNT));

    // Interest rates
    let interest_rate_bps = rng.gen_range(MIN_INTEREST_RATE_BPS..=MAX_INTEREST_RATE_BPS);

    // Deal statuses with realistic distribution
    let deal_statuses = [
        ("active", DEAL_STATUS_ACTIVE_WEIGHT),
        ("originated", DEAL_STATUS_ORIGINATED_WEIGHT),
        ("paid_off", DEAL_STATUS_PAID_OFF_WEIGHT),
        ("closed", DEAL_STATUS_CLOSED_WEIGHT),
    ];

    let total_weight: u32 = deal_statuses.iter().map(|(_, weight)| weight).sum();
    let random_weight = rng.gen_range(0..total_weight);
    let mut cumulative_weight = 0;
    let mut selected_status = "active";

    for (status, weight) in &deal_statuses {
        cumulative_weight += weight;
        if random_weight < cumulative_weight {
            selected_status = status;
            break;
        }
    }

    // Use predefined arrays from constants

    // Risk levels distribution
    let risk_levels = [
        ("low", RISK_LEVEL_LOW_WEIGHT),
        ("medium", RISK_LEVEL_MEDIUM_WEIGHT),
        ("high", RISK_LEVEL_HIGH_WEIGHT),
    ];
    let risk_total: u32 = risk_levels.iter().map(|(_, w)| w).sum();
    let risk_rand = rng.gen_range(0..risk_total);
    let mut risk_cumulative = 0;
    let mut selected_risk = "medium";

    for (risk, weight) in &risk_levels {
        risk_cumulative += weight;
        if risk_rand < risk_cumulative {
            selected_risk = risk;
            break;
        }
    }

    // Calculate maturity date
    let maturity_months = rng.gen_range(MIN_MATURITY_MONTHS..=MAX_MATURITY_MONTHS);
    let originated_at = Utc::now() - Duration::days(rng.gen_range(0..=MAX_DAYS_AGO_ORIGINATED));
    let maturity_date = Some(originated_at + Duration::days(maturity_months * 30));

    // Outstanding balance (percentage of principal for active deals, 0 for paid_off/closed)
    let outstanding_balance = if selected_status == "paid_off" || selected_status == "closed" {
        Decimal::ZERO
    } else {
        let percentage = rng.gen_range(MIN_OUTSTANDING_BALANCE_PCT..=MAX_OUTSTANDING_BALANCE_PCT);
        principal_amount * Decimal::from_str(&format!("0.{}", percentage)).unwrap()
    };

    // Create the deal with custom timestamps
    let deal = create_deal_with_custom_dates(
        pool,
        originator_id,
        investor_id,
        principal_amount,
        interest_rate_bps,
        maturity_date,
        Some(selected_status),
        None,           // custom_deal_id
        Some("active"), // status (negotiation status)
        Some(INDUSTRIES.choose(&mut rng).unwrap()),
        Some(&json!([])),   // document_ids
        Some(title_prefix), // title
        Some(&format!(
            "{} deal in {} sector with {} risk profile. Principal: ${}, Rate: {}% APR",
            ASSET_CLASSES.choose(&mut rng).unwrap(),
            INDUSTRIES.choose(&mut rng).unwrap(),
            selected_risk,
            principal_amount,
            (interest_rate_bps as f64) / 100.0
        )),
        Some(selected_risk),
        Some(ASSET_CLASSES.choose(&mut rng).unwrap()),
        Some(REGIONS.choose(&mut rng).unwrap()),
        None,                              // minimum_investment
        originated_at,                     // custom originated_at
        originated_at,                     // set created_at to same as originated_at
        collateral.as_ref().map(|c| c.id), // Collateral UUID (optional)
    )
    .await?;

    // NEW: If collateral was associated, record transaction
    if let Some(coll) = &collateral {
        println!("  📝 Recording CollateralDealAssociation transaction");
        db::record_collateral_transaction(
            pool,
            coll.id,
            "collateral_deal_association",
            None,          // No specific user for script-generated deals
            Some(deal.id), // Associated with this deal
            Some(serde_json::json!({
                "created_by_script": true,
                "deal_id": deal.id,
            })),
        )
        .await?;
    }

    // Update outstanding balance if different from principal
    if outstanding_balance != principal_amount {
        sqlx::query!(
            "UPDATE deals SET outstanding_balance = $1 WHERE id = $2",
            outstanding_balance,
            deal.id
        )
        .execute(pool)
        .await?;
    }

    // Create state progression history (transaction records showing how deal reached current state)
    create_deal_state_progression(pool, &deal, selected_status, &mut rng).await?;

    let collateral_msg = if collateral.is_some() {
        " with collateral"
    } else {
        ""
    };

    println!(
        "  💰 Created deal: ${} at {}% APR ({}{})",
        principal_amount,
        (interest_rate_bps as f64) / 100.0,
        selected_status,
        collateral_msg
    );

    Ok((deal, collateral))
}
