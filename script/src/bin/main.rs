//! Local SP1 Loan Processing Testing
//!
//! This program provides a comprehensive test of the SP1 loan processing zkVM program.
//! It demonstrates the complete loan lifecycle: user registration, KYC, loan origination,
//! payments, and closeouts - all while maintaining privacy of individual transaction details.
//!
//! Usage:
//! ```shell
//! # Run locally with Core proofs (fast development)
//! cargo run --package arda-credit --bin arda-credit
//!
//! # Run and submit to Sindri for circuit verification
//! cargo run --package arda-credit --bin arda-credit -- --sindri
//! ```

use alloy_sol_types::SolType;
use arda_credit_lib::{
    hash_uuid, LoanBatch, LoanBatchPublicValues, LoanState, LoanTransaction, MetadataArray,
    TransactionType, UserType, ZkVMAds, ZkVMBatchInput,
};
use eyre::Result;
use futures::future::join_all;
use metrics::{counter, histogram};
use std::env;
use std::time::Instant;
use sindri::{client::SindriClient, ProofInput};
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::{error, info};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// This is built by build.rs from the program/ directory.
pub const LOAN_PROGRAM_ELF: &[u8] = include_elf!("program");

#[derive(Debug)]
struct Args {
    sindri: bool,
    benchmark: bool,
    num_proofs: u32,
    num_transactions: u32,
}

#[allow(clippy::cognitive_complexity)]
#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Setup logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Feature-based configuration
    let num_proofs = env::var("NUM_PROOFS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
        
    let num_transactions = env::var("NUM_TRANSACTIONS")
        .ok()  
        .and_then(|s| s.parse().ok())
        .unwrap_or(9);

    let args = Args {
        sindri: cfg!(feature = "sindri"),
        benchmark: cfg!(feature = "benchmark"),
        num_proofs,
        num_transactions,
    };

    if args.benchmark {
        info!("🚀 Starting ZK Observability benchmark suite");
        info!("📊 Configuration: {} proofs with {} transactions each", args.num_proofs, args.num_transactions);
        return run_benchmark_suite(&args).await;
    }

    if args.sindri {
        info!("🌐 Starting SP1 loan processing test with Sindri integration");
        info!("🔐 Will submit circuit to Sindri for verification");
    } else {
        info!("🏦 Starting local SP1 loan processing test");
        info!("🔐 Testing complete loan lifecycle with privacy preservation");
    }

    // Create test scenario: Complete loan workflow
    let (initial_state, batch, _alice_id, _bob_id) = create_test_loan_scenario();

    info!("📊 Test scenario:");
    info!("  Initial users: {}", initial_state.users.len());
    info!("  Initial loans: {}", initial_state.loans.len());
    info!("  Batch transactions: {}", batch.transactions.len());
    info!(
        "  Transaction types: {:?}",
        batch
            .transactions
            .iter()
            .map(|t| t.transaction_type)
            .collect::<Vec<_>>()
    );

    if args.sindri {
        run_with_sindri(&initial_state, &batch).await?;
    } else {
        run_locally(&initial_state, &batch)?;
    }

    Ok(())
}

/// Run the test locally with Core proofs (fast for development)
fn run_locally(initial_state: &LoanState, batch: &LoanBatch) -> Result<()> {
    info!("🔄 Generating Core proof (fast, for development)...");

    // Create a prover client for local testing
    let client = ProverClient::from_env();

    // Create inputs for the zkVM program
    let mut stdin = SP1Stdin::new();

    // Create the ZkVMBatchInput structure expected by the program
    let zkvm_input = ZkVMBatchInput {
        initial_loan_state: initial_state.clone(),
        initial_ads_state: ZkVMAds::new(), // Initialize empty ADS state
        batch: batch.clone(),
    };

    stdin.write(&zkvm_input);

    // Generate a Core proof (fast for local development)
    let (pk, vk) = client.setup(LOAN_PROGRAM_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .core() // Use Core proof mode for speed
        .run()
        .expect("Failed to generate loan processing proof");

    info!("✅ Core proof generated successfully!");

    // Verify the proof
    info!("🔍 Verifying loan processing proof...");
    client
        .verify(&proof, &vk)
        .expect("Failed to verify loan proof");
    info!("✅ Proof verification passed!");

    analyze_proof_output(&proof)?;

    info!("🎉 Local SP1 loan processing test completed successfully!");
    Ok(())
}

/// Run the test with Sindri integration (no local proving)
async fn run_with_sindri(initial_state: &LoanState, batch: &LoanBatch) -> Result<()> {
    info!("🔄 Submitting loan batch directly to Sindri for proving...");

    // Submit directly to Sindri for proving (no local proof generation)
    use std::env;

    // Get Sindri API key from environment
    let api_key = env::var("SINDRI_API_KEY").map_err(|_| {
        eyre::eyre!(
            "SINDRI_API_KEY environment variable not set. Please set it to submit to Sindri."
        )
    })?;

    info!(
        "🔑 Using Sindri API key from environment (length: {} chars)",
        api_key.len()
    );

    // Read the sindri.json configuration (optional - just for validation)
    if std::path::Path::new("sindri.json").exists() {
        let _sindri_config = std::fs::read_to_string("sindri.json").map_err(|_| {
            eyre::eyre!(
                "Could not read sindri.json. Make sure you're running from the project root."
            )
        })?;
        info!("📁 Read Sindri configuration from sindri.json");
    } else {
        info!("📁 No sindri.json found - using default configuration");
    }

    // Create SP1 inputs for loan processing and serialize for Sindri
    info!("📝 Creating SP1 stdin with ZkVMBatchInput for Sindri proving");
    let mut stdin = SP1Stdin::new();

    // Create the ZkVMBatchInput structure expected by the program
    let zkvm_input = ZkVMBatchInput {
        initial_loan_state: initial_state.clone(),
        initial_ads_state: ZkVMAds::new(), // Initialize empty ADS state
        batch: batch.clone(),
    };

    stdin.write(&zkvm_input);

    info!("📝 Serializing SP1 stdin to JSON for Sindri");
    let stdin_json = serde_json::to_string(&stdin).map_err(|e| {
        error!("❌ Failed to serialize SP1 stdin: {}", e);
        eyre::eyre!("Failed to serialize SP1 stdin: {}", e)
    })?;

    info!(
        "✅ SP1 stdin serialized, JSON length: {} chars",
        stdin_json.len()
    );

    let proof_input = ProofInput::from(stdin_json.clone());
    info!("✅ ProofInput created from JSON");

    // Get circuit name with configurable tag from environment
    let circuit_tag = env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("arda-credit:{circuit_tag}");

    info!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);

    info!("🌐 Creating Sindri client...");
    let client = SindriClient::default();
    info!("✅ Sindri client created");

    info!(
        "🚀 Submitting loan batch to Sindri for proving: {}",
        circuit_name
    );
    info!("🔐 Sindri will generate the zero-knowledge proof (no local proving)");
    info!(
        "📊 Request details: stdin_json preview: {}...",
        &stdin_json[..std::cmp::min(100, stdin_json.len())]
    );

    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, None, None, None)
        .await
        .map_err(|e| {
            error!("❌ Sindri API call failed: {}", e);
            eyre::eyre!("Sindri API error: {}", e)
        })?;

    // Check if the proof submission failed immediately
    let status_str = proof_info.status.to_string().to_lowercase();
    if status_str.contains("failed") || status_str.contains("error") {
        let error_details = if let Some(error) = &proof_info.error {
            format!("Error details: {:?}", error)
        } else {
            "No additional error details available".to_string()
        };

        error!(
            "❌ Sindri proof submission failed with status: {:?}",
            proof_info.status
        );
        error!("📋 {}", error_details);

        return Err(eyre::eyre!(
            "Sindri proof submission failed with status: {:?}. {}. \
             Check the Sindri dashboard for more details or try again.",
            proof_info.status,
            error_details
        ));
    }
    // Also warn if status indicates issues
    if status_str.contains("invalid") || status_str.contains("rejected") {
        let error_details = if let Some(error) = &proof_info.error {
            format!("Error details: {:?}", error)
        } else {
            "No additional error details available".to_string()
        };

        error!(
            "⚠️  Sindri proof submission has issues with status: {:?}",
            proof_info.status
        );
        error!("📋 {}", error_details);

        return Err(eyre::eyre!(
            "Sindri proof submission has issues with status: {:?}. {}. \
             Please check your input data and circuit configuration.",
            proof_info.status,
            error_details
        ));
    }

    info!(
        "✅ Sindri proving request submitted successfully! Proof ID: {}, Status: {:?}",
        proof_info.proof_id, proof_info.status
    );

    info!("🎯 Sindri Proving Results:");
    info!("  - Proof ID: {}", proof_info.proof_id);
    info!("  - Status: {:?}", proof_info.status);
    info!("  - Circuit: {}", circuit_name);
    info!("  - Batch transactions: {}", batch.transactions.len());
    info!("  - Batch timestamp: {}", batch.created_at);

    // Provide helpful next steps based on status
    let status_str = proof_info.status.to_string().to_lowercase();
    if status_str.contains("queued") {
        info!("  - Processing: ✅ Proof queued successfully - Sindri will process it shortly");
    } else if status_str.contains("running") {
        info!("  - Processing: 🔄 Proof is currently running on Sindri");
    } else if status_str.contains("completed") {
        info!("  - Processing: 🎉 Proof completed successfully!");
    } else {
        info!("  - Processing: Status '{}'", proof_info.status);
    }

    let verification_command = format!(
        "sindri proof get {} # Check proof status and download when ready",
        proof_info.proof_id
    );

    info!("📋 Next steps: {}", verification_command);
    info!("🔗 Loan batch successfully submitted to Sindri for zero-knowledge proof generation");
    info!("⏱️  Sindri will handle the proving process - no local computation required!");

    info!("🎉 SP1 loan processing test with Sindri integration completed successfully!");
    Ok(())
}

/// Analyze and display the proof output
fn analyze_proof_output(proof: &sp1_sdk::SP1ProofWithPublicValues) -> Result<()> {
    // Decode and analyze the public outputs
    let public_values = proof.public_values.as_slice();
    let output = LoanBatchPublicValues::abi_decode(public_values)
        .expect("Failed to decode loan batch public values");

    info!("📤 Public outputs (what goes on-chain):");
    info!(
        "  Previous state root: 0x{}",
        hex::encode(output.previous_state_root)
    );
    info!("  New state root: 0x{}", hex::encode(output.new_state_root));
    info!("  Transaction count: {}", output.transaction_count);
    info!("  Batch timestamp: {}", output.batch_timestamp);
    info!(
        "  Transaction types hash: 0x{}",
        hex::encode(output.transaction_types_hash)
    );

    // Verify state transition occurred
    if output.previous_state_root != output.new_state_root {
        info!("✅ State transition verified:");
        let prev_hex = hex::encode(output.previous_state_root);
        let new_hex = hex::encode(output.new_state_root);
        info!(
            "  State successfully changed from 0x{} to 0x{}",
            &prev_hex[..16],
            &new_hex[..16]
        );
        info!(
            "🔐 Individual loan amounts, user identities, and transaction details remain private!"
        );
        info!("🎉 Only state root transitions and transaction count are public");
    } else {
        eyre::bail!("❌ No state transition detected - batch may have been empty or failed");
    }

    Ok(())
}

/// Create a comprehensive test scenario demonstrating the loan lifecycle
fn create_test_loan_scenario() -> (LoanState, LoanBatch, [u8; 32], [u8; 32]) {
    let batch_id = [1u8; 32];
    let batch_timestamp = 1_640_995_200; // January 1, 2022

    // Create test user IDs
    let alice_uuid = uuid::Uuid::new_v4();
    let bob_uuid = uuid::Uuid::new_v4();
    let alice_id = hash_uuid(&alice_uuid);
    let bob_id = hash_uuid(&bob_uuid);

    let loan_uuid = uuid::Uuid::new_v4();
    let loan_id = hash_uuid(&loan_uuid);

    // Start with empty state and fund Alice so she can make loans
    let mut initial_state = LoanState::new();

    // Fund Alice with $100,000 so she can lend $50,000 to Bob
    initial_state.balance_state.balances.insert(
        alice_id,
        arda_credit_lib::UserBalance::new(alice_id, 100_000_00, batch_timestamp), // $100,000.00 in cents
    );

    // Create a batch demonstrating the complete loan workflow
    let mut transactions = Vec::new();

    // 1. Register Alice as a lender
    let mut alice_metadata = MetadataArray::empty();
    alice_metadata.as_mut_bytes()[0] = UserType::Lender as u8;

    transactions.push(LoanTransaction {
        transaction_id: [1u8; 32],
        user_id: alice_id,
        loan_id: [0u8; 32], // No loan for user registration
        transaction_type: TransactionType::UserRegistration,
        amount: 0,
        created_at: batch_timestamp,
        metadata: alice_metadata,
    });

    // 2. Register Bob as an investor
    let mut bob_metadata = MetadataArray::empty();
    bob_metadata.as_mut_bytes()[0] = UserType::Investor as u8;

    transactions.push(LoanTransaction {
        transaction_id: [2u8; 32],
        user_id: bob_id,
        loan_id: [0u8; 32],
        transaction_type: TransactionType::UserRegistration,
        amount: 0,
        created_at: batch_timestamp + 100,
        metadata: bob_metadata,
    });

    // 3. Alice submits KYC
    transactions.push(LoanTransaction {
        transaction_id: [3u8; 32],
        user_id: alice_id,
        loan_id: [0u8; 32],
        transaction_type: TransactionType::KycSubmission,
        amount: 0,
        created_at: batch_timestamp + 200,
        metadata: MetadataArray::empty(),
    });

    // 4. Bob submits KYC
    transactions.push(LoanTransaction {
        transaction_id: [4u8; 32],
        user_id: bob_id,
        loan_id: [0u8; 32],
        transaction_type: TransactionType::KycSubmission,
        amount: 0,
        created_at: batch_timestamp + 300,
        metadata: MetadataArray::empty(),
    });

    // 5. KYC verification for Alice (approved)
    let mut alice_kyc_metadata = MetadataArray::empty();
    alice_kyc_metadata.as_mut_bytes()[0] = 1; // Approved

    transactions.push(LoanTransaction {
        transaction_id: [5u8; 32],
        user_id: alice_id,
        loan_id: [0u8; 32],
        transaction_type: TransactionType::KycVerification,
        amount: 0,
        created_at: batch_timestamp + 400,
        metadata: alice_kyc_metadata,
    });

    // 6. KYC verification for Bob (approved)
    let mut bob_kyc_metadata = MetadataArray::empty();
    bob_kyc_metadata.as_mut_bytes()[0] = 1; // Approved

    transactions.push(LoanTransaction {
        transaction_id: [6u8; 32],
        user_id: bob_id,
        loan_id: [0u8; 32],
        transaction_type: TransactionType::KycVerification,
        amount: 0,
        created_at: batch_timestamp + 500,
        metadata: bob_kyc_metadata,
    });

    // 7. Loan origination: Bob borrows $50,000 from Alice
    let mut loan_metadata = MetadataArray::empty();
    // Pack borrower_id and lender_id into metadata
    loan_metadata.as_mut_bytes()[0..32].copy_from_slice(&bob_id); // borrower
    loan_metadata.as_mut_bytes()[32..64].copy_from_slice(&alice_id); // lender

    transactions.push(LoanTransaction {
        transaction_id: [7u8; 32],
        user_id: alice_id, // Alice initiates the loan
        loan_id,
        transaction_type: TransactionType::LoanOrigination,
        amount: 5_000_000, // $50,000 in cents
        created_at: batch_timestamp + 600,
        metadata: loan_metadata,
    });

    // 8. First loan payment: $2,500
    transactions.push(LoanTransaction {
        transaction_id: [8u8; 32],
        user_id: bob_id,
        loan_id,
        transaction_type: TransactionType::LoanPayment,
        amount: 250_000, // $2,500 in cents
        created_at: batch_timestamp + 700,
        metadata: MetadataArray::empty(),
    });

    // 9. Second loan payment: $2,500
    transactions.push(LoanTransaction {
        transaction_id: [9u8; 32],
        user_id: bob_id,
        loan_id,
        transaction_type: TransactionType::LoanPayment,
        amount: 250_000, // $2,500 in cents
        created_at: batch_timestamp + 800,
        metadata: MetadataArray::empty(),
    });

    let batch = LoanBatch {
        batch_id,
        transactions,
        previous_state_root: initial_state.compute_state_hash(),
        created_at: batch_timestamp,
    };

    (initial_state, batch, alice_id, bob_id)
}

// Essential benchmarking structures and functions

#[derive(Debug, Default)]
struct ThroughputAnalysis {
    concurrent_proofs: u32,
    successful_submissions: u32,
    failed_submissions: u32,
    total_duration_ms: f64,
    avg_queue_time_ms: f64,
    proofs_per_minute: f64,
    transactions_per_proof: u32,
    total_transactions: u32,
    proof_ids: Vec<String>,
}

async fn run_benchmark_suite(args: &Args) -> Result<()> {
    let start_time = Instant::now();
    
    info!("🔧 Creating benchmark scenario with {} transactions per proof", args.num_transactions);
    let (initial_state, batch) = create_benchmark_scenario(args.num_transactions);
    
    if args.sindri {
        info!("🌐 Running Sindri HIGH-THROUGHPUT test");
        let analysis = run_sindri_throughput_test(&initial_state, &batch, args).await?;
        
        info!("🎯 **SINDRI THROUGHPUT RESULTS**");
        info!("   📊 Concurrent Proofs Attempted: {}", analysis.concurrent_proofs);
        info!("   ✅ Successful Submissions: {}", analysis.successful_submissions);
        info!("   ❌ Failed Submissions: {}", analysis.failed_submissions);
        info!("   ⏱️ Total Duration: {:.2}s", analysis.total_duration_ms / 1000.0);
        info!("   🚀 Effective Throughput: {:.2} proofs/minute", analysis.proofs_per_minute);
        info!("   📈 Transaction Throughput: {:.2} txs/minute", 
              analysis.proofs_per_minute * analysis.transactions_per_proof as f64);
        info!("   🔢 Total Transactions Processed: {}", analysis.total_transactions);
        
        counter!("arda_credit.benchmark.sindri.total_proofs", analysis.concurrent_proofs as u64);
        counter!("arda_credit.benchmark.sindri.total_transactions", analysis.total_transactions as u64);
        histogram!("arda_credit.benchmark.sindri.throughput_proofs_per_minute", analysis.proofs_per_minute);
    } else {
        info!("🏠 Running local benchmark test");
        let client = ProverClient::from_env();
        let mut stdin = SP1Stdin::new();
        
        let zkvm_input = ZkVMBatchInput {
            initial_loan_state: initial_state,
            initial_ads_state: ZkVMAds::new(),
            batch,
        };
        stdin.write(&zkvm_input);
        
        let (_public_values, _execution_report) = client.execute(LOAN_PROGRAM_ELF, stdin)?;
        info!("✅ Local benchmark completed successfully");
    }
    
    let total_duration = start_time.elapsed();
    info!("✅ Benchmark suite completed in {:.2}s", total_duration.as_secs_f64());
    
    Ok(())
}

async fn run_sindri_throughput_test(initial_state: &LoanState, batch: &LoanBatch, args: &Args) -> Result<ThroughputAnalysis> {
    let start_time = Instant::now();
    
    info!("🚀 Submitting {} concurrent proofs to test Sindri capacity", args.num_proofs);
    
    let mut tasks = Vec::new();
    
    for i in 0..args.num_proofs {
        let initial_state_clone = initial_state.clone();
        let mut batch_clone = batch.clone();
        
        // Scale batch to requested transaction count with timestamp bounds checking
        if args.num_transactions != batch_clone.transactions.len() as u32 {
            scale_batch_transactions(&mut batch_clone, args.num_transactions, i);
        }
        
        let task = tokio::spawn(async move {
            submit_sindri_proof(initial_state_clone, batch_clone, i).await
        });
        
        tasks.push(task);
    }
    
    info!("⏳ Waiting for all {} proofs to complete...", args.num_proofs);
    let results = join_all(tasks).await;
    
    let mut successful_proofs = 0;
    let mut failed_proofs = 0;
    let mut proof_ids = Vec::new();
    
    for result in results {
        match result {
            Ok(Ok(proof_id)) => {
                successful_proofs += 1;
                proof_ids.push(proof_id);
            }
            _ => failed_proofs += 1,
        }
    }
    
    let total_duration = start_time.elapsed();
    let proofs_per_minute = (successful_proofs as f64 / total_duration.as_secs_f64()) * 60.0;
    
    Ok(ThroughputAnalysis {
        concurrent_proofs: args.num_proofs,
        successful_submissions: successful_proofs,
        failed_submissions: failed_proofs,
        total_duration_ms: total_duration.as_millis() as f64,
        avg_queue_time_ms: total_duration.as_millis() as f64 / successful_proofs.max(1) as f64,
        proofs_per_minute,
        transactions_per_proof: args.num_transactions,
        total_transactions: successful_proofs * args.num_transactions,
        proof_ids,
    })
}

async fn submit_sindri_proof(initial_state: LoanState, batch: LoanBatch, _proof_index: u32) -> Result<String> {
    let mut stdin = SP1Stdin::new();
    let zkvm_input = ZkVMBatchInput {
        initial_loan_state: initial_state,
        initial_ads_state: ZkVMAds::new(),
        batch,
    };
    stdin.write(&zkvm_input);
    
    info!("🔑 Submitting proof with {} transactions", zkvm_input.batch.transactions.len());
    
    let client = SindriClient::default();
    let stdin_json = serde_json::to_string(&stdin)?;
    let proof_input = ProofInput::from(stdin_json);
    
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or("latest".to_string());
    let circuit_name = format!("arda-credit:{circuit_tag}");
    
    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, Some(true), None, None)
        .await
        .map_err(|e| eyre::eyre!("Sindri API call failed: {}", e))?;
    
    // Log cycle counts if available
    if let Some(ref sp1_info) = proof_info.sp1_info {
        if let Some(cycle_count) = sp1_info.cycle_count {
            info!("🔢 Cycle Count: {} cycles", cycle_count);
        }
    }
    
    Ok(proof_info.proof_id)
}

fn create_benchmark_scenario(num_transactions: u32) -> (LoanState, LoanBatch) {
    let initial_state = LoanState::new();
    let batch_timestamp = 1_700_000_000u64;
    let batch_id = [1u8; 32];
    
    // Start with basic valid transactions
    let mut transactions = vec![
        // User registration
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: batch_timestamp + 100,
            metadata: MetadataArray::empty(),
        },
        // Loan origination  
        LoanTransaction {
            transaction_id: [2u8; 32],
            user_id: [1u8; 32],
            loan_id: [1u8; 32],
            transaction_type: TransactionType::LoanOrigination,
            amount: 100_000,
            created_at: batch_timestamp + 200,
            metadata: MetadataArray::empty(),
        },
    ];
    
    // Add payment transactions up to requested count with timestamp bounds checking
    let base_timestamp = batch_timestamp + 300;
    const MAX_VALID_TIMESTAMP: u64 = 1893456000; // Maximum valid timestamp
    
    for i in 2..num_transactions {
        let increment = (i - 2) as u64 + 1;
        let timestamp = base_timestamp + increment;
        
        // Ensure we don't exceed maximum valid timestamp
        let safe_timestamp = if timestamp > MAX_VALID_TIMESTAMP {
            MAX_VALID_TIMESTAMP - (increment % 1000)
        } else {
            timestamp
        };
        
        transactions.push(LoanTransaction {
            transaction_id: [i as u8; 32],
            user_id: [1u8; 32],
            loan_id: [1u8; 32],
            transaction_type: TransactionType::LoanPayment,
            amount: (i as u64 % 10) + 1, // Small payments: $0.01 to $0.10
            created_at: safe_timestamp,
            metadata: MetadataArray::empty(),
        });
    }
    
    let batch = LoanBatch {
        batch_id,
        transactions,
        previous_state_root: initial_state.compute_state_hash(),
        created_at: batch_timestamp,
    };
    
    (initial_state, batch)
}

fn scale_batch_transactions(batch: &mut LoanBatch, target_count: u32, proof_index: u32) {
    let current_count = batch.transactions.len();
    
    if target_count as usize <= current_count {
        batch.transactions.truncate(target_count as usize);
        return;
    }
    
    // Find valid payment transactions to duplicate
    let payment_txs: Vec<LoanTransaction> = batch.transactions
        .iter()
        .filter(|tx| tx.transaction_type == TransactionType::LoanPayment)
        .cloned()
        .collect();
    
    if payment_txs.is_empty() {
        return; // Can't scale without valid payments
    }
    
    // Add more payment transactions with unique IDs and safe timestamps
    for i in current_count..(target_count as usize) {
        let base_payment = &payment_txs[i % payment_txs.len()];
        let mut new_payment = base_payment.clone();
        
        // Make transaction ID unique using proof index and transaction index
        new_payment.transaction_id[0] = (i % 256) as u8;
        new_payment.transaction_id[1] = ((i / 256) % 256) as u8;
        new_payment.transaction_id[2] = (proof_index % 256) as u8;
        new_payment.transaction_id[3] = ((proof_index / 256) % 256) as u8;
        
        // Small payment amounts to avoid balance issues
        new_payment.amount = ((i % 10) + 1) as u64;
        
        // Safe timestamp calculation with bounds checking
        let increment = (i - current_count + 1) as u64;
        let max_existing_timestamp = batch.transactions
            .iter()
            .map(|tx| tx.created_at)
            .max()
            .unwrap_or(base_payment.created_at);
        
        const MAX_VALID_TIMESTAMP: u64 = 1893456000;
        let next_timestamp = max_existing_timestamp + increment;
        
        new_payment.created_at = if next_timestamp > MAX_VALID_TIMESTAMP {
            MAX_VALID_TIMESTAMP - (increment % 1000)
        } else {
            next_timestamp
        };
        
        batch.transactions.push(new_payment);
    }
}
