//! Deal API integration tests
//!
//! This module contains comprehensive integration tests for the Deal API,
//! including all consolidated marketplace and transfer functionality.
//!
//! ## Test Coverage
//!
//! - **Core Deal Operations**: Create, read, update, delete deals
//! - **Deal Marketplace**: Browse, list, and manage deal sales
//! - **Deal Transfers**: Ownership transfer and validation
//! - **Deal Payments**: Payment processing and history
//! - **Error Handling**: Invalid inputs and edge cases
//!
//! ## Recent Changes
//! Converted from loan-based tests to deal-based tests:
//! - All loan test functions → deal test functions
//! - Marketplace tests consolidated from separate file
//! - Transfer tests consolidated from separate file
//! - All endpoints updated to use deal terminology
//! - All field names updated (borrower→originator, lender→investor)
//!
//! ## Test Structure
//!
//! Tests are organized into functional groups:
//! 1. Core deal management tests
//! 2. Consolidated marketplace tests
//! 3. Consolidated transfer tests
//! 4. Payment processing tests
//! 5. Error handling tests
//! 6. Integration scenario tests

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::{get, post, put, delete},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

use crate::test_utils::*;
use crate::handlers::deals::deals::{
    create_deal_endpoint, get_deal_details_endpoint, list_deals_endpoint,
    update_deal_endpoint, delete_deal_endpoint, record_deal_payment_endpoint,
    get_deal_payment_history_endpoint,
    // Consolidated marketplace functions
    browse_deals_for_sale_endpoint, list_deal_for_sale_endpoint,
    remove_deal_from_sale_endpoint, deal_marketplace_stats_endpoint,
    // Consolidated transfer functions
    transfer_deal_ownership_endpoint, get_deal_transfer_history_endpoint,
    get_user_deal_transfer_history_endpoint, validate_deal_transfer_endpoint,
};
use crate::types::ApiState;

/// Test creating a new deal
#[tokio::test]
async fn test_create_deal_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test originator (merchant)
    let test_originator = TestUser::new_merchant();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_originator).await.unwrap();
    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create test liquidity pool
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_investor).await.unwrap();
    let pool_id = factory.create_liquidity_pool(
        test_investor.id,
        "Test Pool",
        0.8,
        0.12
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal", post(create_deal_endpoint))
        .with_state(api_state);

    let deal_request = json!({
        "originator_id": originator_id,
        "investor_id": test_investor.id,
        "principal_amount": "10000.00",
        "interest_rate_bps": 1200,
        "term_days": 365,
        "purpose": "Working capital"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/deal")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&deal_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("deal_id").is_some());
    assert_eq!(response_json["loan_status"], "originated");
    assert_eq!(response_json["principal_amount"], "10000.00");
    assert_eq!(response_json["originator_id"], originator_id.to_string());
    assert_eq!(response_json["investor_id"], test_investor.id.to_string());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting deal details
#[tokio::test]
async fn test_get_deal_details_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create a test deal first
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "originated"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id", get(get_deal_details_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert_eq!(response_json["originator_id"], originator_id.to_string());
    assert_eq!(response_json["investor_id"], test_investor.id.to_string());
    assert_eq!(response_json["principal_amount"], "10000.00");
    assert_eq!(response_json["loan_status"], "originated");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting deal details for non-existent deal
#[tokio::test]
async fn test_get_deal_details_not_found() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal/:deal_id", get(get_deal_details_endpoint))
        .with_state(api_state);

    let deal_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent deal
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing deals with pagination
#[tokio::test]
async fn test_list_deals_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data with multiple deals
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create multiple deals
    for i in 0..3 {
        factory.create_deal(
            originator_id,
            test_investor.id,
            rust_decimal::Decimal::new(1000000 + i * 100000, 2), // $10,000.00, $11,000.00, $12,000.00
            800 + i * 100, // 8%, 9%, 10% interest
            "originated"
        ).await.unwrap();
    }

    let app = Router::new()
        .route("/api/v1/deals", get(list_deals_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/deals?page=1&limit=10&loan_status=originated")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("deals").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("page").is_some());

    let deals = response_json["deals"].as_array().unwrap();
    assert_eq!(deals.len(), 3);

    // Verify deal structure
    for deal in deals {
        assert!(deal.get("deal_id").is_some());
        assert!(deal.get("originator_id").is_some());
        assert!(deal.get("investor_id").is_some());
        assert!(deal.get("principal_amount").is_some());
        assert!(deal.get("loan_status").is_some());
    }

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing deals with empty result set
#[tokio::test]
async fn test_list_deals_empty() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deals", get(list_deals_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/deals?page=1&limit=10&loan_status=active")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("deals").is_some());
    assert!(response_json.get("total_count").is_some());

    let deals = response_json["deals"].as_array().unwrap();
    assert_eq!(deals.len(), 0);
    assert_eq!(response_json["total_count"], 0);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test updating deal status
#[tokio::test]
async fn test_update_deal_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create a test deal first
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "originated"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id", put(update_deal_endpoint))
        .with_state(api_state);

    let update_request = json!({
        "outstanding_balance": "9500.00",
        "loan_status": "active"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert_eq!(response_json["outstanding_balance"], "9500.00");
    assert_eq!(response_json["loan_status"], "active");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test updating non-existent deal
#[tokio::test]
async fn test_update_deal_not_found() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal/:deal_id", put(update_deal_endpoint))
        .with_state(api_state);

    let deal_id = uuid::Uuid::new_v4();
    let update_request = json!({
        "outstanding_balance": "9500.00",
        "loan_status": "active"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent deal
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test deleting a deal
#[tokio::test]
async fn test_delete_deal_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create a test deal first
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "originated"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id", delete(delete_deal_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deal is deleted
    let get_app = Router::new()
        .route("/api/v1/deal/:deal_id", get(get_deal_details_endpoint))
        .with_state(api_state.clone());

    let get_request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}", deal_id))
        .body(Body::empty())
        .unwrap();

    let get_response = get_app.oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test recording deal payment
#[tokio::test]
async fn test_record_deal_payment_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create a test deal first
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/payments", post(record_deal_payment_endpoint))
        .with_state(api_state);

    let payment_request = json!({
        "amount": "850.00",
        "payment_type": "PRINCIPAL_AND_INTEREST",
        "transaction_hash": "0x123456789abcdef"
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/deal/{}/payments", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payment_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("payment_id").is_some());
    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert_eq!(response_json["amount"], "850.00");
    assert_eq!(response_json["payment_type"], "PRINCIPAL_AND_INTEREST");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test recording payment for non-existent deal
#[tokio::test]
async fn test_record_deal_payment_not_found() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/payments", post(record_deal_payment_endpoint))
        .with_state(api_state);

    let deal_id = uuid::Uuid::new_v4();
    let payment_request = json!({
        "amount": "850.00",
        "payment_type": "PRINCIPAL_AND_INTEREST",
        "transaction_hash": "0x123456789abcdef"
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/deal/{}/payments", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payment_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent deal
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting deal payment history
#[tokio::test]
async fn test_get_deal_payment_history_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create a test deal first
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    // Create some payments
    factory.create_deal_payment(
        deal_id,
        test_originator.id,
        rust_decimal::Decimal::new(85000, 2), // $850.00
        "PRINCIPAL_AND_INTEREST",
        Some("0x123456789abcdef".to_string())
    ).await.unwrap();

    factory.create_deal_payment(
        deal_id,
        test_originator.id,
        rust_decimal::Decimal::new(85000, 2), // $850.00
        "PRINCIPAL_AND_INTEREST",
        Some("0x987654321fedcba".to_string())
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/payments", get(get_deal_payment_history_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}/payments?limit=10&offset=0", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("payments").is_some());
    assert!(response_json.get("total_count").is_some());

    let payments = response_json["payments"].as_array().unwrap();
    assert_eq!(payments.len(), 2);

    // Verify payment structure
    for payment in payments {
        assert!(payment.get("payment_id").is_some());
        assert!(payment.get("deal_id").is_some());
        assert!(payment.get("amount").is_some());
        assert!(payment.get("payment_type").is_some());
    }

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting payment history for non-existent deal
#[tokio::test]
async fn test_get_deal_payment_history_not_found() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/payments", get(get_deal_payment_history_endpoint))
        .with_state(api_state);

    let deal_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}/payments?limit=10&offset=0", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent deal
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

/// Test deal creation with invalid originator ID
#[tokio::test]
async fn test_create_deal_invalid_originator_id() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal", post(create_deal_endpoint))
        .with_state(api_state);

    let invalid_request = json!({
        "originator_id": "invalid-uuid",
        "investor_id": "123e4567-e89b-12d3-a456-426614174000",
        "principal_amount": "10000.00",
        "interest_rate_bps": 1200,
        "term_days": 365
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/deal")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test deal creation with negative amount
#[tokio::test]
async fn test_create_deal_negative_amount() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal", post(create_deal_endpoint))
        .with_state(api_state);

    let invalid_request = json!({
        "originator_id": "123e4567-e89b-12d3-a456-426614174000",
        "investor_id": "987fcdeb-51a2-43d7-8920-123456789012",
        "principal_amount": "-1000.00",
        "interest_rate_bps": 1200,
        "term_days": 365
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/deal")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test deal creation with missing required fields
#[tokio::test]
async fn test_create_deal_missing_required_fields() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deal", post(create_deal_endpoint))
        .with_state(api_state);

    // Missing required fields
    let invalid_request = json!({
        "originator_id": "123e4567-e89b-12d3-a456-426614174000"
        // Missing investor_id, principal_amount, etc.
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/deal")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

// ============================================================================
// CONSOLIDATED MARKETPLACE TESTS (moved from separate test file)
// ============================================================================

/// Test browsing deals for sale
#[tokio::test]
async fn test_browse_deals_for_sale_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    // Create and list a deal for sale
    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    // List the deal for sale
    factory.create_deal_listing(
        deal_id,
        test_investor.id,
        rust_decimal::Decimal::new(950000, 2), // $9,500.00 asking price
        Some("Great investment opportunity".to_string()),
        None
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deals/for-sale", get(browse_deals_for_sale_endpoint))
        .with_state(api_state);

    let response = app
        .oneshot(Request::builder()
            .method("GET")
            .uri("/api/v1/deals/for-sale")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("deals").is_some());
    let deals = response_json["deals"].as_array().unwrap();
    assert_eq!(deals.len(), 1);

    let deal = &deals[0];
    assert_eq!(deal["deal_id"], deal_id.to_string());
    assert!(deal.get("asking_price").is_some());
    assert!(deal.get("seller_id").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing a deal for sale
#[tokio::test]
async fn test_list_deal_for_sale_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/list-for-sale", post(list_deal_for_sale_endpoint))
        .with_state(api_state);

    let listing_request = json!({
        "seller_id": test_investor.id,
        "asking_price": "9500.00",
        "description": "Excellent deal opportunity",
        "expires_at": null
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/deal/{}/list-for-sale", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&listing_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("listing_id").is_some());
    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert_eq!(response_json["asking_price"], "9500.00");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test removing a deal from sale
#[tokio::test]
async fn test_remove_deal_from_sale_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data and list deal for sale
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    factory.create_deal_listing(
        deal_id,
        test_investor.id,
        rust_decimal::Decimal::new(950000, 2), // $9,500.00 asking price
        Some("Great investment opportunity".to_string()),
        None
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/remove-from-sale", delete(remove_deal_from_sale_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/v1/deal/{}/remove-from-sale", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify deal is no longer listed
    let browse_app = Router::new()
        .route("/api/v1/deals/for-sale", get(browse_deals_for_sale_endpoint))
        .with_state(api_state.clone());

    let browse_response = browse_app
        .oneshot(Request::builder()
            .method("GET")
            .uri("/api/v1/deals/for-sale")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    let body_bytes = hyper::body::to_bytes(browse_response.into_body()).await.unwrap();
    let browse_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let deals = browse_json["deals"].as_array().unwrap();
    assert_eq!(deals.len(), 0);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test deal marketplace statistics
#[tokio::test]
async fn test_deal_marketplace_stats_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/deals/marketplace/stats", get(deal_marketplace_stats_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/deals/marketplace/stats")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("total_listings").is_some());
    assert!(response_json.get("active_listings").is_some());
    assert!(response_json.get("total_volume").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

// ============================================================================
// CONSOLIDATED TRANSFER TESTS (moved from separate test file)
// ============================================================================

/// Test transferring deal ownership
#[tokio::test]
async fn test_transfer_deal_ownership_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_current_investor = TestUser::new_investor();
    let test_new_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_current_investor).await.unwrap();
    factory.create_user(&test_new_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    let deal_id = factory.create_deal(
        originator_id,
        test_current_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/transfer", post(transfer_deal_ownership_endpoint))
        .with_state(api_state);

    let transfer_request = json!({
        "from_investor_id": test_current_investor.id,
        "to_investor_id": test_new_investor.id,
        "transfer_price": "9500.00"
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/deal/{}/transfer", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&transfer_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("transfer_id").is_some());
    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert_eq!(response_json["from_investor_id"], test_current_investor.id.to_string());
    assert_eq!(response_json["to_investor_id"], test_new_investor.id.to_string());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting deal transfer history
#[tokio::test]
async fn test_get_deal_transfer_history_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data and transfer
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_current_investor = TestUser::new_investor();
    let test_new_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_current_investor).await.unwrap();
    factory.create_user(&test_new_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    let deal_id = factory.create_deal(
        originator_id,
        test_current_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    factory.create_deal_transfer(
        deal_id,
        test_current_investor.id,
        test_new_investor.id,
        rust_decimal::Decimal::new(950000, 2), // $9,500.00
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/transfers", get(get_deal_transfer_history_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/deal/{}/transfers?limit=10&offset=0", deal_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("transfers").is_some());
    assert!(response_json.get("total_count").is_some());

    let transfers = response_json["transfers"].as_array().unwrap();
    assert_eq!(transfers.len(), 1);

    let transfer = &transfers[0];
    assert_eq!(transfer["deal_id"], deal_id.to_string());
    assert_eq!(transfer["from_investor_id"], test_current_investor.id.to_string());
    assert_eq!(transfer["to_investor_id"], test_new_investor.id.to_string());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test validating deal transfer
#[tokio::test]
async fn test_validate_deal_transfer_success() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test data
    let factory = TestDataFactory::new().await;
    let test_originator = TestUser::new_merchant();
    let test_investor = TestUser::new_investor();
    factory.create_user(&test_originator).await.unwrap();
    factory.create_user(&test_investor).await.unwrap();

    let originator_id = factory.create_merchant_profile(
        test_originator.id,
        "Retail",
        50000.0
    ).await.unwrap();

    let deal_id = factory.create_deal(
        originator_id,
        test_investor.id,
        rust_decimal::Decimal::new(1000000, 2), // $10,000.00
        800, // 8% interest
        "active"
    ).await.unwrap();

    let app = Router::new()
        .route("/api/v1/deal/:deal_id/transfer/validate", post(validate_deal_transfer_endpoint))
        .with_state(api_state);

    let validation_request = json!({
        "current_investor_id": test_investor.id
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/deal/{}/transfer/validate", deal_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&validation_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["deal_id"], deal_id.to_string());
    assert!(response_json.get("is_valid").is_some());
    assert!(response_json.get("validation_errors").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

// ============================================================================
// TEST HELPER FUNCTIONS (updated for deals)
// ============================================================================

/// Create a test deal for testing purposes
async fn create_test_deal_helper(  // was create_test_loan
    factory: &TestDataFactory,
    originator_id: uuid::Uuid,  // was borrower_id
    investor_id: uuid::Uuid,    // was lender_id
    principal_amount: rust_decimal::Decimal,
    interest_rate_bps: i32,
    status: &str,
) -> uuid::Uuid {   // was Loan ID
    factory.create_deal(
        originator_id,
        investor_id,
        principal_amount,
        interest_rate_bps,
        status
    ).await.unwrap()
}

/// List a deal for sale for testing marketplace functionality
async fn list_deal_for_sale_helper(  // was list_loan_for_sale
    factory: &TestDataFactory,
    deal_id: uuid::Uuid,   // was loan_id
    seller_id: uuid::Uuid,
    asking_price: rust_decimal::Decimal,
) -> uuid::Uuid {  // was LoanListing ID
    factory.create_deal_listing(
        deal_id,
        seller_id,
        asking_price,
        Some("Test deal listing".to_string()),  // was "Test loan listing"
        None
    ).await.unwrap()
}

/// Create a test transfer for testing transfer functionality
async fn create_test_transfer_helper(  // was create_test_loan_transfer
    factory: &TestDataFactory,
    deal_id: uuid::Uuid,  // was loan_id
    from_investor_id: uuid::Uuid,
    to_investor_id: uuid::Uuid,
    transfer_price: rust_decimal::Decimal,
) -> uuid::Uuid {  // was LoanTransfer ID
    factory.create_deal_transfer(
        deal_id,
        from_investor_id,
        to_investor_id,
        transfer_price
    ).await.unwrap()
}
