//! Arda Credit Core Library
//!
//! This library provides the core business logic and data structures for the
//! Arda Credit privacy-preserving deal system built on SP1 zero-knowledge proofs.
//!
//! ## Core Modules
//!
//! - [`deal`] - Deal management logic and validation (converted from loan)
//! - [`deal_processor`] - Batch processing for deal transactions (converted from loan_processor)
//! - [`transactions`] - Transaction type definitions (updated with deal terminology)
//! - [`user`] - User management and KYC logic
//! - [`balance`] - Balance tracking and management
//!
//! ## Recent Changes
//!
//! Converted from loan-based terminology to deal-based terminology:
//! - `loan` module → `deal` module
//! - `loan_processor` module → `deal_processor` module
//! - `LoanOrigination` → `DealOrigination`
//! - `LoanPayment` → `DealPayment`
//! - `LoanCloseout` → `DealCloseout`
//! - All processing functions updated accordingly
//!
//! Business logic and ZK proof compatibility maintained.

use alloy_sol_types::sol;

// Separated modules for better maintainability
pub mod balance;
pub mod collateral;
pub mod deal;
pub mod deal_processor;
pub mod transactions;
pub mod user;
pub mod utils;
pub mod zkvm_ads; // zkVM-compatible ADS operations // NEW: Collateral state management

#[cfg(test)]
mod zkvm_ads_consistency_tests; // Consistency tests for zkVM ADS

// Proof module only available for host-side operations
#[cfg(feature = "sp1")]
pub mod proof;

// Re-export proof types (conditionally)
#[cfg(feature = "sp1")]
pub use proof::{get_sindri_proof_data, SindriProofData};

sol! {
    /// Loan batch public values - what gets committed on-chain for loan processing
    /// Individual loan details, amounts, and user identities remain private.
    struct LoanBatchPublicValues {
        bytes32 previous_state_root;     // Previous Merkle state root
        bytes32 new_state_root;          // New Merkle state root
        uint32 transaction_count;        // Number of transactions processed
        uint64 batch_timestamp;          // When batch was created
        bytes32 transaction_types_hash;  // Hash of transaction types (for transparency)
        uint128 previousTotalBalance;    // Previous total debits + credits of all loans
        uint128 currentTotalBalance;     // Current total debits + credits of all loans
    }
}

// Re-export core processing functions and state
pub use deal_processor::{
    process_deal_batch, process_loan_operations, validate_transaction, GlobalLoanState,
    LoanOperation, MerkleTree, ProcessingError,
};

// Re-export from new modules
pub use balance::{
    BalanceError, BalanceInclusionProof, BalanceState, BalanceUpdate, BalanceUpdateType,
    UserBalance,
};
pub use collateral::{Collateral, CollateralAssociation, CollateralError, CollateralState};
pub use deal::{Deal, DealError, LoanStatus, LoanTerms};
pub use transactions::*;
pub use user::{KycStatus, User, UserError, UserState, UserType};
pub use utils::{cents_to_decimal, decimal_to_cents, hash_uuid};
pub use zkvm_ads::{StateTransition, ZkVMAds, ZkVMNullifier};

// Keep the old LoanState type alias for backward compatibility
pub type LoanState = GlobalLoanState;

// Input structure for zkVM program with ADS state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ZkVMBatchInput {
    pub initial_loan_state: GlobalLoanState,
    pub initial_ads_state: ZkVMAds,
    pub batch: DealBatch,
}
