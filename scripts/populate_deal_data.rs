#!/usr/bin/env cargo run --bin populate_deal_data --
//! Deal data population script for the Arda Credit database
//!
//! This script creates realistic deal data for testing the deals API endpoint
//! and automatically refreshes the metrics cache for dashboard visualization.
//! It supports two modes:
//! - Random mode: Generate random deals with random companies (no --email)
//! - User mode: Generate deals for a specific user's companies (with --email)
//!
//! After generating deals, the script automatically calculates and caches:
//! - Market value time series (both market growth and Arda growth)
//! - Pie chart distributions (industry, region, asset class breakdowns)
//!
//! Run with: cargo run --bin populate_deal_data

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
    create_identity, get_identity_by_email, init_db_with_url, list_deals, Deal, User,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::json;
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
) -> Result<Deal, sqlx::Error> {
    let id = Uuid::new_v4();
    let default_json = serde_json::Value::Array(vec![]);
    let doc_ids = document_ids.unwrap_or(&default_json);

    sqlx::query!(
        r#"
        INSERT INTO deals (
            id, originator_id, investor_id, principal_amount, outstanding_balance, interest_rate_bps, deal_status, maturity_date, originated_at, created_at, updated_at,
            custom_deal_id, status, industry, document_ids,
            title, description, risk_level, asset_class, region, minimum_investment
        )
        VALUES (
            $1, $2, $3, $4, $4, $5, $6, $7, $8, $9, $9,
            $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19
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
        minimum_investment
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
    };

    Ok(deal)
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
        .get_matches();

    let email = matches.get_one::<String>("email");
    let count = *matches.get_one::<u32>("count").unwrap();
    let clear_data = matches.get_flag("clear");
    let csv_mode = matches.get_flag("csv");

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
            generate_user_deals(&pool, user_email, count).await?;
        }
        None => {
            println!("🎲 Random mode: Generating {} random deals", count);
            generate_random_deals(&pool, count).await?;
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

    println!("✅ Cleared existing deal, metrics, and test company data");
    Ok(())
}

/// Generate deals for a specific user
async fn generate_user_deals(
    pool: &sqlx::PgPool,
    email: &str,
    count: u32,
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
            user
        }
    };

    // Ensure user has companies
    let (investor_company, originator_company) = ensure_user_companies(pool, &user).await?;

    // Get or create additional random companies for pairing
    let random_companies = ensure_random_companies(pool, 5).await?;

    // Generate deals with 50/50 split
    let investor_deals = count / 2;
    let originator_deals = count - investor_deals;

    println!("💰 Creating {} deals as investor", investor_deals);
    for i in 0..investor_deals {
        let originator = random_companies
            .choose(&mut thread_rng())
            .unwrap_or(&originator_company);

        create_realistic_deal(
            pool,
            originator.id,
            Some(investor_company.id),
            &format!("User Investment Deal #{}", i + 1),
        )
        .await?;
    }

    println!("🏢 Creating {} deals as originator", originator_deals);
    for i in 0..originator_deals {
        let investor = random_companies
            .choose(&mut thread_rng())
            .unwrap_or(&investor_company);

        create_realistic_deal(
            pool,
            originator_company.id,
            Some(investor.id),
            &format!("User Origination Deal #{}", i + 1),
        )
        .await?;
    }

    println!("✅ Created {} deals for user {}", count, email);

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
) -> Result<(), Box<dyn std::error::Error>> {
    // Create random companies for originators and investors
    let companies = ensure_random_companies(
        pool,
        (count / COMPANY_COUNT_DIVISOR).max(MIN_RANDOM_COMPANIES as u32) as usize,
    )
    .await?;

    println!("💰 Generating {} random deals", count);
    for i in 0..count {
        let originator = companies.choose(&mut thread_rng()).unwrap();
        let investor_candidates: Vec<&Company> =
            companies.iter().filter(|c| c.id != originator.id).collect();
        let investor = investor_candidates.choose(&mut thread_rng()).unwrap();

        create_realistic_deal(
            pool,
            originator.id,
            Some(investor.id),
            &format!("Random Deal #{}", i + 1),
        )
        .await?;
    }

    println!("✅ Generated {} random deals", count);

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

/// Create a realistic deal with randomized but reasonable data
async fn create_realistic_deal(
    pool: &sqlx::PgPool,
    originator_id: Uuid,
    investor_id: Option<Uuid>,
    title_prefix: &str,
) -> Result<Deal, Box<dyn std::error::Error>> {
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
        None,          // minimum_investment
        originated_at, // custom originated_at
        originated_at, // set created_at to same as originated_at
    )
    .await?;

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

    println!(
        "  💰 Created deal: ${} at {}% APR ({})",
        principal_amount,
        (interest_rate_bps as f64) / 100.0,
        selected_status
    );

    Ok(deal)
}
