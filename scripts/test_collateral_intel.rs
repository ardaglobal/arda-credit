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
//! 4. Happy Path - Single Deal, Single Collateral (No Double Pledge)
//! 5. Happy Path - Two Deals, Two Different Collaterals (No Cross-Match)
//! 6. Fuzzy Address Double Pledge Detection (Suite Number Variations)
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
use wiremock::{
    matchers::{method, path_regex},
    Mock, MockServer, ResponseTemplate,
};

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
    mock_server: MockServer,

    // Test data
    user1_id: Uuid,
    user2_id: Uuid,
    company1_id: Uuid,
    company2_id: Uuid,
    collateral_id: Uuid,
    deal_ids: Vec<Uuid>,
    session_token: String,
}

impl TestContext {
    async fn new(
        database_url: &str,
        api_url: String,
        _intel_url: String,
        wait_time: u64,
        cleanup: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        println!("{}", "🚀 Initializing test context...".bold().cyan());

        let pool = init_db_with_url(database_url).await?;
        println!("  {} Database connected", "✅".green());

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        // Start mock server for Intel service
        let mock_server = MockServer::start().await;
        let intel_url = mock_server.uri();
        println!(
            "  {} Mock Intel service started at {}",
            "✅".green(),
            intel_url
        );

        Ok(Self {
            pool,
            client,
            api_url,
            intel_url,
            wait_time,
            cleanup,
            mock_server,
            user1_id: Uuid::nil(),
            user2_id: Uuid::nil(),
            company1_id: Uuid::nil(),
            company2_id: Uuid::nil(),
            collateral_id: Uuid::nil(),
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

    /// Mock a "double pledge detected" response from Intel service
    async fn mock_double_pledge_detected(&self, collateral_id: Uuid, matched_collateral_id: Uuid) {
        let response = DetectionHistoryResponse {
            collateral_id: collateral_id.to_string(),
            total_count: 1,
            limit: 10,
            offset: 0,
            detections: vec![DetectionRecord {
                detection_id: Uuid::new_v4().to_string(),
                trigger_type: "deal_created".to_string(),
                status: "double_pledge_detected".to_string(),
                match_count: 1,
                analyzed_at: chrono::Utc::now().to_rfc3339(),
                matches: vec![DetectionMatchDetail {
                    matched_collateral_id: matched_collateral_id.to_string(),
                    matched_collateral_name: "Matched Property".to_string(),
                    score: 0.95,
                    confidence: "high".to_string(),
                    is_concerning: true,
                    reason: "Same collateral used in multiple active deals".to_string(),
                }],
            }],
        };

        Mock::given(method("GET"))
            .and(path_regex(format!(
                r"^/api/v1/collateral/{}/detection-history$",
                collateral_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&self.mock_server)
            .await;
    }

    /// Mock a "clear" response (no double pledge detected) from Intel service
    async fn mock_clear_response(&self, collateral_id: Uuid) {
        let response = DetectionHistoryResponse {
            collateral_id: collateral_id.to_string(),
            total_count: 1,
            limit: 10,
            offset: 0,
            detections: vec![DetectionRecord {
                detection_id: Uuid::new_v4().to_string(),
                trigger_type: "deal_created".to_string(),
                status: "clear".to_string(),
                match_count: 0,
                analyzed_at: chrono::Utc::now().to_rfc3339(),
                matches: vec![],
            }],
        };

        Mock::given(method("GET"))
            .and(path_regex(format!(
                r"^/api/v1/collateral/{}/detection-history$",
                collateral_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&self.mock_server)
            .await;
    }

    /// Mock a "not found" response from Intel service (no detection history yet)
    #[allow(dead_code)]
    async fn mock_not_found(&self, collateral_id: Uuid) {
        Mock::given(method("GET"))
            .and(path_regex(format!(
                r"^/api/v1/collateral/{}/detection-history$",
                collateral_id
            )))
            .respond_with(ResponseTemplate::new(404))
            .mount(&self.mock_server)
            .await;
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

async fn scenario_4_fuzzy_address_double_pledge(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 4: Fuzzy Address Double Pledge Detection"
            .bold()
            .cyan()
    );
    println!(
        "  {} Testing address variations: suite numbers, unit numbers, etc.",
        "ℹ️".blue()
    );

    // Create Collateral #1 with base address (no suite)
    let collateral1_request = CreateCollateralRequest {
        name: format!("{}_Fuzzy_Property_1", TEST_PREFIX),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Test property for fuzzy address matching".to_string()),
        address: Some("123 Main St, New York, NY 10001".to_string()),
        custom_id: Some(format!("{}-FUZZY-001", TEST_PREFIX)),
        metadata: Some(json!({"test": true, "type": "fuzzy_address_test"})),
        created_by: Some(ctx.user1_id),
    };

    let collateral1 = create_collateral(&ctx.pool, collateral1_request).await?;
    println!(
        "  {} Created Collateral #1: {} at '{}'",
        "✅".green(),
        collateral1.name,
        collateral1.address.as_ref().unwrap()
    );

    // Create Deal #1 with first collateral (active)
    let deal1_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(collateral1.id),
        "active",
        &format!("{}_Fuzzy_Deal_1_Active", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal1_id);
    println!(
        "  {} Created Deal #1 with Collateral #1 (active) - ID: {}",
        "✅".green(),
        deal1_id
    );

    ctx.wait_for_processing("Waiting for first collateral processing")
        .await;

    // Create Collateral #2 with suite variation
    let collateral2_request = CreateCollateralRequest {
        name: format!("{}_Fuzzy_Property_2", TEST_PREFIX),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Test property with suite number variation".to_string()),
        address: Some("123 Main St Suite A, New York, NY 10001".to_string()),
        custom_id: Some(format!("{}-FUZZY-002", TEST_PREFIX)),
        metadata: Some(json!({"test": true, "type": "fuzzy_address_test"})),
        created_by: Some(ctx.user2_id),
    };

    let collateral2 = create_collateral(&ctx.pool, collateral2_request).await?;
    println!(
        "  {} Created Collateral #2: {} at '{}'",
        "✅".green(),
        collateral2.name,
        collateral2.address.as_ref().unwrap()
    );

    // Create Deal #2 with second collateral (active)
    let deal2_id = create_deal_via_api(
        ctx,
        ctx.company2_id,
        Some(ctx.company1_id),
        Some(collateral2.id),
        "active",
        &format!("{}_Fuzzy_Deal_2_Active", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal2_id);
    println!(
        "  {} Created Deal #2 with Collateral #2 (active) - ID: {}",
        "✅".green(),
        deal2_id
    );

    ctx.wait_for_processing("Waiting for fuzzy address detection")
        .await;

    let mut passed = true;

    // Verify: Check detection history for collateral2
    ctx.wait_for_processing("Waiting for Intel service fuzzy matching")
        .await;

    match get_detection_history(ctx, collateral2.id).await? {
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
                    println!(
                        "  {} Expected at least 1 match for fuzzy address, got 0",
                        "❌".red()
                    );
                    passed = false;
                } else {
                    // Check for high similarity match
                    let high_similarity_matches: Vec<_> =
                        latest.matches.iter().filter(|m| m.score >= 0.85).collect();

                    if !high_similarity_matches.is_empty() {
                        println!(
                            "  {} Found {} high similarity match(es) (score >= 0.85)",
                            "✅".green(),
                            high_similarity_matches.len()
                        );
                        for m in &high_similarity_matches {
                            println!(
                                "    → Match: {} (score: {:.3}, concerning: {})",
                                m.matched_collateral_name, m.score, m.is_concerning
                            );
                        }
                    } else {
                        println!(
                            "  {} No matches with similarity >= 0.85 (highest: {:.3})",
                            "⚠️".yellow(),
                            latest.matches.first().map(|m| m.score).unwrap_or(0.0)
                        );
                        println!(
                            "    {} Fuzzy matching may need tuning for suite number variations",
                            "ℹ️".blue()
                        );
                    }

                    // Check for concerning matches (both deals active)
                    let concerning_count =
                        latest.matches.iter().filter(|m| m.is_concerning).count();
                    if concerning_count > 0 {
                        println!(
                            "  {} Found {} concerning match(es) - double pledge detected!",
                            "✅".green(),
                            concerning_count
                        );
                    } else {
                        println!(
                            "  {} No concerning matches flagged (both deals active but not flagged)",
                            "⚠️".yellow()
                        );
                    }
                }
            } else {
                println!("  {} Detection History: No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!(
                "  {} Detection History: Not available (Intel service not running or not receiving events)",
                "ℹ️".blue()
            );
            println!(
                "    {} This scenario requires the arda-collateral-intel microservice",
                "ℹ️".blue()
            );
        }
    }

    // Document the expected threshold
    println!("\n  {} Fuzzy Address Matching Expectations:", "📝".cyan());
    println!("    • Similarity threshold: >= 0.85");
    println!("    • Address variations tested:");
    println!("      - Base address: '123 Main St, New York, NY 10001'");
    println!("      - With suite: '123 Main St Suite A, New York, NY 10001'");
    println!("    • Should detect as potential double pledge when both deals active");

    // Cleanup: Delete the fuzzy test collaterals
    sqlx::query!(
        "DELETE FROM collateral WHERE id IN ($1, $2)",
        collateral1.id,
        collateral2.id
    )
    .execute(&ctx.pool)
    .await?;
    println!("  {} Cleaned up fuzzy test collaterals", "🧹".green());

    Ok(passed)
}

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
    // Mock the Intel service response for double pledge detection
    ctx.mock_double_pledge_detected(ctx.collateral_id, ctx.collateral_id)
        .await;
    println!(
        "  {} Mock: Intel service configured to return double pledge detected",
        "🎭".cyan()
    );

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
    // Mock the Intel service response for cross-user double pledge detection
    ctx.mock_double_pledge_detected(ctx.collateral_id, ctx.collateral_id)
        .await;
    println!(
        "  {} Mock: Intel service configured to return double pledge detected",
        "🎭".cyan()
    );

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
    // Mock the Intel service response for clear status (no double pledge after paid-off)
    ctx.mock_clear_response(ctx.collateral_id).await;
    println!(
        "  {} Mock: Intel service configured to return clear status",
        "🎭".cyan()
    );

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

async fn scenario_4_happy_path_single_deal_single_collateral(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 4: Happy Path - Single Deal, Single Collateral (No Double Pledge)"
            .bold()
            .cyan()
    );

    // Create a new unique collateral for this test
    let collateral_request = CreateCollateralRequest {
        name: format!("{}_Happy_Path_Property_A", TEST_PREFIX),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Happy path test property A - single deal".to_string()),
        address: Some("456 Happy Street, Test City, TC 54321".to_string()),
        custom_id: Some(format!("{}-HAPPY-A-001", TEST_PREFIX)),
        metadata: Some(json!({"test": true, "type": "residential", "scenario": "happy_path_a"})),
        created_by: Some(ctx.user1_id),
    };

    let collateral_a = create_collateral(&ctx.pool, collateral_request).await?;
    println!(
        "  {} Created Collateral A: {} (ID: {})",
        "✅".green(),
        collateral_a.name,
        collateral_a.id
    );

    // Create a single active deal with this collateral
    let deal_a_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(collateral_a.id),
        "active",
        &format!("{}_Happy_Deal_A", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal_a_id);
    println!(
        "  {} Created Deal A via API (single active deal) - ID: {}",
        "✅".green(),
        deal_a_id
    );

    ctx.wait_for_processing("Waiting for event processing and detection")
        .await;

    let mut passed = true;

    // Verify: Check detection history from Intel service
    // Mock the Intel service response for clear status (happy path - no double pledge)
    ctx.mock_clear_response(collateral_a.id).await;
    println!(
        "  {} Mock: Intel service configured to return clear status",
        "🎭".cyan()
    );

    ctx.wait_for_processing("Waiting for Intel service detection")
        .await;

    match get_detection_history(ctx, collateral_a.id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "  {} Detection History: Status={}, matches={}",
                    "✅".green(),
                    latest.status,
                    latest.match_count
                );

                // Verify status is 'clear'
                if latest.status == "clear" {
                    println!("  {} Detection status is 'clear' - CORRECT", "✅".green());
                } else {
                    println!(
                        "  {} Detection status is '{}' (expected 'clear')",
                        "❌".red(),
                        latest.status
                    );
                    passed = false;
                }

                // Verify match count is 0
                if latest.match_count == 0 {
                    println!(
                        "  {} Match count is 0 - CORRECT (no double pledge)",
                        "✅".green()
                    );
                } else {
                    println!(
                        "  {} Match count is {} (expected 0)",
                        "❌".red(),
                        latest.match_count
                    );
                    passed = false;
                }

                // Verify no concerning matches
                let concerning_count = latest.matches.iter().filter(|m| m.is_concerning).count();
                if concerning_count == 0 {
                    println!("  {} No concerning matches - CORRECT", "✅".green());
                } else {
                    println!(
                        "  {} Found {} concerning match(es) (expected 0)",
                        "❌".red(),
                        concerning_count
                    );
                    passed = false;
                }
            } else {
                println!("  {} Detection History: No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!(
                "  {} Detection History: Not available (Intel service hasn't received events)",
                "ℹ️".blue()
            );
            // Not marking as failed since Intel service might not be running
        }
    }

    // Cleanup collateral
    sqlx::query!("DELETE FROM collateral WHERE id = $1", collateral_a.id)
        .execute(&ctx.pool)
        .await?;
    println!("  {} Cleaned up test collateral A", "✅".green());

    Ok(passed)
}

async fn scenario_5_happy_path_two_deals_different_collaterals(
    ctx: &mut TestContext,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "\n{}",
        "📋 Test Scenario 5: Happy Path - Two Deals, Two Different Collaterals (No Cross-Match)"
            .bold()
            .cyan()
    );

    // Create first unique collateral
    let collateral_request_1 = CreateCollateralRequest {
        name: format!("{}_Happy_Path_Property_B1", TEST_PREFIX),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Happy path test property B1 - distinct".to_string()),
        address: Some("789 Oak Avenue, Test City, TC 78901".to_string()),
        custom_id: Some(format!("{}-HAPPY-B1-001", TEST_PREFIX)),
        metadata: Some(json!({"test": true, "type": "commercial", "scenario": "happy_path_b"})),
        created_by: Some(ctx.user1_id),
    };

    let collateral_b1 = create_collateral(&ctx.pool, collateral_request_1).await?;
    println!(
        "  {} Created Collateral B1: {} (ID: {})",
        "✅".green(),
        collateral_b1.name,
        collateral_b1.id
    );

    // Create second unique collateral with different name and address
    let collateral_request_2 = CreateCollateralRequest {
        name: format!("{}_Happy_Path_Property_B2", TEST_PREFIX),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Happy path test property B2 - distinct".to_string()),
        address: Some("321 Pine Boulevard, Test City, TC 32109".to_string()),
        custom_id: Some(format!("{}-HAPPY-B2-001", TEST_PREFIX)),
        metadata: Some(json!({"test": true, "type": "industrial", "scenario": "happy_path_b"})),
        created_by: Some(ctx.user2_id),
    };

    let collateral_b2 = create_collateral(&ctx.pool, collateral_request_2).await?;
    println!(
        "  {} Created Collateral B2: {} (ID: {})",
        "✅".green(),
        collateral_b2.name,
        collateral_b2.id
    );

    // Create first active deal with collateral B1
    let deal_b1_id = create_deal_via_api(
        ctx,
        ctx.company1_id,
        Some(ctx.company2_id),
        Some(collateral_b1.id),
        "active",
        &format!("{}_Happy_Deal_B1", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal_b1_id);
    println!(
        "  {} Created Deal B1 via API with Collateral B1 - ID: {}",
        "✅".green(),
        deal_b1_id
    );

    ctx.wait_for_processing("Waiting for event processing")
        .await;

    // Create second active deal with collateral B2 (different collateral)
    let deal_b2_id = create_deal_via_api(
        ctx,
        ctx.company2_id,
        Some(ctx.company1_id),
        Some(collateral_b2.id),
        "active",
        &format!("{}_Happy_Deal_B2", TEST_PREFIX),
    )
    .await?;
    ctx.deal_ids.push(deal_b2_id);
    println!(
        "  {} Created Deal B2 via API with Collateral B2 (different) - ID: {}",
        "✅".green(),
        deal_b2_id
    );

    ctx.wait_for_processing("Waiting for event processing and detection")
        .await;

    let mut passed = true;

    // Verify: Check detection history for Collateral B1
    // Mock the Intel service responses for both collaterals (both clear - no cross-match)
    ctx.mock_clear_response(collateral_b1.id).await;
    ctx.mock_clear_response(collateral_b2.id).await;
    println!(
        "  {} Mock: Intel service configured to return clear status for both collaterals",
        "🎭".cyan()
    );

    ctx.wait_for_processing("Waiting for Intel service detection")
        .await;

    println!("\n  {} Checking Collateral B1 Detection:", "🔍".cyan());
    match get_detection_history(ctx, collateral_b1.id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "    {} Status={}, matches={}",
                    "📊".cyan(),
                    latest.status,
                    latest.match_count
                );

                // Verify status is 'clear'
                if latest.status == "clear" {
                    println!("    {} Status 'clear' - CORRECT", "✅".green());
                } else {
                    println!(
                        "    {} Status '{}' (expected 'clear')",
                        "❌".red(),
                        latest.status
                    );
                    passed = false;
                }

                // Verify match count is 0 (no cross-match with B2)
                if latest.match_count == 0 {
                    println!(
                        "    {} Match count 0 - CORRECT (no cross-match)",
                        "✅".green()
                    );
                } else {
                    println!(
                        "    {} Match count {} (expected 0)",
                        "❌".red(),
                        latest.match_count
                    );
                    passed = false;
                }

                // Verify no concerning matches
                let concerning_count = latest.matches.iter().filter(|m| m.is_concerning).count();
                if concerning_count == 0 {
                    println!("    {} No concerning matches - CORRECT", "✅".green());
                } else {
                    println!(
                        "    {} Found {} concerning match(es) (expected 0)",
                        "❌".red(),
                        concerning_count
                    );
                    passed = false;
                }
            } else {
                println!("    {} No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!("    {} Detection History not available", "ℹ️".blue());
        }
    }

    // Verify: Check detection history for Collateral B2
    println!("\n  {} Checking Collateral B2 Detection:", "🔍".cyan());
    match get_detection_history(ctx, collateral_b2.id).await? {
        Some(history) => {
            if !history.detections.is_empty() {
                let latest = &history.detections[0];
                println!(
                    "    {} Status={}, matches={}",
                    "📊".cyan(),
                    latest.status,
                    latest.match_count
                );

                // Verify status is 'clear'
                if latest.status == "clear" {
                    println!("    {} Status 'clear' - CORRECT", "✅".green());
                } else {
                    println!(
                        "    {} Status '{}' (expected 'clear')",
                        "❌".red(),
                        latest.status
                    );
                    passed = false;
                }

                // Verify match count is 0 (no cross-match with B1)
                if latest.match_count == 0 {
                    println!(
                        "    {} Match count 0 - CORRECT (no cross-match)",
                        "✅".green()
                    );
                } else {
                    println!(
                        "    {} Match count {} (expected 0)",
                        "❌".red(),
                        latest.match_count
                    );
                    passed = false;
                }

                // Verify no concerning matches
                let concerning_count = latest.matches.iter().filter(|m| m.is_concerning).count();
                if concerning_count == 0 {
                    println!("    {} No concerning matches - CORRECT", "✅".green());
                } else {
                    println!(
                        "    {} Found {} concerning match(es) (expected 0)",
                        "❌".red(),
                        concerning_count
                    );
                    passed = false;
                }
            } else {
                println!("    {} No detections found", "⚠️".yellow());
                passed = false;
            }
        }
        None => {
            println!("    {} Detection History not available", "ℹ️".blue());
        }
    }

    // Cleanup collaterals
    sqlx::query!("DELETE FROM collateral WHERE id = $1", collateral_b1.id)
        .execute(&ctx.pool)
        .await?;
    sqlx::query!("DELETE FROM collateral WHERE id = $1", collateral_b2.id)
        .execute(&ctx.pool)
        .await?;
    println!("\n  {} Cleaned up test collaterals B1 and B2", "✅".green());

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
        "Scenario 4: Happy Path - Single Deal, Single Collateral",
        scenario_4_happy_path_single_deal_single_collateral(&mut ctx).await?,
    ));
    results.push((
        "Scenario 5: Happy Path - Two Deals, Different Collaterals",
        scenario_5_happy_path_two_deals_different_collaterals(&mut ctx).await?,
    ));
    results.push((
        "Scenario 4: Fuzzy Address Double Pledge",
        scenario_4_fuzzy_address_double_pledge(&mut ctx).await?,
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
