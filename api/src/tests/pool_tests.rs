//! Integration tests for liquidity pool endpoints
//!
//! Tests for pool creation, management, transactions, and analytics.

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
use crate::handlers::pools::pools::{
    create_pool, get_pool_details, list_pools, update_pool_status,
    get_pool_utilization, get_pool_transactions, get_investor_dashboard,
    get_investor_pool_summaries, get_market_overview, get_pool_performance,
};
use crate::config::types::ApiState;

/// Test creating a liquidity pool
#[tokio::test]
async fn test_create_pool() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test investor
    let test_investor = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_investor).await.unwrap();

    let app = Router::new()
        .route("/api/v1/pools", post(create_pool))
        .with_state(api_state);

    let pool_request = json!({
        "name": "Test Investment Pool",
        "max_ltv_ratio": "0.80",
        "max_interest_rate": "0.12",
        "repayment_period_days": 30,
        "investor_id": test_investor.id
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/pools")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&pool_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("pool_id").is_some());
    assert_eq!(response_json["name"], "Test Investment Pool");
    assert_eq!(response_json["status"], "ACTIVE");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test listing pools with filters
#[tokio::test]
async fn test_list_pools() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools", get(list_pools))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/pools?status=ACTIVE&limit=10&offset=0")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("pools").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("page_info").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting pool details
#[tokio::test]
async fn test_get_pool_details() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools/:pool_id", get(get_pool_details))
        .with_state(api_state);

    let pool_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/pools/{}", pool_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent pool
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test updating pool status
#[tokio::test]
async fn test_update_pool_status() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools/:pool_id", put(update_pool_status))
        .with_state(api_state);

    let pool_id = uuid::Uuid::new_v4();
    let update_request = json!({
        "status": "PAUSED"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/pools/{}", pool_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent pool
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting pool utilization
#[tokio::test]
async fn test_get_pool_utilization() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools/:pool_id/utilization", get(get_pool_utilization))
        .with_state(api_state);

    let pool_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/pools/{}/utilization?days=30&interval=1 day", pool_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent pool
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting pool transactions
#[tokio::test]
async fn test_get_pool_transactions() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/pools/:pool_id/transactions", get(get_pool_transactions))
        .with_state(api_state);

    let pool_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/pools/{}/transactions?limit=10&offset=0", pool_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for non-existent pool
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test investor dashboard
#[tokio::test]
async fn test_investor_dashboard() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/investor/dashboard", get(get_investor_dashboard))
        .with_state(api_state);

    let investor_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/investor/dashboard?investor_id={}", investor_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST); // No investor found

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test market overview
#[tokio::test]
async fn test_market_overview() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/analytics/market-overview", get(get_market_overview))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/analytics/market-overview")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("platform_metrics").is_some());
    assert!(response_json.get("market_trends").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}
