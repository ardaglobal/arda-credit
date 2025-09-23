//! Performance tests for magic link authentication
//!
//! Tests system behavior under load and performance characteristics

use std::time::{Duration, Instant};
use tokio::time::sleep;
use futures::future::join_all;
use axum::http::StatusCode;

use crate::magic_link_types::{MagicLinkRequest, VerifyMagicLinkRequest};
use crate::test_utils::{TestContext, TestApp, create_test_app_with_magic_links, create_test_user};
use crate::types::ApiState;

/// Performance test context
struct PerformanceTestContext {
    pub app: TestApp,
    pub pool: sqlx::PgPool,
    pub state: ApiState,
}

impl PerformanceTestContext {
    async fn new() -> Self {
        let ctx = TestContext::new().await;
        let app = create_test_app_with_magic_links(ctx.state.clone()).await;
        Self {
            app,
            pool: ctx.pool,
            state: ctx.state,
        }
    }

    async fn create_multiple_users(&self, count: usize) -> Vec<crate::test_utils::TestUser> {
        let mut users = Vec::new();
        for i in 0..count {
            let email = format!("perf_test_{}@example.com", i);
            let user = create_test_user(&self.pool, &email, "investor").await;
            users.push(user);
        }
        users
    }
}

#[tokio::test]
async fn test_concurrent_magic_link_requests() {
    let ctx = PerformanceTestContext::new().await;

    // Create multiple test users
    let users = ctx.create_multiple_users(10).await;

    let start = Instant::now();

    // Make concurrent magic link requests
    let tasks: Vec<_> = users.into_iter().map(|user| {
        let app = ctx.app.clone();
        tokio::spawn(async move {
            let request = MagicLinkRequest {
                email: user.email,
                redirect_uri: None,
            };

            let response = app
                .oneshot(axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/magic-link")
                    .header("content-type", "application/json")
                    .body(serde_json::to_string(&request).unwrap().into())
                    .unwrap())
                .await
                .unwrap();

            (response.status(), user.email)
        })
    }).collect();

    let results = join_all(tasks).await;
    let duration = start.elapsed();

    // All requests should complete within reasonable time
    assert!(duration < Duration::from_secs(5),
            "Concurrent requests took too long: {:?}", duration);

    // Most requests should succeed (some may be rate limited)
    let success_count = results.iter()
        .filter(|(status, _)| status.is_success())
        .count();

    println!("Concurrent requests: {} successes out of {} total in {:?}",
             success_count, results.len(), duration);

    assert!(success_count >= results.len() / 2,
            "At least half of concurrent requests should succeed, got {}/{}",
            success_count, results.len());
}

#[tokio::test]
async fn test_session_validation_performance() {
    let ctx = PerformanceTestContext::new().await;

    // Create and authenticate a user to get session cookie
    let user = create_test_user(&ctx.pool, "perf@example.com", "investor").await;

    // Complete auth flow to get session
    let session_cookie = complete_auth_flow(&ctx.app, &ctx.pool, user.id).await;

    let start = Instant::now();
    let num_requests = 100;

    // Make many concurrent authenticated requests
    let tasks: Vec<_> = (0..num_requests).map(|_| {
        let app = ctx.app.clone();
        let cookie = session_cookie.clone();
        tokio::spawn(async move {
            make_authenticated_request(&app, &cookie, "/api/v1/auth/me").await
        })
    }).collect();

    let results = join_all(tasks).await;
    let duration = start.elapsed();

    println!("Processed {} authenticated requests in {:?}", num_requests, duration);

    // Should handle reasonable load
    assert!(duration < Duration::from_secs(10),
            "Session validation took too long: {:?}", duration);

    // All requests should succeed
    let success_count = results.iter()
        .filter(|result| matches!(result, Ok(response) if response.status().is_success()))
        .count();

    assert_eq!(success_count, num_requests,
               "All {} authenticated requests should succeed, got {}",
               num_requests, success_count);
}

#[tokio::test]
async fn test_magic_link_verification_performance() {
    let ctx = PerformanceTestContext::new().await;

    // Create multiple magic links for testing
    let mut magic_links = Vec::new();
    for i in 0..10 {
        let email = format!("verify_perf_{}@example.com", i);
        let user = create_test_user(&ctx.pool, &email, "investor").await;

        // Create magic link directly in database for testing
        let magic_link = create_test_magic_link(&ctx.pool, user.id).await;
        magic_links.push(magic_link);
    }

    let start = Instant::now();

    // Verify all magic links concurrently
    let tasks: Vec<_> = magic_links.into_iter().map(|magic_link| {
        let app = ctx.app.clone();
        tokio::spawn(async move {
            let request = VerifyMagicLinkRequest {
                token: magic_link.token,
                state: magic_link.state,
                redirect_uri: None,
            };

            app.oneshot(axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(serde_json::to_string(&request).unwrap().into())
                .unwrap())
            .await
        })
    }).collect();

    let results = join_all(tasks).await;
    let duration = start.elapsed();

    println!("Verified {} magic links in {:?}", results.len(), duration);

    // Should complete reasonably quickly
    assert!(duration < Duration::from_secs(3),
            "Magic link verification took too long: {:?}", duration);

    // All verifications should succeed
    let success_count = results.iter()
        .filter(|result| matches!(result, Ok(response) if response.status().is_success()))
        .count();

    assert_eq!(success_count, results.len(),
               "All magic link verifications should succeed");
}

#[tokio::test]
async fn test_database_performance_under_load() {
    let ctx = PerformanceTestContext::new().await;

    // Test magic link cleanup performance
    let start = Instant::now();

    // Create many expired magic links
    let expired_links_count = 100;
    create_many_expired_magic_links(&ctx.pool, expired_links_count).await;

    // Run cleanup (this would typically be done by a background job)
    // For testing, we'll simulate the cleanup logic
    let cleanup_result = cleanup_expired_magic_links(&ctx.pool).await;

    let duration = start.elapsed();

    assert!(cleanup_result.is_ok(), "Cleanup should succeed");
    assert!(duration < Duration::from_secs(5),
            "Cleanup took too long: {:?}", duration);

    println!("Cleaned up {} expired links in {:?}", cleanup_result.unwrap(), duration);

    // Verify cleanup worked
    let remaining_expired = count_expired_magic_links(&ctx.pool).await;
    assert_eq!(remaining_expired, 0, "All expired links should be cleaned up");
}

#[tokio::test]
async fn test_rate_limiting_performance() {
    let ctx = PerformanceTestContext::new().await;

    let user = create_test_user(&ctx.pool, "rate_limit@example.com", "investor").await;

    let start = Instant::now();

    // Make many requests quickly to test rate limiting performance
    let num_requests = 10; // More than the rate limit of 3
    let mut results = Vec::new();

    for i in 0..num_requests {
        let request = MagicLinkRequest {
            email: "rate_limit@example.com".to_string(),
            redirect_uri: None,
        };

        let response = ctx.app
            .clone()
            .oneshot(axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/magic-link")
                .header("content-type", "application/json")
                .body(serde_json::to_string(&request).unwrap().into())
                .unwrap())
            .await
            .unwrap();

        results.push(response.status());

        // Small delay between requests
        if i < num_requests - 1 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    let duration = start.elapsed();

    println!("Rate limiting test: {} requests in {:?}", num_requests, duration);

    // Should complete reasonably quickly even with rate limiting
    assert!(duration < Duration::from_secs(2),
            "Rate limiting test took too long: {:?}", duration);

    // Should have some successes and some rate limited responses
    let success_count = results.iter().filter(|status| status.is_success()).count();
    let rate_limited_count = results.iter().filter(|status| **status == StatusCode::TOO_MANY_REQUESTS).count();

    assert!(success_count >= 3, "Should allow at least 3 requests before rate limiting");
    assert!(rate_limited_count > 0, "Should have some rate limited requests");
}

#[tokio::test]
async fn test_memory_usage_under_load() {
    let ctx = PerformanceTestContext::new().await;

    // Create many concurrent sessions
    let num_sessions = 20;
    let mut session_cookies = Vec::new();

    for i in 0..num_sessions {
        let email = format!("memory_test_{}@example.com", i);
        let user = create_test_user(&ctx.pool, &email, "investor").await;

        // Complete auth flow
        let session_cookie = complete_auth_flow(&ctx.app, &ctx.pool, user.id).await;
        session_cookies.push(session_cookie);
    }

    let start = Instant::now();

    // Make many requests with different sessions concurrently
    let tasks: Vec<_> = session_cookies.iter().enumerate().map(|(i, cookie)| {
        let app = ctx.app.clone();
        let cookie = cookie.clone();
        tokio::spawn(async move {
            for _ in 0..5 { // 5 requests per session
                let response = make_authenticated_request(&app, &cookie, "/api/v1/auth/me").await;
                assert!(response.status().is_success());
            }
        })
    }).collect();

    let results = join_all(tasks).await;
    let duration = start.elapsed();

    println!("Memory usage test: {} sessions with {} requests each in {:?}",
             num_sessions, 5, duration);

    // All tasks should complete successfully
    for result in results {
        assert!(result.is_ok(), "All concurrent session requests should succeed");
    }

    // Should handle the load reasonably well
    assert!(duration < Duration::from_secs(10),
            "Concurrent session load test took too long: {:?}", duration);
}

#[tokio::test]
async fn test_token_generation_performance() {
    use crate::magic_link_service::MagicLinkService;

    let start = Instant::now();
    let num_tokens = 1000;

    // Generate many tokens
    for _ in 0..num_tokens {
        let _token = MagicLinkService::generate_token();
        let _state = MagicLinkService::generate_state();
    }

    let duration = start.elapsed();

    println!("Generated {} token pairs in {:?}", num_tokens, duration);

    // Token generation should be fast
    assert!(duration < Duration::from_secs(1),
            "Token generation took too long: {:?}", duration);

    // Test hash performance
    let test_token = "test-token-for-hashing-12345678901234567890123456789012";
    let start = Instant::now();

    for _ in 0..num_tokens {
        let _hash = MagicLinkService::hash_token(test_token);
    }

    let hash_duration = start.elapsed();

    println!("Hashed {} tokens in {:?}", num_tokens, hash_duration);

    // Hashing should also be fast
    assert!(hash_duration < Duration::from_secs(2),
            "Token hashing took too long: {:?}", hash_duration);
}

#[tokio::test]
async fn test_database_query_performance() {
    let ctx = PerformanceTestContext::new().await;

    // Create many test records
    let num_records = 50;
    for i in 0..num_records {
        let email = format!("query_perf_{}@example.com", i);
        let user = create_test_user(&ctx.pool, &email, "investor").await;

        // Create some magic links
        let _ = create_test_magic_link(&ctx.pool, user.id).await;
    }

    let start = Instant::now();

    // Test various database queries that would be common in magic link auth
    let user_count = sqlx::query!("SELECT COUNT(*) as count FROM identities")
        .fetch_one(&ctx.pool)
        .await
        .unwrap()
        .count
        .unwrap_or(0);

    let magic_link_count = sqlx::query!("SELECT COUNT(*) as count FROM magic_links")
        .fetch_one(&ctx.pool)
        .await
        .unwrap()
        .count
        .unwrap_or(0);

    let session_count = sqlx::query!("SELECT COUNT(*) as count FROM sessions")
        .fetch_one(&ctx.pool)
        .await
        .unwrap()
        .count
        .unwrap_or(0);

    let duration = start.elapsed();

    println!("Database queries completed in {:?}: {} users, {} magic links, {} sessions",
             duration, user_count, magic_link_count, session_count);

    // Database queries should be fast
    assert!(duration < Duration::from_millis(500),
            "Database queries took too long: {:?}", duration);

    assert!(user_count >= num_records as i64, "Should have at least {} users", num_records);
}

// Helper functions

async fn complete_auth_flow(app: &TestApp, pool: &sqlx::PgPool, user_id: uuid::Uuid) -> String {
    // Create magic link for the user
    let magic_link = create_test_magic_link(pool, user_id).await;

    // Verify magic link
    let verify_request = VerifyMagicLinkRequest {
        token: magic_link.token,
        state: magic_link.state,
        redirect_uri: None,
    };

    let response = app
        .clone()
        .oneshot(axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/verify")
            .header("content-type", "application/json")
            .body(serde_json::to_string(&verify_request).unwrap().into())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Extract session cookie
    let cookies: Vec<_> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .collect();

    let session_cookie = cookies.iter()
        .find(|cookie| cookie.to_str().unwrap().contains("session_token"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    session_cookie
}

async fn make_authenticated_request(app: &TestApp, session_cookie: &str, uri: &str) -> Result<axum::http::Response<axum::body::Body>, Box<dyn std::error::Error + Send + Sync>> {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .header("cookie", format!("session_token={}",
                 extract_session_token(session_cookie)))
        .body(axum::body::Body::empty())
        .unwrap();

    Ok(app.clone().oneshot(request).await?)
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

async fn create_test_magic_link(pool: &sqlx::PgPool, user_id: uuid::Uuid) -> crate::test_utils::TestMagicLink {
    use chrono::{Duration, Utc};

    let token = format!("perf-token-{}", uuid::Uuid::new_v4().simple());
    let state = format!("perf-state-{}", uuid::Uuid::new_v4().simple());
    let token_hash = crate::magic_link_service::MagicLinkService::hash_token(&token);
    let expires_at = Utc::now() + Duration::minutes(10);

    sqlx::query!(
        r#"
        INSERT INTO magic_links (user_id, token_hash, state, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
        user_id,
        token_hash,
        state,
        expires_at
    )
    .execute(pool)
    .await
    .unwrap();

    crate::test_utils::TestMagicLink {
        id: uuid::Uuid::new_v4(),
        user_id,
        token,
        state,
        redirect_uri: None,
        expires_at,
        created_at: Utc::now(),
    }
}

async fn create_many_expired_magic_links(pool: &sqlx::PgPool, count: usize) {
    use chrono::{Duration, Utc};

    let expired_time = Utc::now() - Duration::hours(1); // 1 hour ago

    for i in 0..count {
        let user_id = uuid::Uuid::new_v4();
        let token = format!("expired-token-{}", i);
        let state = format!("expired-state-{}", i);
        let token_hash = crate::magic_link_service::MagicLinkService::hash_token(&token);

        // Create user first
        sqlx::query!(
            r#"
            INSERT INTO identities (id, email, user_type, kyc_status, created_at, updated_at)
            VALUES ($1, $2, 'investor', 'NOT_SUBMITTED', NOW(), NOW())
            "#,
            user_id,
            format!("expired_user_{}@example.com", i)
        )
        .execute(pool)
        .await
        .unwrap();

        // Create expired magic link
        sqlx::query!(
            r#"
            INSERT INTO magic_links (user_id, token_hash, state, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
            user_id,
            token_hash,
            state,
            expired_time
        )
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn cleanup_expired_magic_links(pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM magic_links
        WHERE expires_at < NOW() AND consumed_at IS NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as i64)
}

async fn count_expired_magic_links(pool: &sqlx::PgPool) -> i64 {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM magic_links
        WHERE expires_at < NOW() AND consumed_at IS NULL
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    result.count.unwrap_or(0)
}
