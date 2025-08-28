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
use clap::Parser;
use eyre::Result;
use sindri::{client::SindriClient, ProofInput};
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::{error, info};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// This is built by build.rs from the program/ directory.
pub const LOAN_PROGRAM_ELF: &[u8] = include_elf!("program");

#[derive(Parser, Debug)]
#[command(name = "arda-credit")]
#[command(about = "SP1 Loan Processing Test with optional Sindri integration")]
struct Args {
    /// Submit the circuit to Sindri for verification
    #[arg(long)]
    sindri: bool,
}

#[allow(clippy::cognitive_complexity)]
#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Setup logging
    tracing_subscriber::fmt().with_env_filter("info").init();

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
    submit_to_sindri_for_proving(initial_state, batch).await?;

    info!("🎉 SP1 loan processing test with Sindri integration completed successfully!");
    Ok(())
}

/// Submit loan batch directly to Sindri for proving (no local proof generation)
async fn submit_to_sindri_for_proving(initial_state: &LoanState, batch: &LoanBatch) -> Result<()> {
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
    let circuit_name = format!("arda-credit:{}", circuit_tag);

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
    info!("  - Processing: Sindri is now generating the zero-knowledge proof");

    let verification_command = format!(
        "sindri proof get {} # Check proof status and download when ready",
        proof_info.proof_id
    );

    info!("📋 Next steps: {}", verification_command);
    info!("🔗 Loan batch successfully submitted to Sindri for zero-knowledge proof generation");
    info!("⏱️  Sindri will handle the proving process - no local computation required!");

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
        hex::encode(&output.previous_state_root)
    );
    info!(
        "  New state root: 0x{}",
        hex::encode(&output.new_state_root)
    );
    info!("  Transaction count: {}", output.transaction_count);
    info!("  Batch timestamp: {}", output.batch_timestamp);
    info!(
        "  Transaction types hash: 0x{}",
        hex::encode(&output.transaction_types_hash)
    );

    // Verify state transition occurred
    if output.previous_state_root != output.new_state_root {
        info!("✅ State transition verified:");
        let prev_hex = hex::encode(&output.previous_state_root);
        let new_hex = hex::encode(&output.new_state_root);
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
    let batch_timestamp = 1640995200; // January 1, 2022

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
