//! Integration tests for user management endpoints
//!
//! Tests for user profile management, KYC submission, and user data retrieval.

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
use crate::handlers::users::users::{
    get_user_details_endpoint, get_user_balance_endpoint, submit_kyc_endpoint,
    update_user_endpoint, get_user_transactions_endpoint,
};
use crate::types::ApiState;

/// Test getting user details
#[tokio::test]
async fn test_get_user_details() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/users/:user_id", get(get_user_details_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/users/{}", test_user.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["email"], test_user.email);
    assert_eq!(response_json["user_type"], test_user.user_type);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting user details for non-existent user
#[tokio::test]
async fn test_get_nonexistent_user_details() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/users/:user_id", get(get_user_details_endpoint))
        .with_state(api_state);

    let nonexistent_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/users/{}", nonexistent_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test updating user profile
#[tokio::test]
async fn test_update_user_profile() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_merchant();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/users/:user_id", put(update_user_endpoint))
        .with_state(api_state);

    let update_request = json!({
        "first_name": "Updated",
        "last_name": "Name",
        "jurisdiction": "US"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/users/{}", test_user.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["first_name"], "Updated");
    assert_eq!(response_json["last_name"], "Name");
    assert_eq!(response_json["jurisdiction"], "US");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test KYC submission
#[tokio::test]
async fn test_submit_kyc() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/users/:user_id/kyc", post(submit_kyc_endpoint))
        .with_state(api_state);

    let kyc_request = json!({
        "document_type": "passport",
        "document_number": "P123456789",
        "issuing_country": "US",
        "document_images": ["image1.jpg", "image2.jpg"]
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/users/{}/kyc", test_user.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&kyc_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_json["kyc_status"], "PENDING");
    assert!(response_json.get("kyc_submitted_at").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting user balance
#[tokio::test]
async fn test_get_user_balance() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/users/:user_id/balance", get(get_user_balance_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/users/{}/balance", test_user.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Balance should be 0 for new user
    assert!(response_json.get("available_balance").is_some());
    assert!(response_json.get("total_balance").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting user transactions
#[tokio::test]
async fn test_get_user_transactions() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_merchant();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/users/:user_id/transactions", get(get_user_transactions_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/users/{}/transactions?limit=10&offset=0", test_user.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Should return empty transaction list for new user
    assert!(response_json.get("transactions").is_some());
    assert!(response_json.get("total_count").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test invalid user ID format
#[tokio::test]
async fn test_invalid_user_id_format() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/users/:user_id", get(get_user_details_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/users/invalid-uuid")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}
