//! Common testing utilities for API integration tests
//!
//! This module provides shared utilities for testing the Arda Credit API,
//! including database setup/teardown, mock data generation, and common test helpers.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::test_utils::*;
//!
//! #[tokio::test]
//! async fn test_my_endpoint() {
//!     let test_db = TestDatabase::new().await;
//!     let api_state = create_test_api_state(&test_db.pool).await;
//!
//!     // Test your endpoint here
//!     let response = your_test_endpoint(State(api_state)).await;
//!
//!     // Assert your expectations
//! }
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::ApiState;
use arda_credit_db::{AdsConfig, IndexedMerkleTreeADS};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

/// Test user data structure
#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: Uuid,
    pub email: String,
    pub user_type: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub wallet_address: Option<String>,
}

impl TestUser {
    /// Create a new investor test user
    pub fn new_investor() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("investor_{}@test.com", Uuid::new_v4().simple()),
            user_type: "investor".to_string(),
            first_name: Some("Test".to_string()),
            last_name: Some("Investor".to_string()),
            wallet_address: Some(format!("0x{}", hex::encode(rand::random::<[u8; 20]>()))),
        }
    }

    /// Create a new merchant test user
    pub fn new_merchant() -> Self {
        Self {
            id: Uuid::new_v4(),
            email: format!("merchant_{}@test.com", Uuid::new_v4().simple()),
            user_type: "merchant".to_string(),
            first_name: Some("Test".to_string()),
            last_name: Some("Merchant".to_string()),
            wallet_address: Some(format!("0x{}", hex::encode(rand::random::<[u8; 20]>()))),
        }
    }
}

/// Test database wrapper that handles setup and cleanup
pub struct TestDatabase {
    pub pool: PgPool,
}

impl TestDatabase {
    /// Create a new test database connection
    pub async fn new() -> Self {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:password@localhost:5432/arda_credit_test".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("../db/migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        Self { pool }
    }

    /// Clean up all test data
    pub async fn cleanup(&self) -> Result<(), sqlx::Error> {
        // Clear all tables in reverse dependency order
        sqlx::query("DELETE FROM financial_transactions")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM credit_line_requests")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM loan_payments")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM loans").execute(&self.pool).await?;
        sqlx::query("DELETE FROM pool_transactions")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM liquidity_pools")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM merchant_turnover")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM merchant_profiles")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM merchant")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM investor_profiles")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM identities")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM batches")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM transactions")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Create a test API state with all required services
pub async fn create_test_api_state(pool: &PgPool) -> ApiState {
    use crate::{
        auth::AuthService, batch_processor::BatchProcessorHandle, BalanceVerificationService,
        WithdrawalTransactionService,
    };

    // Create ADS service
    let ads_service = Arc::new(RwLock::new(
        IndexedMerkleTreeADS::new(pool.clone(), AdsConfig::default())
            .await
            .unwrap(),
    ));

    // Create auth service
    let auth_service = Arc::new(AuthService::new(
        "test-jwt-secret-key-minimum-32-characters-for-testing".to_string(),
        Some(24),
    ));

    // Create withdrawal service
    let withdrawal_service = WithdrawalTransactionService::new(pool.clone());

    // Create balance service
    let balance_service = BalanceVerificationService::new(pool.clone());

    ApiState {
        pool: pool.clone(),
        config: crate::router::ApiConfig {
            server_name: "Test API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 10,
            enable_debug_endpoints: true,
        },
        batch_processor: None,
        ads_service,
        withdrawal_service,
        balance_service,
        metrics_handle: None, // No metrics in tests
        session_service: Arc::new(crate::session_service::SessionService::new(pool.clone())),
        magic_link_service: None,
        email_rate_limiter: None,
        withdrawal_executor: None,
    }
}

/// Create a test API state with magic link services enabled
pub async fn create_test_api_state_with_magic_links(pool: &PgPool) -> ApiState {
    use crate::{
        auth::AuthService,
        batch_processor::BatchProcessorHandle,
        email_service::EmailService,
        magic_link_service::{MagicLinkConfig, MagicLinkService},
        middleware::EmailRateLimiter,
        session_service::SessionService,
        BalanceVerificationService, WithdrawalTransactionService,
    };

    // Create ADS service
    let ads_service = Arc::new(RwLock::new(
        IndexedMerkleTreeADS::new(pool.clone(), AdsConfig::default())
            .await
            .unwrap(),
    ));

    // Create auth service
    let auth_service = Arc::new(AuthService::new(
        "test-jwt-secret-key-minimum-32-characters-for-testing".to_string(),
        Some(24),
    ));

    // Create withdrawal service
    let withdrawal_service = WithdrawalTransactionService::new(pool.clone());

    // Create balance service
    let balance_service = BalanceVerificationService::new(pool.clone());

    // Create session service
    let session_service = Arc::new(SessionService::new(pool.clone()));

    // Create mock email service for testing
    let email_service = Arc::new(create_mock_email_service().await);

    // Create email rate limiter
    let email_rate_limiter = Arc::new(EmailRateLimiter::new()); // 3 per hour

    // Create magic link service
    let magic_link_config = MagicLinkConfig {
        token_ttl_minutes: 10,
        base_url: "https://app.arda.xyz/auth/verify".to_string(),
        max_tokens_per_user: 3,
    };

    let magic_link_service = Arc::new(MagicLinkService::new(
        pool.clone(),
        magic_link_config,
        email_service,
        Arc::new(crate::middleware::RateLimiter::new(100)), // Allow 100 requests per minute
        session_service.clone(),
    ));

    ApiState {
        pool: pool.clone(),
        config: crate::router::ApiConfig {
            server_name: "Test API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 50,
            enable_debug_endpoints: true,
        },
        batch_processor: None,
        ads_service,
        session_service,
        magic_link_service: Some(magic_link_service),
        withdrawal_service,
        balance_service,
        withdrawal_executor: None,
        metrics_handle: None,
        email_rate_limiter: Some(email_rate_limiter),
    }
}

/// Create a test app (router) with the given state
pub async fn create_test_app(state: ApiState) -> TestApp {
    crate::router::create_router(state)
}

/// Create a test app with magic link services enabled
pub async fn create_test_app_with_magic_links(state: ApiState) -> TestApp {
    crate::router::create_router(state)
}

/// Create a test user in the database
pub async fn create_test_user(pool: &PgPool, email: &str, user_type: &str) -> TestUser {
    let user_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO identities (
            id, email, user_type, first_name, last_name, wallet_address,
            kyc_status, created_at, updated_at
        ) VALUES ($1, $2, $3, 'Test', 'User', NULL, 'NOT_SUBMITTED', NOW(), NOW())
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind(user_type)
    .execute(pool)
    .await
    .unwrap();

    TestUser {
        id: user_id,
        email: email.to_string(),
        user_type: user_type.to_string(),
        first_name: Some("Test".to_string()),
        last_name: Some("User".to_string()),
        wallet_address: None,
    }
}

/// Create a mock email service for testing
pub async fn create_mock_email_service() -> crate::email_service::EmailService {
    // This is a simplified mock - in a real implementation you'd use a proper mock
    // For now, we'll create a service that doesn't actually send emails
    use crate::email_config::EmailConfig;
    use crate::email_service::EmailService;

    // Create a disabled email service for testing
    let config = EmailConfig {
        ses_region: "us-east-1".to_string(),
        email_from: "test@example.com".to_string(),
        email_reply_to: "support@example.com".to_string(),
        configuration_set: None,
        magic_base_url: "https://app.arda.xyz/auth/verify".to_string(),
        enabled: false,
        debug_mode: false,
    };

    EmailService::new(config).await.unwrap()
}

/// Test magic link data for testing
#[derive(Debug, Clone)]
pub struct TestMagicLink {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub state: String,
    pub redirect_uri: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Test app type alias for convenience
pub type TestApp = axum::Router;

/// Test context for integration testing
#[derive(Clone)]
pub struct TestContext {
    pub pool: PgPool,
    pub state: ApiState,
}

impl TestContext {
    /// Create a new test context with database and API state
    pub async fn new() -> Self {
        let test_db = TestDatabase::new().await;
        let state = create_test_api_state_with_magic_links(&test_db.pool).await;
        Self {
            pool: test_db.pool,
            state,
        }
    }
}

/// Test data factory for creating consistent test data
pub struct TestDataFactory {
    pub db: TestDatabase,
}

impl TestDataFactory {
    pub async fn new() -> Self {
        Self {
            db: TestDatabase::new().await,
        }
    }

    /// Create a test user in the database
    pub async fn create_user(&self, user: &TestUser) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO identities (
                id, email, user_type, first_name, last_name, wallet_address,
                kyc_status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, 'NOT_SUBMITTED', NOW(), NOW())
            "#,
        )
        .bind(user.id)
        .bind(&user.email)
        .bind(&user.user_type)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.wallet_address)
        .execute(&self.db.pool)
        .await?;

        Ok(())
    }

    /// Create a test liquidity pool
    pub async fn create_liquidity_pool(
        &self,
        investor_id: Uuid,
        name: &str,
        max_ltv_ratio: f64,
        max_interest_rate: f64,
    ) -> Result<Uuid, sqlx::Error> {
        let pool_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO liquidity_pools (
                id, investor_id, name, max_ltv_ratio, max_interest_rate,
                repayment_period_days, status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, 30, 'ACTIVE', NOW(), NOW())
            "#,
        )
        .bind(pool_id)
        .bind(investor_id)
        .bind(name)
        .bind(max_ltv_ratio)
        .bind(max_interest_rate)
        .execute(&self.db.pool)
        .await?;

        Ok(pool_id)
    }

    /// Create a test merchant profile
    pub async fn create_merchant_profile(
        &self,
        user_id: Uuid,
        industry: &str,
        monthly_revenue: f64,
    ) -> Result<Uuid, sqlx::Error> {
        let merchant_id = Uuid::new_v4();

        // First create merchant
        sqlx::query(
            r#"
            INSERT INTO merchant (id, user_id, mcc, created_at, updated_at)
            VALUES ($1, $2, '1234', NOW(), NOW())
            "#,
        )
        .bind(merchant_id)
        .bind(user_id)
        .execute(&self.db.pool)
        .await?;

        // Then create merchant profile
        sqlx::query(
            r#"
            INSERT INTO merchant_profiles (
                merchant_id, industry, monthly_revenue_estimate, employee_count,
                years_in_business, profile_completed, created_at, updated_at
            ) VALUES ($1, $2, $3, 10, 5, true, NOW(), NOW())
            "#,
        )
        .bind(merchant_id)
        .bind(industry)
        .bind(monthly_revenue)
        .execute(&self.db.pool)
        .await?;

        Ok(merchant_id)
    }
}

/// HTTP client utilities for integration testing
pub mod http_utils {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Create a test HTTP request
    pub fn create_test_request(
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> Request<Body> {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");

        if let Some(body) = body {
            req.body(Body::from(serde_json::to_string(&body).unwrap()))
        } else {
            req.body(Body::empty())
        }
        .unwrap()
    }

    /// Extract JSON response body
    pub async fn extract_json_response<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let body = response.into_body();
        let bytes = axum::body::to_bytes(body, usize::MAX).await?;
        let json: T = serde_json::from_slice(&bytes)?;
        Ok(json)
    }
}

/// Mock data generators
pub mod mock_data {
    use super::TestUser;

    /// Generate a random email for testing
    pub fn random_email() -> String {
        format!("test_{}@example.com", uuid::Uuid::new_v4().simple())
    }

    /// Generate a random wallet address
    pub fn random_wallet_address() -> String {
        format!("0x{}", hex::encode(uuid::Uuid::new_v4().as_bytes()))
    }

    /// Generate test users of different types
    pub fn generate_test_users(count: usize) -> Vec<TestUser> {
        let mut users = Vec::new();

        for i in 0..count {
            let user_type = match i % 3 {
                0 => "investor",
                1 => "originator",
                _ => "admin",
            };

            users.push(TestUser {
                id: uuid::Uuid::new_v4(),
                email: format!("{}_{}@test.com", user_type, i),
                user_type: user_type.to_string(),
                first_name: Some(format!("Test{}", i)),
                last_name: Some(format!("User{}", i)),
                wallet_address: Some(random_wallet_address()),
            });
        }

        users
    }
}

/// Test assertion helpers
pub mod assertions {
    use axum::http::StatusCode;

    /// Assert that a response has the expected status code
    pub fn assert_status_code(response: &axum::response::Response, expected: StatusCode) {
        assert_eq!(
            response.status(),
            expected,
            "Expected status {}, got {}",
            expected,
            response.status()
        );
    }

    /// Assert that a response contains the expected JSON structure
    pub fn assert_json_contains<T: serde::Serialize>(
        response_body: &serde_json::Value,
        key: &str,
        expected: T,
    ) {
        if let Some(actual) = response_body.get(key) {
            let expected_json = serde_json::to_value(expected).unwrap();
            assert_eq!(
                actual, &expected_json,
                "Expected {} to be {:?}, got {:?}",
                key, expected_json, actual
            );
        } else {
            panic!("Key '{}' not found in response", key);
        }
    }

    /// Assert that a response does not contain a specific key
    pub fn assert_json_missing(response_body: &serde_json::Value, key: &str) {
        assert!(
            response_body.get(key).is_none(),
            "Expected key '{}' to be missing from response",
            key
        );
    }
}

/// Performance testing utilities
pub mod performance {
    use std::time::{Duration, Instant};

    /// Measure execution time of an async function
    pub async fn measure_time<F, Fut, T>(f: F) -> (T, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Assert that an operation completes within a time limit
    pub fn assert_within_time_limit(duration: Duration, limit_ms: u64) {
        let limit = Duration::from_millis(limit_ms);
        assert!(
            duration <= limit,
            "Operation took {:?}, which exceeds the limit of {:?}",
            duration,
            limit
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_database_creation() {
        let test_db = TestDatabase::new().await;
        // Verify we can execute a simple query
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(result.0, 1);
    }

    #[tokio::test]
    async fn test_test_user_creation() {
        let investor = TestUser::new_investor();
        assert_eq!(investor.user_type, "investor");
        assert!(investor.email.contains("investor_"));
        assert!(investor.first_name.is_some());
        assert!(investor.last_name.is_some());
        assert!(investor.wallet_address.is_some());
    }

    #[tokio::test]
    async fn test_mock_data_generation() {
        let users = mock_data::generate_test_users(5);
        assert_eq!(users.len(), 5);

        for user in &users {
            assert!(!user.email.is_empty());
            assert!(!user.user_type.is_empty());
        }
    }
}
