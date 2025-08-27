//! Arda Credit Loan Processing Program with ADS Integration
//!
//! This SP1 zkVM program processes batches of loan transactions while maintaining privacy
//! and generating cryptographically consistent state roots via Indexed Merkle Tree ADS.
//! It handles the complete loan lifecycle: user registration, KYC, loan origination,
//! payments, and closeouts. Individual transaction details remain private, while only
//! ADS-generated state root transitions and transaction counts are made public.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::private::FixedBytes;
use alloy_sol_types::SolType;
use arda_credit_lib::{
    process_loan_batch, validate_transaction, LoanBatchPublicValues, ZkVMBatchInput,
};
use sha2::{Digest, Sha256};

pub fn main() {
    // Read the complete input structure with ADS state
    let input = sp1_zkvm::io::read::<ZkVMBatchInput>();
    
    // Validate all transactions in the batch first
    for transaction in &input.batch.transactions {
        validate_transaction(transaction).expect("Invalid transaction in batch");
    }

    // Process the loan batch for business logic validation and balance computation
    let final_loan_state = process_loan_batch(
        input.initial_loan_state.clone(), 
        &input.batch
    ).expect("Failed to process loan batch");

    // CRITICAL: Process through ADS to get real state roots
    let mut ads_state = input.initial_ads_state;
    let state_transition = ads_state.process_transactions(&input.batch.transactions)
        .expect("Failed to process ADS state transition");

    // Use ADS-generated state roots (not simple hashes)
    let previous_state_root = state_transition.old_root;
    let new_state_root = state_transition.new_root;

    // Compute total portfolio balances from loan state (for transparency)
    let previous_total = input.initial_loan_state.compute_total_balance();
    let current_total = final_loan_state.compute_total_balance();

    // Create hash of transaction types for transparency (without revealing details)
    let mut types_hasher = Sha256::new();
    for transaction in &input.batch.transactions {
        types_hasher.update(&[transaction.transaction_type as u8]);
    }
    let transaction_types_hash: [u8; 32] = types_hasher.finalize().into();

    // Encode the public values with REAL ADS state roots
    let public_values = LoanBatchPublicValues {
        previous_state_root: FixedBytes(previous_state_root),
        new_state_root: FixedBytes(new_state_root),
        transaction_count: input.batch.transactions.len() as u32,
        batch_timestamp: input.batch.created_at,
        transaction_types_hash: FixedBytes(transaction_types_hash),
        previousTotalBalance: previous_total,
        currentTotalBalance: current_total,
    };

    let bytes = LoanBatchPublicValues::abi_encode(&public_values);

    // Commit to the public values with real ADS-generated state roots. The final proof will 
    // have a commitment to all the bytes that were committed to. Individual loan amounts, 
    // user details, and transaction specifics remain private within the zkVM execution, 
    // while the state roots are cryptographically consistent with off-chain ADS processing.
    sp1_zkvm::io::commit_slice(&bytes);
}
