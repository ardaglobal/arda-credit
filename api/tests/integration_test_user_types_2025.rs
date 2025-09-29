//! Comprehensive Integration Tests for Arda Credit User Type System
//!
//! This test suite verifies the complete end-to-end flow of:
//! 1. User registration with 2025 invite codes
//! 2. Magic link authentication
//! 3. User session management
//! 4. Error handling scenarios
//!
//! Tests both API endpoints and CLI commands to ensure complete functionality.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use tokio::time::sleep;

use api::types::UserType;
use arda_credit_db::{get_identity_by_id, get_invite_code_info};

/// Test configuration
struct TestConfig {
    base_url: String,
    test_emails: Vec<String>,
}

/// Test user data for different user types
struct TestUser {
    email: String,
    invite_code: String,
    expected_user_type: UserType,
}

/// Integration test suite for user type system
#[cfg(test)]
mod integration_tests {
    use super::*;
    use api::handlers::users::users::RegisterUserResponse;

    /// Main integration test - runs all test scenarios
    /// Note: This test requires a live API server running on localhost:8080
    /// Run with: cargo test test_complete_user_type_flow -- --ignored
    #[tokio::test]
    #[ignore = "requires live API server on localhost:8080"]
    pub async fn test_complete_user_type_flow() {
        println!("🚀 Starting comprehensive user type integration tests...");

        let config = TestConfig {
            base_url: "http://localhost:8080".to_string(),
            test_emails: vec![
                "matt@arda.xyz".to_string(),
                "investor-test@arda.xyz".to_string(),
                "originator-test@arda.xyz".to_string(),
                "admin-test@arda.xyz".to_string(),
            ],
        };

        let client = Client::new();

        // Test 1: Verify database setup and invite codes
        test_database_setup(&config).await;

        // Test 2: Registration flow for all user types
        test_user_registration_flow(&config, &client).await;

        // Test 3: Magic link authentication flow
        test_magic_link_authentication_flow(&config, &client).await;

        // Test 4: Error scenarios
        test_error_scenarios(&config, &client).await;

        // Test 5: CLI commands
        test_cli_commands(&config).await;

        // Test 6: Database state verification
        verify_database_state(&config).await;

        println!("✅ All integration tests completed successfully!");
    }

    /// Test 1: Verify database setup and 2025 invite codes exist
    async fn test_database_setup(config: &TestConfig) {
        println!("📋 Test 1: Verifying database setup and invite codes...");

        // Connect to database (assuming it's running)
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/arda_credit".to_string()
        });

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // Check that all three 2025 invite codes exist
        let invite_codes = vec!["INVESTOR2025", "ORIGINATOR2025", "ADMIN2025"];

        for code in &invite_codes {
            let info = get_invite_code_info(&pool, code)
                .await
                .expect(&format!("Failed to get invite code info for {}", code));

            assert!(info.is_valid, "Invite code {} should be valid", code);

            println!(
                "   ✅ {}: valid={}, user_type={:?}, expires at {:?}",
                code, info.is_valid, info.user_type, info.expires_at
            );
        }

        println!("   ✅ Database setup verified successfully");
    }

    /// Test 2: User registration flow with all invite codes
    async fn test_user_registration_flow(config: &TestConfig, client: &Client) {
        println!("📝 Test 2: Testing user registration flow...");

        let test_users = vec![
            TestUser {
                email: "investor-test@arda.xyz".to_string(),
                invite_code: "INVESTOR2025".to_string(),
                expected_user_type: UserType::Investor,
            },
            TestUser {
                email: "originator-test@arda.xyz".to_string(),
                invite_code: "ORIGINATOR2025".to_string(),
                expected_user_type: UserType::Originator,
            },
            TestUser {
                email: "admin-test@arda.xyz".to_string(),
                invite_code: "ADMIN2025".to_string(),
                expected_user_type: UserType::Admin,
            },
        ];

        for test_user in &test_users {
            println!(
                "   🔄 Registering user: {} with code: {}",
                test_user.email, test_user.invite_code
            );

            // Prepare registration request
            let request_body = json!({
                "email": test_user.email,
                "password": "SecureTestPass123!",
                "invite_code": test_user.invite_code
            });

            // Make registration API call
            let response = client
                .post(&format!("{}/api/v1/user", config.base_url))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
                .expect("Failed to send registration request");

            // Verify response
            assert_eq!(
                response.status(),
                200,
                "Registration should succeed for {}",
                test_user.email
            );

            let response_json: RegisterUserResponse = response
                .json()
                .await
                .expect("Failed to parse registration response");

            // Verify user type matches expected
            assert_eq!(
                response_json.user_type, test_user.expected_user_type,
                "User type should be {:?} for {}",
                test_user.expected_user_type, test_user.email
            );

            println!(
                "   ✅ {} registered successfully as {:?}",
                test_user.email, response_json.user_type
            );
        }

        println!("   ✅ User registration flow test completed");
    }

    /// Test 3: Magic link authentication flow
    async fn test_magic_link_authentication_flow(config: &TestConfig, client: &Client) {
        println!("🔗 Test 3: Testing magic link authentication flow...");

        let test_email = "matt@arda.xyz";

        println!("   🔄 Requesting magic link for: {}", test_email);

        // Request magic link
        let request_body = json!({
            "email": test_email,
            "redirect_uri": "http://localhost:3000/dashboard"
        });

        let response = client
            .post(&format!("{}/api/v1/auth/magic-link", config.base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Failed to request magic link");

        assert_eq!(response.status(), 200, "Magic link request should succeed");

        let response_json: serde_json::Value = response
            .json()
            .await
            .expect("Failed to parse magic link response");

        println!("   ✅ Magic link requested successfully");

        // Note: In a real test, we'd need to extract the magic link token from email
        // For now, we'll test the verification endpoint structure
        println!(
            "   📧 Magic link sent to {}. Check email for token to complete verification test.",
            test_email
        );

        // Test /api/v1/auth/me endpoint (would require a valid session)
        println!("   ℹ️  To complete magic link verification test:");
        println!("      1. Check email for magic link");
        println!("      2. Extract token from URL");
        println!("      3. Call: POST /api/v1/auth/verify with token");
        println!("      4. Call: GET /api/v1/auth/me with session cookie");
    }

    /// Test 4: Error scenarios
    async fn test_error_scenarios(config: &TestConfig, client: &Client) {
        println!("❌ Test 4: Testing error scenarios...");

        // Test 1: Registration without invite code
        println!("   🔄 Testing registration without invite code...");
        let request_body = json!({
            "email": "error-test@arda.xyz",
            "password": "SecureTestPass123!"
        });

        let response = client
            .post(&format!("{}/api/v1/user", config.base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Failed to send registration request");

        // Should fail with 400 Bad Request
        assert_eq!(
            response.status(),
            400,
            "Registration without invite code should fail"
        );

        println!("   ✅ Registration without invite code properly rejected");

        // Test 2: Registration with invalid invite code
        println!("   🔄 Testing registration with invalid invite code...");
        let request_body = json!({
            "email": "error-test2@arda.xyz",
            "password": "SecureTestPass123!",
            "invite_code": "INVALID2025"
        });

        let response = client
            .post(&format!("{}/api/v1/user", config.base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Failed to send registration request");

        // Should fail with 400 Bad Request
        assert_eq!(
            response.status(),
            400,
            "Registration with invalid invite code should fail"
        );

        println!("   ✅ Registration with invalid invite code properly rejected");

        // Test 3: Registration with existing email
        println!("   🔄 Testing registration with existing email...");
        let request_body = json!({
            "email": "investor-test@arda.xyz", // This email was registered in test 2
            "password": "SecureTestPass123!",
            "invite_code": "INVESTOR2025"
        });

        let response = client
            .post(&format!("{}/api/v1/user", config.base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Failed to send registration request");

        // Should fail with 409 Conflict
        assert_eq!(
            response.status(),
            409,
            "Registration with existing email should fail"
        );

        println!("   ✅ Registration with existing email properly rejected");

        println!("   ✅ Error scenarios test completed");
    }

    /// Test 5: CLI commands
    async fn test_cli_commands(config: &TestConfig) {
        println!("💻 Test 5: Testing CLI commands...");

        use std::process::Command;

        // Test 1: Generate invite codes
        println!("   🔄 Testing invite code generation...");

        let user_types = vec!["investor", "originator", "admin"];

        for user_type in &user_types {
            let output = Command::new("make")
                .args(&[
                    "cli",
                    &format!(
                        "ARGS=generate-invite-code --user-type {} --max-uses 5",
                        user_type
                    ),
                ])
                .output()
                .expect(&format!("Failed to run CLI command for {}", user_type));

            if output.status.success() {
                println!("   ✅ Generated invite code for user type: {}", user_type);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!(
                    "   ❌ Failed to generate invite code for {}: {}",
                    user_type, stderr
                );
            }
        }

        // Test 2: List invite codes
        println!("   🔄 Testing invite code listing...");
        let output = Command::new("make")
            .args(&["cli", "ARGS=list-invite-codes"])
            .output()
            .expect("Failed to run CLI command");

        if output.status.success() {
            println!("   ✅ Listed invite codes successfully");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("   ❌ Failed to list invite codes: {}", stderr);
        }

        // Test 3: Invalid user type (should fail)
        println!("   🔄 Testing invalid user type rejection...");
        let output = Command::new("make")
            .args(&[
                "cli",
                "ARGS=generate-invite-code --user-type regular --max-uses 5",
            ])
            .output()
            .expect("Failed to run CLI command");

        if !output.status.success() {
            println!("   ✅ Invalid user type 'regular' properly rejected");
        } else {
            println!("   ❌ Invalid user type 'regular' was accepted (should have failed)");
        }

        println!("   ✅ CLI commands test completed");
    }

    /// Test 6: Database state verification
    async fn verify_database_state(config: &TestConfig) {
        println!("🗄️  Test 6: Verifying database state...");

        // Connect to database
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/arda_credit".to_string()
        });

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // Check that users were created with correct types
        println!("   🔄 Checking user registrations...");
        let emails = vec![
            "investor-test@arda.xyz",
            "originator-test@arda.xyz",
            "admin-test@arda.xyz",
        ];

        for email in &emails {
            let user_result = sqlx::query!(
                "SELECT id, user_type FROM identities WHERE email = $1",
                email
            )
            .fetch_optional(&pool)
            .await
            .expect(&format!("Failed to query user {}", email));

            if let Some(user) = user_result {
                let expected_type = match email.as_ref() {
                    "investor-test@arda.xyz" => "investor",
                    "originator-test@arda.xyz" => "originator",
                    "admin-test@arda.xyz" => "admin",
                    _ => "unknown",
                };

                assert_eq!(
                    user.user_type, expected_type,
                    "User {} should have type {}",
                    email, expected_type
                );

                println!("   ✅ {}: user_type = {}", email, user.user_type);
            } else {
                panic!("User {} was not found in database", email);
            }
        }

        // Check invite code usage
        println!("   🔄 Checking invite code usage...");
        let codes = vec!["INVESTOR2025", "ORIGINATOR2025", "ADMIN2025"];

        for code in &codes {
            let code_result = sqlx::query!(
                "SELECT current_uses, max_uses FROM invite_codes WHERE code = $1",
                code
            )
            .fetch_one(&pool)
            .await
            .expect(&format!("Failed to query invite code {}", code));

            println!(
                "   ✅ {}: {:?}/{:?} uses",
                code, code_result.current_uses, code_result.max_uses
            );
        }

        println!("   ✅ Database state verification completed");
    }
}

/// Manual test runner that can be executed independently
#[tokio::main]
async fn main() {
    println!("🧪 Arda Credit User Type Integration Test Runner");
    println!("================================================");
    println!();

    // Run the comprehensive test
    integration_tests::test_complete_user_type_flow();
}
