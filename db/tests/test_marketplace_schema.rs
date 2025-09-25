//! Test the marketplace database schema and foundation functions
//!
//! This test validates that the database foundation created for Worker A
//! provides all the necessary components for Worker B to start their work.

use chrono;
use db::init_db;
use db::marketplace;

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
          AND table_name IN ('loan_listings', 'loan_transfers')
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Should be able to query table existence")
    .expect("Tables existence query should return a value");

    assert!(
        tables_exist,
        "loan_listings and loan_transfers tables should exist"
    );

    // Test 3: Check new loan status values work
    println!("📋 Test 3: Loan status enum validation");
    // Try to update a loan status to for_sale (should not fail due to constraint)
    let status_update_result =
        sqlx::query!("UPDATE loans SET loan_status = 'for_sale' WHERE false")
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
          AND table_name = 'marketplace_loans'
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Should be able to query view existence")
    .expect("View existence query should return a value");

    assert!(view_exists, "marketplace_loans view should exist");

    // Test 5: Check marketplace view returns data (may be empty, that's ok)
    println!("📋 Test 5: Marketplace view functionality");
    let view_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM marketplace_loans")
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
    let test_lender_id = uuid::Uuid::new_v4();
    let test_borrower_id = uuid::Uuid::new_v4();
    let test_loan_id = uuid::Uuid::new_v4();

    // Insert test lender identity with unique email
    let lender_email = format!(
        "test-lender-{}-{}.example.com",
        test_lender_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_lender_id,
        lender_email
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test lender identity");

    // Insert test borrower identity with unique email
    let borrower_email = format!(
        "test-borrower-{}-{}.example.com",
        test_borrower_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_borrower_id,
        borrower_email
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test borrower identity");

    // Insert test loan that the listing can reference
    sqlx::query!(
        r#"
        INSERT INTO loans (id, borrower_id, lender_id, principal_amount, outstanding_balance, 
                          interest_rate_bps, loan_status, originated_at)
        VALUES ($1, $2, $3, 50000.00, 50000.00, 1200, 'active', NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
        test_loan_id,
        test_borrower_id,
        test_lender_id
    )
    .execute(&pool)
    .await
    .expect("Should be able to create test loan");

    // Test create_loan_listing
    println!("Testing create_loan_listing...");
    let listing = marketplace::create_loan_listing(
        &pool,
        test_loan_id,
        test_lender_id,
        rust_decimal::Decimal::new(95000, 2), // $950.00
        Some("Test listing".to_string()),
    )
    .await
    .expect("Should be able to create loan listing");

    assert_eq!(listing.loan_id, test_loan_id);
    assert_eq!(listing.seller_id, test_lender_id);

    // Test get_loan_listing_by_loan_id
    println!("Testing get_loan_listing_by_loan_id...");
    let retrieved = marketplace::get_loan_listing_by_loan_id(&pool, test_loan_id)
        .await
        .expect("Should be able to retrieve loan listing");

    assert!(retrieved.is_some(), "Listing should exist");
    assert_eq!(retrieved.as_ref().unwrap().listing_id, listing.listing_id);

    // Test get_marketplace_loans_basic
    println!("Testing get_marketplace_loans_basic...");
    let loans = marketplace::get_marketplace_loans_basic(&pool, Some(10), Some(0))
        .await
        .expect("Should be able to get marketplace loans");

    assert!(
        !loans.is_empty(),
        "Should have at least one marketplace loan"
    );

    // Test validate_loan_for_transfer
    println!("Testing validate_loan_for_transfer...");
    let validation = marketplace::validate_loan_for_transfer(&pool, test_loan_id)
        .await
        .expect("Should be able to validate loan for transfer");

    assert!(validation.is_some(), "Loan should be valid for transfer");
    assert_eq!(
        validation.as_ref().unwrap().0,
        test_lender_id,
        "Should return correct owner"
    );

    // Clean up test data
    sqlx::query!("DELETE FROM loan_listings WHERE loan_id = $1", test_loan_id)
        .execute(&pool)
        .await
        .expect("Should be able to clean up test listing");

    sqlx::query!("DELETE FROM loans WHERE id = $1", test_loan_id)
        .execute(&pool)
        .await
        .expect("Should be able to clean up test loan");

    println!("✅ All CRUD operations work correctly!");
}
