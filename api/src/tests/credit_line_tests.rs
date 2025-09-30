//! Integration tests for credit line endpoints

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
use crate::handlers::credit::credit_lines::{
    check_credit_eligibility, submit_credit_line_request, list_credit_line_requests,
    browse_available_pools,
};
use crate::config::types::ApiState;

/// Test credit eligibility check
#[tokio::test]
async fn test_credit_eligibility_check() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/credit-lines/eligibility", get(check_credit_eligibility))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/credit-lines/eligibility")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("eligible").is_some());
    assert!(response_json.get("overall_score").is_some());
    assert!(response_json.get("checks").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test credit line request submission
#[tokio::test]
async fn test_submit_credit_line_request() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/credit-lines/request", post(submit_credit_line_request))
        .with_state(api_state);

    let request_body = json!({
        "requested_amount": 5000000,
        "requested_rate_apr": 1200,
        "requested_tenor_days": 30,
        "request_description": "Working capital for expansion"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/credit-lines/request")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("request_id").is_some());
    assert!(response_json.get("merchant_id").is_some());
    assert!(response_json.get("status").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing credit line requests
#[tokio::test]
async fn test_list_credit_line_requests() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/credit-lines/requests", get(list_credit_line_requests))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/credit-lines/requests?limit=10&offset=0")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("requests").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("has_more").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test browsing available pools
#[tokio::test]
async fn test_browse_available_pools() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools/available", get(browse_available_pools))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/pools/available?min_amount=1000000&max_ltv=8000")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let _response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Response should be an array of pools
    assert!(body_bytes.len() >= 2); // At least []

    // Cleanup
    test_db.cleanup().await.unwrap();
}
