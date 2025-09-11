//! Integration tests for admin endpoints

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

use crate::test_utils::*;
use crate::admin_handlers::{
    list_pending_kyc_endpoint, get_kyc_statistics_endpoint, bulk_kyc_approval_endpoint,
};
use crate::types::ApiState;

/// Test listing pending KYC applications
#[tokio::test]
async fn test_list_pending_kyc() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/admin/kyc/pending", get(list_pending_kyc_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/admin/kyc/pending?limit=10&offset=0")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("applications").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("has_more").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting KYC statistics
#[tokio::test]
async fn test_get_kyc_statistics() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/admin/kyc/stats", get(get_kyc_statistics_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/admin/kyc/stats")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("total_users").is_some());
    assert!(response_json.get("approved_count").is_some());
    assert!(response_json.get("pending_count").is_some());
    assert!(response_json.get("rejected_count").is_some());
    assert!(response_json.get("approval_rate").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test bulk KYC approval
#[tokio::test]
async fn test_bulk_kyc_approval() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/admin/kyc/bulk-approve", post(bulk_kyc_approval_endpoint))
        .with_state(api_state);

    let approval_request = json!({
        "user_ids": [uuid::Uuid::new_v4().to_string()],
        "notes": "Bulk approval for testing"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/kyc/bulk-approve")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&approval_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("approved_count").is_some());
    assert!(response_json.get("failed_count").is_some());
    assert!(response_json.get("message").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test bulk KYC approval with empty user list
#[tokio::test]
async fn test_bulk_kyc_approval_empty_list() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/admin/kyc/bulk-approve", post(bulk_kyc_approval_endpoint))
        .with_state(api_state);

    let approval_request = json!({
        "user_ids": [],
        "notes": "Empty list test"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/kyc/bulk-approve")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&approval_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}
