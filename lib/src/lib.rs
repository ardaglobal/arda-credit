use alloy_sol_types::sol;

// Loan-specific types and operations
pub mod loan_processor;
pub mod loan_types;

// Proof module only available for host-side operations
#[cfg(feature = "sp1")]
pub mod proof;

sol! {
    /// Loan batch public values - what gets committed on-chain for loan processing
    /// Individual loan details, amounts, and user identities remain private.
    struct LoanBatchPublicValues {
        bytes32 previous_state_root;     // Previous Merkle state root
        bytes32 new_state_root;          // New Merkle state root
        uint32 transaction_count;        // Number of transactions processed
        uint64 batch_timestamp;          // When batch was created
        bytes32 transaction_types_hash;  // Hash of transaction types (for transparency)
    }
}

// Re-export loan processing types for convenience
pub use loan_processor::*;
pub use loan_types::*;
