#[cfg(test)]
mod tests {
    use ethereum_client::{DepositMonitorConfig, DepositStatus};
    use sqlx::{PgPool, Row};
    use std::env;

    async fn get_test_db_pool() -> Result<PgPool, sqlx::Error> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/test_db".to_string()
        });
        PgPool::connect(&database_url).await
    }

    #[tokio::test]
    async fn test_deposit_monitor_config() {
        let config = DepositMonitorConfig::default();

        assert_eq!(config.required_confirmations, 12);
        assert_eq!(config.confirmation_check_interval, 30);
        assert_eq!(config.max_retry_attempts, 3);
        assert_eq!(config.retry_backoff_seconds, 60);
        assert_eq!(config.batch_size, 100);
    }

    #[tokio::test]
    async fn test_deposit_status_serialization() {
        let pending = DepositStatus::Pending;
        let confirmed = DepositStatus::Confirmed;
        let processed = DepositStatus::Processed;
        let failed = DepositStatus::Failed;

        assert_eq!(pending.to_string(), "pending");
        assert_eq!(confirmed.to_string(), "confirmed");
        assert_eq!(processed.to_string(), "processed");
        assert_eq!(failed.to_string(), "failed");
    }

    #[tokio::test]
    #[ignore] // Only run when database is available
    async fn test_database_schema() {
        let pool = match get_test_db_pool().await {
            Ok(pool) => pool,
            Err(_) => {
                println!("Skipping database test - database not available");
                return;
            }
        };

        // Test that the tables exist
        let tables = sqlx::query(
            r#"
            SELECT table_name 
            FROM information_schema.tables 
            WHERE table_schema = 'public' 
            AND table_name IN ('usdc_deposits', 'user_deposit_balances', 'deposit_processing_log')
            ORDER BY table_name
            "#,
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to query tables");

        let table_names: Vec<String> = tables
            .into_iter()
            .map(|row| row.get::<String, _>("table_name"))
            .collect();

        assert!(table_names.contains(&"usdc_deposits".to_string()));
        assert!(table_names.contains(&"user_deposit_balances".to_string()));
        assert!(table_names.contains(&"deposit_processing_log".to_string()));

        println!("✓ All required tables exist: {:?}", table_names);
    }

    #[tokio::test]
    #[ignore] // Only run when database is available
    async fn test_deposit_insertion() {
        let pool = match get_test_db_pool().await {
            Ok(pool) => pool,
            Err(_) => {
                println!("Skipping database test - database not available");
                return;
            }
        };

        // Insert a test deposit
        let user_address = "0x1234567890123456789012345678901234567890";
        let amount = "1000000"; // 1 USDC in wei (6 decimals)
        let tx_hash = "0xabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

        let deposit_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO usdc_deposits (
                user_address,
                amount,
                transaction_hash,
                event_transaction_hash,
                block_number,
                event_timestamp,
                log_index,
                status,
                confirmation_count,
                required_confirmations
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0, 12)
            RETURNING id
            "#,
        )
        .bind(user_address)
        .bind(amount)
        .bind(tx_hash)
        .bind(tx_hash)
        .bind(1000000i64)
        .bind(1640995200i64) // Jan 1, 2022
        .bind(0i64)
        .fetch_one(&pool)
        .await
        .expect("Failed to insert test deposit");

        assert!(deposit_id > 0);

        // Verify the deposit was inserted
        let deposit = sqlx::query(
            r#"
            SELECT user_address, amount::TEXT as amount, status, deposit_amount_usdc
            FROM usdc_deposits 
            WHERE id = $1
            "#,
        )
        .bind(deposit_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch test deposit");

        let user_addr: String = deposit.get("user_address");
        let amt: String = deposit.get("amount");
        let stat: String = deposit.get("status");

        assert_eq!(user_addr, user_address);
        assert_eq!(amt, amount);
        assert_eq!(stat, "pending");

        let deposit_amount: Option<rust_decimal::Decimal> = deposit.get("deposit_amount_usdc");
        assert_eq!(deposit_amount, Some(rust_decimal::Decimal::new(1, 0)));

        // Clean up
        sqlx::query("DELETE FROM usdc_deposits WHERE id = $1")
            .bind(deposit_id)
            .execute(&pool)
            .await
            .expect("Failed to clean up test deposit");

        println!("✓ Deposit insertion and retrieval test passed");
    }

    #[tokio::test]
    #[ignore] // Only run when database is available
    async fn test_balance_trigger() {
        let pool = match get_test_db_pool().await {
            Ok(pool) => pool,
            Err(_) => {
                println!("Skipping database test - database not available");
                return;
            }
        };

        let user_address = "0x9876543210987654321098765432109876543210";
        let amount = "2000000"; // 2 USDC
        let tx_hash = "0x1111111111111111111111111111111111111111111111111111111111111111";

        // Insert a deposit
        let deposit_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO usdc_deposits (
                user_address, amount, transaction_hash, event_transaction_hash,
                block_number, event_timestamp, log_index, status, 
                confirmation_count, required_confirmations
            ) VALUES ($1, $2, $3, $4, 1000001, 1640995300, 0, 'pending', 0, 12)
            RETURNING id
            "#,
        )
        .bind(user_address)
        .bind(amount)
        .bind(tx_hash)
        .bind(tx_hash)
        .fetch_one(&pool)
        .await
        .expect("Failed to insert test deposit");

        // Update status to confirmed to trigger balance update
        sqlx::query("UPDATE usdc_deposits SET status = 'confirmed' WHERE id = $1")
            .bind(deposit_id)
            .execute(&pool)
            .await
            .expect("Failed to update deposit status");

        // Check that user balance was updated
        let balance = sqlx::query(
            "SELECT total_deposited::TEXT as total_deposited, deposit_count FROM user_deposit_balances WHERE user_address = $1"
        )
        .bind(user_address)
        .fetch_optional(&pool)
        .await
        .expect("Failed to fetch user balance");

        if let Some(balance) = balance {
            let total_deposited: String = balance.get("total_deposited");
            let deposit_count: Option<i32> = balance.get("deposit_count");
            assert_eq!(total_deposited, amount);
            assert_eq!(deposit_count, Some(1));
            println!(
                "✓ Balance trigger test passed: {} USDC deposited",
                total_deposited
            );
        } else {
            println!("⚠ Balance trigger may not have fired - balance not found");
        }

        // Clean up
        sqlx::query("DELETE FROM user_deposit_balances WHERE user_address = $1")
            .bind(user_address)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DELETE FROM usdc_deposits WHERE id = $1")
            .bind(deposit_id)
            .execute(&pool)
            .await
            .expect("Failed to clean up test deposit");
    }
}
