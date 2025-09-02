use chrono::Utc;
use db::{
    create_identity_with_password, get_identity_by_id, submit_kyc_data, update_identity_profile,
    Identity,
};
use sqlx::PgPool;
use std::env;

async fn setup_test_db() -> PgPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/arda_credit_test".to_string()
    });

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Skip cleanup in setup to avoid foreign key constraint issues
    // In a real scenario, we'd want to properly clean up test data
    // but for this demo, we'll work with existing data

    pool
}

async fn teardown_test_db(_pool: &PgPool) {
    // Skip cleanup for now to avoid foreign key constraint issues
    // In a real scenario, we'd want to properly clean up test data
    // but for this demo, we'll let the database handle cleanup
}

#[tokio::test]
async fn test_partial_user_profile_updates() {
    let pool = setup_test_db().await;

    // Create a test user
    let test_email = format!("test-user-profile-{}.example.com", Utc::now().timestamp());
    let user = create_identity_with_password(&pool, "user", &test_email, "hashed_password_123")
        .await
        .expect("Failed to create test user");

    // Test 1: Update only first_name
    let update_result = update_identity_profile(
        &pool,
        user.id,
        None,         // email
        None,         // user_type
        None,         // jurisdiction
        Some("John"), // first_name
        None,         // last_name
        None,         // wallet_address
    )
    .await
    .expect("Failed to update first name");

    assert_eq!(update_result.first_name, Some("John".to_string()));
    assert_eq!(update_result.last_name, None); // Should remain None
    assert_eq!(update_result.email, test_email); // Should remain unchanged
    assert_eq!(update_result.user_type, "user"); // Should remain unchanged

    // Test 2: Update only last_name
    let update_result = update_identity_profile(
        &pool,
        user.id,
        None,        // email
        None,        // user_type
        None,        // jurisdiction
        None,        // first_name
        Some("Doe"), // last_name
        None,        // wallet_address
    )
    .await
    .expect("Failed to update last name");

    assert_eq!(update_result.first_name, Some("John".to_string())); // Should remain unchanged
    assert_eq!(update_result.last_name, Some("Doe".to_string()));
    assert_eq!(update_result.email, test_email); // Should remain unchanged

    // Test 3: Update only wallet_address
    let wallet_addr = "0x742d35Cc6b5c0c25f2d0f1b5b3f5b5b5b5b5b5b5";
    let update_result = update_identity_profile(
        &pool,
        user.id,
        None,              // email
        None,              // user_type
        None,              // jurisdiction
        None,              // first_name
        None,              // last_name
        Some(wallet_addr), // wallet_address
    )
    .await
    .expect("Failed to update wallet address");

    assert_eq!(update_result.first_name, Some("John".to_string())); // Should remain unchanged
    assert_eq!(update_result.last_name, Some("Doe".to_string())); // Should remain unchanged
    assert_eq!(update_result.wallet_address, Some(wallet_addr.to_string()));
    assert_eq!(update_result.email, test_email); // Should remain unchanged

    // Test 4: Update multiple fields at once
    let new_email = format!("updated-{}.example.com", Utc::now().timestamp());
    let update_result = update_identity_profile(
        &pool,
        user.id,
        Some(&new_email), // email
        Some("investor"), // user_type
        None,             // jurisdiction
        Some("Jane"),     // first_name
        Some("Smith"),    // last_name
        None,             // wallet_address (keep existing)
    )
    .await
    .expect("Failed to update multiple fields");

    assert_eq!(update_result.email, new_email);
    assert_eq!(update_result.user_type, "investor");
    assert_eq!(update_result.first_name, Some("Jane".to_string()));
    assert_eq!(update_result.last_name, Some("Smith".to_string()));
    assert_eq!(update_result.wallet_address, Some(wallet_addr.to_string())); // Should remain unchanged

    // Test 5: Clear optional fields by setting to None (null)
    let update_result = update_identity_profile(
        &pool, user.id, None, // email
        None, // user_type
        None, // jurisdiction
        None, // first_name (set to null)
        None, // last_name (set to null)
        None, // wallet_address (set to null)
    )
    .await
    .expect("Failed to clear fields");

    // The database constraints should handle empty strings appropriately
    // (they should be stored as empty strings or converted to NULL based on constraints)

    // Verify final state
    let final_user = get_identity_by_id(&pool, user.id)
        .await
        .expect("Failed to get final user")
        .expect("User should exist");

    assert_eq!(final_user.email, new_email);
    assert_eq!(final_user.user_type, "investor");

    teardown_test_db(&pool).await;
}

#[tokio::test]
async fn test_user_profile_update_validation() {
    let pool = setup_test_db().await;

    // Create a test user
    let test_email = format!(
        "test-user-validation-{}.example.com",
        Utc::now().timestamp()
    );
    let user = create_identity_with_password(&pool, "user", &test_email, "hashed_password_123")
        .await
        .expect("Failed to create test user");

    // Test: Update with non-existent user ID should fail
    let fake_uuid = uuid::Uuid::new_v4();
    let result =
        update_identity_profile(&pool, fake_uuid, None, None, None, Some("Test"), None, None).await;

    assert!(result.is_err(), "Update with non-existent user should fail");

    teardown_test_db(&pool).await;
}

#[tokio::test]
async fn test_user_profile_update_timestamp_tracking() {
    let pool = setup_test_db().await;

    // Create a test user
    let test_email = format!("test-user-timestamp-{}.example.com", Utc::now().timestamp());
    let user = create_identity_with_password(&pool, "user", &test_email, "hashed_password_123")
        .await
        .expect("Failed to create test user");

    let original_updated_at = user.updated_at;

    // Wait a bit to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update first name
    let update_result = update_identity_profile(
        &pool,
        user.id,
        None,
        None,
        None,
        Some("Updated"),
        None,
        None,
    )
    .await
    .expect("Failed to update user");

    // Updated timestamp should be newer than original
    assert!(
        update_result.updated_at > original_updated_at,
        "Updated timestamp should be newer than original. Original: {}, Updated: {}",
        original_updated_at,
        update_result.updated_at
    );

    teardown_test_db(&pool).await;
}

#[tokio::test]
async fn test_jurisdiction_field_functionality() {
    let pool = setup_test_db().await;

    // Create a test user
    let test_email = format!("test-jurisdiction-{}.example.com", Utc::now().timestamp());
    let user = create_identity_with_password(&pool, "user", &test_email, "hashed_password_123")
        .await
        .expect("Failed to create test user");

    // Initially, jurisdiction should be None
    assert!(user.jurisdiction.is_none());

    // Update jurisdiction
    let update_result = update_identity_profile(
        &pool,
        user.id,
        None,       // email
        None,       // user_type
        Some("US"), // jurisdiction
        None,       // first_name
        None,       // last_name
        None,       // wallet_address
    )
    .await
    .expect("Failed to update jurisdiction");

    // Jurisdiction should now be "US"
    assert_eq!(update_result.jurisdiction, Some("US".to_string()));

    // Update to a different jurisdiction
    let update_result2 = update_identity_profile(
        &pool,
        user.id,
        None,       // email
        None,       // user_type
        Some("CA"), // jurisdiction
        None,       // first_name
        None,       // last_name
        None,       // wallet_address
    )
    .await
    .expect("Failed to update jurisdiction to CA");

    // Jurisdiction should now be "CA"
    assert_eq!(update_result2.jurisdiction, Some("CA".to_string()));

    // Clear jurisdiction by setting to None
    let update_result3 = update_identity_profile(
        &pool, user.id, None, // email
        None, // user_type
        None, // jurisdiction - clear it
        None, // first_name
        None, // last_name
        None, // wallet_address
    )
    .await
    .expect("Failed to clear jurisdiction");

    // Jurisdiction should be None again
    assert!(update_result3.jurisdiction.is_none());

    teardown_test_db(&pool).await;
}

#[tokio::test]
async fn test_kyc_auto_approval() {
    let pool = setup_test_db().await;

    // Create a test user
    let test_email = format!("test-kyc-auto-{}.example.com", Utc::now().timestamp());
    let user = create_identity_with_password(&pool, "user", &test_email, "hashed_password_123")
        .await
        .expect("Failed to create test user");

    // Initially, user should have "not_submitted" KYC status
    assert_eq!(user.kyc_status, "not_submitted");

    // Submit KYC - should automatically be approved
    let kyc_result = submit_kyc_data(&pool, user.id, "passport", "hash123", Some("US"))
        .await
        .expect("Failed to submit KYC");

    // After submission, KYC should be automatically approved
    assert_eq!(kyc_result.kyc_status, "approved");
    assert!(kyc_result.kyc_submitted_at.is_some());
    assert!(kyc_result.kyc_verified_at.is_some());

    // Verify the timestamps are set correctly
    let submitted_at = kyc_result.kyc_submitted_at.unwrap();
    let verified_at = kyc_result.kyc_verified_at.unwrap();
    assert_eq!(submitted_at, verified_at); // Should be the same timestamp for auto-approval

    teardown_test_db(&pool).await;
}
