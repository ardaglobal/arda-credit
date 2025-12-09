//! Integration tests for self double-pledge functionality
//!
//! Tests the ability to intentionally mark collateral as being used across multiple deals.

use arda_credit_db::{
    create_collateral, create_deal, get_collateral_with_deal_count, get_deals_by_collateral_id,
    list_multi_deal_collateral, Collateral, CreateCollateralRequest,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

#[tokio::test]
#[ignore] // Requires database
async fn test_multiple_deals_share_same_collateral() {
    let pool = arda_credit_db::init_db().await.unwrap();

    // Create a collateral
    let collateral_req = CreateCollateralRequest {
        name: "Test Property for Multiple Deals".to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Property intentionally used across multiple deals".to_string()),
        address: Some("123 Test St".to_string()),
        custom_id: Some("MULTI-001".to_string()),
        metadata: None,
        created_by: None,
    };

    let collateral = create_collateral(&pool, collateral_req).await.unwrap();

    // Create two deals that reference the same collateral
    let deal1 = create_test_deal(&pool, collateral.id).await;
    let deal2 = create_test_deal(&pool, collateral.id).await;

    // Get deals by collateral ID
    let deal_ids = get_deals_by_collateral_id(&pool, collateral.id)
        .await
        .unwrap();

    assert_eq!(deal_ids.len(), 2);
    assert!(deal_ids.contains(&deal1.id));
    assert!(deal_ids.contains(&deal2.id));

    // Get collateral with deal count
    let collateral_with_count = get_collateral_with_deal_count(&pool, collateral.id)
        .await
        .unwrap();

    assert_eq!(collateral_with_count.deal_count, 2);
    assert_eq!(collateral_with_count.deal_ids.len(), 2);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_list_multi_deal_collateral() {
    let pool = arda_credit_db::init_db().await.unwrap();

    // Create collateral with single deal
    let single_collateral = create_test_collateral(&pool, "SINGLE-001").await;
    let _deal1 = create_test_deal(&pool, single_collateral.id).await;

    // Create collateral with multiple deals
    let multi_collateral = create_test_collateral(&pool, "MULTI-002").await;
    let _deal2 = create_test_deal(&pool, multi_collateral.id).await;
    let _deal3 = create_test_deal(&pool, multi_collateral.id).await;
    let _deal4 = create_test_deal(&pool, multi_collateral.id).await;

    // List collateral with multiple deals
    let results = list_multi_deal_collateral(&pool, 10, 0).await.unwrap();

    // Should only return collateral with 2+ deals
    let multi_result = results
        .iter()
        .find(|r| r.collateral.id == multi_collateral.id);
    assert!(multi_result.is_some());
    assert_eq!(multi_result.unwrap().deal_count, 3);

    // Single-deal collateral should not appear
    let single_result = results
        .iter()
        .find(|r| r.collateral.id == single_collateral.id);
    assert!(single_result.is_none());
}

#[tokio::test]
#[ignore] // Requires database
async fn test_intentional_vs_detected_double_pledge() {
    let pool = arda_credit_db::init_db().await.unwrap();

    // Create collateral with intentional double pledge
    let collateral = create_test_collateral(&pool, "INTENT-002").await;

    // Create multiple deals
    let _deal1 = create_test_deal(&pool, collateral.id).await;
    let _deal2 = create_test_deal(&pool, collateral.id).await;

    // Verify flags
    let collateral_with_count = get_collateral_with_deal_count(&pool, collateral.id)
        .await
        .unwrap();

    assert_eq!(collateral_with_count.deal_count, 2);

    // Note: has_potential_duplicates is set by external detection service (arda-collateral-intel)
    // intentional_double_pledge can be set via the update endpoint if needed
    // They are independent flags that can coexist
}

// Helper functions

async fn create_test_collateral(pool: &sqlx::PgPool, custom_id: &str) -> Collateral {
    let req = CreateCollateralRequest {
        name: format!("Test Collateral {}", custom_id),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Test collateral".to_string()),
        address: Some("123 Test St".to_string()),
        custom_id: Some(custom_id.to_string()),
        metadata: None,
        created_by: None,
    };
    create_collateral(pool, req).await.unwrap()
}

async fn create_test_deal(pool: &sqlx::PgPool, collateral_id: Uuid) -> arda_credit_db::Deal {
    // Note: This is a simplified test helper. In production, you'd need proper
    // company IDs and user IDs. For testing, we create minimal valid deals.
    let originator_id = Uuid::new_v4();
    let investor_id = Uuid::new_v4();

    create_deal(
        pool,
        originator_id,
        Some(investor_id),
        None, // borrower_id
        Decimal::from_str("100000.00").unwrap(),
        500,           // interest_rate_bps
        None,          // maturity_date
        Some("draft"), // deal_status
        None,          // custom_deal_id
        Some("draft"), // status
        None,          // industry
        None,          // document_ids
        None,          // title
        None,          // description
        None,          // risk_level
        None,          // asset_class
        None,          // region
        None,          // minimum_investment
        Some(collateral_id),
    )
    .await
    .unwrap()
}
