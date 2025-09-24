//! Integration tests for authentication endpoints
//!
//! Tests for login, logout, registration, and JWT token validation.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use serde_json::json;
use tower::ServiceExt;

use crate::test_utils::*;
use crate::authentication_handlers::{
    login_endpoint, logout_endpoint, register_user_endpoint,
};
use crate::types::ApiState;

/// Test user registration endpoint with invite code
#[tokio::test]
async fn test_user_registration() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test router
    let app = Router::new()
        .route("/api/v1/auth/register", post(register_user_endpoint))
        .with_state(api_state);

    // Test data - use INVESTOR2025 invite code which creates investor users
    let request_body = json!({
        "email": "test@example.com",
        "password": "securepassword123",
        "invite_code": "INVESTOR2025"
    });

    // Create request
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    // Execute request
    let response = app.oneshot(request).await.unwrap();

    // Assert response
    assert_eq!(response.status(), StatusCode::CREATED);

    // Extract and validate response body
    let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("user_id").is_some());
    assert!(response_json.get("user_type").is_some());
    assert_eq!(response_json["user_type"], "investor"); // INVESTOR2025 creates investor users

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test user login endpoint
#[tokio::test]
async fn test_user_login() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // First register a user
    let register_request = json!({
        "email": "login_test@example.com",
        "password": "testpassword",
        "invite_code": "ORIGINATOR2025" // Creates originator users
    });

    // Register user first
    let register_app = Router::new()
        .route("/api/v1/auth/register", post(register_user_endpoint))
        .with_state(api_state.clone());

    let register_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&register_request).unwrap()))
        .unwrap();

    let _register_response = register_app.oneshot(register_req).await.unwrap();

    // Now test login
    let login_app = Router::new()
        .route("/api/v1/auth/login", post(login_endpoint))
        .with_state(api_state);

    let login_request = json!({
        "email": "login_test@example.com",
        "password": "testpassword"
    });

    let login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&login_request).unwrap()))
        .unwrap();

    let login_response = login_app.oneshot(login_req).await.unwrap();

    // Assert successful login
    assert_eq!(login_response.status(), StatusCode::OK);

    let body_bytes = hyper::body::to_bytes(login_response.into_body()).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(response_json.get("token").is_some());
    assert!(response_json.get("user").is_some());
    assert_eq!(response_json["user"]["email"], "login_test@example.com");

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test login with invalid credentials
#[tokio::test]
async fn test_invalid_login() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/auth/login", post(login_endpoint))
        .with_state(api_state);

    let invalid_request = json!({
        "email": "nonexistent@example.com",
        "password": "wrongpassword"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test logout endpoint
#[tokio::test]
async fn test_logout() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/auth/logout", post(logout_endpoint))
        .with_state(api_state);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Logout should always succeed (even without valid token)
    assert_eq!(response.status(), StatusCode::OK);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test registration with invalid data
#[tokio::test]
async fn test_invalid_registration() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/auth/register", post(register_user_endpoint))
        .with_state(api_state);

    // Test with missing required fields
    let invalid_request = json!({
        "email": "invalid@example.com"
        // Missing password, user_type, etc.
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_request).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return bad request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test duplicate user registration
#[tokio::test]
async fn test_duplicate_registration() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    let app = Router::new()
        .route("/api/v1/auth/register", post(register_user_endpoint))
        .with_state(api_state.clone());

    let request_body = json!({
        "email": "duplicate@example.com",
        "password": "password123",
        "invite_code": "INVESTOR2025"
    });

    // First registration
    let request1 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::CREATED);

    // Second registration with same email
    let request2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();
    // Should fail due to duplicate email
    assert_eq!(response2.status(), StatusCode::CONFLICT);

    // Cleanup
    test_db.cleanup().await.unwrap();
}

/// Test registration fails without invite code
#[tokio::test]
async fn test_registration_fails_without_invite_code() {
    let test_db = TestDatabase::new().await;
    let api_state = create_test_api_state(&test_db.pool).await;

    // Create test router
    let app = Router::new()
        .route("/api/v1/auth/register", post(register_user_endpoint))
        .with_state(api_state);

    // Test data - missing invite_code (now required)
    let request_body = json!({
        "email": "test@example.com",
        "password": "securepassword123"
    });

    // Create request
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    // Execute request
    let response = app.oneshot(request).await.unwrap();

    // Assert response fails
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    test_db.cleanup().await.unwrap();
}
