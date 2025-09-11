//! Integration tests for loan management endpoints
//!
//! Tests for loan creation, retrieval, updates, and payment processing.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::{get, post, put},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

use crate::test_utils::*;
use crate::loan_handlers::{
    create_loan_endpoint, get_loan_details_endpoint, list_loans_endpoint,
    update_loan_endpoint, record_payment_endpoint, get_loan_payments_endpoint,
};
use crate::types::ApiState;

/// Test creating a new loan
#[tokio::test]
async fn test_create_loan() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test merchant
    let test_merchant = TestUser::new_merchant();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_merchant).await.unwrap();
    let merchant_id = factory.create_merchant_profile(
        test_merchant.id,
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
        .route("/api/v1/loans", post(create_loan_endpoint))
        .with_state(api_state);

    let loan_request = json!({
        "merchant_id": merchant_id,
        "pool_id": pool_id,
        "principal_amount": "10000.00",
        "interest_rate": "0.12",
        "term_months": 12,
        "purpose": "Working capital"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/loans")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&loan_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("loan_id").is_some());
    assert_eq!(response_json["status"], "PENDING");
    assert_eq!(response_json["principal_amount"], "10000.00");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting loan details
#[tokio::test]
async fn test_get_loan_details() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // This would need a loan to exist in the database
    // For now, we'll test the endpoint structure
    let app = Router::new()
        .route("/api/v1/loans/:loan_id", get(get_loan_details_endpoint))
        .with_state(api_state);

    let loan_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/loans/{}", loan_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent loan
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing loans with pagination
#[tokio::test]
async fn test_list_loans() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans", get(list_loans_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/loans?page=1&limit=10&status=ACTIVE")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("loans").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("page").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test updating loan status
#[tokio::test]
async fn test_update_loan() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans/:loan_id", put(update_loan_endpoint))
        .with_state(api_state);

    let loan_id = uuid::Uuid::new_v4();
    let update_request = json!({
        "status": "APPROVED",
        "approved_amount": "9500.00",
        "approved_rate": "0.11"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/loans/{}", loan_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent loan
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test recording loan payment
#[tokio::test]
async fn test_record_payment() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans/:loan_id/payments", post(record_payment_endpoint))
        .with_state(api_state);

    let loan_id = uuid::Uuid::new_v4();
    let payment_request = json!({
        "amount": "850.00",
        "payment_type": "PRINCIPAL_AND_INTEREST",
        "transaction_hash": "0x123456789abcdef"
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/loans/{}/payments", loan_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payment_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent loan
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting loan payment history
#[tokio::test]
async fn test_get_loan_payments() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans/:loan_id/payments", get(get_loan_payments_endpoint))
        .with_state(api_state);

    let loan_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/loans/{}/payments?limit=10&offset=0", loan_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent loan
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test loan creation with invalid data
#[tokio::test]
async fn test_create_loan_invalid_data() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans", post(create_loan_endpoint))
        .with_state(api_state);

    // Missing required fields
    let invalid_request = json!({
        "merchant_id": "invalid-uuid"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/loans")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test loan creation with negative amount
#[tokio::test]
async fn test_create_loan_negative_amount() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/loans", post(create_loan_endpoint))
        .with_state(api_state);

    let invalid_request = json!({
        "merchant_id": uuid::Uuid::new_v4(),
        "pool_id": uuid::Uuid::new_v4(),
        "principal_amount": "-1000.00",
        "interest_rate": "0.12",
        "term_months": 12
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/loans")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}
