#!/usr/bin/env cargo run --bin create_user_and_magic_link
//! Script to create a user and generate a magic link for testing
//!
//! This script performs the following steps:
//! 1. Registers a new user with the specified email using INVESTOR2025 invite code
//! 2. Requests a magic link for login
//!
//! Usage:
//!     cargo run --bin create_user_and_magic_link [email]
//!
//! Arguments:
//!     email - Email address to use (default: test@test.com)
//!
//! Environment variables:
//!     API_BASE_URL - Base URL for the API server (default: http://localhost:8080)

use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};

#[derive(Debug, Serialize)]
struct RegisterUserRequest {
    email: String,
    jurisdiction: Option<String>,
    invite_code: String,
}

#[derive(Debug, Deserialize)]
struct RegisterUserResponse {
    user_id: String,
    user_type: String,
    kyc_status: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct MagicLinkRequest {
    email: String,
    redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MagicLinkResponse {
    success: bool,
    message: String,
    message_id: Option<String>,
    expires_in_minutes: u32,
    timestamp: String,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Email address to use for user creation and magic link
    #[arg(default_value = "test@test.com")]
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = Args::parse();
    let email = args.email.clone();

    // Get API base URL from environment or use default
    let api_base_url =
        env::var("API_BASE_URL").unwrap_or_else(|_| "http://0.0.0.0:8080".to_string());

    info!("🚀 Starting user creation and magic link generation script");
    info!("📧 Email: {}", email);
    info!("📍 API Base URL: {}", api_base_url);

    let client = Client::new();

    // Step 1: Register the user
    info!("👤 Step 1: Registering user {}", email);

    let register_request = RegisterUserRequest {
        email: email.clone(),
        jurisdiction: Some("US".to_string()),
        invite_code: "INVESTOR2025".to_string(),
    };

    let register_url = format!("{}/api/v1/user", api_base_url);
    info!("📡 POST {}", register_url);

    let register_response = client
        .post(&register_url)
        .json(&register_request)
        .send()
        .await?;

    match register_response.status() {
        reqwest::StatusCode::CREATED => {
            let response_body: RegisterUserResponse = register_response.json().await?;
            info!("✅ User registered successfully!");
            info!("   User ID: {}", response_body.user_id);
            info!("   User Type: {}", response_body.user_type);
            info!("   KYC Status: {}", response_body.kyc_status);
            info!("   Created At: {}", response_body.created_at);
        }
        reqwest::StatusCode::CONFLICT => {
            info!("⚠️  User already exists with email {}", email);
            info!("   Continuing to magic link generation...");
        }
        reqwest::StatusCode::OK => {
            let response_body: RegisterUserResponse = register_response.json().await?;
            info!("✅ User registration completed!");
            info!("   User ID: {}", response_body.user_id);
            info!("   User Type: {}", response_body.user_type);
            info!("   KYC Status: {}", response_body.kyc_status);
            info!("   Created At: {}", response_body.created_at);
        }
        status if status.is_client_error() || status.is_server_error() => {
            let error_text = register_response.text().await?;
            error!("❌ Failed to register user: {} - {}", status, error_text);
            return Err("User registration failed".into());
        }
        status => {
            let response_text = register_response.text().await?;
            info!("⚠️  Unexpected status code: {} - {}", status, response_text);
            info!("   Continuing to magic link generation...");
        }
    }

    // Step 2: Request magic link
    info!("🔗 Step 2: Requesting magic link for {}", email);

    let magic_link_request = MagicLinkRequest {
        email: email.clone(),
        redirect_uri: Some("/dashboard".to_string()),
    };

    let magic_link_url = format!("{}/api/v1/auth/magic-link", api_base_url);
    info!("📡 POST {}", magic_link_url);

    let magic_link_response = client
        .post(&magic_link_url)
        .json(&magic_link_request)
        .send()
        .await?;

    match magic_link_response.status() {
        reqwest::StatusCode::OK => {
            let response_body: MagicLinkResponse = magic_link_response.json().await?;
            if response_body.success {
                info!("✅ Magic link sent successfully!");
            } else {
                info!("⚠️  Magic link request completed but may have issues");
            }
            info!("   Message: {}", response_body.message);
            if let Some(message_id) = response_body.message_id {
                info!("   Message ID: {}", message_id);
            }
            info!(
                "   Expires in: {} minutes",
                response_body.expires_in_minutes
            );
            info!("   Timestamp: {}", response_body.timestamp);

            info!("📧 Check your email ({}) for the magic link!", email);
            info!("🔍 You can also check the database directly for the magic link token:");
            info!("   psql $DATABASE_URL -c \"SELECT token, state, expires_at FROM magic_links WHERE email = '{}' ORDER BY created_at DESC LIMIT 1;\"", email);

            info!("🎯 Next steps:");
            info!("1. Copy the magic link from your email or database");
            info!("2. Visit the link in your browser to complete authentication");
            info!("3. You'll be redirected to /dashboard with a session cookie");
        }
        status => {
            let error_text = magic_link_response.text().await?;
            error!(
                "❌ Failed to request magic link: {} - {}",
                status, error_text
            );
            return Err("Magic link request failed".into());
        }
    }

    info!("🎉 Script completed successfully!");
    Ok(())
}
