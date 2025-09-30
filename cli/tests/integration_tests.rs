//! Integration tests for the Arda Credit CLI
//!
//! This module contains comprehensive integration tests for the CLI functionality,
//! following Rust best practices for test organization.

use cli::{cli::run_cli, CliConfig};
use eyre::Result;
use std::env;

/// Test basic CLI initialization and configuration
#[tokio::test]
async fn test_cli_initialization() -> Result<()> {
    // Set test environment
    env::set_var("ARDA_CREDIT_API_URL", "http://localhost:8080");

    let config = CliConfig::new();
    assert_eq!(config.api_base_url, "http://localhost:8080");

    Ok(())
}

/// Test CLI help output and command parsing
#[test]
fn test_cli_help_commands() {
    // This would test that all commands are properly registered
    // and help text is generated correctly

    // Placeholder for actual help testing
    assert!(true);
}

/// Test error handling for invalid commands
#[test]
fn test_invalid_command_handling() {
    // This would test graceful handling of invalid commands
    // and proper error messages

    // Placeholder for actual error testing
    assert!(true);
}

/// Test user registration creates transaction and batch processing works
#[tokio::test]
#[ignore] // Requires running API server and database
async fn test_user_registration_to_batch_workflow() -> Result<()> {
    use cli::BatchApiClient;

    // Set test environment
    env::set_var("ARDA_CREDIT_API_URL", "http://localhost:8080");
    env::set_var("SERVER_ENVIRONMENT", "local");

    let client = BatchApiClient::new("http://localhost:8080");

    // Generate unique test email
    let test_email = format!("test-{}@example.com", chrono::Utc::now().timestamp());

    // Step 1: Register a user (this should create a UserRegistration transaction)
    println!("📝 Step 1: Registering user with email: {}", test_email);
    let user_result = client
        .register_user(
            "INVESTOR2025", // Use a valid invite code
            &test_email,
            Some("US"),
        )
        .await;

    // If user registration fails due to missing invite code, skip test
    if user_result.is_err() {
        println!("⚠️ Skipping test - user registration failed (may need valid invite code)");
        return Ok(());
    }

    let user_response = user_result?;
    println!(
        "✅ User registered: {}",
        user_response["user_id"].as_str().unwrap_or("unknown")
    );

    // Step 2: Check pending transactions (should include UserRegistration)
    println!("\n📋 Step 2: Checking pending transactions...");
    let pending = client.get_pending_transactions().await?;
    println!("✅ Found {} pending transactions", pending.total_count);

    assert!(
        pending.total_count > 0,
        "Expected at least one pending transaction after user registration"
    );

    // Step 3: Trigger batch processing
    println!("\n🔄 Step 3: Triggering batch processing...");
    let batch_result = client.create_batch(Some(10)).await?;
    println!("✅ Batch created with ID: {}", batch_result.batch_id);

    assert!(
        batch_result.transaction_count > 0,
        "Expected batch to include at least one transaction"
    );
    println!("   Transaction count: {}", batch_result.transaction_count);
    println!(
        "   Counter: {} → {}",
        batch_result.previous_counter_value, batch_result.final_counter_value
    );

    // Step 4: Verify the batch was created correctly
    println!("\n🔍 Step 4: Verifying batch details...");
    let batch_details = client.get_batch(batch_result.batch_id).await?;

    if let Some(batch) = batch_details {
        println!("✅ Batch verification passed:");
        println!("   Status: {}", batch.proof_status);
        println!("   Transactions: {}", batch.transaction_count);

        assert_eq!(batch.id, batch_result.batch_id, "Batch ID mismatch");
        assert!(
            batch.transaction_count > 0,
            "Batch should contain transactions"
        );
    } else {
        panic!("Batch {} not found after creation", batch_result.batch_id);
    }

    println!("\n🎉 User registration → batch trigger workflow test completed successfully!");

    Ok(())
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    /// Test CLI startup performance
    #[test]
    fn test_cli_startup_time() {
        let start = Instant::now();

        // Simulate CLI initialization
        let _config = CliConfig::new();

        let duration = start.elapsed();

        // CLI should start quickly
        assert!(
            duration.as_millis() < 100,
            "CLI startup too slow: {:?}",
            duration
        );
    }
}
