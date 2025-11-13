#!/usr/bin/env cargo run --bin populate_deal_data --
//! Deal data population script for the Arda Credit database
//!
//! This script creates realistic deal data for testing the deals API endpoint.
//! It supports two modes:
//! - Random mode: Generate random deals with random companies (no --email)
//! - User mode: Generate deals for a specific user's companies (with --email)
//!
//! Run with: cargo run --bin populate_deal_data

use chrono::{Duration, Utc};
use clap::{Arg, Command};
use db::{
    business::company::{
        create_company, create_membership, Company, CompanyRole, CreateCompanyRequest, ProfileType,
    },
    create_deal, create_identity, get_identity_by_email, init_db_with_url, Deal, User,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use rust_decimal::Decimal;
use serde_json::json;
use std::{env, str::FromStr};
use uuid::Uuid;

// ============================================================================
// CONFIGURATION CONSTANTS
// ============================================================================

// Default values
const DEFAULT_DEAL_COUNT_STR: &str = "25";
const DEFAULT_DATABASE_URL: &str = "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db";

// Deal generation parameters
const MIN_PRINCIPAL_AMOUNT: u32 = 10_000;
const MAX_PRINCIPAL_AMOUNT: u32 = 500_000;
const MIN_INTEREST_RATE_BPS: i32 = 400; // 4% APR
const MAX_INTEREST_RATE_BPS: i32 = 1200; // 12% APR
const MIN_OUTSTANDING_BALANCE_PCT: u32 = 60; // 60% of principal
const MAX_OUTSTANDING_BALANCE_PCT: u32 = 95; // 95% of principal
const MIN_MATURITY_MONTHS: i64 = 12; // 1 year
const MAX_MATURITY_MONTHS: i64 = 60; // 5 years
const MAX_DAYS_AGO_ORIGINATED: i64 = 90; // 0-3 months ago

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = Command::new("Deal Data Populator")
        .about("Generate realistic deal data for testing")
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
        .get_matches();

    let email = matches.get_one::<String>("email");
    let count = *matches.get_one::<u32>("count").unwrap();
    let clear_data = matches.get_flag("clear");

    println!("🚀 Starting deal data population...");

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
    if let Some(user_email) = email {
        println!("👤 User-specific deals:");
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

    // Optionally clear test companies (keep real user companies)
    sqlx::query!("DELETE FROM company_memberships WHERE company_id IN (SELECT id FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%')")
        .execute(pool)
        .await?;
    sqlx::query!(
        "DELETE FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%'"
    )
    .execute(pool)
    .await?;

    println!("✅ Cleared existing deal and test company data");
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

    // Create the deal
    let deal = create_deal(
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
        None, // minimum_investment
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
