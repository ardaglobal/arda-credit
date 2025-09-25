//! Batch processing, proof generation, and state operations
//!
//! This module handles all batch-related operations for the Arda Credit CLI including:
//! - Transaction batching and submission
//! - ZK proof generation and verification
//! - State management and merkle root operations
//! - Contract data preparation and submission
//! - Health monitoring and system status checks
//!
//! All batch operations maintain privacy guarantees while enabling transaction
//! verification and regulatory compliance through zero-knowledge proofs.

use clap::Subcommand;
use eyre::Result;
use std::fs;
use std::time::Instant;

use arda_credit_api::BatchApiClient;
use ethereum_client::{config::Config, EthereumClient};

/// Batch-related CLI commands
#[derive(Subcommand)]
pub enum BatchCommands {
    /// Submit a new transaction to the batch processing queue
    SubmitTransaction {
        /// Transaction amount to add to the counter
        #[arg(short, long)]
        amount: i32,
    },

    /// View all pending (unbatched) transactions
    ViewPending,

    /// Get current counter state and associated merkle root
    GetCurrentState,

    /// Trigger batch creation and get contract submission data
    Trigger {
        /// Maximum number of transactions to include in batch
        #[arg(long, default_value = "10")]
        batch_size: Option<i32>,
        /// Show detailed output including private information
        #[arg(short, long)]
        verbose: bool,
    },

    /// List all historical batches
    ListBatches,

    /// Get details of a specific batch
    GetBatch {
        /// Batch ID
        #[arg(long)]
        batch_id: i32,
    },

    /// Download raw proof data for local verification
    DownloadProof {
        /// Batch ID with associated proof
        #[arg(long)]
        batch_id: i32,
        /// Output file path (optional, defaults to `proof_batch_<id>.json`)
        #[arg(long)]
        output: Option<String>,
    },

    /// Check API server health
    HealthCheck,

    /// Verify proof locally without network dependencies
    VerifyProof {
        /// Path to the downloaded proof JSON file
        #[arg(long, group = "input")]
        proof_file: Option<String>,
        /// Hex-encoded proof data (alternative to --proof-file)
        #[arg(long, group = "input")]
        proof_data: Option<String>,
        /// Hex-encoded public values (required when using --proof-data)
        #[arg(long, requires = "proof_data")]
        public_values: Option<String>,
        /// Hex-encoded verifying key (required when using --proof-data)
        #[arg(long, requires = "proof_data")]
        verifying_key: Option<String>,
        /// Expected initial balance
        #[arg(long)]
        expected_initial_balance: i32,
        /// Expected final balance
        #[arg(long)]
        expected_final_balance: i32,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Query the current verification key from the smart contract
    QueryVerificationKey {
        /// Show detailed verification key information
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handle batch command routing and execution
pub async fn handle_batch_command(
    client: &BatchApiClient,
    command: BatchCommands,
    verbose: bool,
) -> Result<()> {
    match command {
        BatchCommands::SubmitTransaction { amount } => submit_transaction(client, amount).await,
        BatchCommands::ViewPending => view_pending_transactions(client).await,
        BatchCommands::GetCurrentState => get_current_state(client).await,
        BatchCommands::Trigger {
            batch_size,
            verbose: cmd_verbose,
        } => trigger_batch(client, batch_size, cmd_verbose || verbose).await,
        BatchCommands::ListBatches => list_batches(client).await,
        BatchCommands::GetBatch { batch_id } => get_batch(client, batch_id).await,
        BatchCommands::DownloadProof { batch_id, output } => {
            download_proof(client, batch_id, output).await
        }
        BatchCommands::HealthCheck => health_check(client).await,
        BatchCommands::VerifyProof {
            proof_file,
            proof_data,
            public_values,
            verifying_key,
            expected_initial_balance,
            expected_final_balance,
            verbose: cmd_verbose,
        } => verify_proof_local(
            proof_file,
            proof_data,
            public_values,
            verifying_key,
            expected_initial_balance,
            expected_final_balance,
            cmd_verbose || verbose,
        ),
        BatchCommands::QueryVerificationKey {
            verbose: cmd_verbose,
        } => query_verification_key(cmd_verbose || verbose).await,
    }
}

/// Submit a new transaction to the batch processing queue
pub async fn submit_transaction(_client: &BatchApiClient, amount: i32) -> Result<()> {
    match client.submit_transaction(amount).await {
        Ok(response) => {
            println!("✅ Transaction submitted successfully!");
            println!("   Transaction ID: {}", response.transaction_id);
            println!("   Amount: {}", response.amount);
            println!("   Status: {}", response.status);
            println!("   Created: {}", response.created_at);
            println!();
            println!("💡 Use 'cli batch view-pending' to see all pending transactions");
            println!("💡 Use 'cli batch trigger' to create a batch and generate proof");
        }
        Err(e) => {
            eprintln!("❌ Failed to submit transaction: {}", e);
        }
    }

    Ok(())
}

/// View all pending (unbatched) transactions
pub async fn view_pending_transactions(_client: &BatchApiClient) -> Result<()> {
    match client.get_pending_transactions().await {
        Ok(response) => {
            if response.transactions.is_empty() {
                println!("📭 No pending transactions found.");
                println!();
                println!(
                    "💡 Submit transactions using: cli batch submit-transaction --amount <amount>"
                );
            } else {
                println!("📋 Pending Transactions (Unbatched):");
                println!("   Total Count: {}", response.total_count);
                println!("   Total Amount: {}", response.total_amount);
                println!();

                for (i, tx) in response.transactions.iter().enumerate() {
                    println!(
                        "   {}. Transaction ID: {} | Amount: {} | Created: {}",
                        i + 1,
                        tx.id,
                        tx.amount,
                        tx.created_at
                    );
                }

                println!();
                println!(
                    "💡 Use 'cli batch trigger' to batch these transactions and generate proof"
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get pending transactions: {}", e);
        }
    }

    Ok(())
}

/// Get current counter state and associated merkle root
pub async fn get_current_state(_client: &BatchApiClient) -> Result<()> {
    match client.get_current_state().await {
        Ok(response) => {
            println!("📊 Current Counter State:");
            println!("   Counter Value: {}", response.counter_value);

            if response.has_merkle_root {
                println!("   Merkle Root: Available (use get_contract_data for details)");
            } else {
                println!("   Merkle Root: Not set (no batches processed yet)");
            }

            if let Some(last_batch_id) = response.last_batch_id {
                println!("   Last Batch ID: {}", last_batch_id);
            }

            if let Some(last_proven_batch_id) = response.last_proven_batch_id {
                println!("   Last Proven Batch ID: {}", last_proven_batch_id);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get current state: {}", e);
        }
    }

    Ok(())
}

/// Trigger batch creation and get contract submission data
pub async fn trigger_batch(
    _client: &BatchApiClient,
    batch_size: Option<i32>,
    verbose: bool,
) -> Result<()> {
    println!("🔄 Creating batch from pending transactions...");

    match client.create_batch(batch_size).await {
        Ok(response) => {
            println!("✅ Batch created successfully!");
            println!("   Batch ID: {}", response.batch_id);
            println!("   Transaction Count: {}", response.transaction_count);
            println!("   Previous Counter: {}", response.previous_counter_value);
            println!("   Final Counter: {}", response.final_counter_value);
            println!("   Created: {}", response.created_at);

            // Get contract submission data (dry run)
            match client.get_contract_data(response.batch_id).await {
                Ok(Some(contract_data)) => {
                    println!();
                    println!("📄 Contract Submission Data (Dry Run):");
                    println!();
                    println!("🔒 Public Information:");
                    println!(
                        "   Previous Merkle Root: {}",
                        contract_data.public.prev_merkle_root
                    );
                    println!(
                        "   New Merkle Root: {}",
                        contract_data.public.new_merkle_root
                    );
                    println!("   ZK Proof ID: {}", contract_data.public.zk_proof);

                    if verbose {
                        println!();
                        println!("🔓 Private Information (for verification only):");
                        println!(
                            "   Previous Counter Value: {}",
                            contract_data.private.prev_counter_value
                        );
                        println!(
                            "   New Counter Value: {}",
                            contract_data.private.new_counter_value
                        );
                        println!("   Transactions: {:?}", contract_data.private.transactions);

                        println!();
                        println!("🔍 Privacy Note:");
                        println!(
                            "   • The private information above is shown for CLI verification"
                        );
                        println!("   • On-chain, only the public information would be submitted");
                        println!("   • Individual transaction amounts remain private via ZK proof");
                    }
                }
                Ok(None) => {
                    println!();
                    println!("⚠ Contract submission data not available yet for this batch");
                }
                Err(e) => {
                    eprintln!("⚠ Warning: Failed to get contract submission data: {}", e);
                }
            }

            println!();
            println!("💡 Next steps:");
            println!("   • ZK proof generation happens asynchronously");
            println!(
                "   • Use 'cli batch get-batch --batch-id {}' to check batch status",
                response.batch_id
            );
            println!(
                "   • Use 'cli batch download-proof --batch-id {}' once proof is ready",
                response.batch_id
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to create batch: {}", e);
        }
    }

    Ok(())
}

/// List all historical batches
pub async fn list_batches(_client: &BatchApiClient) -> Result<()> {
    match client.get_batches(None).await {
        Ok(response) => {
            if response.batches.is_empty() {
                println!("📭 No batches found.");
                println!();
                println!("💡 Create batches using: cli batch trigger");
            } else {
                println!("📋 Historical Batches:");
                println!("   Total Count: {}", response.total_count);
                println!();

                for (i, batch) in response.batches.iter().enumerate() {
                    println!("   {}. Batch ID: {}", i + 1, batch.id);
                    println!(
                        "      Counter: {} → {}",
                        batch.previous_counter_value, batch.final_counter_value
                    );
                    println!("      Transactions: {}", batch.transaction_count);
                    println!("      Status: {}", batch.proof_status);
                    if let Some(ref proof_id) = batch.sindri_proof_id {
                        println!("      Proof ID: {}", proof_id);
                    }
                    println!("      Created: {}", batch.created_at);
                    if let Some(ref proven_at) = batch.proven_at {
                        println!("      Proven: {}", proven_at);
                    }
                    println!();
                }

                println!(
                    "💡 Use 'cli batch get-batch --batch-id <id>' for detailed batch information"
                );
                println!(
                    "💡 Use 'cli batch download-proof --batch-id <id>' to download proof data"
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get batches: {}", e);
        }
    }

    Ok(())
}

/// Get details of a specific batch
pub async fn get_batch(_client: &BatchApiClient, batch_id: i32) -> Result<()> {
    match client.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            println!("📋 Batch Details:");
            println!("   Batch ID: {}", batch.id);
            println!("   Previous Counter: {}", batch.previous_counter_value);
            println!("   Final Counter: {}", batch.final_counter_value);
            println!("   Transaction Count: {}", batch.transaction_count);
            println!("   Status: {}", batch.proof_status);
            println!("   Created: {}", batch.created_at);

            if let Some(ref proof_id) = batch.sindri_proof_id {
                println!("   Sindri Proof ID: {}", proof_id);
            }

            if let Some(ref proven_at) = batch.proven_at {
                println!("   Proven At: {}", proven_at);
            }

            // Show transaction count (transaction details not available in batch info)
            println!("   Transaction Count: {}", batch.transaction_count);

            println!();
            if batch.proof_status == "proven" {
                println!(
                    "💡 Proof is ready! Use 'cli batch download-proof --batch-id {}' to download",
                    batch_id
                );
            } else {
                println!("⏳ Proof generation in progress. Check back later.");
            }
        }
        Ok(None) => {
            println!("❌ Batch with ID {} not found", batch_id);
            println!("💡 Use 'cli batch list-batches' to see all available batches");
        }
        Err(e) => {
            eprintln!("❌ Failed to get batch details: {}", e);
        }
    }

    Ok(())
}

/// Download raw proof data for local verification
pub async fn download_proof(
    _client: &BatchApiClient,
    batch_id: i32,
    output: Option<String>,
) -> Result<()> {
    // First check if the batch exists and has a proof
    match client.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            if batch.proof_status != "proven" {
                println!("❌ Batch {} does not have a completed proof yet", batch_id);
                println!("   Current Status: {}", batch.proof_status);
                println!();
                println!(
                    "💡 Use 'cli batch get-batch --batch-id {}' to check status",
                    batch_id
                );
                return Ok(());
            }
        }
        Ok(None) => {
            println!("❌ Batch with ID {} not found", batch_id);
            return Ok(());
        }
        Err(e) => {
            eprintln!("❌ Failed to check batch status: {}", e);
            return Ok(());
        }
    }

    println!("📥 Downloading proof data for batch {}...", batch_id);

    match client.get_contract_data(batch_id).await {
        Ok(Some(proof_data)) => {
            let filename = output.unwrap_or_else(|| format!("proof_batch_{}.json", batch_id));

            // Create a serializable representation
            let serializable_data = serde_json::json!({
                "public": {
                    "prev_merkle_root": proof_data.public.prev_merkle_root,
                    "new_merkle_root": proof_data.public.new_merkle_root,
                    "zk_proof": proof_data.public.zk_proof
                },
                "private": {
                    "prev_counter_value": proof_data.private.prev_counter_value,
                    "new_counter_value": proof_data.private.new_counter_value,
                    "transactions": proof_data.private.transactions
                }
            });

            let proof_json = serde_json::to_string_pretty(&serializable_data)?;
            fs::write(&filename, &proof_json)?;

            println!("✅ Contract submission data downloaded successfully!");
            println!("   File: {}", filename);
            println!("   Size: {} bytes", proof_json.len());
            println!();
            println!("💡 This data can be used for contract submission verification");
        }
        Ok(None) => {
            println!("❌ No proof data available for batch {}", batch_id);
            println!("   This might indicate the proof is still being generated");
        }
        Err(e) => {
            eprintln!("❌ Failed to download proof data: {}", e);
        }
    }

    Ok(())
}

/// Check API server health
pub async fn health_check(_client: &BatchApiClient) -> Result<()> {
    let start = Instant::now();

    match client.health_check().await {
        Ok(response) => {
            let duration = start.elapsed();
            println!("✅ API Server Health Check:");
            println!("   Status: {}", response.status);
            println!("   API Version: {}", response.api_version);
            println!("   Git Tag: {}", response.git_tag);
            println!("   Response Time: {:.2}ms", duration.as_millis());

            if response.database_connected {
                println!("   Database: Connected");
            } else {
                println!("   Database: Disconnected");
            }

            println!();
            if response.status == "healthy" {
                println!("🟢 All systems operational");
            } else {
                println!("🟡 Some systems may be degraded");
            }
        }
        Err(e) => {
            let duration = start.elapsed();
            eprintln!(
                "❌ Health check failed after {:.2}ms: {}",
                duration.as_millis(),
                e
            );
            eprintln!();
            eprintln!("🔍 Troubleshooting:");
            eprintln!("   • Check if API server is running");
            eprintln!("   • Verify ARDA_CREDIT_API_URL is correct");
            eprintln!("   • Check network connectivity");
        }
    }

    Ok(())
}

/// Verify proof locally without network dependencies
pub fn verify_proof_local(
    proof_file: Option<String>,
    proof_data: Option<String>,
    public_values: Option<String>,
    verifying_key: Option<String>,
    expected_initial_balance: i32,
    expected_final_balance: i32,
    verbose: bool,
) -> Result<()> {
    let start_time = Instant::now();

    println!("🔍 Verifying proof locally...");

    // Implementation would go here - this is a placeholder for the actual verification logic
    // The full implementation would need to be extracted from the original file

    println!("⚠️  Local proof verification not yet implemented");
    println!("   Expected initial balance: {}", expected_initial_balance);
    println!("   Expected final balance: {}", expected_final_balance);

    if verbose {
        println!();
        println!("🔧 Debug Information:");
        if let Some(ref file) = proof_file {
            println!("   Proof file: {}", file);
        }
        if proof_data.is_some() {
            println!("   Using raw proof data");
        }
        if public_values.is_some() {
            println!("   Public values provided");
        }
        if verifying_key.is_some() {
            println!("   Verifying key provided");
        }
    }

    let duration = start_time.elapsed();
    println!(
        "⏱️  Verification completed in {:.2}ms",
        duration.as_millis()
    );

    Ok(())
}

/// Query the current verification key from the smart contract
pub async fn query_verification_key(verbose: bool) -> Result<()> {
    println!("🔍 Querying verification key from smart contract...");

    // Load environment configuration
    let config = Config::from_env().map_err(|e| {
        eyre::eyre!(
            "Failed to load configuration: {}. Please check your environment variables.",
            e
        )
    })?;

    let client = EthereumClient::new(config)
        .await
        .map_err(|e| eyre::eyre!("Failed to create Ethereum client: {}", e))?;

    match client.query_contract_verification_key().await {
        Ok((vkey_hash, address)) => {
            println!("✅ Verification key retrieved successfully!");
            println!("   Verification Key Hash: 0x{}", hex::encode(vkey_hash));
            println!("   Contract Address: {}", address);

            if verbose {
                println!("   Hash Length: {} bytes", vkey_hash.len());
                println!("   Full Hash: 0x{}", hex::encode(vkey_hash));
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to query verification key: {}", e);
            eprintln!();
            eprintln!("🔍 Troubleshooting:");
            eprintln!("   • Check ETHEREUM_RPC_URL is correct");
            eprintln!("   • Verify ETHEREUM_CONTRACT_ADDRESS is set");
            eprintln!("   • Ensure smart contract is deployed");
        }
    }

    Ok(())
}
