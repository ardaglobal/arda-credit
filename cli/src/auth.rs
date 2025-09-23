//! Magic link authentication testing commands
//!
//! This module provides CLI commands for testing and debugging the magic link
//! authentication system. These commands are primarily for development and testing.

use clap::Subcommand;
use eyre::Result;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

use arda_credit_api::BatchApiClient;

/// Authentication testing CLI commands
#[derive(Subcommand)]
pub enum AuthCommands {
    /// Request a magic link via email (tests API endpoint)
    RequestMagicLink {
        /// Email address to send magic link to
        #[arg(long)]
        email: String,
        /// Redirect URI after authentication (optional)
        #[arg(long)]
        redirect_uri: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get magic link from database (development/testing only)
    GetMagicLink {
        /// Email address to retrieve magic link for
        #[arg(long)]
        email: String,
        /// Show the actual token value (DEVELOPMENT ONLY - security risk)
        #[arg(long)]
        show_token: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Verify a magic link token (tests verification endpoint)
    VerifyMagicLink {
        /// Magic link token
        #[arg(long)]
        token: String,
        /// State parameter (CSRF protection)
        #[arg(long)]
        state: String,
        /// Redirect URI (optional)
        #[arg(long)]
        redirect_uri: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test session validation with a session token
    TestSession {
        /// Session token to validate
        #[arg(long)]
        session_token: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test complete authentication flow (automated)
    TestFlow {
        /// Email address for testing
        #[arg(long)]
        email: String,
        /// Skip actual email sending (use database token)
        #[arg(long)]
        skip_email: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test rate limiting behavior
    TestRateLimits {
        /// Email address for rate limit testing
        #[arg(long)]
        email: String,
        /// Number of requests to attempt
        #[arg(long, default_value = "5")]
        attempts: i32,
        /// Delay between requests in milliseconds
        #[arg(long, default_value = "100")]
        delay_ms: u64,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle auth command routing and execution
pub async fn handle_auth_command(
    client: &BatchApiClient,
    command: AuthCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        AuthCommands::RequestMagicLink {
            email,
            redirect_uri,
            verbose: cmd_verbose,
        } => request_magic_link_test(&email, redirect_uri.as_deref(), cmd_verbose || verbose).await,
        AuthCommands::GetMagicLink {
            email,
            show_token,
            verbose: cmd_verbose,
        } => get_magic_link_from_db(&email, show_token, cmd_verbose || verbose).await,
        AuthCommands::VerifyMagicLink {
            token,
            state,
            redirect_uri,
            verbose: cmd_verbose,
        } => {
            verify_magic_link_test(
                &token,
                &state,
                redirect_uri.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        AuthCommands::TestSession {
            session_token,
            verbose: cmd_verbose,
        } => test_session_validation(&session_token, cmd_verbose || verbose).await,
        AuthCommands::TestFlow {
            email,
            skip_email,
            verbose: cmd_verbose,
        } => test_complete_auth_flow(&email, skip_email, cmd_verbose || verbose).await,
        AuthCommands::TestRateLimits {
            email,
            attempts,
            delay_ms,
            verbose: cmd_verbose,
        } => test_rate_limiting(&email, attempts, delay_ms, cmd_verbose || verbose).await,
    }
}

/// Test magic link request API endpoint
pub async fn request_magic_link_test(
    email: &str,
    redirect_uri: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("🔗 Testing magic link request...");
    println!("   Email: {}", email);
    if let Some(uri) = redirect_uri {
        println!("   Redirect URI: {}", uri);
    }

    let http_client = Client::new();
    let api_url =
        env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/api/v1/auth/magic-link", api_url);

    let mut request_body = json!({
        "email": email
    });

    if let Some(uri) = redirect_uri {
        request_body["redirect_uri"] = json!(uri);
    }

    match http_client.post(&url).json(&request_body).send().await {
        Ok(response) => {
            let status = response.status();
            println!("   HTTP Status: {}", status);

            match response.json::<Value>().await {
                Ok(body) => {
                    if status.is_success() {
                        println!();
                        println!("✅ Magic link request successful!");

                        if let Some(message) = body.get("message").and_then(|v| v.as_str()) {
                            println!("   Message: {}", message);
                        }

                        if let Some(expires) =
                            body.get("expires_in_minutes").and_then(|v| v.as_i64())
                        {
                            println!("   Expires in: {} minutes", expires);
                        }

                        if let Some(message_id) = body.get("message_id").and_then(|v| v.as_str()) {
                            println!("   Email Message ID: {}", message_id);
                        }

                        if verbose {
                            println!();
                            println!("🔧 Full Response:");
                            println!("{}", serde_json::to_string_pretty(&body)?);
                        }

                        println!();
                        println!("💡 Next steps:");
                        println!("   • Check your email for the magic link");
                        println!(
                            "   • Or use: cli auth get-magic-link --email {} --show-token",
                            email
                        );
                        println!(
                            "   • Test complete flow: cli auth test-flow --email {}",
                            email
                        );
                    } else {
                        println!();
                        println!("❌ Magic link request failed!");

                        if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
                            println!("   Error: {}", error);
                        }

                        if let Some(message) = body.get("message").and_then(|v| v.as_str()) {
                            println!("   Message: {}", message);
                        }

                        // Handle specific error cases
                        match status.as_u16() {
                            404 => {
                                println!();
                                println!("💡 User not found - register first:");
                                println!("   make cli ARGS=\"user create --user-type investor --email {} --password test123\"", email);
                            }
                            429 => {
                                println!();
                                println!("💡 Rate limit exceeded - try:");
                                println!("   • Wait 1 hour and try again");
                                println!("   • Use a different email address");
                                println!(
                                    "   • Test rate limits: cli auth test-rate-limits --email {}",
                                    email
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to parse response: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Network error: {}", e);
            println!();
            println!("🔍 Troubleshooting:");
            println!("   • Check if API server is running: make server");
            println!("   • Verify ARDA_CREDIT_API_URL: {}", api_url);
            println!("   • Test connectivity: cli batch health-check");
        }
    }

    Ok(())
}

/// Get magic link from database (development only)
pub async fn get_magic_link_from_db(email: &str, show_token: bool, verbose: bool) -> Result<()> {
    println!("🔍 Retrieving magic link from database...");
    println!("   Email: {}", email);

    if show_token {
        println!("   ⚠️  WARNING: --show-token is enabled (development only!)");
    }

    // Get database URL
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| eyre::eyre!("DATABASE_URL environment variable not set"))?;

    // Connect to database (using sqlx would be ideal, but for CLI simplicity, use psql)
    let query = format!(
        "SELECT ml.id, ml.user_id, ml.state, ml.redirect_uri, ml.expires_at, ml.consumed_at, ml.created_at, i.email
         FROM magic_links ml
         JOIN identities i ON ml.user_id = i.id
         WHERE i.email = '{}'
         AND ml.consumed_at IS NULL
         ORDER BY ml.created_at DESC
         LIMIT 1;",
        email
    );

    // Use psql command to query database
    let output = std::process::Command::new("psql")
        .arg(&database_url)
        .arg("-t") // Tuple only output
        .arg("-c")
        .arg(&query)
        .output()?;

    if !output.status.success() {
        println!("❌ Database query failed");
        println!("   Error: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = result.trim().lines().collect();

    if lines.is_empty() || lines[0].trim().is_empty() {
        println!();
        println!("📭 No active magic links found for: {}", email);
        println!();
        println!("💡 Generate one first:");
        println!(
            "   make cli ARGS=\"auth request-magic-link --email {}\"",
            email
        );
        return Ok(());
    }

    println!();
    println!("✅ Found active magic link:");

    // Parse the result (simplified - would need proper parsing)
    let parts: Vec<&str> = lines[0].split('|').map(|s| s.trim()).collect();
    if parts.len() >= 7 {
        println!("   Magic Link ID: {}", parts[0]);
        println!("   User ID: {}", parts[1]);
        println!("   State: {}", parts[2]);
        if !parts[3].is_empty() {
            println!("   Redirect URI: {}", parts[3]);
        }
        println!("   Expires: {}", parts[4]);
        println!("   Created: {}", parts[6]);
        println!("   Email: {}", parts[7]);
    }

    if show_token {
        println!();
        println!("⚠️  TOKEN RETRIEVAL (DEVELOPMENT ONLY):");
        println!("   🚨 In production, tokens are hashed and cannot be retrieved!");
        println!("   🚨 This is only possible in development for testing!");
        println!();
        println!("   To retrieve the actual token, you would need to:");
        println!("   1. Modify the magic link service to log tokens (DEV ONLY)");
        println!("   2. Check application logs during generation");
        println!("   3. Use the email link that was sent");
        println!();
        println!("   For testing purposes, use the database state value:");
        if parts.len() >= 3 {
            println!("   State Parameter: {}", parts[2]);
        }
    }

    if verbose {
        println!();
        println!("🔧 Database Query Details:");
        println!("   Query: {}", query);
        println!("   Results found: {}", lines.len());
    }

    println!();
    println!("💡 Next steps:");
    println!("   • Check your email for the actual magic link");
    println!(
        "   • Test verification: cli auth verify-magic-link --token TOKEN --state {}",
        parts.get(2).unwrap_or(&"STATE")
    );

    Ok(())
}

/// Test magic link verification endpoint
pub async fn verify_magic_link_test(
    token: &str,
    state: &str,
    redirect_uri: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("🔓 Testing magic link verification...");
    println!("   Token: {}...{}", &token[..8], &token[token.len() - 8..]);
    println!("   State: {}", state);
    if let Some(uri) = redirect_uri {
        println!("   Redirect URI: {}", uri);
    }

    let http_client = Client::new();
    let api_url =
        env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/api/v1/auth/verify", api_url);

    let mut request_body = json!({
        "token": token,
        "state": state
    });

    if let Some(uri) = redirect_uri {
        request_body["redirect_uri"] = json!(uri);
    }

    match http_client.post(&url).json(&request_body).send().await {
        Ok(response) => {
            let status = response.status();
            println!("   HTTP Status: {}", status);

            // Extract session cookie if present
            let session_cookie = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .find_map(|cookie| {
                    let cookie_str = cookie.to_str().ok()?;
                    if cookie_str.contains("session_token=") {
                        // Extract session token value
                        cookie_str.split("session_token=").nth(1)?.split(';').next()
                    } else {
                        None
                    }
                })
                .map(|s| s.to_string());

            match response.json::<Value>().await {
                Ok(body) => {
                    if status.is_success() {
                        println!();
                        println!("✅ Magic link verification successful!");

                        if let Some(user) = body.get("user") {
                            if let Some(email) = user.get("email").and_then(|v| v.as_str()) {
                                println!("   User Email: {}", email);
                            }
                            if let Some(user_id) = user.get("user_id").and_then(|v| v.as_str()) {
                                println!("   User ID: {}", user_id);
                            }
                            if let Some(user_type) = user.get("user_type").and_then(|v| v.as_str())
                            {
                                println!("   User Type: {}", user_type);
                            }
                        }

                        if let Some(session_token) = &session_cookie {
                            println!("   Session Created: ✅");
                            println!(
                                "   Session Token: {}...{}",
                                &session_token[..8],
                                &session_token[session_token.len() - 8..]
                            );

                            println!();
                            println!("💡 Test the session:");
                            println!("   cli auth test-session --session-token {}", session_token);
                        }

                        if verbose {
                            println!();
                            println!("🔧 Full Response:");
                            println!("{}", serde_json::to_string_pretty(&body)?);
                        }
                    } else {
                        println!();
                        println!("❌ Magic link verification failed!");

                        if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
                            println!("   Error: {}", error);
                        }

                        if let Some(message) = body.get("message").and_then(|v| v.as_str()) {
                            println!("   Message: {}", message);
                        }

                        // Handle specific error cases
                        match status.as_u16() {
                            404 => {
                                println!();
                                println!("💡 Magic link not found or expired");
                                println!("   • Request a new magic link");
                                println!("   • Check if the token is correct");
                            }
                            400 => {
                                println!();
                                println!("💡 Invalid request parameters");
                                println!("   • Check token format (should be 43 characters)");
                                println!("   • Verify state parameter is correct");
                            }
                            410 => {
                                println!();
                                println!("💡 Magic link expired or already used");
                                println!("   • Request a new magic link");
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to parse response: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Network error: {}", e);
        }
    }

    Ok(())
}

/// Test session validation
pub async fn test_session_validation(session_token: &str, verbose: bool) -> Result<()> {
    println!("🔐 Testing session validation...");
    println!(
        "   Session Token: {}...{}",
        &session_token[..8],
        &session_token[session_token.len() - 8..]
    );

    let http_client = Client::new();
    let api_url =
        env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/api/v1/auth/me", api_url);

    match http_client
        .get(&url)
        .header("Cookie", format!("session_token={}", session_token))
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            println!("   HTTP Status: {}", status);

            match response.json::<Value>().await {
                Ok(body) => {
                    if status.is_success() {
                        println!();
                        println!("✅ Session validation successful!");

                        if let Some(user_id) = body.get("user_id").and_then(|v| v.as_str()) {
                            println!("   User ID: {}", user_id);
                        }
                        if let Some(email) = body.get("email").and_then(|v| v.as_str()) {
                            println!("   Email: {}", email);
                        }
                        if let Some(user_type) = body.get("user_type").and_then(|v| v.as_str()) {
                            println!("   User Type: {}", user_type);
                        }

                        if verbose {
                            println!();
                            println!("🔧 Full User Data:");
                            println!("{}", serde_json::to_string_pretty(&body)?);
                        }
                    } else {
                        println!();
                        println!("❌ Session validation failed!");

                        if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
                            println!("   Error: {}", error);
                        }

                        match status.as_u16() {
                            401 => {
                                println!("   💡 Session expired or invalid");
                                println!("   • Request a new magic link");
                                println!("   • Complete auth flow again");
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to parse response: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Network error: {}", e);
        }
    }

    Ok(())
}

/// Test complete authentication flow (automated)
pub async fn test_complete_auth_flow(email: &str, skip_email: bool, verbose: bool) -> Result<()> {
    println!("🎯 Testing complete authentication flow...");
    println!("   Email: {}", email);
    println!("   Skip email: {}", skip_email);

    // Step 1: Request magic link
    println!();
    println!("📋 Step 1: Requesting magic link...");
    request_magic_link_test(email, Some("/dashboard"), false).await?;

    if skip_email {
        // Step 2: Get token from database
        println!();
        println!("📋 Step 2: Retrieving token from database...");
        get_magic_link_from_db(email, false, false).await?;

        println!();
        println!("⚠️  INCOMPLETE FLOW:");
        println!("   • Token retrieval from database requires manual intervention");
        println!("   • In a real flow, user would click email link");
        println!("   • For complete testing, check your email and use:");
        println!("     cli auth verify-magic-link --token TOKEN --state STATE");
    } else {
        println!();
        println!("📧 Step 2: Check your email for the magic link");
        println!("   • Click the link or copy the token/state parameters");
        println!("   • Then run: cli auth verify-magic-link --token TOKEN --state STATE");
    }

    println!();
    println!("📋 Remaining Steps (manual):");
    println!("   3. Verify magic link → creates session");
    println!("   4. Test authenticated endpoint → cli auth test-session");
    println!("   5. Logout → POST /api/v1/auth/logout");

    if verbose {
        println!();
        println!("🔧 Complete Flow Commands:");
        println!("   1. cli auth request-magic-link --email {}", email);
        println!("   2. [Check email for TOKEN and STATE]");
        println!("   3. cli auth verify-magic-link --token TOKEN --state STATE");
        println!("   4. cli auth test-session --session-token SESSION_TOKEN");
        println!("   5. curl -X POST http://localhost:8080/api/v1/auth/logout -H \"Cookie: session_token=SESSION_TOKEN\"");
    }

    Ok(())
}

/// Test rate limiting behavior
pub async fn test_rate_limiting(
    email: &str,
    attempts: i32,
    delay_ms: u64,
    verbose: bool,
) -> Result<()> {
    println!("🚦 Testing rate limiting...");
    println!("   Email: {}", email);
    println!("   Attempts: {}", attempts);
    println!("   Delay: {}ms between requests", delay_ms);

    let http_client = Client::new();
    let api_url =
        env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let url = format!("{}/api/v1/auth/magic-link", api_url);

    let request_body = json!({
        "email": email,
        "redirect_uri": "/dashboard"
    });

    let mut successful_requests = 0;
    let mut rate_limited_requests = 0;
    let mut error_requests = 0;

    println!();
    println!("🔄 Making {} requests...", attempts);

    for i in 1..=attempts {
        if verbose {
            println!("   Request {}/{}", i, attempts);
        } else {
            print!(".");
        }

        match http_client.post(&url).json(&request_body).send().await {
            Ok(response) => {
                let status = response.status();

                match status.as_u16() {
                    200 => {
                        successful_requests += 1;
                        if verbose {
                            println!("      ✅ Success (HTTP 200)");
                        }
                    }
                    429 => {
                        rate_limited_requests += 1;
                        if verbose {
                            println!("      🚦 Rate limited (HTTP 429)");
                        }

                        // Parse rate limit details
                        if let Ok(body) = response.json::<Value>().await {
                            if let Some(details) = body.get("details") {
                                if verbose {
                                    if let Some(limit) = details.get("limit") {
                                        println!("         Limit: {} requests", limit);
                                    }
                                    if let Some(window) = details.get("window_seconds") {
                                        println!("         Window: {}s", window);
                                    }
                                    if let Some(retry_after) = details.get("retry_after_seconds") {
                                        println!("         Retry after: {}s", retry_after);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        error_requests += 1;
                        if verbose {
                            println!("      ❌ Error (HTTP {})", status);
                        }
                    }
                }
            }
            Err(e) => {
                error_requests += 1;
                if verbose {
                    println!("      ❌ Network error: {}", e);
                }
            }
        }

        // Delay between requests
        if i < attempts {
            sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    if !verbose {
        println!(); // New line after dots
    }

    println!();
    println!("🎯 Rate Limiting Test Results:");
    println!("   Total Requests: {}", attempts);
    println!("   Successful: {}", successful_requests);
    println!("   Rate Limited: {}", rate_limited_requests);
    println!("   Errors: {}", error_requests);

    // Analyze results
    if rate_limited_requests > 0 {
        println!();
        println!("✅ Rate limiting is working!");
        println!(
            "   • Requests {} blocked after {} successful",
            rate_limited_requests, successful_requests
        );
        println!(
            "   • Rate limit threshold appears to be ~{} requests",
            successful_requests
        );
    } else if successful_requests == attempts {
        println!();
        println!("⚠️  Rate limiting may not be working");
        println!("   • All {} requests succeeded", attempts);
        println!("   • Expected some to be rate limited");
        println!("   • Check rate limiting configuration");
    } else {
        println!();
        println!("❓ Mixed results - investigate further");
        println!("   • Some requests failed for other reasons");
        println!("   • Check error messages above");
    }

    if verbose {
        println!();
        println!("🔧 Rate Limiting Analysis:");
        println!(
            "   Success Rate: {:.1}%",
            (successful_requests as f64 / attempts as f64) * 100.0
        );
        if rate_limited_requests > 0 {
            println!("   Rate Limit Threshold: ~{} requests", successful_requests);
            println!("   Protection Effectiveness: ✅");
        } else {
            println!("   Rate Limit Threshold: Not detected");
            println!("   Protection Effectiveness: ⚠️  Needs investigation");
        }
    }

    Ok(())
}
