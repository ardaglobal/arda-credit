//! Integration tests for financial transaction endpoints

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
use crate::financial_transaction_handlers::{
    record_account_deposit, record_account_withdrawal, get_account_financial_transactions,
    record_pool_funding, record_pool_withdrawal, get_pool_financial_transactions,
    record_credit_line_draw, record_credit_line_payment, get_credit_line_financial_transactions,
    get_user_financial_transactions, get_user_financial_summary,
};
use crate::types::ApiState;

/// Test recording account deposit
#[tokio::test]
async fn test_record_account_deposit() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/user/:user_id/financial/account/deposit", post(record_account_deposit))
        .with_state(api_state);

    let deposit_request = json!({
        "user_id": test_user.id,
        "transaction_type": "DEPOSIT",
        "amount_usdc": "1000.50",
        "transaction_hash": "0x123456789abcdef",
        "blockchain_status": "CONFIRMED",
        "block_number": 12345
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/user/{}/financial/account/deposit", test_user.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&deposit_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("id").is_some());
    assert_eq!(response_json["amount_usdc"], "1000.5");
    assert_eq!(response_json["transaction_type"], "DEPOSIT");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test recording account withdrawal
#[tokio::test]
async fn test_record_account_withdrawal() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/user/:user_id/financial/account/withdrawal", post(record_account_withdrawal))
        .with_state(api_state);

    let withdrawal_request = json!({
        "user_id": test_user.id,
        "transaction_type": "WITHDRAWAL",
        "amount_usdc": "500.00",
        "transaction_hash": "0xabcdef123456789",
        "blockchain_status": "PENDING"
    });

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/user/{}/financial/account/withdrawal", test_user.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&withdrawal_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("id").is_some());
    assert_eq!(response_json["amount_usdc"], "500");
    assert_eq!(response_json["transaction_type"], "WITHDRAWAL");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting account financial transactions
#[tokio::test]
async fn test_get_account_financial_transactions() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/user/:user_id/financial/account", get(get_account_financial_transactions))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/user/{}/financial/account?limit=10&offset=0", test_user.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("transactions").is_some());
    assert!(response_json.get("total_count").is_some());
    assert!(response_json.get("limit").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test getting user financial summary
#[tokio::test]
async fn test_get_user_financial_summary() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test user
    let test_user = TestUser::new_investor();
    let factory = TestDataFactory::new().await;
    factory.create_user(&test_user).await.unwrap();

    let app = Router::new()
        .route("/api/v1/user/:user_id/financial/summary", get(get_user_financial_summary))
        .with_state(api_state);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/user/{}/financial/summary", test_user.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("total_deposits_usdc").is_some());
    assert!(response_json.get("total_withdrawals_usdc").is_some());
    assert!(response_json.get("net_account_flow_usdc").is_some());

    // Cleanup
    test_db.cleanup().await.unwrap();
}
