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
