use arda_credit_db::db::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::env;

#[tokio::test]
async fn test_remove_deal_listing_functionality() {
    // Get database URL from environment
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/arda_credit_db?sslmode=disable".to_string()
    });

    // Connect to database
    let pool = PgPool::connect(&database_url).await.unwrap();

    // Create test data
    let test_investor_id = uuid::Uuid::new_v4();
    let test_originator_id = uuid::Uuid::new_v4();
    let test_deal_id = uuid::Uuid::new_v4();

    println!("🧪 Testing remove deal listing functionality");
    println!("Test investor ID: {}", test_investor_id);
    println!("Test originator ID: {}", test_originator_id);
    println!("Test deal ID: {}", test_deal_id);

    // Insert test investor identity
    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_investor_id,
        format!("test-investor-{}.example.com", test_investor_id.simple())
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert test originator identity
    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'originator')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_originator_id,
        format!(
            "test-originator-{}.example.com",
            test_originator_id.simple()
        )
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert test deal
    sqlx::query!(
        r#"
        INSERT INTO deals (id, originator_id, investor_id, principal_amount, outstanding_balance,
                          interest_rate_bps, status, originated_at)
        VALUES ($1, $2, $3, 50000.00, 50000.00, 1200, 'active', NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
        test_deal_id,
        test_originator_id, // originator (different from investor)
        test_investor_id
    )
    .execute(&pool)
    .await
    .unwrap();

    println!("✅ Test data created");

    // Test 1: Create a deal listing
    println!("📝 Creating deal listing...");
    let listing = arda_credit_db::marketplace::create_deal_listing(
        &pool,
        test_deal_id,
        test_investor_id,
        Decimal::new(47500, 2), // $475.00 asking price
        Some("Test listing for remove functionality".to_string()),
        None, // no expiration
    )
    .await
    .unwrap();

    println!("✅ Deal listing created with ID: {}", listing.listing_id);
    assert_eq!(listing.status, "active");

    // Verify deal status changed to 'for_sale'
    let deal_status = sqlx::query!("SELECT status FROM deals WHERE id = $1", test_deal_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("📊 Deal status after listing: {:?}", deal_status.status);
    assert_eq!(deal_status.status, Some("for_sale".to_string()));

    // Test 2: Verify we can retrieve the active listing
    println!("🔍 Verifying listing exists...");
    let retrieved = arda_credit_db::marketplace::get_deal_listing_by_deal_id(&pool, test_deal_id)
        .await
        .unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().status, "active");

    // Test 3: Remove the listing (this should set status to 'delisted')
    println!("🗑️ Removing deal listing...");
    let remove_result =
        arda_credit_db::marketplace::remove_deal_listing(&pool, test_deal_id, test_investor_id)
            .await
            .unwrap();

    println!("✅ Remove result: {}", remove_result);
    assert!(remove_result); // Should return true for successful removal

    // Test 4: Verify listing status changed to 'delisted'
    println!("🔍 Verifying listing status after removal...");
    let updated_listing = sqlx::query!(
        "SELECT status, delisted_at FROM deal_listings WHERE deal_id = $1",
        test_deal_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    println!(
        "📊 Listing status after removal: {}",
        updated_listing.status
    );
    assert_eq!(updated_listing.status, "delisted");
    assert!(updated_listing.delisted_at.is_some()); // Should have delisted_at timestamp

    // Test 5: Verify deal status changed back to 'active'
    let updated_deal_status = sqlx::query!("SELECT status FROM deals WHERE id = $1", test_deal_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!(
        "📊 Deal status after removal: {:?}",
        updated_deal_status.status
    );
    assert_eq!(updated_deal_status.status, Some("active".to_string()));

    // Test 6: Verify listing no longer appears in marketplace browse
    println!("🏪 Verifying listing doesn't appear in marketplace...");
    let marketplace_deals =
        arda_credit_db::marketplace::browse_marketplace_deals(&pool, Some(10), None)
            .await
            .unwrap();

    let delisted_deal_found = marketplace_deals
        .iter()
        .any(|deal| deal.deal_id == test_deal_id);
    println!(
        "📊 Delisted deal found in marketplace: {}",
        delisted_deal_found
    );
    assert!(!delisted_deal_found); // Should not appear in marketplace

    // Test 7: Test validation for transfer (should fail for delisted deals)
    println!("🔒 Verifying delisted deal cannot be transferred...");
    let transfer_validation =
        arda_credit_db::marketplace::validate_deal_for_transfer(&pool, test_deal_id)
            .await
            .unwrap();
    println!("📊 Transfer validation result: {:?}", transfer_validation);
    assert!(transfer_validation.is_none()); // Should not be valid for transfer

    println!("🎉 All tests passed! Remove from sale functionality works correctly.");

    // Clean up test data (delete in reverse dependency order)
    sqlx::query!("DELETE FROM deal_listings WHERE deal_id = $1", test_deal_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query!("DELETE FROM deals WHERE id = $1", test_deal_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query!(
        "DELETE FROM balances WHERE identity_id = $1",
        test_investor_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "DELETE FROM balances WHERE identity_id = $1",
        test_originator_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "DELETE FROM user_balances WHERE user_id = $1",
        test_investor_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "DELETE FROM user_balances WHERE user_id = $1",
        test_originator_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("DELETE FROM identities WHERE id = $1", test_investor_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query!("DELETE FROM identities WHERE id = $1", test_originator_id)
        .execute(&pool)
        .await
        .unwrap();

    println!("🧹 Test data cleaned up");
}
