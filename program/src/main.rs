//! Arda Credit Loan Processing Program
//!
//! This SP1 zkVM program processes batches of loan transactions while maintaining privacy.
//! It handles the complete loan lifecycle: user registration, KYC, loan origination,
//! payments, and closeouts. Individual transaction details remain private, while only
//! state root transitions and transaction counts are made public.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::private::FixedBytes;
use alloy_sol_types::SolType;
use lib::{
    process_loan_batch, validate_transaction, LoanBatch, LoanBatchPublicValues, LoanState,
};
use sha2::{Digest, Sha256};

pub fn main() {
    // Read the initial loan state from the prover
    let initial_state = sp1_zkvm::io::read::<LoanState>();

    // Read the loan batch to process
    let batch = sp1_zkvm::io::read::<LoanBatch>();

    // Validate all transactions in the batch first
    for transaction in &batch.transactions {
        validate_transaction(transaction).expect("Invalid transaction in batch");
    }

    // Process the loan batch - this performs all the state transitions
    // while keeping individual transaction details private within the zkVM
    let final_state =
        process_loan_batch(initial_state.clone(), &batch).expect("Failed to process loan batch");

    // Compute state roots for public commitment
    let previous_state_root = initial_state.compute_state_hash();
    let new_state_root = final_state.compute_state_hash();

    // Create hash of transaction types for transparency (without revealing details)
    let mut types_hasher = Sha256::new();
    for transaction in &batch.transactions {
        types_hasher.update(&[transaction.transaction_type as u8]);
    }
    let transaction_types_hash: [u8; 32] = types_hasher.finalize().into();

    // Encode the public values that will be committed on-chain
    // This preserves privacy while enabling verification of state transitions
    let public_values = LoanBatchPublicValues {
        previous_state_root: FixedBytes(previous_state_root),
        new_state_root: FixedBytes(new_state_root),
        transaction_count: batch.transactions.len() as u32,
        batch_timestamp: batch.created_at,
        transaction_types_hash: FixedBytes(transaction_types_hash),
    };

    let bytes = LoanBatchPublicValues::abi_encode(&public_values);

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to. Individual loan amounts, user details, and transaction
    // specifics remain private within the zkVM execution.
    sp1_zkvm::io::commit_slice(&bytes);
}
