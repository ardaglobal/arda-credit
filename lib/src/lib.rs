use alloy_sol_types::sol;

// Separated modules for better maintainability
pub mod balance;
pub mod credit_evaluation;
pub mod credit_line;
pub mod loan;
pub mod loan_processor;
pub mod merchant;
pub mod pool;
pub mod transactions;
pub mod user;
pub mod utils;
pub mod zkvm_ads; // zkVM-compatible ADS operations

#[cfg(test)]
mod zkvm_ads_consistency_tests; // Consistency tests for zkVM ADS

// Proof module only available for host-side operations
#[cfg(feature = "sp1")]
pub mod proof;

// Tracing utilities for comprehensive observability
pub mod tracing_utils;

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
pub use loan_processor::{
    process_loan_batch, process_loan_operations, validate_transaction, GlobalLoanState,
    LoanOperation, MerkleTree, ProcessingError,
};

// Re-export from new modules
pub use balance::{
    BalanceError, BalanceInclusionProof, BalanceState, BalanceUpdate, BalanceUpdateType,
    UserBalance,
};
pub use credit_evaluation::{
    calculate_30_day_average, evaluate_credit_request, get_required_documents_for_mcc,
    is_restricted_mcc, validate_required_documents, CreditEvaluator, DailyTurnover,
    DocumentVerification, MerchantProfile,
};
pub use credit_line::{
    CreditLine, CreditLineError, CreditLineRequest, CreditLineRequestStatus, CreditLineState,
    CreditLineStatus, CreditUtilization, CreditUtilizationType, EligibilityResult,
    PoolMatchingResult,
};
pub use loan::{
    LiquidityPool, Loan, LoanError, LoanStatus, LoanTerms, PoolDeploymentTerms, PoolError,
    PoolState, PoolStatus,
};
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
    pub batch: LoanBatch,
}
