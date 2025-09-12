//! Investor registration, profile management, and KYC operations
//!
//! This module handles all investor-related operations including:
//! - Investor registration and onboarding
//! - Profile management and updates
//! - KYC verification processes
//! - Investment capacity tracking
//! - Regulatory jurisdiction handling

use arda_credit_api::BatchApiClient;
use eyre::Result;

/// Register a new investor
pub async fn register_investor(
    client: &BatchApiClient,
    email: &str,
    first_name: Option<&str>,
    last_name: Option<&str>,
    jurisdiction: Option<&str>,
    accredited_investor: Option<bool>,
    investment_capacity: Option<&str>,
    regulatory_jurisdiction: Option<&str>,
    invite_code: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("💼 Registering new investor...");

    // Parse investment capacity from string to f64 if provided
    let investment_capacity_f64 = if let Some(capacity_str) = investment_capacity {
        match capacity_str.parse::<f64>() {
            Ok(capacity) => Some(capacity),
            Err(_) => {
                println!("❌ Invalid investment capacity format: {}", capacity_str);
                return Ok(());
            }
        }
    } else {
        None
    };

    println!("   Email: {}", email);
    if let Some(first_name) = first_name {
        println!("   First Name: {}", first_name);
    }
    if let Some(last_name) = last_name {
        println!("   Last Name: {}", last_name);
    }
    println!("   Jurisdiction: {}", jurisdiction.unwrap_or("US"));
    println!("   Accredited: {}", accredited_investor.unwrap_or(false));
    if let Some(capacity) = investment_capacity_f64 {
        println!("   Investment Capacity: ${:.2}", capacity);
    }

    // Call the actual API
    match client
        .register_investor(
            email,
            first_name,
            last_name,
            jurisdiction,
            accredited_investor,
            investment_capacity_f64,
            regulatory_jurisdiction,
            invite_code,
        )
        .await
    {
        Ok(response) => {
            println!();
            println!("✅ Investor registered successfully!");

            // Extract real data from API response
            if let Some(user_id) = response.get("user_id").and_then(|v| v.as_str()) {
                println!("   Investor ID: {}", user_id);
            }
            if let Some(status) = response.get("status").and_then(|v| v.as_str()) {
                println!("   Status: {}", status);
            }
            if let Some(kyc_status) = response.get("kyc_status").and_then(|v| v.as_str()) {
                println!("   KYC Status: {}", kyc_status);
            }
            if let Some(created_at) = response.get("created_at").and_then(|v| v.as_str()) {
                println!("   Created: {}", created_at);
            }

            if verbose {
                println!();
                println!("🔧 Registration Details:");
                println!(
                    "   Regulatory Jurisdiction: {}",
                    regulatory_jurisdiction.unwrap_or("US")
                );
                if let Some(invite_code) = invite_code {
                    println!("   Invite Code Used: {}", invite_code);
                }
                if let Some(user_type) = response.get("user_type").and_then(|v| v.as_str()) {
                    println!("   User Type: {}", user_type);
                }
                println!("   Profile Completion: 40%");
                println!("   Required Actions: KYC submission, Investment preferences");
            }

            println!();
            println!("💡 Next steps:");
            if let Some(user_id) = response.get("user_id").and_then(|v| v.as_str()) {
                println!(
                    "   • Submit KYC documents: cli user submit-kyc --user-id {}",
                    user_id
                );
                println!(
                    "   • Update profile: cli loan update-investor-profile --user-id {}",
                    user_id
                );
            }
            println!("   • Create liquidity pools: cli pool create");
        }
        Err(e) => {
            eprintln!("❌ Failed to register investor: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Get investor profile details
pub async fn get_investor_profile(
    client: &BatchApiClient,
    user_id: &str,
    verbose: bool,
) -> Result<()> {
    println!("👤 Retrieving investor profile: {}", user_id);

    // Call the actual API
    match client.get_investor_profile(user_id).await {
        Ok(response) => {
            println!();
            println!("✅ Investor Profile:");

            // Extract basic profile information
            if let Some(user_id) = response.get("user_id").and_then(|v| v.as_str()) {
                println!("   User ID: {}", user_id);
            }
            if let Some(email) = response.get("email").and_then(|v| v.as_str()) {
                println!("   Email: {}", email);
            }
            if let Some(status) = response.get("status").and_then(|v| v.as_str()) {
                println!("   Status: {}", status);
            }
            if let Some(kyc_status) = response.get("kyc_status").and_then(|v| v.as_str()) {
                println!("   KYC Status: {}", kyc_status);
            }

            // Extract name information
            let first_name = response.get("first_name").and_then(|v| v.as_str());
            let last_name = response.get("last_name").and_then(|v| v.as_str());
            if first_name.is_some() || last_name.is_some() {
                let full_name_string =
                    format!("{} {}", first_name.unwrap_or(""), last_name.unwrap_or(""));
                let full_name = full_name_string.trim();
                if !full_name.is_empty() {
                    println!("   Name: {}", full_name);
                }
            }

            println!();
            println!("💰 Investment Information:");

            // Extract investment information
            if let Some(accredited) = response
                .get("accredited_investor")
                .and_then(|v| v.as_bool())
            {
                println!(
                    "   Accredited Investor: {}",
                    if accredited { "Yes" } else { "No" }
                );
            }
            if let Some(capacity) = response.get("investment_capacity").and_then(|v| v.as_f64()) {
                println!("   Investment Capacity: ${:.2}", capacity);
            }
            if let Some(jurisdiction) = response
                .get("regulatory_jurisdiction")
                .and_then(|v| v.as_str())
            {
                println!("   Regulatory Jurisdiction: {}", jurisdiction);
            }

            // Calculate and display current investments and available capacity if possible
            if let (Some(capacity), Some(current_investments)) = (
                response.get("investment_capacity").and_then(|v| v.as_f64()),
                response.get("current_investments").and_then(|v| v.as_f64()),
            ) {
                println!("   Current Investments: ${:.2}", current_investments);
                let available = capacity - current_investments;
                println!("   Available Capacity: ${:.2}", available.max(0.0));
            } else if let Some(capacity) =
                response.get("investment_capacity").and_then(|v| v.as_f64())
            {
                println!("   Available Capacity: ${:.2}", capacity);
            }

            if verbose {
                println!();
                println!("🔧 Extended Profile Details:");

                if let Some(created_at) = response.get("created_at").and_then(|v| v.as_str()) {
                    println!("   Registration Date: {}", created_at);
                }
                if let Some(updated_at) = response.get("updated_at").and_then(|v| v.as_str()) {
                    println!("   Last Activity: {}", updated_at);
                }
                if let Some(total_returns) = response.get("total_returns").and_then(|v| v.as_f64())
                {
                    println!("   Total Returns: ${:.2}", total_returns);
                }
                if let Some(return_percentage) =
                    response.get("return_percentage").and_then(|v| v.as_f64())
                {
                    println!("   Return Percentage: {:.2}%", return_percentage);
                }
                if let Some(active_pools) = response.get("active_pools").and_then(|v| v.as_u64()) {
                    println!("   Active Pools: {}", active_pools);
                }
                if let Some(risk_rating) = response.get("risk_rating").and_then(|v| v.as_str()) {
                    println!("   Risk Rating: {}", risk_rating);
                }
                if let Some(compliance_status) =
                    response.get("compliance_status").and_then(|v| v.as_str())
                {
                    println!("   Compliance Status: {}", compliance_status);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to retrieve investor profile: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Update investor profile settings
pub async fn update_investor_profile(
    client: &BatchApiClient,
    user_id: &str,
    accredited_investor: Option<bool>,
    investment_capacity: Option<&str>,
    regulatory_jurisdiction: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("✏️  Updating investor profile: {}", user_id);

    // Parse investment capacity from string to f64 if provided
    let investment_capacity_f64 = if let Some(capacity_str) = investment_capacity {
        match capacity_str.parse::<f64>() {
            Ok(capacity) => Some(capacity),
            Err(_) => {
                println!("❌ Invalid investment capacity format: {}", capacity_str);
                return Ok(());
            }
        }
    } else {
        None
    };

    let mut updates = Vec::new();
    if let Some(accredited) = accredited_investor {
        updates.push(format!("Accredited Status: {}", accredited));
    }
    if let Some(capacity) = investment_capacity_f64 {
        updates.push(format!("Investment Capacity: ${:.2}", capacity));
    }
    if let Some(jurisdiction) = regulatory_jurisdiction {
        updates.push(format!("Regulatory Jurisdiction: {}", jurisdiction));
    }

    if updates.is_empty() {
        println!("⚠️  No updates specified");
        return Ok(());
    }

    println!("   Updates:");
    for update in &updates {
        println!("   • {}", update);
    }

    // Call the actual API
    match client
        .update_investor_profile(
            user_id,
            accredited_investor,
            investment_capacity_f64,
            regulatory_jurisdiction,
        )
        .await
    {
        Ok(response) => {
            println!();
            println!("✅ Investor profile updated successfully!");

            if verbose {
                println!();
                println!("🔧 Update Details:");
                println!("   Fields Updated: {}", updates.len());

                // Extract additional information from response
                if let Some(updated_at) = response.get("updated_at").and_then(|v| v.as_str()) {
                    println!("   Updated At: {}", updated_at);
                }
                if let Some(compliance_review) = response
                    .get("compliance_review_triggered")
                    .and_then(|v| v.as_bool())
                {
                    println!(
                        "   Compliance Review: {}",
                        if compliance_review {
                            "Triggered"
                        } else {
                            "Not Required"
                        }
                    );
                }
                if let Some(profile_completion) = response
                    .get("profile_completion_percentage")
                    .and_then(|v| v.as_u64())
                {
                    println!("   Profile Completion: {}%", profile_completion);
                }
                if let Some(status) = response.get("status").and_then(|v| v.as_str()) {
                    println!("   Status: {}", status);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to update investor profile: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
