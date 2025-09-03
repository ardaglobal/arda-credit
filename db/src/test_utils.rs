use sqlx::PgPool;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

// Simple atomic counter to help serialize test database operations
static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TestDatabase {
    pub pool: PgPool,
    pub database_name: String,
    pub original_db_url: Option<String>,
}

impl TestDatabase {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Simple serialization using atomic counter
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);

        let database_name = format!("test_arda_credit_{}_{}", Uuid::new_v4().simple(), counter);

        // Save original DATABASE_URL to restore later
        let original_db_url = env::var("DATABASE_URL").ok();

        // Use environment-compatible connection URL (supports both localhost and 127.0.0.1)
        let admin_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| {
                "postgresql://postgres:password@127.0.0.1:5432/postgres".to_string()
            })
            .replace("/arda_credit_db", "/postgres")
            .replace("/postgres?", "/postgres?"); // Handle existing query params

        // Create database with proper error handling
        let admin_pool = sqlx::postgres::PgPool::connect(&admin_url)
            .await
            .map_err(|e| format!("Failed to connect to admin database: {}", e))?;

        // First drop database if it exists (cleanup from previous failed tests)
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database_name}\""))
            .execute(&admin_pool)
            .await;

        sqlx::query(&format!("CREATE DATABASE \"{database_name}\""))
            .execute(&admin_pool)
            .await
            .map_err(|e| format!("Failed to create test database: {}", e))?;

        admin_pool.close().await;

        // Extract host from admin_url for consistency
        let host = if admin_url.contains("localhost") {
            "localhost"
        } else {
            "127.0.0.1"
        };
        let test_db_url = format!("postgresql://postgres:password@{host}:5432/{database_name}");

        // Set DATABASE_URL for this test
        env::set_var("DATABASE_URL", &test_db_url);

        let pool = crate::db::init_db()
            .await
            .map_err(|e| format!("Failed to initialize test database: {}", e))?;

        Ok(Self {
            pool,
            database_name: database_name.clone(),
            original_db_url,
        })
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Restore original DATABASE_URL immediately
        if let Some(url) = &self.original_db_url {
            env::set_var("DATABASE_URL", url);
        } else {
            env::remove_var("DATABASE_URL");
        }

        // Store cleanup info for async execution
        let database_name = self.database_name.clone();

        // Spawn cleanup task using existing runtime
        tokio::spawn(async move {
            let admin_url = env::var("DATABASE_URL")
                .unwrap_or_else(|_| {
                    "postgresql://postgres:password@127.0.0.1:5432/postgres".to_string()
                })
                .replace("/arda_credit_db", "/postgres")
                .replace("/postgres?", "/postgres?");

            // Try to connect and drop the database
            if let Ok(admin_pool) = sqlx::postgres::PgPool::connect(&admin_url).await {
                // Terminate any active connections to the test database first
                let _ = sqlx::query(&format!(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
                    database_name
                ))
                .execute(&admin_pool)
                .await;

                // Wait a moment for connections to terminate
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                // Drop the database
                let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database_name}\""))
                    .execute(&admin_pool)
                    .await;
                let _ = admin_pool.close().await;
            }
        });
    }
}
