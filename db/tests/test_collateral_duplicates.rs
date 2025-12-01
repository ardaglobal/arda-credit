//! Tests for collateral duplicate detection and bidirectional double pledge tracking

use db::taxonomy::collateral::{
    check_for_duplicates, create_collateral, get_collateral_by_id, process_duplicate_check,
    CreateCollateralRequest,
};
use db::test_utils::TestDatabase;

#[tokio::test]
async fn test_bidirectional_double_pledge_simple_check() {
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let pool = &test_db.pool;

    // Create first collateral
    let req1 = CreateCollateralRequest {
        name: "John Deere 9RX Series Tractors".to_string(),
        collateral_type: Some("mining_equipment".to_string()),
        description: Some("Heavy duty mining tractors".to_string()),
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral1 = create_collateral(&pool, req1)
        .await
        .expect("Failed to create first collateral");
    assert!(collateral1.double_pledged.is_none());

    // Create second collateral with same name
    let req2 = CreateCollateralRequest {
        name: "John Deere 9RX Series Tractors".to_string(),
        collateral_type: Some("mining_equipment".to_string()),
        description: Some("Duplicate tractors".to_string()),
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral2 = create_collateral(&pool, req2)
        .await
        .expect("Failed to create second collateral");

    // Run duplicate check on the second collateral
    check_for_duplicates(&pool, collateral2.id)
        .await
        .expect("Failed to check for duplicates");

    // Verify bidirectional references
    let updated_collateral1 = get_collateral_by_id(&pool, collateral1.id)
        .await
        .expect("Failed to get first collateral");
    let updated_collateral2 = get_collateral_by_id(&pool, collateral2.id)
        .await
        .expect("Failed to get second collateral");

    // Both should reference each other
    assert_eq!(
        updated_collateral2.double_pledged,
        Some(collateral1.id),
        "Second collateral should reference first"
    );
    assert_eq!(
        updated_collateral1.double_pledged,
        Some(collateral2.id),
        "First collateral should reference second (bidirectional)"
    );

    println!("✓ Bidirectional double pledge test passed!");
}

#[tokio::test]
async fn test_bidirectional_double_pledge_advanced_check() {
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let pool = &test_db.pool;

    // Create first collateral
    let req1 = CreateCollateralRequest {
        name: "Caterpillar D11 Bulldozer".to_string(),
        collateral_type: Some("heavy_equipment".to_string()),
        description: Some("Large bulldozer for mining".to_string()),
        address: Some("Mine Site A".to_string()),
        custom_id: Some("CAT-D11-001".to_string()),
        metadata: None,
        created_by: None,
    };

    let collateral1 = create_collateral(&pool, req1)
        .await
        .expect("Failed to create first collateral");

    // Create second collateral with same name but different details
    let req2 = CreateCollateralRequest {
        name: "Caterpillar D11 Bulldozer".to_string(),
        collateral_type: Some("heavy_equipment".to_string()),
        description: Some("Heavy construction bulldozer".to_string()),
        address: Some("Mine Site B".to_string()),
        custom_id: Some("CAT-D11-002".to_string()),
        metadata: None,
        created_by: None,
    };

    let collateral2 = create_collateral(&pool, req2)
        .await
        .expect("Failed to create second collateral");

    // Run advanced duplicate check on the second collateral
    process_duplicate_check(pool, collateral2.id)
        .await
        .expect("Failed to perform duplicate check");

    // Verify bidirectional references
    let updated_collateral1 = get_collateral_by_id(&pool, collateral1.id)
        .await
        .expect("Failed to get first collateral");
    let updated_collateral2 = get_collateral_by_id(&pool, collateral2.id)
        .await
        .expect("Failed to get second collateral");

    // Both should reference each other
    assert_eq!(
        updated_collateral2.double_pledged,
        Some(collateral1.id),
        "Second collateral should reference first"
    );
    assert_eq!(
        updated_collateral2.has_potential_duplicates,
        Some(true),
        "Second collateral should be flagged with potential duplicates"
    );
    assert_eq!(
        updated_collateral1.double_pledged,
        Some(collateral2.id),
        "First collateral should reference second (bidirectional)"
    );
    assert_eq!(
        updated_collateral1.has_potential_duplicates,
        Some(true),
        "First collateral should be flagged with potential duplicates"
    );

    println!("✓ Advanced bidirectional double pledge test passed!");
}

#[tokio::test]
async fn test_no_overwrite_existing_double_pledge() {
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let pool = &test_db.pool;

    // Create three collaterals with the same name
    let req1 = CreateCollateralRequest {
        name: "Generic Mining Equipment".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral1 = create_collateral(pool, req1)
        .await
        .expect("Failed to create first collateral");

    let req2 = CreateCollateralRequest {
        name: "Generic Mining Equipment".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral2 = create_collateral(pool, req2)
        .await
        .expect("Failed to create second collateral");

    // Link collateral2 to collateral1
    check_for_duplicates(&pool, collateral2.id)
        .await
        .expect("Failed to check for duplicates");

    // Verify initial bidirectional link
    let updated_collateral1 = get_collateral_by_id(&pool, collateral1.id)
        .await
        .expect("Failed to get first collateral");
    let updated_collateral2 = get_collateral_by_id(&pool, collateral2.id)
        .await
        .expect("Failed to get second collateral");

    assert_eq!(updated_collateral1.double_pledged, Some(collateral2.id));
    assert_eq!(updated_collateral2.double_pledged, Some(collateral1.id));

    // Create third collateral with same name
    let req3 = CreateCollateralRequest {
        name: "Generic Mining Equipment".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral3 = create_collateral(pool, req3)
        .await
        .expect("Failed to create third collateral");

    // Run check on third collateral
    check_for_duplicates(&pool, collateral3.id)
        .await
        .expect("Failed to check for duplicates");

    // Third collateral should link to first (or second)
    let updated_collateral3 = get_collateral_by_id(&pool, collateral3.id)
        .await
        .expect("Failed to get third collateral");
    assert!(updated_collateral3.double_pledged.is_some());

    // First and second should maintain their original links (not overwritten)
    let final_collateral1 = get_collateral_by_id(&pool, collateral1.id)
        .await
        .expect("Failed to get first collateral");
    let final_collateral2 = get_collateral_by_id(&pool, collateral2.id)
        .await
        .expect("Failed to get second collateral");

    // The WHERE clause includes "AND double_pledged IS NULL" so existing links aren't overwritten
    assert_eq!(
        final_collateral1.double_pledged,
        Some(collateral2.id),
        "First collateral should maintain link to second"
    );
    assert_eq!(
        final_collateral2.double_pledged,
        Some(collateral1.id),
        "Second collateral should maintain link to first"
    );

    println!("✓ No overwrite test passed - existing double pledges are preserved!");
}

#[tokio::test]
async fn test_unique_collateral_no_double_pledge() {
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let pool = &test_db.pool;

    // Create collateral with unique name
    let req = CreateCollateralRequest {
        name: "Unique Asset XYZ-12345".to_string(),
        collateral_type: Some("equipment".to_string()),
        description: Some("One of a kind".to_string()),
        address: None,
        custom_id: None,
        metadata: None,
        created_by: None,
    };

    let collateral = create_collateral(&pool, req)
        .await
        .expect("Failed to create collateral");

    // Run duplicate check
    check_for_duplicates(&pool, collateral.id)
        .await
        .expect("Failed to check for duplicates");

    // Verify no double pledge reference
    let updated_collateral = get_collateral_by_id(&pool, collateral.id)
        .await
        .expect("Failed to get collateral");

    assert!(
        updated_collateral.double_pledged.is_none(),
        "Unique collateral should not have double_pledged reference"
    );

    println!("✓ Unique collateral test passed - no false positives!");
}
