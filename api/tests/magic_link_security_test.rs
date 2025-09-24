//! Security tests for magic link authentication
//!
//! Tests rate limiting, CSRF protection, and other security measures

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::Value;
use std::time::{Duration as StdDuration, Instant};
use tokio::time::sleep;
use tower::ServiceExt;

use api::magic_link_types::{MagicLinkRequest, VerifyMagicLinkRequest};
use api::test_utils::{
    create_test_app_with_magic_links, create_test_user, TestApp, TestContext, TestMagicLink,
    TestUser,
};
use api::types::ApiState;

/// Test context for magic link security tests
struct SecurityTestContext {
    pub app: TestApp,
    pub pool: sqlx::PgPool,
    pub state: ApiState,
}

impl SecurityTestContext {
    async fn new() -> Self {
        let ctx = TestContext::new().await;
        let app = create_test_app_with_magic_links(ctx.state.clone()).await;
        Self {
            app,
            pool: ctx.pool,
            state: ctx.state,
        }
    }

    async fn create_user(&self, email: &str, user_type: &str) -> TestUser {
        create_test_user(&self.pool, email, user_type).await
    }

    async fn make_request(
        &self,
        request: &MagicLinkRequest,
    ) -> axum::http::Response<axum::body::Body> {
        self.app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/magic-link")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(axum::body::Body::from(
                        serde_json::to_string(request).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn make_verify_request(
        &self,
        request: &VerifyMagicLinkRequest,
    ) -> axum::http::Response<axum::body::Body> {
        self.app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/verify")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(request).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn test_email_rate_limiting() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    let request = MagicLinkRequest {
        email: "test@example.com".to_string(),
        redirect_uri: None,
    };

    // First 3 requests should succeed (rate limit is 3 per hour)
    for i in 0..3 {
        let response = ctx.make_request(&request).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Request {} should succeed",
            i + 1
        );

        // Small delay to avoid timing issues
        sleep(StdDuration::from_millis(10)).await;
    }

    // 4th request should be rate limited
    let response = ctx.make_request(&request).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let body: Value = parse_response_body(response).await;
    assert_eq!(body["error"], "RATE_LIMIT_EXCEEDED");
    assert!(body["message"].as_str().unwrap().contains("rate limit"));
}

#[tokio::test]
async fn test_email_rate_limiting_different_ips() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Test with different IP addresses - should still be limited per email
    let ips = vec!["192.168.1.1", "192.168.1.2", "192.168.1.3", "192.168.1.4"];

    for (i, ip) in ips.iter().enumerate() {
        let response = ctx
            .app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/magic-link")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", *ip)
                    .body(axum::body::Body::from(r#"{"email":"test@example.com"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        if i < 3 {
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Request {} from IP {} should succeed",
                i + 1,
                ip
            );
        } else {
            assert_eq!(
                response.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "Request {} from IP {} should be rate limited",
                i + 1,
                ip
            );
        }

        sleep(StdDuration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn test_csrf_state_validation() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Request magic link
    let request = MagicLinkRequest {
        email: "test@example.com".to_string(),
        redirect_uri: None,
    };

    let response = ctx.make_request(&request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get the magic link from database to extract state
    let magic_link = get_latest_magic_link(&ctx.pool, user.id).await;

    // Try to verify with wrong state parameter
    let verify_request = VerifyMagicLinkRequest {
        token: magic_link.token,
        state: "wrong-state-parameter".to_string(),
        redirect_uri: None,
    };

    let response = ctx.make_verify_request(&verify_request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = parse_response_body(response).await;
    assert_eq!(body["error"], "INVALID_STATE");
    assert!(body["message"].as_str().unwrap().contains("state"));
}

#[tokio::test]
async fn test_csrf_state_required() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Get a magic link
    let magic_link = get_latest_magic_link(&ctx.pool, user.id).await;

    // Try to verify without state parameter
    let verify_request = VerifyMagicLinkRequest {
        token: magic_link.token,
        state: "".to_string(),
        redirect_uri: None,
    };

    let response = ctx.make_verify_request(&verify_request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = parse_response_body(response).await;
    assert_eq!(body["error"], "INVALID_STATE");
}

#[tokio::test]
async fn test_concurrent_token_limits() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Create 3 magic links (max allowed per user)
    for _ in 0..3 {
        let request = MagicLinkRequest {
            email: "test@example.com".to_string(),
            redirect_uri: None,
        };
        let response = ctx.make_request(&request).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Small delay to avoid rate limiting
        sleep(StdDuration::from_millis(100)).await;
    }

    // 4th request should fail due to concurrent token limit
    let request = MagicLinkRequest {
        email: "test@example.com".to_string(),
        redirect_uri: None,
    };
    let response = ctx.make_request(&request).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let body: Value = parse_response_body(response).await;
    assert!(
        body["error"].as_str().unwrap().contains("TOO_MANY")
            || body["error"].as_str().unwrap().contains("RATE")
    );
}

#[tokio::test]
async fn test_session_security_attributes() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Complete auth flow to get session
    let (session_cookie, _) = complete_auth_flow(&ctx.app, &ctx.pool, user.id).await;

    // Verify session cookie has proper security attributes
    assert!(
        session_cookie.contains("HttpOnly"),
        "Session cookie should be HttpOnly"
    );
    assert!(
        session_cookie.contains("Secure"),
        "Session cookie should be Secure"
    );
    assert!(
        session_cookie.contains("SameSite=Lax"),
        "Session cookie should have SameSite=Lax"
    );

    // Test that session works for authenticated requests
    let response = make_authenticated_request(&ctx.app, &session_cookie, "/api/v1/auth/me").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Test session invalidation on logout
    let logout_response =
        make_authenticated_request(&ctx.app, &session_cookie, "/api/v1/auth/logout").await;
    assert_eq!(logout_response.status(), StatusCode::OK);

    // Session should no longer work
    let me_response =
        make_authenticated_request(&ctx.app, &session_cookie, "/api/v1/auth/me").await;
    assert_eq!(me_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_expiry() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Create a session that expires in 1 second
    let session_token = create_short_lived_session(&ctx.pool, user.id).await;

    // Session should work initially
    let response = make_authenticated_request(&ctx.app, &session_token, "/api/v1/auth/me").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Wait for session to expire
    sleep(StdDuration::from_secs(2)).await;

    // Session should no longer work
    let response = make_authenticated_request(&ctx.app, &session_token, "/api/v1/auth/me").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_session_tokens() {
    let ctx = SecurityTestContext::new().await;

    // Test various invalid session tokens
    let toolong_token = "a".repeat(100);
    let invalid_tokens = vec![
        "",
        "invalid-token",
        "too-short",
        &toolong_token, // Too long
        "special-chars-@#$%",
        "unicode-🚀-test",
    ];

    for token in invalid_tokens {
        let response = make_authenticated_request(&ctx.app, token, "/api/v1/auth/me").await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "Token '{}' should result in UNAUTHORIZED",
            token
        );
    }
}

#[tokio::test]
async fn test_brute_force_token_attempts() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;

    // Get a valid magic link
    let magic_link = get_latest_magic_link(&ctx.pool, user.id).await;

    // Try many invalid tokens
    for i in 0..10 {
        let invalid_token = format!("invalid-token-{}", i);
        let verify_request = VerifyMagicLinkRequest {
            token: invalid_token,
            state: magic_link.state.clone(),
            redirect_uri: None,
        };

        let response = ctx.make_verify_request(&verify_request).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Invalid token {} should result in BAD_REQUEST",
            i
        );
    }

    // Valid token should still work
    let verify_request = VerifyMagicLinkRequest {
        token: magic_link.token,
        state: magic_link.state,
        redirect_uri: None,
    };

    let response = ctx.make_verify_request(&verify_request).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_timing_attack_resistance() {
    let ctx = SecurityTestContext::new().await;

    let user = ctx.create_user("test@example.com", "investor").await;
    let magic_link = get_latest_magic_link(&ctx.pool, user.id).await;

    let start = Instant::now();

    // Test with valid token
    let valid_request = VerifyMagicLinkRequest {
        token: magic_link.token.clone(),
        state: magic_link.state.clone(),
        redirect_uri: None,
    };

    let response1 = ctx.make_verify_request(&valid_request).await;
    let valid_time = start.elapsed();

    // Reset timer
    let start = Instant::now();

    // Test with invalid token
    let invalid_request = VerifyMagicLinkRequest {
        token: "invalid-token-12345678901234567890123456789012".to_string(),
        state: magic_link.state,
        redirect_uri: None,
    };

    let response2 = ctx.make_verify_request(&invalid_request).await;
    let invalid_time = start.elapsed();

    // Both should fail or succeed consistently in timing
    // (This is a basic check - real timing attack protection would be more sophisticated)
    assert!(
        valid_time.as_millis().abs_diff(invalid_time.as_millis()) < 100,
        "Timing difference too large: valid={}ms, invalid={}ms",
        valid_time.as_millis(),
        invalid_time.as_millis()
    );

    assert_eq!(response1.status(), StatusCode::OK);
    assert_eq!(response2.status(), StatusCode::BAD_REQUEST);
}

// Helper functions

async fn parse_response_body(response: axum::http::Response<axum::body::Body>) -> Value {
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body_bytes).unwrap()
}

async fn get_latest_magic_link(pool: &sqlx::PgPool, user_id: uuid::Uuid) -> TestMagicLink {
    // First ensure there's a magic link
    sqlx::query!(
        r#"
        INSERT INTO magic_links (user_id, token_hash, state, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
        user_id,
        "dummy-hash".as_bytes(),
        "test-state-1234567890123456789012345678901",
        Utc::now() + Duration::minutes(10)
    )
    .execute(pool)
    .await
    .unwrap();

    let row = sqlx::query!(
        r#"
        SELECT id, state, expires_at, created_at
        FROM magic_links
        WHERE user_id = $1 AND consumed_at IS NULL
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .unwrap();

    TestMagicLink {
        id: row.id,
        user_id,
        token: "test-token-12345678901234567890123456789012".to_string(),
        state: row.state,
        redirect_uri: None,
        expires_at: row.expires_at,
        created_at: row.created_at.expect("created_at should always be set"),
    }
}

async fn complete_auth_flow(
    app: &TestApp,
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> (String, TestMagicLink) {
    // Get magic link
    let magic_link = get_latest_magic_link(pool, user_id).await;

    // Verify magic link
    let verify_request = VerifyMagicLinkRequest {
        token: magic_link.token.clone(),
        state: magic_link.state.clone(),
        redirect_uri: None,
    };

    let verify_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&verify_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify_response.status(), StatusCode::OK);

    // Extract session cookie
    let cookies: Vec<_> = verify_response
        .headers()
        .get_all("set-cookie")
        .iter()
        .collect();

    let session_cookie = cookies
        .iter()
        .find(|cookie| cookie.to_str().unwrap().contains("session_token"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    (session_cookie, magic_link)
}

async fn make_authenticated_request(
    app: &TestApp,
    session_cookie: &str,
    uri: &str,
) -> axum::http::Response<axum::body::Body> {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .header(
            "cookie",
            format!("session_token={}", extract_session_token(session_cookie)),
        )
        .body(axum::body::Body::empty())
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

fn extract_session_token(cookie_header: &str) -> &str {
    if let Some(start) = cookie_header.find("session_token=") {
        let start = start + "session_token=".len();
        if let Some(end) = cookie_header[start..].find(';') {
            &cookie_header[start..start + end]
        } else {
            &cookie_header[start..]
        }
    } else {
        ""
    }
}

async fn create_short_lived_session(pool: &sqlx::PgPool, user_id: uuid::Uuid) -> String {
    let session_token =
        "short-lived-session-token-123456789012345678901234567890123456789012345678901234567890";
    let expires_at = Utc::now() + Duration::seconds(1); // Expires in 1 second

    sqlx::query!(
        r#"
        INSERT INTO sessions (user_id, session_token, expires_at)
        VALUES ($1, $2, $3)
        "#,
        user_id,
        session_token,
        expires_at
    )
    .execute(pool)
    .await
    .unwrap();

    session_token.to_string()
}
