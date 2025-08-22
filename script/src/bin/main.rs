//! Local SP1 Loan Processing Testing
//!
//! This program provides a comprehensive test of the SP1 loan processing zkVM program.
//! It demonstrates the complete loan lifecycle: user registration, KYC, loan origination,
//! payments, and closeouts - all while maintaining privacy of individual transaction details.
//!
//! Usage:
//! ```shell
//! cargo run --package arda-credit --bin arda-credit
//! ```

use alloy_sol_types::SolType;
use lib::{
    hash_uuid, LoanBatch, LoanBatchPublicValues, LoanState, LoanTransaction, MetadataArray,
    TransactionType, UserType,
};
use eyre::Result;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::info;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// This is built by build.rs from the program/ directory.
pub const LOAN_PROGRAM_ELF: &[u8] = include_elf!("program");

#[allow(clippy::cognitive_complexity)]
fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🏦 Starting local SP1 loan processing test");
    info!("🔐 Testing complete loan lifecycle with privacy preservation");

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

    // Decode and analyze the public outputs
    let public_values = proof.public_values;
    let output = LoanBatchPublicValues::abi_decode(public_values.as_slice())
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
        info!("🎉 Local SP1 loan processing test completed successfully!");
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
