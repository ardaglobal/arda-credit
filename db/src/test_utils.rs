use crate::{create_merchant, Merchant};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

pub struct TestDatabase {
    pub pool: PgPool,
    pub database_name: String,
}

impl TestDatabase {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let database_name = format!("test_arda_credit_{}", Uuid::new_v4().simple());

        // Use environment-compatible connection URL (supports both localhost and 127.0.0.1)
        let admin_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| {
                "postgresql://postgres:password@127.0.0.1:5432/postgres".to_string()
            })
            .replace("/arda_credit_db", "/postgres")
            .replace("/postgres?", "/postgres?"); // Handle existing query params

        let admin_pool = sqlx::postgres::PgPool::connect(&admin_url).await?;

        sqlx::query(&format!("CREATE DATABASE \"{database_name}\""))
            .execute(&admin_pool)
            .await?;

        admin_pool.close().await;

        // Extract host from admin_url for consistency
        let host = if admin_url.contains("localhost") {
            "localhost"
        } else {
            "127.0.0.1"
        };
        let test_db_url = format!("postgresql://postgres:password@{host}:5432/{database_name}");

        env::set_var("DATABASE_URL", &test_db_url);

        let pool = crate::db::init_db().await?;

        Ok(Self {
            pool,
            database_name: database_name.clone(),
        })
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let database_name = self.database_name.clone();
        tokio::spawn(async move {
            let admin_url = env::var("DATABASE_URL")
                .unwrap_or_else(|_| {
                    "postgresql://postgres:password@127.0.0.1:5432/postgres".to_string()
                })
                .replace("/arda_credit_db", "/postgres")
                .replace("/postgres?", "/postgres?");

            if let Ok(admin_pool) = sqlx::postgres::PgPool::connect(&admin_url).await {
                let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database_name}\""))
                    .execute(&admin_pool)
                    .await;
                admin_pool.close().await;
            }
        });
    }
}

/// Setup a test database for testing
pub async fn setup_test_db() -> PgPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/arda_credit_test".to_string()
    });

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    pool
}

/// Create a test merchant for testing
pub async fn create_test_merchant(pool: &PgPool) -> Result<Merchant, sqlx::Error> {
    use crate::{create_identity_with_password, CreateMerchantRequest};
    use chrono::Utc;

    // First create a test identity (required for foreign key constraint)
    let test_email = format!("test-merchant-{}.example.com", Utc::now().timestamp());
    let identity =
        create_identity_with_password(pool, "merchant", &test_email, "test_password_hash").await?;

    let request = CreateMerchantRequest {
        legal_name: "Test Merchant LLC".to_string(),
        dba: Some("Test Store".to_string()),
        tax_id: Some("12-3456789".to_string()),
        mcc: Some("5411".to_string()),
        country: "US".to_string(),
        business_type: Some("LLC".to_string()),
    };

    create_merchant(pool, identity.id, &request).await
}
