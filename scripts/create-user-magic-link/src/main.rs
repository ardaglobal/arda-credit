#!/usr/bin/env cargo run --bin create_user_and_magic_link
//! Script to create a user with email matt@arda.xyz and generate a magic link for testing
//!
//! This script performs the following steps:
//! 1. Registers a new user with email matt@arda.xyz using INVESTOR2025 invite code
//! 2. Requests a magic link for login
//!
//! Usage:
//!     cargo run --bin create_user_and_magic_link
//!
//! Environment variables:
//!     API_BASE_URL - Base URL for the API server (default: http://localhost:8080)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};

#[derive(Debug, Serialize)]
struct RegisterUserRequest {
    email: String,
    password: String,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get API base URL from environment or use default
    let api_base_url = env::var("API_BASE_URL")
        .unwrap_or_else(|_| "http://0.0.0.0:8080".to_string());

    info!("🚀 Starting user creation and magic link generation script");
    info!("📍 API Base URL: {}", api_base_url);

    let client = Client::new();

    // Step 1: Register the user
    info!("👤 Step 1: Registering user matt@arda.xyz");

    let register_request = RegisterUserRequest {
        email: "matt@arda.xyz".to_string(),
        password: "testpassword123".to_string(), // This will be placeholder during magic link transition
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
            info!("⚠️  User already exists with email matt@arda.xyz");
            info!("   Continuing to magic link generation...");
        }
        status => {
            let error_text = register_response.text().await?;
            error!("❌ Failed to register user: {} - {}", status, error_text);
            return Err("User registration failed".into());
        }
    }

    // Step 2: Request magic link
    info!("🔗 Step 2: Requesting magic link for matt@arda.xyz");

    let magic_link_request = MagicLinkRequest {
        email: "matt@arda.xyz".to_string(),
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
            info!("   Expires in: {} minutes", response_body.expires_in_minutes);
            info!("   Timestamp: {}", response_body.timestamp);

            info!("📧 Check your email (matt@arda.xyz) for the magic link!");
            info!("🔍 You can also check the database directly for the magic link token:");
            info!("   psql $DATABASE_URL -c \"SELECT token, state, expires_at FROM magic_links WHERE email = 'matt@arda.xyz' ORDER BY created_at DESC LIMIT 1;\"");

            info!("🎯 Next steps:");
            info!("1. Copy the magic link from your email or database");
            info!("2. Visit the link in your browser to complete authentication");
            info!("3. You'll be redirected to /dashboard with a session cookie");
        }
        status => {
            let error_text = magic_link_response.text().await?;
            error!("❌ Failed to request magic link: {} - {}", status, error_text);
            return Err("Magic link request failed".into());
        }
    }

    info!("🎉 Script completed successfully!");
    Ok(())
}
