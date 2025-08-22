use alloy_sol_types::sol;

// Separated modules for better maintainability
pub mod loan;
pub mod loan_processor;
pub mod transactions;
pub mod user;

// Legacy loan_types module - keeping for backward compatibility but deprecated
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

// Re-export core processing functions and state
pub use loan_processor::{process_loan_batch, GlobalLoanState, ProcessingError};

// Re-export from new modules
pub use loan::{Loan, LoanError, LoanStatus};
pub use transactions::*;
pub use user::{KycStatus, User, UserError, UserState, UserType};

// Legacy re-exports from loan_types for backward compatibility
pub use loan_types::{LoanTerms, cents_to_decimal, decimal_to_cents, hash_uuid};

// Keep the old LoanState type alias for backward compatibility
pub type LoanState = GlobalLoanState;
