//! Test script specifically for collateral duplicate detection
//!
//! Usage:
//!   cargo run --bin test_collateral_duplicates -- --scenarios all
//!   cargo run --bin test_collateral_duplicates -- --scenarios exact_name
//!   cargo run --bin test_collateral_duplicates -- --scenarios fuzzy_name

use clap::{Arg, Command};
use db::{create_collateral, get_collateral_matches, init_db_with_url, CreateCollateralRequest};
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Collateral Duplicate Detection Test")
        .about("Test various duplicate detection scenarios")
        .arg(
            Arg::new("scenarios")
                .long("scenarios")
                .value_name("SCENARIO")
                .help("Test scenario: exact_name, custom_id, address, fuzzy_name, or all")
                .default_value("all"),
        )
        .get_matches();

    let scenario = matches.get_one::<String>("scenarios").unwrap();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db".to_string()
    });

    let pool = init_db_with_url(&database_url).await?;

    println!("🧪 Testing Collateral Duplicate Detection");
    println!("==========================================\n");

    match scenario.as_str() {
        "exact_name" => test_exact_name_match(&pool).await?,
        "custom_id" => test_custom_id_match(&pool).await?,
        "address" => test_address_match(&pool).await?,
        "fuzzy_name" => test_fuzzy_name_match(&pool).await?,
        "all" => {
            test_exact_name_match(&pool).await?;
            test_custom_id_match(&pool).await?;
            test_address_match(&pool).await?;
            test_fuzzy_name_match(&pool).await?;
        }
        _ => println!("❌ Unknown scenario: {}", scenario),
    }

    Ok(())
}

async fn test_exact_name_match(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("📝 Test 1: Exact Name Match");
    println!("---------------------------");

    let req1 = CreateCollateralRequest {
        name: "Test Property Alpha".to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: Some("First instance".to_string()),
        address: Some("123 Test St".to_string()),
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c1 = create_collateral(pool, req1).await?;
    println!("  ✅ Created collateral 1: {} ({})", c1.name, c1.id);

    // Wait a moment
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let req2 = CreateCollateralRequest {
        name: "Test Property Alpha".to_string(), // EXACT MATCH
        collateral_type: Some("real_estate".to_string()),
        description: Some("Second instance".to_string()),
        address: Some("456 Different St".to_string()),
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c2 = create_collateral(pool, req2).await?;
    println!("  ✅ Created collateral 2: {} ({})", c2.name, c2.id);

    // Wait for background processing
    println!("  ⏳ Waiting for duplicate detection (5 seconds)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Check results
    check_duplicate_detection(pool, c2.id, "exact name match").await?;

    println!();
    Ok(())
}

async fn test_custom_id_match(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 Test 2: Custom ID Match");
    println!("---------------------------");

    let custom_id = format!("PROP-TEST-{}", Uuid::new_v4());

    let req1 = CreateCollateralRequest {
        name: "Different Name One".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: None,
        address: None,
        custom_id: Some(custom_id.clone()),
        metadata: None,
        created_by: None,
    };

    let c1 = create_collateral(pool, req1).await?;
    println!("  ✅ Created collateral 1: custom_id={}", custom_id);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let req2 = CreateCollateralRequest {
        name: "Different Name Two".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: None,
        address: None,
        custom_id: Some(custom_id.clone()), // SAME CUSTOM ID
        metadata: None,
        created_by: None,
    };

    let c2 = create_collateral(pool, req2).await?;
    println!("  ✅ Created collateral 2: custom_id={}", custom_id);

    println!("  ⏳ Waiting for duplicate detection (5 seconds)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    check_duplicate_detection(pool, c2.id, "custom ID match").await?;

    println!();
    Ok(())
}

async fn test_address_match(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("📍 Test 3: Address Match");
    println!("-------------------------");

    let address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb1";

    let req1 = CreateCollateralRequest {
        name: "Property One".to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: None,
        address: Some(address.to_string()),
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c1 = create_collateral(pool, req1).await?;
    println!("  ✅ Created collateral 1: address={}", address);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let req2 = CreateCollateralRequest {
        name: "Property Two".to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: None,
        address: Some(address.to_string()), // SAME ADDRESS
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c2 = create_collateral(pool, req2).await?;
    println!("  ✅ Created collateral 2: address={}", address);

    println!("  ⏳ Waiting for duplicate detection (5 seconds)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    check_duplicate_detection(pool, c2.id, "address match").await?;

    println!();
    Ok(())
}

async fn test_fuzzy_name_match(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Test 4: Fuzzy Name Match");
    println!("---------------------------");

    let req1 = CreateCollateralRequest {
        name: "Boeing 737-800 Aircraft".to_string(),
        collateral_type: Some("aircraft".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c1 = create_collateral(pool, req1).await?;
    println!("  ✅ Created collateral 1: 'Boeing 737-800 Aircraft'");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let req2 = CreateCollateralRequest {
        name: "Boeing 737 Aircraft Model 800".to_string(), // SIMILAR BUT NOT EXACT
        collateral_type: Some("aircraft".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let c2 = create_collateral(pool, req2).await?;
    println!("  ✅ Created collateral 2: 'Boeing 737 Aircraft Model 800'");

    println!("  ⏳ Waiting for duplicate detection (5 seconds)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    check_duplicate_detection(pool, c2.id, "fuzzy name match").await?;

    println!();
    Ok(())
}

async fn check_duplicate_detection(
    pool: &sqlx::PgPool,
    collateral_id: Uuid,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let matches = get_collateral_matches(pool, collateral_id).await?;

    if matches.is_empty() {
        println!("  ❌ FAIL: No duplicates detected for {}", test_name);
    } else {
        println!(
            "  ✅ PASS: Detected {} duplicate(s) for {}",
            matches.len(),
            test_name
        );
        for m in &matches {
            println!(
                "     - Match: '{}' (score: {}, type: {})",
                m.matched_collateral.name, m.match_score, m.match_type
            );
        }
    }

    Ok(())
}
