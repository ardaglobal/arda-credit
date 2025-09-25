//! Transfer CLI commands for loan trading
//!
//! This module provides command-line interface for loan transfer operations including
//! executing transfers, viewing history, and managing transfer lifecycle.

use clap::{Args, Subcommand};
use eyre::Result;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

use crate::cli::{get_api_base_url, handle_api_error};

/// Transfer management commands
#[derive(Debug, Subcommand)]
pub enum TransferCommands {
    /// Execute loan ownership transfer
    Execute(ExecuteTransferArgs),

    /// View transfer history for a loan
    LoanHistory(LoanTransferHistoryArgs),

    /// View transfer history for a user
    UserHistory(UserTransferHistoryArgs),

    /// Validate transfer before executing
    Validate(ValidateTransferArgs),
}

/// Arguments for executing transfer
#[derive(Debug, Args)]
pub struct ExecuteTransferArgs {
    /// UUID of the loan to transfer
    #[arg(long)]
    pub loan_id: String,

    /// UUID of the buyer (new owner)
    #[arg(long)]
    pub buyer_id: String,

    /// Purchase price in USDC
    #[arg(long)]
    pub purchase_price: String,

    /// Optional transfer notes
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for loan transfer history
#[derive(Debug, Args)]
pub struct LoanTransferHistoryArgs {
    /// UUID of the loan
    #[arg(long)]
    pub loan_id: String,

    /// Show verbose output with all details
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for user transfer history
#[derive(Debug, Args)]
pub struct UserTransferHistoryArgs {
    /// UUID of the user
    #[arg(long)]
    pub user_id: String,

    /// Maximum number of results to return
    #[arg(long, default_value = "20")]
    pub limit: u32,

    /// Number of results to skip (for pagination)
    #[arg(long, default_value = "0")]
    pub offset: u32,

    /// Show verbose output with all details
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for validating transfer
#[derive(Debug, Args)]
pub struct ValidateTransferArgs {
    /// UUID of the loan to transfer
    #[arg(long)]
    pub loan_id: String,

    /// UUID of the buyer (new owner)
    #[arg(long)]
    pub buyer_id: String,

    /// Purchase price in USDC
    #[arg(long)]
    pub purchase_price: String,
}

/// Handle transfer commands
pub async fn handle_transfer_command(command: TransferCommands) -> Result<()> {
    match command {
        TransferCommands::Execute(args) => execute_transfer(args).await,
        TransferCommands::LoanHistory(args) => loan_transfer_history(args).await,
        TransferCommands::UserHistory(args) => user_transfer_history(args).await,
        TransferCommands::Validate(args) => validate_transfer(args).await,
    }
}

/// Execute loan ownership transfer
async fn execute_transfer(args: ExecuteTransferArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate input formats
    Uuid::parse_str(&args.loan_id).map_err(|_| eyre::eyre!("Invalid loan ID format"))?;
    Uuid::parse_str(&args.buyer_id).map_err(|_| eyre::eyre!("Invalid buyer ID format"))?;
    args.purchase_price
        .parse::<f64>()
        .map_err(|_| eyre::eyre!("Invalid purchase price format"))?;

    println!("🔄 Executing loan transfer...");
    println!("   📋 Loan ID: {}", args.loan_id);
    println!("   🧑 Buyer ID: {}", args.buyer_id);
    println!("   💰 Purchase Price: ${}", args.purchase_price);

    if let Some(ref notes) = args.notes {
        println!("   📝 Notes: {}", notes);
    }

    let request_body = json!({
        "new_lender_id": args.buyer_id,
        "purchase_price": args.purchase_price,
        "transfer_notes": args.notes
    });

    // Call your transfer API endpoint
    let response = client
        .post(&format!(
            "{}/api/v1/loan/{}/transfer",
            base_url, args.loan_id
        ))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    // Parse response
    let transfer_id = result["transfer_id"].as_str().unwrap_or("unknown");
    let transfer_date = result["transfer_date"].as_str().unwrap_or("unknown");
    let previous_owner = result["previous_lender_id"].as_str().unwrap_or("unknown");
    let new_owner = result["new_lender_id"].as_str().unwrap_or("unknown");
    let settlement = &result["payment_settlement"];

    println!("");
    println!("✅ Loan transfer completed successfully!");
    println!("   🆔 Transfer ID: {}", transfer_id);
    println!("   📅 Transfer Date: {}", transfer_date);
    println!("   👥 Ownership: {} → {}", previous_owner, new_owner);
    println!("   💰 Amount: ${}", args.purchase_price);

    if let Some(buyer_tx) = settlement["buyer_transaction_id"].as_str() {
        println!("   🧾 Buyer Transaction: {}", buyer_tx);
    }
    if let Some(seller_tx) = settlement["seller_transaction_id"].as_str() {
        println!("   🧾 Seller Transaction: {}", seller_tx);
    }

    let updated_loan = &result["updated_loan"];
    if let Some(status) = updated_loan["loan_status"].as_str() {
        println!("   🔘 Loan Status: {}", status);
    }

    Ok(())
}

/// Validate transfer before executing
async fn validate_transfer(args: ValidateTransferArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate input formats
    Uuid::parse_str(&args.loan_id).map_err(|_| eyre::eyre!("Invalid loan ID format"))?;
    Uuid::parse_str(&args.buyer_id).map_err(|_| eyre::eyre!("Invalid buyer ID format"))?;
    args.purchase_price
        .parse::<f64>()
        .map_err(|_| eyre::eyre!("Invalid purchase price format"))?;

    println!("🔍 Validating transfer...");
    println!("   📋 Loan ID: {}", args.loan_id);
    println!("   🧑 Buyer ID: {}", args.buyer_id);
    println!("   💰 Purchase Price: ${}", args.purchase_price);

    let request_body = json!({
        "new_lender_id": args.buyer_id,
        "purchase_price": args.purchase_price
    });

    // Call your validation API endpoint
    let response = client
        .post(&format!(
            "{}/api/v1/loan/{}/transfer/validate",
            base_url, args.loan_id
        ))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    println!("");

    // Parse validation results
    let is_valid = result["is_valid"].as_bool().unwrap_or(false);
    let loan_exists = result["loan_exists"].as_bool().unwrap_or(false);
    let loan_for_sale = result["loan_for_sale"].as_bool().unwrap_or(false);
    let buyer_has_funds = result["buyer_has_funds"].as_bool().unwrap_or(false);
    let buyer_is_not_owner = result["buyer_is_not_owner"].as_bool().unwrap_or(false);
    let buyer_kyc_approved = result["buyer_kyc_approved"].as_bool().unwrap_or(false);
    let price_matches = result["price_matches"].as_bool().unwrap_or(false);

    if is_valid {
        println!("✅ Transfer validation: PASSED");
        println!("   🎉 This transfer can be executed successfully!");
    } else {
        println!("❌ Transfer validation: FAILED");

        // Show detailed validation results
        println!(
            "   📋 Loan exists: {}",
            if loan_exists { "✅" } else { "❌" }
        );
        println!(
            "   🛒 Loan for sale: {}",
            if loan_for_sale { "✅" } else { "❌" }
        );
        println!(
            "   💰 Buyer has funds: {}",
            if buyer_has_funds { "✅" } else { "❌" }
        );
        println!(
            "   👤 Buyer != owner: {}",
            if buyer_is_not_owner { "✅" } else { "❌" }
        );
        println!(
            "   🆔 Buyer KYC approved: {}",
            if buyer_kyc_approved { "✅" } else { "❌" }
        );
        println!(
            "   💲 Price matches: {}",
            if price_matches { "✅" } else { "❌" }
        );

        if let Some(balance) = result["available_balance"].as_str() {
            println!("   💳 Available balance: ${}", balance);
        }
    }

    // Show validation errors if any
    if let Some(errors) = result["validation_errors"].as_array() {
        if !errors.is_empty() {
            println!("");
            println!("❌ Validation errors:");
            for error in errors {
                if let Some(error_str) = error.as_str() {
                    println!("   • {}", error_str);
                }
            }
        }
    }

    Ok(())
}

/// View loan transfer history
async fn loan_transfer_history(args: LoanTransferHistoryArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate loan ID
    Uuid::parse_str(&args.loan_id).map_err(|_| eyre::eyre!("Invalid loan ID format"))?;

    println!("📋 Getting transfer history for loan {}...", args.loan_id);

    // Call your transfer history API endpoint
    let response = client
        .get(&format!(
            "{}/api/v1/loan/{}/transfers",
            base_url, args.loan_id
        ))
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    let empty_vec = vec![];
    let transfers = result["transfers"].as_array().unwrap_or(&empty_vec);
    let total_transfers = result["total_transfers"].as_i64().unwrap_or(0);

    if transfers.is_empty() {
        println!("📭 No transfers found for this loan");
        return Ok(());
    }

    println!(
        "📊 Found {} transfers for loan {}",
        total_transfers, args.loan_id
    );
    println!("");

    // Display transfers
    for (i, transfer) in transfers.iter().enumerate() {
        let transfer_id = transfer["transfer_id"].as_str().unwrap_or("unknown");
        let from_email = transfer["from_lender_email"].as_str().unwrap_or("unknown");
        let to_email = transfer["to_lender_email"].as_str().unwrap_or("unknown");
        let price = transfer["purchase_price"].as_str().unwrap_or("0");
        let status = transfer["transfer_status"].as_str().unwrap_or("unknown");
        let date = transfer["transfer_date"].as_str().unwrap_or("unknown");

        println!(
            "{}. Transfer {} ({})",
            i + 1,
            transfer_id,
            status.to_uppercase()
        );
        println!("   👥 {} → {}", from_email, to_email);
        println!("   💰 Purchase Price: ${}", price);
        println!("   📅 Date: {}", date);

        if args.verbose {
            if let Some(notes) = transfer["notes"].as_str() {
                if !notes.is_empty() {
                    println!("   📝 Notes: {}", notes);
                }
            }

            if let Some(completed) = transfer["completed_at"].as_str() {
                println!("   ✅ Completed: {}", completed);
            }
        }

        println!("");
    }

    Ok(())
}

/// View user transfer history
async fn user_transfer_history(args: UserTransferHistoryArgs) -> Result<()> {
    let client = Client::new();
    let base_url = get_api_base_url();

    // Validate user ID
    Uuid::parse_str(&args.user_id).map_err(|_| eyre::eyre!("Invalid user ID format"))?;

    println!("👤 Getting transfer history for user {}...", args.user_id);

    // Call your user transfer history API endpoint
    let response = client
        .get(&format!(
            "{}/api/v1/user/{}/transfers",
            base_url, args.user_id
        ))
        .query(&[
            ("limit", args.limit.to_string()),
            ("offset", args.offset.to_string()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        handle_api_error(response).await?;
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    let empty_vec = vec![];
    let transfers = result["transfers"].as_array().unwrap_or(&empty_vec);
    let has_more = result["has_more"].as_bool().unwrap_or(false);

    if transfers.is_empty() {
        println!("📭 No transfers found for this user");
        return Ok(());
    }

    println!("📊 Found {} transfers", transfers.len());
    println!("");

    // Display transfers
    for (i, transfer) in transfers.iter().enumerate() {
        let transfer_id = transfer["transfer_id"].as_str().unwrap_or("unknown");
        let loan_id = transfer["loan_id"].as_str().unwrap_or("unknown");
        let from_email = transfer["from_lender_email"].as_str().unwrap_or("unknown");
        let to_email = transfer["to_lender_email"].as_str().unwrap_or("unknown");
        let price = transfer["purchase_price"].as_str().unwrap_or("0");
        let status = transfer["transfer_status"].as_str().unwrap_or("unknown");
        let date = transfer["transfer_date"].as_str().unwrap_or("unknown");

        // Determine if user was buyer or seller
        let user_role = if transfer["from_lender_id"].as_str() == Some(&args.user_id) {
            "SELLER"
        } else {
            "BUYER"
        };

        println!(
            "{}. {} - {} ({})",
            i + 1,
            user_role,
            loan_id,
            status.to_uppercase()
        );
        println!("   👥 {} → {}", from_email, to_email);
        println!("   💰 Price: ${}", price);
        println!("   📅 Date: {}", date);

        if args.verbose {
            println!("   🆔 Transfer ID: {}", transfer_id);
            if let Some(notes) = transfer["notes"].as_str() {
                if !notes.is_empty() {
                    println!("   📝 Notes: {}", notes);
                }
            }
        }

        println!("");
    }

    if has_more {
        println!(
            "💡 Use --offset {} to see more results",
            args.offset + args.limit
        );
    }

    Ok(())
}
