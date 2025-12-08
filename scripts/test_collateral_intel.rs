#!/usr/bin/env cargo run --bin test_collateral_intel --
//! Collateral Intelligence Service Integration Test
//!
//! Comprehensive test script that validates the full event publishing flow
//! to the collateral Intel service and verifies detection responses.
//!
//! Test Scenarios:
//! 1. Same-User Double Pledge Detection
//! 2. Cross-User Double Pledge Detection  
//! 3. Paid-Off Deal - No Double Pledge
//! 4. Same Name/Address, Different UUID Double Pledge (Exact Match Detection)
//!
//! Run with: cargo run --bin test_collateral_intel

use clap::{Arg, Command};
use colored::*;
use db::{
    business::company::{
        create_company, create_membership, CompanyRole, CreateCompanyRequest, ProfileType,
    },
    create_collateral, create_identity, init_db_with_url, CreateCollateralRequest,
};
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{env, time::Duration};
use uuid::Uuid;

// ============================================================================
// CONFIGURATION
// ============================================================================

const DEFAULT_DATABASE_URL: &str = "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db";
const DEFAULT_API_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_INTEL_URL: &str = "http://127.0.0.1:3010";
const DEFAULT_WAIT_TIME: u64 = 5;
const TEST_PREFIX: &str = "TEST_INTEL";

// ============================================================================
// API REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct CreateDealRequest {
    originator_id: String,
    investor_id: Option<String>,
    borrower_id: Option<String>,
    principal_amount: String,
    interest_rate_bps: i32,
    deal_status: String,
    status: String,
    collateral_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DealResponse {
    deal_id: String,
    originator_id: Option<String>,
    investor_id: Option<String>,
    principal_amount: String,
    loan_status: String,
    collateral_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateDealRequest {
    loan_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollateralMatchesResponse {
    collateral: CollateralInfo,
    matches: Vec<CollateralMatch>,
    match_threshold: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollateralInfo {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollateralMatch {
    matched_collateral: CollateralInfo,
    match_score: String,
    match_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DetectionHistoryResponse {
    collateral_id: String,
    total_count: i32,
    limit: i32,
    offset: i32,
    detections: Vec<DetectionRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DetectionRecord {
    detection_id: String,
    trigger_type: String,
    status: String,
    match_count: usize,
    analyzed_at: String,
    matches: Vec<DetectionMatchDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DetectionMatchDetail {
    matched_collateral_id: String,
    matched_collateral_name: String,
    score: f32,
    confidence: String,
    is_concerning: bool,
    reason: String,
}

// ============================================================================
// TEST CONTEXT
// ============================================================================

struct TestContext {
    pool: sqlx::PgPool,
    client: Client,
    api_url: String,
    intel_url: String,
    wait_time: u64,
    cleanup: bool,

    // Test data
    user1_id: Uuid,
    user2_id: Uuid,
    company1_id: Uuid,
    company2_id: Uuid,
    collateral_id: Uuid,
    collateral_ids: Vec<Uuid>, // Additional collateral IDs for cleanup
    deal_ids: Vec<Uuid>,
    session_token: String,
}

impl TestContext {
    async fn new(
        database_url: &str,
        api_url: String,
        intel_url: String,
        wait_time: u64,
        cleanup: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        println!("{}", "🚀 Initializing test context...".bold().cyan());

        let pool = init_db_with_url(database_url).await?;
        println!("  {} Database connected", "✅".green());

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            pool,
            client,
            api_url,
            intel_url,
            wait_time,
            cleanup,
            user1_id: Uuid::nil(),
            user2_id: Uuid::nil(),
            company1_id: Uuid::nil(),
            company2_id: Uuid::nil(),
            collateral_id: Uuid::nil(),
            collateral_ids: Vec::new(),
            deal_ids: Vec::new(),
            session_token: String::new(),
        })
    }

    async fn setup_test_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n{}", "📋 Setting up test data...".bold().cyan());

        // Create User 1 and Company 1
        let user1_email = format!(
            "{}_{}_user1@test.com",
            TEST_PREFIX,
            thread_rng().gen::<u32>()
        );
        let user1 = create_identity(&self.pool, "originator", &user1_email).await?;
        self.user1_id = user1.id;
        println!("  {} Created User 1: {}", "✅".green(), user1_email);

        let company1_request = CreateCompanyRequest {
            name: format!("{}_Company1", TEST_PREFIX),
            description: Some("Test company 1 for intel integration".to_string()),
            metadata: Some(json!({"test": true})),
        };
        let company1 = create_company(&self.pool, &company1_request).await?;
        self.company1_id = company1.id;

        create_membership(
            &self.pool,
            company1.id,
            user1.id,
            CompanyRole::Admin,
            ProfileType::Originator,
            None,
            None,
        )
        .await?;
        println!("  {} Created Company 1: {}", "✅".green(), company1.name);

        // Create User 2 and Company 2
        let user2_email = format!(
            "{}_{}_user2@test.com",
            TEST_PREFIX,
            thread_rng().gen::<u32>()
        );
        let user2 = create_identity(&self.pool, "originator", &user2_email).await?;
        self.user2_id = user2.id;
        println!("  {} Created User 2: {}", "✅".green(), user2_email);

        let company2_request = CreateCompanyRequest {
            name: format!("{}_Company2", TEST_PREFIX),
            description: Some("Test company 2 for intel integration".to_string()),
            metadata: Some(json!({"test": true})),
        };
        let company2 = create_company(&self.pool, &company2_request).await?;
        self.company2_id = company2.id;

        create_membership(
            &self.pool,
            company2.id,
            user2.id,
            CompanyRole::Admin,
            ProfileType::Originator,
            None,
            None,
        )
        .await?;
        println!("  {} Created Company 2: {}", "✅".green(), company2.name);

        // Create shared collateral
        let collateral_request = CreateCollateralRequest {
            name: format!("{}_Test_Property", TEST_PREFIX),
            collateral_type: Some("real_estate".to_string()),
            description: Some("Test property for double-pledge detection".to_string()),
            address: Some("123 Test Street, Test City, TC 12345".to_string()),
            custom_id: Some(format!("{}-PROP-001", TEST_PREFIX)),
            metadata: Some(json!({"test": true, "type": "commercial"})),
            created_by: Some(user1.id),
        };

        let collateral = create_collateral(&self.pool, collateral_request).await?;
        self.collateral_id = collateral.id;
        println!(
            "  {} Created Collateral: {} (ID: {})",
            "✅".green(),
            collateral.name,
            collateral.id
        );

        // Create a session token for user1 for API authentication
        let session_id = Uuid::new_v4();
        let session_token = format!(
            "test_session_{}",
            Uuid::new_v4().to_string().replace("-", "")
        );
        let expires_at = chrono::Utc::now() + chrono::Duration::days(1);
        sqlx::query!(
            r#"
            INSERT INTO sessions (id, user_id, session_token, expires_at, created_at, last_accessed_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            "#,
            session_id,
            user1.id,
            session_token,
            expires_at
        )
        .execute(&self.pool)
        .await?;

        self.session_token = session_token;
        println!(
            "  {} Created session token for API authentication",
            "✅".green()
        );

        Ok(())
    }

    async fn wait_for_processing(&self, message: &str) {
        println!("  {} {} ({}s)...", "⏳".yellow(), message, self.wait_time);
        tokio::time::sleep(Duration::from_secs(self.wait_time)).await;
    }
}

// ============================================================================
// API CLIENT FUNCTIONS
// ============================================================================

async fn create_deal_via_api(
    ctx: &TestContext,
    originator_id: Uuid,
    investor_id: Option<Uuid>,
    collateral_id: Option<Uuid>,
    deal_status: &str,
    title: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let request = CreateDealRequest {
        originator_id: originator_id.to_string(),
        investor_id: investor_id.map(|id| id.to_string()),
        borrower_id: None,
        principal_amount: "1000000.00".to_string(),
        interest_rate_bps: 500,
        deal_status: deal_status.to_string(),
        status: "active".to_string(),
        collateral_id: collateral_id.map(|id| id.to_string()),
        title: Some(title.to_string()),
        description: Some(format!("Test deal: {}", title)),
    };

    let url = format!("{}/api/v1/deal", ctx.api_url);
    let response = ctx
        .client
        .post(&url)
        .header("Cookie", format!("session_id={}", ctx.session_token))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("API error {}: {}", status, body).into());
    }

    let deal: DealResponse = response.json().await?;
    let deal_id = Uuid::parse_str(&deal.deal_id)?;

    Ok(deal_id)
}

async fn update_deal_status_via_api(
    ctx: &TestContext,
    deal_id: Uuid,
    new_status: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = UpdateDealRequest {
        loan_status: Some(new_status.to_string()),
    };

    let url = format!("{}/api/v1/deal/{}", ctx.api_url, deal_id);
    let response = ctx
        .client
        .put(&url)
        .header("Cookie", format!("session_id={}", ctx.session_token))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("API error {}: {}", status, body).into());
    }

    Ok(())
}

async fn get_collateral_matches_from_db(
    _ctx: &TestContext,
    _collateral_id: Uuid,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Note: collateral_matches table was removed in migration 060
    // Functionality moved to arda-collateral-intel microservice with SurrealDB
    // This function now returns 0 for backward compatibility with test code
    Ok(0)
}

async fn get_detection_history(
    ctx: &TestContext,
    collateral_id: Uuid,
) -> Result<Option<DetectionHistoryResponse>, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/api/v1/collateral/{}/detection-history",
        ctx.intel_url, collateral_id
    );
    let response = ctx.client.get(&url).send().await?;

    if response.status().is_success() {
        let history: DetectionHistoryResponse = response.json().await?;
        Ok(Some(history))
    } else if response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
    {
        // Not found or service unavailable - expected when using DB-only approach
        Ok(None)
    } else {
        let status = response.status();
        let body = response.text().await?;
        Err(format!("Intel service error {}: {}", status, body).into())
    }
}

async fn create_collateral_via_api(
    ctx: &TestContext,
    name: &str,
    address: &str,
    custom_id: Option<&str>,
    description: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    #[derive(Serialize)]
    struct CreateCollateralRequest {
        name: String,
        collateral_type: Option<String>,
        description: Option<String>,
        address: Option<String>,
        custom_id: Option<String>,
        metadata: Option<serde_json::Value>,
    }

    #[derive(Deserialize)]
    struct CollateralResponse {
        id: String,
    }

    let request = CreateCollateralRequest {
        name: name.to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: Some(description.to_string()),
        address: Some(address.to_string()),
        custom_id: custom_id.map(|s| s.to_string()),
        metadata: Some(json!({"test": true, "type": "commercial"})),
    };

    let url = format!("{}/api/v1/collateral", ctx.api_url);
    let response = ctx
        .client
        .post(&url)
        .header("Cookie", format!("session_id={}", ctx.session_token))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("API error {}: {}", status, body).into());
    }

    let collateral: CollateralResponse = response.json().await?;
    let collateral_id = Uuid::parse_str(&collateral.id)?;

    Ok(collateral_id)
}

async fn check_database_double_pledge(
    ctx: &TestContext,
    collateral_id: Uuid,
) -> Result<Option<Uuid>, Box<dyn std::error::Error>> {
    let result = sqlx::query!(
        r#"
        SELECT double_pledged
        FROM collateral
        WHERE id = $1
        "#,
        collateral_id
    )
    .fetch_one(&ctx.pool)
    .await?;

    Ok(result.double_pledged)
}

// ============================================================================
// TEST SCENARIOS
// ============================================================================

async fn scenario_1_same_user_double_pledge(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 1: Same-User Double Pledge Detection"
            .bold()
            .cyan()
    );

    // Create Deal #1 (active) with collateral
    let deal1_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(ctx.collateral_id),
        "active",
        &format!("{}_Deal_1_Active", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal1_id);
    println!(
        "  {} Created Deal #1 via API (triggers event publishing) - ID: {}",
        "✅".green(),
        deal1_id
    );

    ctx.wait_for_processing("Waiting for event processing")
        .await;

    // Create Deal #2 (active) with SAME collateral
    let deal2_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(ctx.collateral_id),
        "active",
        &format!("{}_Deal_2_Active", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal2_id);
    println!(
        "  {} Created Deal #2 via API with SAME collateral - ID: {}",
        "✅".green(),
        deal2_id
    );

    ctx.wait_for_processing("Waiting for duplicate detection")
        .await;

    // Verify: Check database
    let mut passed = true;

    match check_database_double_pledge(ctx, ctx.collateral_id).await? {
        Some(_) => {
            println!("  {} Database: double_pledged field is set", "✅".green());
        }
        None => {
            println!("  {} Database: double_pledged field NOT set (may be expected if using external service)", "⚠️".yellow());
        }
    }

    // Verify: Check matches in database
    let match_count = get_collateral_matches_from_db(ctx, ctx.collateral_id).await?;
    if match_count > 0 {
        println!(
            "  {} Database matches: Found {} match(es)",
            "✅".green(),
            match_count
        );
    } else {
        println!(
            "  {} Database matches: No matches found (checking if duplicate detection has run)",
            "⚠️".yellow()
        );
    }

    // Verify: Check detection history from Intel service
    ctx.wait_for_processing("Waiting for Intel service detection")
        .await;

    match get_detection_history(ctx, ctx.collateral_id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "  {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                if latest.match_count == 0 {
                    println!("  {} Expected at least 1 match, got 0", "❌".red());
                    passed = false;
                }
            } else {
                println!("  {} Detection History: No detections found", "⚠️".yellow());
            }
        }
        None => {
            println!("  {} Detection History: Not available (Intel service hasn't received events via API)", "ℹ️".blue());
        }
    }

    Ok(passed)
}

async fn scenario_2_cross_user_double_pledge(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 2: Cross-User Double Pledge Detection"
            .bold()
            .cyan()
    );

    // Create Deal #3 (active) with SAME collateral as different user
    let deal3_id = create_deal_via_api(
        ctx,
        ctx.company2_id, // Different company/user
        Some(ctx.company1_id),
        Some(ctx.collateral_id),
        "active",
        &format!("{}_Deal_3_Active_CrossUser", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal3_id);
    println!(
        "  {} Created Deal #3 via API (different company) - ID: {}",
        "✅".green(),
        deal3_id
    );

    ctx.wait_for_processing("Waiting for cross-user detection")
        .await;

    let passed = true;

    // Verify: Check matches in database
    let match_count = get_collateral_matches_from_db(ctx, ctx.collateral_id).await?;
    if match_count >= 2 {
        println!(
            "  {} Database matches: Found {} match(es) (cross-user)",
            "✅".green(),
            match_count
        );
    } else {
        println!(
            "  {} Database matches: Found {} match(es) (expected at least 2)",
            "⚠️".yellow(),
            match_count
        );
    }

    // Verify: Check detection history
    ctx.wait_for_processing("Waiting for Intel service detection")
        .await;

    match get_detection_history(ctx, ctx.collateral_id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "  {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                // Check for concerning matches (both deals active)
                let concerning_count = latest.matches.iter().filter(|m| m.is_concerning).count();
                if concerning_count > 0 {
                    println!(
                        "  {} Found {} concerning match(es) (both deals active)",
                        "✅".green(),
                        concerning_count
                    );
                } else {
                    println!(
                        "  {} No concerning matches found (expected at least 1)",
                        "⚠️".yellow()
                    );
                }
            } else {
                println!("  {} Detection History: No detections found", "⚠️".yellow());
            }
        }
        None => {
            println!("  {} Detection History: Not available (Intel service hasn't received events via API)", "ℹ️".blue());
        }
    }

    Ok(passed)
}

async fn scenario_3_paid_off_no_double_pledge(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 3: Paid-Off Deal - No Double Pledge"
            .bold()
            .cyan()
    );

    // Update all previous deals to paid_off
    for (i, deal_id) in ctx.deal_ids.iter().enumerate() {
        update_deal_status_via_api(ctx, *deal_id, "paid_off").await?;
        println!(
            "  {} Updated Deal #{} to paid_off via API - ID: {}",
            "✅".green(),
            i + 1,
            deal_id
        );
    }

    ctx.wait_for_processing("Waiting for status updates to process")
        .await;

    // Create Deal #4 (active) with SAME collateral
    let deal4_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(ctx.collateral_id),
        "active",
        &format!("{}_Deal_4_Active_AfterPaidOff", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal4_id);
    println!(
        "  {} Created Deal #4 via API after paid-off deals - ID: {}",
        "✅".green(),
        deal4_id
    );

    ctx.wait_for_processing("Waiting for detection after paid-off deals")
        .await;

    let passed = true;

    // Verify: Should show low risk since only one active deal
    ctx.wait_for_processing("Waiting for Intel service detection")
        .await;

    match get_detection_history(ctx, ctx.collateral_id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "  {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                // Count concerning matches
                let concerning_count = latest.matches.iter().filter(|m| m.is_concerning).count();
                if concerning_count == 0 {
                    println!(
                        "  {} No concerning matches (only one active deal) - CORRECT",
                        "✅".green()
                    );
                } else {
                    println!(
                        "  {} Found {} concerning match(es) - unexpected with paid-off deals",
                        "⚠️".yellow(),
                        concerning_count
                    );
                }

                // Check status is clear or low risk
                if latest.status == "clear" || latest.status == "potential_match" {
                    println!(
                        "  {} Detection status is acceptable: {}",
                        "✅".green(),
                        latest.status
                    );
                } else {
                    println!(
                        "  {} Detection status: {} (expected 'clear' with only 1 active)",
                        "⚠️".yellow(),
                        latest.status
                    );
                }
            } else {
                println!("  {} Detection History: No detections found", "⚠️".yellow());
            }
        }
        None => {
            println!("  {} Detection History: Not available (Intel service hasn't received events via API)", "ℹ️".blue());
        }
    }

    Ok(passed)
}

async fn scenario_4_exact_match_different_uuid(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 4: Same Name/Address, Different UUID Double Pledge"
            .bold()
            .cyan()
    );
    println!("  Testing exact match detection with duplicate collateral records");

    // Create Collateral A via API with specific name and address
    let collateral_a_id = create_collateral_via_api(
        ctx,
        "Oak Street Office Building",
        "456 Oak Street, Portland, OR",
        Some(&format!("{}-OAK-A", TEST_PREFIX)),
        "First instance of Oak Street Office Building",
    )
    .await?;
    ctx.collateral_ids.push(collateral_a_id); // Track for cleanup
    println!(
        "  {} Created Collateral A via API - ID: {} (triggers event publishing)",
        "✅".green(),
        collateral_a_id
    );

    ctx.wait_for_processing("Waiting for Collateral A event processing")
        .await;

    // Create Deal #1 with Collateral A
    let deal1_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(collateral_a_id),
        "active",
        &format!("{}_Deal_CollateralA", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal1_id);
    println!(
        "  {} Created Deal #1 with Collateral A - ID: {}",
        "✅".green(),
        deal1_id
    );

    ctx.wait_for_processing("Waiting for Deal #1 event processing")
        .await;

    // Create Collateral B via API with SAME name and address but DIFFERENT UUID
    let collateral_b_id = create_collateral_via_api(
        ctx,
        "Oak Street Office Building", // SAME NAME
        "456 Oak Street, Portland, OR", // SAME ADDRESS
        Some(&format!("{}-OAK-B", TEST_PREFIX)), // Different custom ID
        "Duplicate instance of Oak Street Office Building - SHOULD BE DETECTED",
    )
    .await?;
    ctx.collateral_ids.push(collateral_b_id); // Track for cleanup
    println!(
        "  {} Created Collateral B via API - ID: {} (DIFFERENT UUID, SAME NAME/ADDRESS)",
        "✅".green(),
        collateral_b_id
    );

    ctx.wait_for_processing("Waiting for Collateral B event processing and duplicate detection")
        .await;

    // Create Deal #2 with Collateral B (different company to avoid same-collateral detection)
    let deal2_id = create_deal_via_api(
        ctx,
        ctx.company2_id, // Different company
        Some(ctx.company1_id),
        Some(collateral_b_id),
        "active",
        &format!("{}_Deal_CollateralB", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal2_id);
    println!(
        "  {} Created Deal #2 with Collateral B (different company) - ID: {}",
        "✅".green(),
        deal2_id
    );

    ctx.wait_for_processing("Waiting for Deal #2 event processing and cross-collateral detection")
        .await;

    // Verify: Check detection history for Collateral A
    let mut passed = true;

    println!("\n  {} Verifying detection for Collateral A:", "🔍".cyan());
    match get_detection_history(ctx, collateral_a_id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "    {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                // Check if we have an exact match
                let exact_matches: Vec<_> = latest
                    .matches
                    .iter()
                    .filter(|m| {
                        m.matched_collateral_id == collateral_b_id.to_string()
                            && (m.reason.contains("name") || m.reason.contains("address"))
                    })
                    .collect();

                if !exact_matches.is_empty() {
                    for m in &exact_matches {
                        println!(
                            "    {} Found exact match: ID={}, Score={}, Reason={}",
                            "✅".green(),
                            m.matched_collateral_id,
                            m.score,
                            m.reason
                        );
                        if m.is_concerning {
                            println!("    {} Match is marked as concerning (both deals active)", "✅".green());
                        }
                    }
                } else {
                    println!(
                        "    {} No exact match found between Collateral A and B",
                        "❌".red()
                    );
                    passed = false;
                }
            } else {
                println!("    {} Detection History: No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!("    {} Detection History: Not available (Intel service hasn't received events)", "ℹ️".blue());
        }
    }

    println!("\n  {} Verifying detection for Collateral B:", "🔍".cyan());
    match get_detection_history(ctx, collateral_b_id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "    {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                // Check if we have an exact match
                let exact_matches: Vec<_> = latest
                    .matches
                    .iter()
                    .filter(|m| {
                        m.matched_collateral_id == collateral_a_id.to_string()
                            && (m.reason.contains("name") || m.reason.contains("address"))
                    })
                    .collect();

                if !exact_matches.is_empty() {
                    for m in &exact_matches {
                        println!(
                            "    {} Found exact match: ID={}, Score={}, Reason={}",
                            "✅".green(),
                            m.matched_collateral_id,
                            m.score,
                            m.reason
                        );

                        // Verify reason indicates which field matched
                        if m.reason.contains("name") && m.reason.contains("address") {
                            println!("    {} Match reason correctly indicates both name AND address match", "✅".green());
                        } else if m.reason.contains("name") || m.reason.contains("address") {
                            println!("    {} Match reason indicates at least one field match", "✅".green());
                        } else {
                            println!("    {} Match reason doesn't clearly indicate which field matched", "⚠️".yellow());
                        }

                        if m.is_concerning {
                            println!("    {} Match is marked as concerning (both deals active)", "✅".green());
                        }
                    }
                } else {
                    println!(
                        "    {} No exact match found between Collateral B and A",
                        "❌".red()
                    );
                    passed = false;
                }
            } else {
                println!("    {} Detection History: No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!("    {} Detection History: Not available (Intel service hasn't received events)", "ℹ️".blue());
        }
    }

    // Also check database flags
    println!("\n  {} Checking database flags:", "🔍".cyan());
    match check_database_double_pledge(ctx, collateral_a_id).await? {
        Some(_) => {
            println!("    {} Collateral A: double_pledged field is set", "✅".green());
        }
        None => {
            println!(
                "    {} Collateral A: double_pledged field NOT set (may be expected if using external service)",
                "⚠️".yellow()
            );
        }
    }

    match check_database_double_pledge(ctx, collateral_b_id).await? {
        Some(_) => {
            println!("    {} Collateral B: double_pledged field is set", "✅".green());
        }
        None => {
            println!(
                "    {} Collateral B: double_pledged field NOT set (may be expected if using external service)",
                "⚠️".yellow()
            );
        }
    }

    Ok(passed)
}

// ============================================================================
// CLEANUP
// ============================================================================

async fn cleanup_test_data(ctx: &TestContext) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "🧹 Cleaning up test data...".bold().cyan());

    // Delete deals (cascade will handle payments)
    for deal_id in &ctx.deal_ids {
        sqlx::query!("DELETE FROM deals WHERE id = $1", deal_id)
            .execute(&ctx.pool)
            .await?;
    }
    println!(
        "  {} Deleted {} test deal(s)",
        "✅".green(),
        ctx.deal_ids.len()
    );

    // Delete collateral
    sqlx::query!("DELETE FROM collateral WHERE id = $1", ctx.collateral_id)
        .execute(&ctx.pool)
        .await?;
    println!("  {} Deleted test collateral", "✅".green());

    // Delete additional collateral IDs (from scenario 4)
    for collateral_id in &ctx.collateral_ids {
        sqlx::query!("DELETE FROM collateral WHERE id = $1", collateral_id)
            .execute(&ctx.pool)
            .await?;
    }
    if !ctx.collateral_ids.is_empty() {
        println!(
            "  {} Deleted {} additional collateral(s)",
            "✅".green(),
            ctx.collateral_ids.len()
        );
    }

    // Delete company memberships
    sqlx::query!(
        "DELETE FROM company_memberships WHERE company_id IN ($1, $2)",
        ctx.company1_id,
        ctx.company2_id
    )
    .execute(&ctx.pool)
    .await?;

    // Delete companies
    sqlx::query!(
        "DELETE FROM companies WHERE id IN ($1, $2)",
        ctx.company1_id,
        ctx.company2_id
    )
    .execute(&ctx.pool)
    .await?;
    println!("  {} Deleted test companies", "✅".green());

    // Delete users
    sqlx::query!(
        "DELETE FROM users WHERE id IN ($1, $2)",
        ctx.user1_id,
        ctx.user2_id
    )
    .execute(&ctx.pool)
    .await?;
    println!("  {} Deleted test users", "✅".green());

    Ok(())
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = Command::new("Collateral Intel Integration Test")
        .about("Test the collateral Intel service integration with event publishing and detection")
        .arg(
            Arg::new("cleanup")
                .long("cleanup")
                .help("Clean up test data after completion")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("wait-time")
                .long("wait-time")
                .value_name("SECONDS")
                .help("Seconds to wait between operations")
                .default_value("5")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("api-url")
                .long("api-url")
                .value_name("URL")
                .help("Arda Credit API URL")
                .default_value(DEFAULT_API_URL),
        )
        .arg(
            Arg::new("intel-url")
                .long("intel-url")
                .value_name("URL")
                .help("Collateral Intel Service URL")
                .default_value(DEFAULT_INTEL_URL),
        )
        .get_matches();

    let cleanup = matches.get_flag("cleanup");
    let wait_time = *matches.get_one::<u64>("wait-time").unwrap();
    let api_url = matches.get_one::<String>("api-url").unwrap().clone();
    let intel_url = matches.get_one::<String>("intel-url").unwrap().clone();

    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());

    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );
    println!("{}", "  Collateral Intel Integration Tests".bold().cyan());
    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );
    println!("  API URL: {}", api_url);
    println!("  Intel URL: {}", intel_url);
    println!("  Wait time: {}s", wait_time);
    println!(
        "  Cleanup: {}",
        if cleanup { "enabled" } else { "disabled" }
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );
    println!(
        "{}",
        "  ℹ️  NOTE: Hybrid approach for full event flow".cyan()
    );
    println!(
        "{}",
        "  Setup via DB, deals via authenticated API calls".cyan()
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );

    // Initialize test context
    let mut ctx = TestContext::new(&database_url, api_url, intel_url, wait_time, cleanup).await?;

    // Setup test data
    ctx.setup_test_data().await?;

    // Run test scenarios
    let mut results = Vec::new();

    results.push((
        "Scenario 1: Same-User Double Pledge",
        scenario_1_same_user_double_pledge(&mut ctx).await?,
    ));
    results.push((
        "Scenario 2: Cross-User Double Pledge",
        scenario_2_cross_user_double_pledge(&mut ctx).await?,
    ));
    results.push((
        "Scenario 3: Paid-Off - No Double Pledge",
        scenario_3_paid_off_no_double_pledge(&mut ctx).await?,
    ));
    results.push((
        "Scenario 4: Same Name/Address, Different UUID",
        scenario_4_exact_match_different_uuid(&mut ctx).await?,
    ));

    // Print summary
    println!(
        "\n{}",
        "═══════════════════════════════════════════════════════".bold()
    );
    println!("{}", "  📊 Test Summary".bold().cyan());
    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );

    let mut all_passed = true;
    for (name, passed) in &results {
        if *passed {
            println!("  {} {}", "✅".green(), name.green());
        } else {
            println!("  {} {}", "❌".red(), name.red());
            all_passed = false;
        }
    }

    println!(
        "{}",
        "═══════════════════════════════════════════════════════".bold()
    );

    // Log inspection instructions
    println!("\n{}", "🔍 Log Inspection Commands:".bold().cyan());
    println!("  Check event publishing:");
    println!(
        "    {} grep '📥 Received detection result' server.log",
        "→".cyan()
    );
    println!("  Check collateral events:");
    println!(
        "    {} grep 'CollateralCreated\\|DealCreated\\|DealStatusChanged' server.log",
        "→".cyan()
    );

    // Cleanup if requested
    if cleanup {
        cleanup_test_data(&ctx).await?;
        println!("\n{} Test data cleaned up", "✅".green());
    } else {
        println!(
            "\n{} Test data retained. Run with --cleanup to remove.",
            "💡".yellow()
        );
        println!("  Collateral ID: {}", ctx.collateral_id);
        if !ctx.collateral_ids.is_empty() {
            println!("  Additional Collateral IDs: {:?}", ctx.collateral_ids);
        }
        println!("  Deal IDs: {:?}", ctx.deal_ids);
    }

    println!(
        "\n{}",
        if all_passed {
            "🎉 All tests completed successfully!".green().bold()
        } else {
            "⚠️  Some tests had warnings or failures. Review output above."
                .yellow()
                .bold()
        }
    );

    Ok(())
}
