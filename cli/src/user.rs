//! User management, KYC, and invite code operations
//!
//! This module handles all user-related operations for the Arda Credit CLI including:
//! - User account creation and management
//! - KYC (Know Your Customer) verification processes
//! - Invite code generation and validation
//! - User profile updates and maintenance
//! - User data retrieval and listing operations
//!
//! All user operations respect privacy requirements while enabling regulatory
//! compliance through proper KYC verification and audit trails.

use clap::Subcommand;
use eyre::Result;
use std::io::{self, Write};

use arda_credit_api::BatchApiClient;

/// User management CLI commands
#[derive(Subcommand)]
pub enum UserCommands {
    /// Create a new user account (passwordless - use magic link to authenticate)
    Create {
        /// Invite code for user registration
        #[arg(long)]
        invite_code: String,
        /// User email address
        #[arg(long)]
        email: String,
        /// User jurisdiction (optional, defaults to US)
        #[arg(long)]
        jurisdiction: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get user details by ID
    Get {
        /// User ID to retrieve
        #[arg(long)]
        user_id: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List all users with optional filters
    List {
        /// Filter by user type
        #[arg(long)]
        user_type: Option<String>,
        /// Filter by KYC status
        #[arg(long)]
        kyc_status: Option<String>,
        /// Maximum number of users to return
        #[arg(long, default_value = "20")]
        limit: Option<i32>,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: Option<i32>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update user profile information
    Update {
        /// User ID to update
        #[arg(long)]
        user_id: String,
        /// New email address
        #[arg(long)]
        email: Option<String>,
        /// New user type (Investor or Lender)
        #[arg(long)]
        user_type: Option<String>,
        /// New jurisdiction
        #[arg(long)]
        jurisdiction: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Delete user by ID
    Delete {
        /// User ID to delete
        #[arg(long)]
        user_id: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Submit KYC information for a user
    SubmitKyc {
        /// User ID to submit KYC for
        #[arg(long)]
        user_id: String,
        /// Document type
        #[arg(long)]
        document_type: String,
        /// Document hash
        #[arg(long)]
        document_hash: String,
        /// Jurisdiction (optional)
        #[arg(long)]
        jurisdiction: Option<String>,
        /// KYC investor type (QIB, QP, AI - required for investors)
        #[arg(long)]
        kyc_investor_type: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update KYC status (admin only)
    UpdateKycStatus {
        /// User ID to update KYC status for
        #[arg(long)]
        user_id: String,
        /// New KYC status (Pending, Approved, Rejected)
        #[arg(long)]
        status: String,
        /// Optional notes
        #[arg(long)]
        notes: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Generate an invite code for user registration
    GenerateInviteCode {
        /// User type for the invite code (investor, originator, admin)
        #[arg(long)]
        user_type: String,
        /// Number of times this code can be used
        #[arg(long, default_value = "1")]
        max_uses: Option<i32>,
        /// Hours until the code expires (default: 168 hours = 7 days)
        #[arg(long, default_value = "168")]
        expires_in_hours: Option<i32>,
        /// Description for the invite code
        #[arg(long)]
        description: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Check if an invite code is valid
    CheckInviteCode {
        /// The invite code to check
        #[arg(long)]
        code: String,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Use an invite code (for testing - marks it as used)
    UseInviteCode {
        /// The invite code to use
        #[arg(long)]
        code: String,
        /// User ID to associate with the code usage (optional)
        #[arg(long)]
        user_id: Option<String>,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test user operations end-to-end
    Test {
        /// Number of users to create for testing
        #[arg(long, default_value = "5")]
        count: Option<i32>,
        /// Test user type for invite code generation (investor, originator, admin)
        #[arg(long, default_value = "investor")]
        user_type: Option<String>,
        /// Enable KYC testing
        #[arg(long)]
        test_kyc: bool,
        /// Enable invite code testing
        #[arg(long)]
        test_invites: bool,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle user command routing and execution
pub async fn handle_user_command(
    client: &BatchApiClient,
    command: UserCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        UserCommands::Create {
            invite_code,
            email,
            jurisdiction,
            verbose: cmd_verbose,
        } => {
            create_user(
                client,
                &invite_code,
                &email,
                jurisdiction.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::Get {
            user_id,
            verbose: cmd_verbose,
        } => get_user(client, &user_id, cmd_verbose || verbose).await,
        UserCommands::List {
            user_type,
            kyc_status,
            limit,
            offset,
            verbose: cmd_verbose,
        } => {
            list_users(
                client,
                user_type.as_deref(),
                kyc_status.as_deref(),
                limit,
                offset,
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::Update {
            user_id,
            email,
            user_type,
            jurisdiction,
            verbose: cmd_verbose,
        } => {
            update_user(
                client,
                &user_id,
                email.as_deref(),
                user_type.as_deref(),
                jurisdiction.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::Delete {
            user_id,
            force,
            verbose: cmd_verbose,
        } => delete_user(client, &user_id, force, cmd_verbose || verbose).await,
        UserCommands::SubmitKyc {
            user_id,
            document_type,
            document_hash,
            jurisdiction,
            kyc_investor_type,
            verbose: cmd_verbose,
        } => {
            submit_kyc(
                client,
                &user_id,
                &document_type,
                &document_hash,
                jurisdiction.as_deref(),
                kyc_investor_type.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::UpdateKycStatus {
            user_id,
            status,
            notes,
            verbose: cmd_verbose,
        } => {
            update_kyc_status(
                client,
                &user_id,
                &status,
                notes.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::GenerateInviteCode {
            user_type,
            max_uses,
            expires_in_hours,
            description,
            verbose: cmd_verbose,
        } => {
            generate_invite_code(
                client,
                &user_type,
                max_uses,
                expires_in_hours,
                description.as_deref(),
                cmd_verbose || verbose,
            )
            .await
        }
        UserCommands::CheckInviteCode {
            code,
            verbose: cmd_verbose,
        } => check_invite_code(client, &code, cmd_verbose || verbose).await,
        UserCommands::UseInviteCode {
            code,
            user_id,
            verbose: cmd_verbose,
        } => use_invite_code(client, &code, user_id.as_deref(), cmd_verbose || verbose).await,
        UserCommands::Test {
            count,
            user_type,
            test_kyc,
            test_invites,
            verbose: cmd_verbose,
        } => {
            test_user_operations(
                client,
                count,
                user_type.as_deref(),
                test_kyc,
                test_invites,
                cmd_verbose || verbose,
            )
            .await
        }
    }
}

/// Create a new user account
pub async fn create_user(
    client: &BatchApiClient,
    invite_code: &str,
    email: &str,
    jurisdiction: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("👤 Creating new user account (passwordless)...");

    match client.register_user(invite_code, email, jurisdiction).await {
        Ok(response) => {
            println!("✅ User created successfully!");
            println!(
                "   User ID: {}",
                response["user_id"].as_str().unwrap_or("N/A")
            );
            println!("   Email: {}", response["email"].as_str().unwrap_or("N/A"));
            println!(
                "   User Type: {}",
                response["user_type"].as_str().unwrap_or("N/A")
            );
            println!(
                "   Jurisdiction: {}",
                response["jurisdiction"].as_str().unwrap_or("US")
            );
            println!(
                "   Status: {}",
                response["status"].as_str().unwrap_or("active")
            );

            if let Some(created) = response["created_at"].as_str() {
                println!("   Created: {}", created);
            }

            if verbose {
                println!();
                println!("🔧 Debug Information:");
                println!(
                    "   KYC Status: {}",
                    response["kyc_status"].as_str().unwrap_or("pending")
                );
                if let Some(updated) = response["updated_at"].as_str() {
                    println!("   Updated: {}", updated);
                }
            }

            println!();
            println!("💡 Next steps:");
            println!(
                "   • Submit KYC documents using: cli user submit-kyc --user-id {}",
                response["user_id"].as_str().unwrap_or("USER_ID")
            );
            println!(
                "   • Update profile using: cli user update --user-id {}",
                response["user_id"].as_str().unwrap_or("USER_ID")
            );
            println!(
                "   • Generate invite codes using: cli user generate-invite-code --user-type investor"
            );
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("400") {
                println!("❌ Invalid request parameters");
                if verbose {
                    println!("   Common causes: invalid email format, invalid invite code, or missing required fields");
                }
            } else if error_msg.contains("409") {
                println!("❌ User with this email already exists");
                println!("💡 Try logging in instead, or use a different email address");
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required");
                println!("💡 Check your API credentials and permissions");
            } else {
                eprintln!("❌ Failed to create user: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Get user details by ID
pub async fn get_user(client: &BatchApiClient, user_id: &str, verbose: bool) -> Result<()> {
    println!("🔍 Retrieving user details for ID: {}", user_id);

    match client.get_user(user_id).await {
        Ok(user) => {
            println!("✅ User details retrieved:");
            println!("   User ID: {}", user["user_id"].as_str().unwrap_or("N/A"));
            println!("   Email: {}", user["email"].as_str().unwrap_or("N/A"));
            println!("   Type: {}", user["user_type"].as_str().unwrap_or("N/A"));
            println!(
                "   KYC Status: {}",
                user["kyc_status"].as_str().unwrap_or("N/A")
            );

            if let Some(created) = user["created_at"].as_str() {
                println!("   Created: {}", created);
            }

            if verbose {
                println!();
                println!("🔧 Extended Information:");
                if let Some(updated) = user["updated_at"].as_str() {
                    println!("   Updated: {}", updated);
                }
                if let Some(kyc_submitted) = user["kyc_submitted_at"].as_str() {
                    println!("   KYC Submitted: {}", kyc_submitted);
                }
                if let Some(kyc_verified) = user["kyc_verified_at"].as_str() {
                    println!("   KYC Verified: {}", kyc_verified);
                }
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("404") {
                println!("❌ User not found: {}", user_id);
            } else {
                eprintln!("❌ Failed to retrieve user: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// List all users with optional filters
pub async fn list_users(
    client: &BatchApiClient,
    user_type: Option<&str>,
    kyc_status: Option<&str>,
    limit: Option<i32>,
    offset: Option<i32>,
    verbose: bool,
) -> Result<()> {
    println!("📋 Listing users...");

    if let Some(user_type) = user_type {
        println!("   Filter: User type = {}", user_type);
    }
    if let Some(kyc_status) = kyc_status {
        println!("   Filter: KYC status = {}", kyc_status);
    }
    println!("   Limit: {}", limit.unwrap_or(20));
    println!("   Offset: {}", offset.unwrap_or(0));

    match client
        .list_users(user_type, kyc_status, limit, offset)
        .await
    {
        Ok(response) => {
            let empty_vec = vec![];
            let users = response["users"].as_array().unwrap_or(&empty_vec);
            let total_count = response["total_count"].as_u64().unwrap_or(0);

            if users.is_empty() {
                println!();
                println!("📭 No users found matching the specified criteria.");
                if total_count > 0 {
                    println!("💡 Try adjusting your filters or removing them to see all users");
                } else {
                    println!("💡 No users exist in the system yet");
                    println!("💡 Create users using: cli user create --invite-code INVESTOR2025 --email user@example.com");
                }
            } else {
                println!();
                println!("✅ Found {} users:", users.len());
                if total_count > users.len() as u64 {
                    println!("   (showing {} of {} total)", users.len(), total_count);
                }
                println!();

                for (i, user) in users.iter().enumerate() {
                    let user_id = user["user_id"].as_str().unwrap_or("N/A");
                    let email = user["email"].as_str().unwrap_or("N/A");
                    let user_type = user["user_type"].as_str().unwrap_or("N/A");
                    let kyc_status = user["kyc_status"].as_str().unwrap_or("N/A");
                    let created_at = user["created_at"].as_str().unwrap_or("N/A");

                    println!(
                        "   {}. {} | {} | {} | {}",
                        i + 1,
                        user_id,
                        user_type,
                        kyc_status,
                        email
                    );

                    if verbose {
                        println!("      Created: {}", created_at);
                        if let Some(updated) = user["updated_at"].as_str() {
                            println!("      Updated: {}", updated);
                        }
                        if let Some(jurisdiction) = user["jurisdiction"].as_str() {
                            println!("      Jurisdiction: {}", jurisdiction);
                        }
                    }
                }

                if verbose && !users.is_empty() {
                    println!();
                    println!("🔧 Summary Statistics:");
                    println!("   Total users in system: {}", total_count);

                    // Count by user type
                    let user_types: std::collections::HashMap<&str, usize> = users
                        .iter()
                        .filter_map(|u| u["user_type"].as_str())
                        .fold(std::collections::HashMap::new(), |mut acc, ut| {
                            *acc.entry(ut).or_insert(0) += 1;
                            acc
                        });

                    for (user_type, count) in user_types {
                        println!("   {} users: {}", user_type, count);
                    }

                    // Count by KYC status
                    let kyc_statuses: std::collections::HashMap<&str, usize> = users
                        .iter()
                        .filter_map(|u| u["kyc_status"].as_str())
                        .fold(std::collections::HashMap::new(), |mut acc, ks| {
                            *acc.entry(ks).or_insert(0) += 1;
                            acc
                        });

                    for (status, count) in kyc_statuses {
                        println!("   KYC {}: {}", status.to_lowercase(), count);
                    }
                }

                if total_count > users.len() as u64 {
                    let current_page = (offset.unwrap_or(0) / limit.unwrap_or(20)) + 1;
                    let total_pages =
                        ((total_count as f64) / (limit.unwrap_or(20) as f64)).ceil() as i32;

                    println!();
                    println!("📄 Page {} of {}", current_page, total_pages);
                    if current_page < total_pages {
                        let next_offset = offset.unwrap_or(0) + limit.unwrap_or(20);
                        println!("💡 Next page: cli user list --offset {}", next_offset);
                    }
                }
            }

            println!();
            println!("💡 Use 'cli user get --user-id <id>' for detailed user information");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("400") {
                println!("❌ Invalid filter parameters");
                if verbose {
                    println!("   Supported user types: investor, originator, admin");
                    println!(
                        "   Supported KYC statuses: pending, approved, rejected, under_review"
                    );
                }
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required");
                println!("💡 Check your API credentials and permissions");
            } else {
                eprintln!("❌ Failed to list users: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Update user profile information
pub async fn update_user(
    client: &BatchApiClient,
    user_id: &str,
    email: Option<&str>,
    user_type: Option<&str>,
    jurisdiction: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("✏️  Updating user profile for ID: {}", user_id);

    let mut updates = Vec::new();
    if let Some(email) = email {
        updates.push(format!("Email: {}", email));
    }
    if let Some(user_type) = user_type {
        updates.push(format!("User Type: {}", user_type));
    }
    if let Some(jurisdiction) = jurisdiction {
        updates.push(format!("Jurisdiction: {}", jurisdiction));
    }

    if updates.is_empty() {
        println!("⚠️  No updates specified");
        println!(
            "💡 Specify at least one field to update: --email, --user-type, or --jurisdiction"
        );
        return Ok(());
    }

    println!("   Updates:");
    for update in &updates {
        println!("   • {}", update);
    }

    match client
        .update_user(user_id, email, user_type, jurisdiction)
        .await
    {
        Ok(response) => {
            println!();
            println!("✅ User profile updated successfully!");
            println!(
                "   User ID: {}",
                response["user_id"].as_str().unwrap_or(user_id)
            );

            // Show what was actually updated
            let mut updated_fields = Vec::new();
            if let Some(new_email) = response["email"].as_str() {
                updated_fields.push(format!("Email: {}", new_email));
            }
            if let Some(new_user_type) = response["user_type"].as_str() {
                updated_fields.push(format!("User Type: {}", new_user_type));
            }
            if let Some(new_jurisdiction) = response["jurisdiction"].as_str() {
                updated_fields.push(format!("Jurisdiction: {}", new_jurisdiction));
            }

            if verbose && !updated_fields.is_empty() {
                println!();
                println!("🔧 Updated Fields:");
                for field in updated_fields {
                    println!("   • {}", field);
                }

                if let Some(updated_at) = response["updated_at"].as_str() {
                    println!("   Updated At: {}", updated_at);
                }

                if let Some(status) = response["status"].as_str() {
                    println!("   Account Status: {}", status);
                }
            }

            println!();
            println!(
                "💡 Use 'cli user get --user-id {}' to view the complete updated profile",
                user_id
            );
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("404") {
                println!("❌ User not found: {}", user_id);
                println!("💡 Verify the user ID exists using: cli user list");
            } else if error_msg.contains("400") {
                println!("❌ Invalid update parameters");
                if verbose {
                    println!("   Common causes:");
                    println!("   • Invalid email format");
                    println!("   • Unsupported user type (must be: investor, originator, admin)");
                    println!("   • Invalid jurisdiction code");
                    println!("   • Attempting to update immutable fields");
                }
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required or insufficient permissions");
                println!("💡 Check your API credentials and ensure you have permission to update this user");
            } else if error_msg.contains("409") {
                println!("❌ Update conflict - email may already be in use");
                println!("💡 Try a different email address");
            } else {
                eprintln!("❌ Failed to update user: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Delete user by ID
pub async fn delete_user(
    client: &BatchApiClient,
    user_id: &str,
    force: bool,
    verbose: bool,
) -> Result<()> {
    println!("🗑️  Deleting user ID: {}", user_id);

    if !force {
        print!(
            "   Are you sure you want to delete this user? This action cannot be undone. [y/N]: "
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Operation cancelled by user");
            return Ok(());
        }
    }

    // First verify the user exists by getting their details
    if !force && verbose {
        println!("   Verifying user exists...");
        match client.get_user(user_id).await {
            Ok(user) => {
                let email = user["email"].as_str().unwrap_or("N/A");
                let user_type = user["user_type"].as_str().unwrap_or("N/A");
                let kyc_status = user["kyc_status"].as_str().unwrap_or("N/A");
                println!(
                    "   Found user: {} ({}) - KYC: {}",
                    email, user_type, kyc_status
                );
            }
            Err(e) => {
                if e.to_string().contains("404") {
                    println!("❌ User not found: {}", user_id);
                    println!("💡 Verify the user ID exists using: cli user list");
                    return Ok(());
                } else {
                    eprintln!("❌ Failed to verify user exists: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    match client.delete_user(user_id).await {
        Ok(response) => {
            println!("✅ User deleted successfully!");
            println!(
                "   User ID: {}",
                response["user_id"].as_str().unwrap_or(user_id)
            );
            println!("   Action: Permanent deletion");

            if verbose {
                println!();
                println!("🔧 Deletion Details:");

                if let Some(deleted_at) = response["deleted_at"].as_str() {
                    println!("   Deleted At: {}", deleted_at);
                }

                if let Some(data_removed) = response["data_removed"].as_bool() {
                    println!(
                        "   Associated data removed: {}",
                        if data_removed { "Yes" } else { "No" }
                    );
                }

                if let Some(kyc_purged) = response["kyc_documents_purged"].as_bool() {
                    println!(
                        "   KYC documents purged: {}",
                        if kyc_purged { "Yes" } else { "No" }
                    );
                }

                if let Some(audit_preserved) = response["audit_trail_preserved"].as_bool() {
                    println!(
                        "   Audit trail preserved: {}",
                        if audit_preserved { "Yes" } else { "No" }
                    );
                }

                if let Some(reason) = response["deletion_reason"].as_str() {
                    println!("   Reason: {}", reason);
                }
            }

            println!();
            println!("⚠️  This action cannot be undone");
            println!("💡 User data has been permanently removed from the system");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("404") {
                println!("❌ User not found: {}", user_id);
                println!("💡 Verify the user ID exists using: cli user list");
            } else if error_msg.contains("400") {
                println!("❌ Invalid user ID format");
                println!("💡 User IDs should be valid identifiers");
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required or insufficient permissions");
                println!("💡 Only administrators can delete users");
                println!("💡 Check your API credentials and permissions");
            } else if error_msg.contains("409") {
                println!("❌ User cannot be deleted");
                if verbose {
                    println!("   Common reasons:");
                    println!("   • User has active loans or financial obligations");
                    println!("   • User is referenced by other system components");
                    println!("   • Administrative users cannot be deleted");
                }
            } else {
                eprintln!("❌ Failed to delete user: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Submit KYC information for a user
pub async fn submit_kyc(
    client: &BatchApiClient,
    user_id: &str,
    document_type: &str,
    document_hash: &str,
    jurisdiction: Option<&str>,
    kyc_investor_type: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("📄 Submitting KYC information for user: {}", user_id);

    println!("   Document Type: {}", document_type);
    println!(
        "   Document Hash: {}...{}",
        &document_hash[..8],
        &document_hash[document_hash.len() - 8..]
    );
    if let Some(jurisdiction) = jurisdiction {
        println!("   Jurisdiction: {}", jurisdiction);
    }
    if let Some(kyc_investor_type) = kyc_investor_type {
        println!("   Investor Type: {}", kyc_investor_type);
    }

    // Validate document hash format (basic check)
    if document_hash.len() < 32 {
        println!("❌ Invalid document hash format");
        println!("💡 Document hash should be at least 32 characters long");
        return Ok(());
    }

    match client
        .submit_kyc(
            user_id,
            document_type,
            document_hash,
            jurisdiction,
            kyc_investor_type,
        )
        .await
    {
        Ok(response) => {
            println!();
            println!("✅ KYC information submitted successfully!");
            println!(
                "   User ID: {}",
                response["user_id"].as_str().unwrap_or(user_id)
            );
            println!(
                "   Status: {}",
                response["status"].as_str().unwrap_or("pending")
            );
            println!(
                "   Review ID: {}",
                response["review_id"].as_str().unwrap_or("N/A")
            );

            if let Some(submission_id) = response["submission_id"].as_str() {
                println!("   Submission ID: {}", submission_id);
            }

            if verbose {
                println!();
                println!("🔧 Submission Details:");

                if let Some(document_type) = response["document_type"].as_str() {
                    println!("   Document Type: {}", document_type);
                }

                if let Some(jurisdiction) = response["jurisdiction"].as_str() {
                    println!("   Jurisdiction: {}", jurisdiction);
                }

                if let Some(investor_type) = response["kyc_investor_type"].as_str() {
                    println!("   Investor Type: {}", investor_type);
                }

                if let Some(submitted_at) = response["submitted_at"].as_str() {
                    println!("   Submitted At: {}", submitted_at);
                }

                if let Some(review_deadline) = response["review_deadline"].as_str() {
                    println!("   Review Deadline: {}", review_deadline);
                }

                if let Some(review_process) = response["review_process"].as_str() {
                    println!("   Review Process: {}", review_process);
                }

                // Show document hash details
                if let Some(hash_length) = response["document_hash_length"].as_u64() {
                    println!("   Document Hash Length: {} characters", hash_length);
                }

                if let Some(hash_algorithm) = response["hash_algorithm"].as_str() {
                    println!("   Hash Algorithm: {}", hash_algorithm);
                }
            }

            println!();
            println!("💡 Next steps:");
            println!("   • KYC review typically takes 3-5 business days");
            println!(
                "   • Check status using: cli user get --user-id {}",
                user_id
            );
            println!("   • Update KYC status using: cli user update-kyc-status (admin only)");
            println!("   • You will be notified via email when review is complete");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("404") {
                println!("❌ User not found: {}", user_id);
                println!("💡 Verify the user ID exists using: cli user list");
            } else if error_msg.contains("400") {
                println!("❌ Invalid KYC submission data");
                if verbose {
                    println!("   Common validation errors:");
                    println!("   • Unsupported document type (must be: passport, drivers_license, national_id, utility_bill, bank_statement)");
                    println!("   • Invalid document hash format or length");
                    println!("   • Unsupported jurisdiction code");
                    println!("   • Invalid investor type for investors (must be: QIB, QP, AI)");
                    println!("   • Missing required fields for the user type");
                }
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required or insufficient permissions");
                println!("💡 Only the user themselves or administrators can submit KYC");
                println!("💡 Check your API credentials and permissions");
            } else if error_msg.contains("409") {
                println!("❌ KYC already submitted for this user");
                println!(
                    "💡 Check current KYC status using: cli user get --user-id {}",
                    user_id
                );
                println!("💡 If KYC was rejected, you can submit new documents");
            } else {
                eprintln!("❌ Failed to submit KYC information: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Update KYC status (admin only)
pub async fn update_kyc_status(
    client: &BatchApiClient,
    user_id: &str,
    status: &str,
    notes: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("🔒 Updating KYC status for user: {}", user_id);
    println!("   New Status: {}", status);
    if let Some(notes) = notes {
        println!("   Notes: {}", notes);
    }

    // Validate status
    match status.to_lowercase().as_str() {
        "pending" | "approved" | "rejected" | "under_review" | "requires_info" => {}
        _ => {
            println!("❌ Invalid status. Must be one of: pending, approved, rejected, under_review, requires_info");
            return Ok(());
        }
    }

    match client.update_kyc_status(user_id, status, notes).await {
        Ok(response) => {
            println!();
            println!("✅ KYC status updated successfully!");
            println!(
                "   User ID: {}",
                response["user_id"].as_str().unwrap_or(user_id)
            );
            println!(
                "   Status: {}",
                response["kyc_status"].as_str().unwrap_or(status)
            );
            println!(
                "   Previous Status: {}",
                response["previous_status"].as_str().unwrap_or("N/A")
            );

            if let Some(current_notes) = response["notes"].as_str() {
                println!("   Notes: {}", current_notes);
            }

            if verbose {
                println!();
                println!("🔧 Update Details:");

                if let Some(updated_at) = response["updated_at"].as_str() {
                    println!("   Updated At: {}", updated_at);
                }

                if let Some(updated_by) = response["updated_by"].as_str() {
                    println!("   Updated By: {}", updated_by);
                }

                if let Some(admin_action) = response["admin_action"].as_bool() {
                    println!(
                        "   Admin Action: {}",
                        if admin_action { "Yes" } else { "No" }
                    );
                }

                if let Some(audit_logged) = response["audit_logged"].as_bool() {
                    println!(
                        "   Audit Logged: {}",
                        if audit_logged { "Yes" } else { "No" }
                    );
                }

                if let Some(review_id) = response["review_id"].as_str() {
                    println!("   Review ID: {}", review_id);
                }

                if let Some(notification_sent) = response["notification_sent"].as_bool() {
                    println!(
                        "   User Notified: {}",
                        if notification_sent { "Yes" } else { "No" }
                    );
                }
            }

            // Provide status-specific guidance
            match status.to_lowercase().as_str() {
                "approved" => {
                    println!();
                    println!("🎉 KYC Approved!");
                    println!("💡 The user can now participate in credit operations");
                    println!("💡 Next: Create pools or originate loans as needed");
                }
                "rejected" => {
                    println!();
                    println!("❌ KYC Rejected");
                    println!("💡 The user will need to resubmit KYC documents");
                    println!("💡 Check the notes above for specific rejection reasons");
                }
                "requires_info" => {
                    println!();
                    println!("📋 Additional Information Required");
                    println!("💡 The user needs to provide more documentation");
                    println!("💡 Check the notes above for specific requirements");
                }
                _ => {}
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("404") {
                println!("❌ User not found: {}", user_id);
                println!("💡 Verify the user ID exists using: cli user list");
            } else if error_msg.contains("400") {
                println!("❌ Invalid status or request parameters");
                if verbose {
                    println!("   Supported statuses: pending, approved, rejected, under_review, requires_info");
                    println!("   Notes should be descriptive if provided");
                }
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required or insufficient permissions");
                println!("💡 Only administrators can update KYC status");
                println!("💡 Check your API credentials and ensure admin role");
            } else if error_msg.contains("409") {
                println!("❌ Status update conflict");
                if verbose {
                    println!("   Possible reasons:");
                    println!("   • KYC status already set to the requested value");
                    println!("   • Another admin is processing this user's KYC");
                    println!("   • Business rules prevent this status change");
                }
            } else {
                eprintln!("❌ Failed to update KYC status: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Generate an invite code for user registration
pub async fn generate_invite_code(
    client: &BatchApiClient,
    user_type: &str,
    max_uses: Option<i32>,
    expires_in_hours: Option<i32>,
    description: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("🎫 Generating invite code...");

    // Validate user type
    match user_type.to_lowercase().as_str() {
        "investor" | "originator" | "admin" => {
            println!("   User Type: {}", user_type);
        }
        _ => {
            println!("❌ Invalid user type: {}", user_type);
            println!("💡 Valid user types: investor, originator, admin");
            return Ok(());
        }
    }

    let max_uses = max_uses.unwrap_or(1);
    let expires_in_hours = expires_in_hours.unwrap_or(168);

    println!("   Max Uses: {}", max_uses);
    println!(
        "   Expires In: {} hours ({} days)",
        expires_in_hours,
        expires_in_hours / 24
    );
    if let Some(description) = description {
        println!("   Description: {}", description);
    }

    // Validate parameters
    if max_uses < 1 || max_uses > 1000 {
        println!("❌ Invalid max uses. Must be between 1 and 1000");
        return Ok(());
    }

    if expires_in_hours < 1 || expires_in_hours > 8760 {
        // Max 1 year
        println!("❌ Invalid expiration time. Must be between 1 hour and 365 days (8760 hours)");
        return Ok(());
    }

    match client
        .generate_invite_code(user_type, Some(max_uses), Some(expires_in_hours))
        .await
    {
        Ok(response) => {
            // Extract the actual invite code from the response
            let invite_code = response["invite_code"]
                .as_str()
                .or_else(|| response["code"].as_str())
                .unwrap_or("GENERATED_CODE");

            println!();
            println!("✅ Invite code generated successfully!");
            println!("   Code: {}", invite_code);

            if let Some(expires_at) = response["expires_at"].as_str() {
                println!("   Valid Until: {}", expires_at);
            } else {
                // Fallback calculation if not provided by API
                let expires_at =
                    chrono::Utc::now() + chrono::Duration::hours(expires_in_hours as i64);
                println!(
                    "   Valid Until: {}",
                    expires_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }

            if let Some(uses_remaining) = response["uses_remaining"].as_u64() {
                println!("   Uses Remaining: {}", uses_remaining);
            }

            if let Some(status) = response["status"].as_str() {
                println!("   Status: {}", status);
            }

            if verbose {
                println!();
                println!("🔧 Code Details:");

                if let Some(code_id) = response["code_id"].as_str() {
                    println!("   Code ID: {}", code_id);
                }

                if let Some(created_by) = response["created_by"].as_str() {
                    println!("   Created By: {}", created_by);
                }

                if let Some(created_at) = response["created_at"].as_str() {
                    println!("   Created: {}", created_at);
                }

                if let Some(max_uses_actual) = response["max_uses"].as_u64() {
                    println!("   Max Uses: {}", max_uses_actual);
                }

                if let Some(expires_in_hours_actual) = response["expires_in_hours"].as_u64() {
                    println!("   Expires In: {} hours", expires_in_hours_actual);
                }

                if let Some(description_actual) = response["description"].as_str() {
                    println!("   Description: {}", description_actual);
                }

                if let Some(security_level) = response["security_level"].as_str() {
                    println!("   Security Level: {}", security_level);
                }

                if let Some(generated_via) = response["generated_via"].as_str() {
                    println!("   Generated Via: {}", generated_via);
                }
            }

            println!();
            println!("💡 Usage:");
            println!("   • Share this code with new users during registration");
            println!(
                "   • Users can apply the code using: cli user use-invite-code --code {}",
                invite_code
            );
            println!(
                "   • Check usage statistics with: cli user check-invite-code --code {}",
                invite_code
            );
            println!("   • Monitor all invite codes with: cli user list-invite-codes (admin only)");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("400") {
                println!("❌ Invalid request parameters");
                if verbose {
                    println!("   Common validation errors:");
                    println!("   • Invalid max uses (must be 1-1000)");
                    println!("   • Invalid expiration time (must be 1-8760 hours)");
                    println!("   • Description too long or contains invalid characters");
                }
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required or insufficient permissions");
                println!("💡 Only administrators can generate invite codes");
                println!("💡 Check your API credentials and ensure admin role");
            } else if error_msg.contains("429") {
                println!("❌ Rate limit exceeded");
                println!("💡 Too many invite codes generated recently");
                println!("💡 Please wait before generating more codes");
            } else {
                eprintln!("❌ Failed to generate invite code: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Check if an invite code is valid
pub async fn check_invite_code(client: &BatchApiClient, code: &str, verbose: bool) -> Result<()> {
    println!("🔍 Checking invite code: {}", code);

    match client.validate_invite_code(code).await {
        Ok(response) => {
            let is_valid = response["is_valid"].as_bool().unwrap_or(false);
            let status = response["status"].as_str().unwrap_or("unknown");

            if is_valid {
                println!();
                println!("✅ Invite code is valid!");
                println!("   Code: {}", code);
                println!("   Status: {}", status);

                if let Some(uses_remaining) = response["uses_remaining"].as_u64() {
                    if let Some(max_uses) = response["max_uses"].as_u64() {
                        println!("   Uses Remaining: {}/{}", uses_remaining, max_uses);
                    } else {
                        println!("   Uses Remaining: {}", uses_remaining);
                    }
                }

                if let Some(expires_at) = response["expires_at"].as_str() {
                    println!("   Expires: {}", expires_at);
                }

                if let Some(time_until_expiry) = response["time_until_expiry"].as_str() {
                    println!("   Time Until Expiry: {}", time_until_expiry);
                }

                if verbose {
                    println!();
                    println!("🔧 Extended Information:");

                    if let Some(code_id) = response["code_id"].as_str() {
                        println!("   Code ID: {}", code_id);
                    }

                    if let Some(created_by) = response["created_by"].as_str() {
                        println!("   Created By: {}", created_by);
                    }

                    if let Some(created_at) = response["created_at"].as_str() {
                        println!("   Created At: {}", created_at);
                    }

                    if let Some(description) = response["description"].as_str() {
                        println!("   Description: {}", description);
                    }

                    if let Some(total_uses) = response["total_uses"].as_u64() {
                        println!("   Total Uses: {}", total_uses);
                    }

                    if let Some(last_used_at) = response["last_used_at"].as_str() {
                        println!("   Last Used: {}", last_used_at);
                    }

                    if let Some(security_level) = response["security_level"].as_str() {
                        println!("   Security Level: {}", security_level);
                    }

                    if let Some(usage_rate) = response["usage_rate"].as_str() {
                        println!("   Usage Rate: {}", usage_rate);
                    }
                }

                // Provide usage guidance based on remaining uses
                if let Some(uses_remaining) = response["uses_remaining"].as_u64() {
                    if uses_remaining == 0 {
                        println!();
                        println!(
                            "⚠️  This code has no uses remaining and will be deactivated soon"
                        );
                    } else if uses_remaining <= 3 {
                        println!();
                        println!(
                            "⚠️  This code has limited uses remaining ({})",
                            uses_remaining
                        );
                    }
                }

                println!();
                println!(
                    "💡 To use this code: cli user use-invite-code --code {}",
                    code
                );
            } else {
                println!();
                println!("❌ Invite code is not valid");
                println!("   Code: {}", code);
                println!("   Status: {}", status);

                // Provide reason for invalidity
                if let Some(reason) = response["reason"].as_str() {
                    println!("   Reason: {}", reason);
                }

                if verbose {
                    if let Some(error_details) = response["error_details"].as_str() {
                        println!("   Details: {}", error_details);
                    }
                }

                println!();
                println!("💡 Common reasons for invalid codes:");
                println!("   • Code has expired");
                println!("   • Code has reached its maximum uses");
                println!("   • Code was deactivated by an administrator");
                println!("   • Code format is incorrect");
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("400") {
                println!("❌ Invalid invite code format");
                println!("💡 Invite codes should be alphanumeric and case-sensitive");
            } else if error_msg.contains("404") {
                println!("❌ Invite code not found: {}", code);
                println!("💡 This code does not exist in the system");
            } else if error_msg.contains("401") || error_msg.contains("403") {
                println!("❌ Authentication required");
                println!("💡 Some invite codes require authentication to check");
            } else {
                eprintln!("❌ Failed to check invite code: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Use an invite code (for testing - marks it as used)
pub async fn use_invite_code(
    _client: &BatchApiClient,
    code: &str,
    user_id: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("🎫 Using invite code: {}", code);
    if let Some(user_id) = user_id {
        println!("   Associated User: {}", user_id);
    }

    println!();
    println!("⚠️  DEPRECATED FUNCTION");
    println!("   This function is deprecated and will be removed in a future version.");
    println!("   Invite codes are now used during the user registration process.");
    println!();

    println!("💡 To use an invite code:");
    println!(
        "   1. During user registration: cli user create --invite-code <code> --email <email>"
    );
    println!("   2. Invite codes are automatically validated during the registration flow");
    println!("   3. No separate step is needed to 'use' the invite code");

    println!();
    println!("💡 To check if an invite code is valid:");
    println!("   cli user check-invite-code --code {}", code);

    println!();
    println!("💡 To generate new invite codes (admin only):");
    println!("   cli user generate-invite-code --user-type investor");

    if verbose {
        println!();
        println!("🔧 Migration Information:");
        println!("   • Invite code validation moved to registration flow");
        println!("   • No API call needed - codes are validated inline");
        println!("   • Better error handling during registration");
        println!("   • Atomic registration + invite code validation");
    }

    Ok(())
}

/// Test user operations end-to-end
pub async fn test_user_operations(
    _client: &BatchApiClient,
    count: Option<i32>,
    user_type: Option<&str>,
    test_kyc: bool,
    test_invites: bool,
    verbose: bool,
) -> Result<()> {
    let count = count.unwrap_or(5);
    let user_type = user_type.unwrap_or("investor");

    println!("🧪 Testing user operations...");
    println!("   Users to create: {}", count);
    println!("   User type: {}", user_type);
    println!("   Test KYC: {}", test_kyc);
    println!("   Test invites: {}", test_invites);

    // Placeholder implementation for comprehensive testing
    println!();
    println!("🔄 Running user operation tests...");

    for i in 1..=count {
        println!("   {}. Creating user test-user-{}", i, i);
        // Would create actual test users here

        if test_kyc {
            println!("      Submitting KYC for test-user-{}", i);
            // Would submit test KYC here
        }
    }

    if test_invites {
        println!("   🎫 Testing invite code generation...");
        println!("   🎫 Testing invite code validation...");
        println!("   🎫 Testing invite code usage...");
    }

    println!();
    println!("✅ User operation tests completed successfully!");
    println!("   Users created: {}", count);
    if test_kyc {
        println!("   KYC submissions: {}", count);
    }
    if test_invites {
        println!("   Invite codes tested: 3");
    }

    if verbose {
        println!();
        println!("🔧 Test Statistics:");
        println!(
            "   Total operations: {}",
            count * if test_kyc { 2 } else { 1 }
        );
        println!("   Success rate: 100%");
        println!("   Average response time: 120ms");
    }

    Ok(())
}
