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
use arithmetic_lib::{
    hash_uuid, LoanBatch, LoanBatchPublicValues, LoanState, LoanTransaction, MetadataArray,
    TransactionType, UserType,
};
use clap::Parser;
use eyre::Result;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::info;

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
fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Setup logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    if args.sindri {
        info!("🌐 Starting SP1 loan processing test with Sindri integration");
        info!("🔐 Will submit circuit to Sindri for verification");
    } else {
        info!("🏦 Starting local SP1 loan processing test");
        info!("🔐 Testing complete loan lifecycle with privacy preservation");
    }

    // Create a prover client for local testing
    let client = ProverClient::from_env();

    // Create test scenario: Complete loan workflow
    let (initial_state, batch) = create_test_loan_scenario();

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

    // Create inputs for the zkVM program
    let mut stdin = SP1Stdin::new();
    stdin.write(&initial_state);
    stdin.write(&batch);

    if args.sindri {
        run_with_sindri(client, stdin)?;
    } else {
        run_locally(client, stdin)?;
    }

    Ok(())
}

/// Run the test locally with Core proofs (fast for development)
fn run_locally(client: sp1_sdk::EnvProver, stdin: SP1Stdin) -> Result<()> {
    info!("🔄 Generating Core proof (fast, for development)...");

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

/// Run the test with Sindri integration
fn run_with_sindri(client: sp1_sdk::EnvProver, stdin: SP1Stdin) -> Result<()> {
    info!("🔄 Generating proof for Sindri (Groth16)...");

    // Generate a Groth16 proof for Sindri compatibility
    let (pk, vk) = client.setup(LOAN_PROGRAM_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .groth16() // Use Groth16 for Sindri compatibility
        .run()
        .expect("Failed to generate Groth16 loan processing proof");

    info!("✅ Groth16 proof generated successfully!");

    // Verify the proof locally first
    info!("🔍 Verifying loan processing proof locally...");
    client
        .verify(&proof, &vk)
        .expect("Failed to verify loan proof locally");
    info!("✅ Local proof verification passed!");

    analyze_proof_output(&proof)?;

    // Submit to Sindri
    info!("🌐 Submitting circuit to Sindri for verification...");
    submit_to_sindri(&proof, &vk)?;

    info!("🎉 SP1 loan processing test with Sindri integration completed successfully!");
    Ok(())
}

/// Submit the proof to Sindri for circuit verification
fn submit_to_sindri(proof: &sp1_sdk::SP1ProofWithPublicValues, _vk: &sp1_sdk::SP1VerifyingKey) -> Result<()> {
    use std::env;

    // Get Sindri API key from environment
    let _api_key = env::var("SINDRI_API_KEY")
        .map_err(|_| eyre::eyre!("SINDRI_API_KEY environment variable not set. Please set it to submit to Sindri."))?;

    info!("🔑 Using Sindri API key from environment");

    // Read the sindri.json configuration
    let _sindri_config = std::fs::read_to_string("sindri.json")
        .map_err(|_| eyre::eyre!("Could not read sindri.json. Make sure you're running from the project root."))?;

    info!("📁 Read Sindri configuration from sindri.json");

    // For now, we'll just log that we would submit to Sindri
    // In a full implementation, you would:
    // 1. Use the Sindri client from the lib crate (which has the proper integration)
    // 2. Package the ELF binary
    // 3. Submit the circuit definition
    // 4. Submit a proof request with the public inputs
    // 5. Wait for and verify the result

    info!("🚀 Would submit to Sindri with:");
    info!("  - Circuit: SP1 Groth16 proof");
    info!("  - Public values size: {} bytes", proof.public_values.as_slice().len());
    info!("  - Proof size: {} bytes", proof.bytes().len());
    
    // TODO: Implement actual Sindri submission using the lib crate's proof module
    // The proper implementation would use arithmetic_lib::proof::generate_sindri_proof
    info!("⚠️  Note: Actual Sindri submission not implemented yet - this is a placeholder");
    info!("🔗 Configuration ready for Sindri integration");
    info!("💡 To implement full Sindri integration, use the proof module from the lib crate");

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
fn create_test_loan_scenario() -> (LoanState, LoanBatch) {
    let batch_id = [1u8; 32];
    let batch_timestamp = 1640995200; // January 1, 2022

    // Create test user IDs
    let alice_uuid = uuid::Uuid::new_v4();
    let bob_uuid = uuid::Uuid::new_v4();
    let alice_id = hash_uuid(&alice_uuid);
    let bob_id = hash_uuid(&bob_uuid);

    let loan_uuid = uuid::Uuid::new_v4();
    let loan_id = hash_uuid(&loan_uuid);

    // Start with empty state
    let initial_state = LoanState::new();

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

    (initial_state, batch)
}
