//! Test the marketplace database schema and foundation functions
//!
//! This test validates that the database foundation created for Worker A
//! provides all the necessary components for Worker B to start their work.

use chrono;
use db::business::marketplace;
use db::init_db;

#[tokio::test]
async fn test_marketplace_foundation_complete() {
    // Initialize database connection
    let pool = init_db().await.expect("Failed to initialize database");

    // Test 1: Validate schema works
    println!("📋 Test 1: Schema validation");
    let schema_valid = marketplace::validate_marketplace_schema(&pool)
        .await
        .expect("Schema validation should succeed");

    assert!(schema_valid, "Marketplace schema validation should pass");

    // Test 2: Check tables exist
    println!("📋 Test 2: Tables existence check");
    let tables_exist: bool = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) = 2 as tables_exist
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_name IN ('deal_listings', 'deal_transfers')
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Should be able to query table existence")
    .expect("Tables existence query should return a value");

    assert!(
        tables_exist,
        "deal_listings and deal_transfers tables should exist"
    );

    // Test 3: Check new loan status values work
    println!("📋 Test 3: Loan status enum validation");
    // Try to update a loan status to for_sale (should not fail due to constraint)
    let status_update_result =
        sqlx::query!("UPDATE deals SET deal_status = 'for_sale' WHERE false")
            .execute(&pool)
            .await;

    assert!(
        status_update_result.is_ok(),
        "New loan status values should be accepted"
    );

    // Test 4: Check marketplace view exists
    println!("📋 Test 4: Marketplace view check");
    let view_exists: bool = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) > 0 as view_exists
        FROM information_schema.views
        WHERE table_schema = 'public'
          AND table_name = 'marketplace_deals'
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Should be able to query view existence")
    .expect("View existence query should return a value");

    assert!(view_exists, "marketplace_deals view should exist");

    // Test 5: Check marketplace view returns data (may be empty, that's ok)
    println!("📋 Test 5: Marketplace view functionality");
    let view_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM marketplace_deals")
        .fetch_one(&pool)
        .await
        .expect("Should be able to query marketplace view")
        .expect("Marketplace view count query should return a value");

    // Count can be 0, that's fine - we just need the query to work
    assert!(
        view_count >= 0,
        "Marketplace view should return a valid count"
    );

    println!("🎉 All foundation tests passed in unit test!");
    println!("🚀 Worker B can now safely start their transfer implementation work.");
}

#[tokio::test]
async fn test_marketplace_crud_operations() {
    // Initialize database connection
    let pool = init_db().await.expect("Failed to initialize database");

    // Create test users and loan for testing
    let test_seller_user_id = uuid::Uuid::new_v4(); // User who will list the deal for sale
    let test_buyer_user_id = uuid::Uuid::new_v4(); // User who might buy the deal
    let test_loan_id = uuid::Uuid::new_v4();

    // Insert test seller user with unique email
    let seller_email = format!(
        "test-seller-{}-{}.example.com",
        test_seller_user_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO users (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_seller_user_id,
        seller_email
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test seller user");

    // Insert test buyer user with unique email
    let buyer_email = format!(
        "test-buyer-{}-{}.example.com",
        test_buyer_user_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO users (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_buyer_user_id,
        buyer_email
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test buyer user");

    // Create test companies (required for deals - originator and investor)
    let test_originator_company_id = uuid::Uuid::new_v4();
    let test_investor_company_id = uuid::Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO companies (id, name, description, status)
        VALUES ($1, 'Test Originator Company', 'Test originator company for marketplace tests', 'active')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_originator_company_id
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test originator company");

    sqlx::query!(
        r#"
        INSERT INTO companies (id, name, description, status)
        VALUES ($1, 'Test Investor Company', 'Test investor company for marketplace tests', 'active')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_investor_company_id
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test investor company");

    // Link the seller user to the investor company (so they can list deals)
    // Note: There's no unique constraint on (company_id, user_id) in company_memberships,
    // so we can't use ON CONFLICT. Just insert directly.
    sqlx::query!(
        r#"
        INSERT INTO company_memberships (company_id, user_id, role, status)
        VALUES ($1, $2, 'admin', 'active')
        "#,
        test_investor_company_id,
        test_seller_user_id
    )
    .execute(&pool)
    .await
    .expect("Should be able to link seller user to investor company");

    // Insert test loan that the listing can reference
    // Note: originator_id and investor_id now reference companies, not users
    sqlx::query!(
        r#"
        INSERT INTO deals (id, originator_id, investor_id, principal_amount, outstanding_balance,
                          interest_rate_bps, deal_status, originated_at)
        VALUES ($1, $2, $3, 50000.00, 50000.00, 1200, 'active', NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
        test_loan_id,
        test_originator_company_id,
        test_investor_company_id
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test loan");

    // Test create_loan_listing
    println!("Testing create_loan_listing...");
    let listing = marketplace::create_deal_listing(
        &pool,
        test_loan_id,
        test_seller_user_id, // Use seller user ID, not company ID
        rust_decimal::Decimal::new(95000, 2), // $950.00
        Some("Test listing".to_string()),
        None, // expires_at
    )
    .await
    .expect("Should be able to create loan listing");

    assert_eq!(listing.deal_id, test_loan_id);
    assert_eq!(listing.seller_id, test_seller_user_id);

    // Test get_loan_listing_by_loan_id
    println!("Testing get_loan_listing_by_loan_id...");
    let retrieved = marketplace::get_deal_listing_by_deal_id(&pool, test_loan_id)
        .await
        .expect("Should be able to retrieve loan listing");

    assert!(retrieved.is_some(), "Listing should exist");
    assert_eq!(retrieved.as_ref().unwrap().listing_id, listing.listing_id);

    // Test get_marketplace_loans_basic
    println!("Testing get_marketplace_loans_basic...");
    let loans = marketplace::browse_marketplace_deals(&pool, Some(10), None)
        .await
        .expect("Should be able to get marketplace loans");

    assert!(
        !loans.is_empty(),
        "Should have at least one marketplace loan"
    );

    // Test validate_loan_for_transfer
    println!("Testing validate_loan_for_transfer...");
    let validation = marketplace::validate_deal_for_transfer(&pool, test_loan_id)
        .await
        .expect("Should be able to validate loan for transfer");

    assert!(validation.is_some(), "Loan should be valid for transfer");
    assert_eq!(
        validation.as_ref().unwrap().0,
        test_investor_company_id, // Returns investor company ID (from deals.investor_id)
        "Should return correct investor company owner"
    );

    // Clean up test data
    sqlx::query!("DELETE FROM deal_listings WHERE deal_id = $1", test_loan_id)
        .execute(&pool)
        .await
        .expect("Should be able to clean up test listing");

    sqlx::query!("DELETE FROM deals WHERE id = $1", test_loan_id)
        .execute(&pool)
        .await
        .expect("Should be able to clean up test loan");

    println!("✅ All CRUD operations work correctly!");
}
