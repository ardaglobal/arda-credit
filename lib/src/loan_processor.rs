//! Core loan processing logic for zkVM execution
//!
//! This module contains the core state transition logic that will run inside
//! the SP1 zkVM. It processes batches of mixed transaction types while maintaining
//! privacy guarantees.

#[cfg(test)]
use crate::balance::BalanceUpdateType;
use crate::balance::{BalanceError, BalanceState, BalanceUpdate};
use crate::loan::LoanTerms;
use crate::loan::{Loan, LoanError};
use crate::transactions::{LoanBatch, LoanTransaction, TransactionType};
use crate::user::{User, UserError, UserState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during loan processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Loan error: {0}")]
    LoanError(#[from] LoanError),

    #[error("Balance error: {0}")]
    BalanceError(#[from] BalanceError),

    #[error("Invalid transaction type for operation")]
    InvalidTransactionType,

    #[error("Invalid timestamp: {timestamp} is outside acceptable range (must be between {min_timestamp} and {max_timestamp})")]
    InvalidTimestamp {
        timestamp: u64,
        min_timestamp: u64,
        max_timestamp: u64,
    },

    #[error("Transaction timestamp {transaction_timestamp} cannot be older than current state timestamp {state_timestamp}")]
    TimestampGoesBackward {
        transaction_timestamp: u64,
        state_timestamp: u64,
    },
}

/// Global state for loan processing (maintains user and loan state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,

    /// All loans in the system
    pub loans: HashMap<[u8; 32], Loan>,

    /// User balance tracking state
    pub balance_state: BalanceState,

    /// Configurable loan terms
    pub loan_terms: LoanTerms,

    /// Transaction counter for ordering
    pub transaction_count: u64,

    /// Last update timestamp
    pub last_updated: u64,
}

impl Default for GlobalLoanState {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            balance_state: BalanceState::new(),
            loan_terms: LoanTerms::default(),
            transaction_count: 0,
            last_updated: 0,
        }
    }
}

impl GlobalLoanState {
    /// Create a new empty loan state with default loan terms
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new loan state with custom loan terms
    pub fn with_loan_terms(loan_terms: LoanTerms) -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            balance_state: BalanceState::new(),
            loan_terms,
            transaction_count: 0,
            last_updated: 0,
        }
    }

    /// Update loan terms configuration
    pub fn set_loan_terms(&mut self, loan_terms: LoanTerms) {
        self.loan_terms = loan_terms;
    }

    /// Process a single transaction and update state
    pub fn process_transaction(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        // Validate transaction with current state context
        validate_transaction_with_state(transaction, self)?;

        match transaction.transaction_type {
            TransactionType::UserRegistration => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_user_registration(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycSubmission => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_kyc_submission(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycVerification => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_kyc_verification(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::LoanOrigination => {
                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_origination(transaction, &self.users, &self.loan_terms)?;
                self.loans = loan_state.loans;

                // Create balance updates for loan origination
                let metadata = transaction.metadata.as_bytes();
                let mut borrower_id = [0u8; 32];
                let mut lender_id = [0u8; 32];
                borrower_id.copy_from_slice(&metadata[0..32]);
                lender_id.copy_from_slice(&metadata[32..64]);

                let balance_updates = self.balance_state.create_origination_updates(
                    transaction.transaction_id,
                    transaction.loan_id,
                    borrower_id,
                    lender_id,
                    transaction.amount as u128,
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::LoanPayment => {
                // Get loan info before processing payment for balance updates
                let loan =
                    self.loans
                        .get(&transaction.loan_id)
                        .ok_or(ProcessingError::LoanError(LoanError::LoanNotFound {
                            loan_id: transaction.loan_id,
                        }))?;
                let lender_id = loan.lender_id;
                let payer_id = transaction.user_id;

                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_payment(transaction, &self.users)?;
                self.loans = loan_state.loans;

                // Create balance updates for loan payment
                let balance_updates = self.balance_state.create_payment_updates(
                    transaction.transaction_id,
                    transaction.loan_id,
                    payer_id,
                    lender_id,
                    transaction.amount as u128,
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::LoanCloseout => {
                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_closeout(transaction, &self.users)?;
                self.loans = loan_state.loans;
            }
        }

        // Update global state
        self.transaction_count += 1;
        self.last_updated = transaction.created_at;

        Ok(())
    }

    /// Process a batch of transactions
    pub fn process_batch(&mut self, batch: &LoanBatch) -> Result<(), ProcessingError> {
        // Validate batch timestamp bounds
        validate_timestamp_bounds(batch.created_at)?;

        // Validate temporal ordering within the batch
        validate_batch_temporal_ordering(batch)?;

        for transaction in &batch.transactions {
            self.process_transaction(transaction)?;
        }
        Ok(())
    }

    /// Get user by ID (read-only)
    pub fn get_user(&self, user_id: &[u8; 32]) -> Option<&User> {
        self.users.get(user_id)
    }

    /// Get loan by ID (read-only)
    pub fn get_loan(&self, loan_id: &[u8; 32]) -> Option<&Loan> {
        self.loans.get(loan_id)
    }

    /// Get current state hash (for Merkle tree)
    pub fn compute_state_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let serialized = serde_json::to_string(&self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        hasher.finalize().into()
    }

    /// Compute total portfolio balance across all loans
    /// This aggregates all outstanding loan balances for portfolio visibility
    pub fn compute_total_balance(&self) -> u128 {
        let mut total_balance = 0u128;

        for loan in self.loans.values() {
            // Add outstanding balance for each active loan
            if matches!(
                loan.loan_status,
                crate::loan::LoanStatus::Active | crate::loan::LoanStatus::Originated
            ) {
                total_balance = total_balance.saturating_add(loan.outstanding_balance as u128);
            }
        }

        total_balance
    }

    /// Get user's current balance
    pub fn get_user_balance(&self, user_id: &[u8; 32]) -> u128 {
        self.balance_state.get_balance(user_id)
    }

    /// Get balance update history for a user
    pub fn get_user_balance_history(
        &self,
        user_id: &[u8; 32],
        limit: Option<usize>,
    ) -> Vec<&BalanceUpdate> {
        self.balance_state.get_user_update_history(user_id, limit)
    }

    /// Get balance updates for a specific loan
    pub fn get_loan_balance_updates(&self, loan_id: &[u8; 32]) -> Vec<&BalanceUpdate> {
        self.balance_state.get_loan_balance_updates(loan_id)
    }

    /// Get total value of all user balances
    pub fn compute_total_user_balance(&self) -> u128 {
        self.balance_state.compute_total_balance()
    }
}

/// Process a complete loan batch and return the state transition
pub fn process_loan_batch(
    initial_state: GlobalLoanState,
    batch: &LoanBatch,
) -> Result<GlobalLoanState, ProcessingError> {
    let mut state = initial_state;
    // Additional validation: ensure batch timestamp is not older than current state
    if batch.created_at < state.last_updated {
        return Err(ProcessingError::TimestampGoesBackward {
            transaction_timestamp: batch.created_at,
            state_timestamp: state.last_updated,
        });
    }

    state.process_batch(batch)?;
    Ok(state)
}

/// Validate a loan transaction before processing
pub fn validate_transaction(transaction: &LoanTransaction) -> Result<(), ProcessingError> {
    // Basic validation
    match transaction.transaction_type {
        TransactionType::LoanOrigination | TransactionType::LoanPayment => {
            if transaction.amount == 0 {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        _ => {}
    }

    // Validate all zeros loan_id for non-loan transactions
    let zero_loan_id = [0u8; 32];
    match transaction.transaction_type {
        TransactionType::UserRegistration
        | TransactionType::KycSubmission
        | TransactionType::KycVerification => {
            if transaction.loan_id != zero_loan_id {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        _ => {}
    }

    // Additional validation for timestamp bounds
    validate_timestamp_bounds(transaction.created_at)?;

    Ok(())
}

/// Validate a loan transaction with state context for temporal ordering
pub fn validate_transaction_with_state(
    transaction: &LoanTransaction,
    current_state: &GlobalLoanState,
) -> Result<(), ProcessingError> {
    // Run basic validation first
    validate_transaction(transaction)?;

    // Validate temporal ordering - transaction cannot be older than current state
    if transaction.created_at < current_state.last_updated {
        return Err(ProcessingError::TimestampGoesBackward {
            transaction_timestamp: transaction.created_at,
            state_timestamp: current_state.last_updated,
        });
    }

    Ok(())
}

/// Validate that a timestamp is within reasonable bounds for production use
fn validate_timestamp_bounds(timestamp: u64) -> Result<(), ProcessingError> {
    // Define reasonable bounds for loan system timestamps
    // Min: January 1, 2020 (1577836800)
    // Max: January 1, 2030 (1893456000) - 10 years from now for reasonable future limit
    const MIN_TIMESTAMP: u64 = 1577836800; // Jan 1, 2020
    const MAX_TIMESTAMP: u64 = 1893456000; // Jan 1, 2030

    if timestamp < MIN_TIMESTAMP || timestamp > MAX_TIMESTAMP {
        return Err(ProcessingError::InvalidTimestamp {
            timestamp,
            min_timestamp: MIN_TIMESTAMP,
            max_timestamp: MAX_TIMESTAMP,
        });
    }

    Ok(())
}

/// Validate temporal ordering within a batch of transactions
fn validate_batch_temporal_ordering(batch: &LoanBatch) -> Result<(), ProcessingError> {
    let mut prev_timestamp = 0u64;

    for transaction in &batch.transactions {
        // Ensure transactions within batch are not going backwards in time
        if transaction.created_at < prev_timestamp {
            return Err(ProcessingError::TimestampGoesBackward {
                transaction_timestamp: transaction.created_at,
                state_timestamp: prev_timestamp,
            });
        }

        // Validate each transaction's timestamp bounds
        validate_timestamp_bounds(transaction.created_at)?;

        prev_timestamp = transaction.created_at;
    }

    Ok(())
}

// Re-export the old LoanState name for backward compatibility
pub type LoanState = GlobalLoanState;

// ============================================================================
// NEW LOAN OPERATIONS INTERFACE
// ============================================================================

/// Operations for loan processing (alternative to LoanTransaction)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoanOperation {
    /// Create a new loan
    LoanCreation {
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        principal: u128,
        interest_rate: u64,
        term_months: u32,
    },
    /// Make a payment towards a loan
    Payment {
        loan_id: [u8; 32],
        payer_id: [u8; 32],
        amount: u128,
    },
    /// Fund an originated loan
    LoanFunding {
        loan_id: [u8; 32],
        lender_id: [u8; 32],
    },
    /// Update balance (for reconciliation)
    BalanceUpdate {
        loan_id: [u8; 32],
        new_balance: u128,
    },
}

impl LoanOperation {
    /// Get the transaction type for this operation
    pub fn transaction_type(&self) -> u8 {
        match self {
            LoanOperation::LoanCreation { .. } => 1,
            LoanOperation::Payment { .. } => 2,
            LoanOperation::LoanFunding { .. } => 3,
            LoanOperation::BalanceUpdate { .. } => 4,
        }
    }

    /// Convert operation to nullifier hash for Merkle tree
    pub fn to_nullifier(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        match self {
            LoanOperation::LoanCreation {
                loan_id,
                borrower_id,
                lender_id,
                principal,
                ..
            } => {
                hasher.update(b"loan_creation");
                hasher.update(loan_id);
                hasher.update(borrower_id);
                hasher.update(lender_id);
                hasher.update(&principal.to_le_bytes());
            }
            LoanOperation::Payment {
                loan_id,
                payer_id,
                amount,
            } => {
                hasher.update(b"payment");
                hasher.update(loan_id);
                hasher.update(payer_id);
                hasher.update(&amount.to_le_bytes());
            }
            LoanOperation::LoanFunding { loan_id, lender_id } => {
                hasher.update(b"funding");
                hasher.update(loan_id);
                hasher.update(lender_id);
            }
            LoanOperation::BalanceUpdate {
                loan_id,
                new_balance,
            } => {
                hasher.update(b"balance_update");
                hasher.update(loan_id);
                hasher.update(&new_balance.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }
}

/// Simple Merkle tree implementation for loan operations
#[derive(Debug, Clone)]
pub struct MerkleTree {
    root: [u8; 32],
    leaves: Vec<[u8; 32]>,
}

impl MerkleTree {
    /// Create a new Merkle tree from an existing root
    pub fn from_root(root: [u8; 32]) -> Self {
        Self {
            root,
            leaves: Vec::new(),
        }
    }

    /// Insert a new leaf (nullifier) into the tree
    pub fn insert(&mut self, leaf: [u8; 32]) {
        self.leaves.push(leaf);
        self.root = self.compute_root();
    }

    /// Get the current root hash
    pub fn root(&self) -> [u8; 32] {
        self.root
    }

    /// Compute the Merkle root from all leaves
    fn compute_root(&self) -> [u8; 32] {
        if self.leaves.is_empty() {
            return [0u8; 32];
        }

        use sha2::{Digest, Sha256};
        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(&chunk[0]);
                if chunk.len() > 1 {
                    hasher.update(&chunk[1]);
                } else {
                    hasher.update(&chunk[0]); // Duplicate if odd number
                }
                next_level.push(hasher.finalize().into());
            }

            current_level = next_level;
        }

        current_level[0]
    }
}

/// Process a batch of loan operations and return public state transition
pub fn process_loan_operations(
    previous_balance: u128,
    operations: &[LoanOperation],
    previous_root: [u8; 32],
    batch_timestamp: u64,
) -> crate::LoanBatchPublicValues {
    let mut current_balance = previous_balance;
    let mut merkle_tree = MerkleTree::from_root(previous_root);

    // Compute transaction types hash
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for operation in operations {
        hasher.update(&[operation.transaction_type()]);
    }
    let transaction_types_hash: [u8; 32] = hasher.finalize().into();

    // Process operations
    for operation in operations {
        // Update balance based on operation type
        current_balance = apply_operation_to_balance(current_balance, operation);

        // Update Merkle tree with operation commitment
        let nullifier = operation.to_nullifier();
        merkle_tree.insert(nullifier);
    }

    let new_root = merkle_tree.root();

    crate::LoanBatchPublicValues {
        previous_state_root: previous_root.into(),
        new_state_root: new_root.into(),
        transaction_count: operations.len() as u32,
        batch_timestamp,
        transaction_types_hash: transaction_types_hash.into(),
        previousTotalBalance: previous_balance,
        currentTotalBalance: current_balance,
    }
}

/// Apply a loan operation to the current balance
fn apply_operation_to_balance(balance: u128, operation: &LoanOperation) -> u128 {
    match operation {
        LoanOperation::LoanCreation { principal, .. } => balance.saturating_add(*principal),
        LoanOperation::Payment { amount, .. } => balance.saturating_add(*amount),
        LoanOperation::LoanFunding { .. } => balance, // No balance change, just state change
        LoanOperation::BalanceUpdate { new_balance, .. } => *new_balance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loan::LoanStatus;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1640995200, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&[user_type]),
        }
    }

    fn create_kyc_verification(user_id: [u8; 32], approved: bool) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1640995300, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&[if approved { 1 } else { 0 }]),
        }
    }

    fn create_loan_origination(
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&borrower_id);
        metadata[32..64].copy_from_slice(&lender_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount,
            created_at: 1640995400, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_full_loan_lifecycle() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // 1. Register users
        let borrower_reg = create_test_user_registration(borrower_id, 1); // Investor
        let lender_reg = create_test_user_registration(lender_id, 2); // Lender

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();

        // 2. KYC verification
        let borrower_kyc = create_kyc_verification(borrower_id, true);
        let lender_kyc = create_kyc_verification(lender_id, true);

        state.process_transaction(&borrower_kyc).unwrap();
        state.process_transaction(&lender_kyc).unwrap();

        // Verify users are registered and KYC approved
        let borrower = state.get_user(&borrower_id).unwrap();
        let lender = state.get_user(&lender_id).unwrap();
        assert_eq!(borrower.kyc_status, KycStatus::Approved);
        assert_eq!(lender.kyc_status, KycStatus::Approved);

        // 3. Give lender sufficient balance for loan origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [99u8; 32], // funding transaction ID
            lender_id,
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            100000,     // Give enough for the 50000 loan
            0,          // previous balance
            1640995100, // before loan origination
            Some("Test funding".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        // 4. Loan origination
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        state.process_transaction(&loan_origination).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Originated);
        assert_eq!(loan.principal_amount, 50000);

        // 5. Loan payment
        let payment = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 25000,          // Partial payment
            created_at: 1640995500, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&payment).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Active); // Now active
        assert_eq!(loan.outstanding_balance, 25000);

        // 5. Final payment
        let final_payment = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 25000,          // Final payment
            created_at: 1640995600, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&final_payment).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::PaidOff);
        assert_eq!(loan.outstanding_balance, 0);

        // 6. Loan closeout
        let closeout = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: lender_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1640995700, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&closeout).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Closed);

        // Verify transaction count
        assert_eq!(state.transaction_count, 8); // 2 registrations + 2 KYC verifications + 1 origination + 2 payments + 1 closeout
    }

    #[test]
    fn test_loan_origination_without_kyc_fails() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Register users but don't KYC them
        let borrower_reg = create_test_user_registration(borrower_id, 1);
        let lender_reg = create_test_user_registration(lender_id, 2);

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();

        // Try to originate loan without KYC
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        let result = state.process_transaction(&loan_origination);

        assert!(matches!(
            result,
            Err(ProcessingError::LoanError(LoanError::UserError(
                UserError::KycNotApproved { .. }
            )))
        ));
    }

    #[test]
    fn test_process_batch() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        let batch = LoanBatch {
            batch_id: [100u8; 32],
            transactions: vec![
                create_test_user_registration(borrower_id, 1),
                create_test_user_registration(lender_id, 2),
                create_kyc_verification(borrower_id, true),
                create_kyc_verification(lender_id, true),
            ],
            previous_state_root: [0u8; 32],
            created_at: 1640995200, // January 1, 2022 - valid timestamp
        };

        let result = state.process_batch(&batch);
        assert!(result.is_ok());

        assert_eq!(state.users.len(), 2);
        assert_eq!(state.transaction_count, 4);

        // Verify both users are KYC approved
        assert!(state.get_user(&borrower_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&lender_id).unwrap().is_kyc_approved());
    }

    #[test]
    fn test_configurable_loan_terms() {
        // Test with custom loan terms
        let custom_terms = LoanTerms::new(1200, 60); // 12% APR, 5 year term
        let mut state = GlobalLoanState::with_loan_terms(custom_terms);

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [3u8; 32];

        // Register and KYC both users
        setup_kycd_users(&mut state, borrower_id, lender_id);

        // Create loan origination transaction
        let mut loan_metadata = MetadataArray::empty();
        loan_metadata.as_mut_bytes()[0..32].copy_from_slice(&borrower_id);
        loan_metadata.as_mut_bytes()[32..64].copy_from_slice(&lender_id);

        let base_timestamp = 1_640_995_200; // January 1, 2022
        let loan_tx = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id: lender_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount: 1_000_000, // $10,000 in cents
            created_at: base_timestamp + 5000,
            metadata: loan_metadata,
        };

        // Give lender sufficient balance for loan origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [98u8; 32], // funding transaction ID
            lender_id,
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            2_000_000,      // Give enough for the 1,000,000 loan
            0,              // previous balance
            base_timestamp, // same timeframe
            Some("Test funding for configurable terms".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        state.process_transaction(&loan_tx).unwrap();

        // Verify loan uses custom terms
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.interest_rate, 1200); // Custom 12% APR
        assert_eq!(loan.term_months, 60); // Custom 5 year term
        assert_eq!(loan.principal_amount, 1_000_000);
        assert_eq!(loan.outstanding_balance, 1_000_000);

        // Verify maturity date calculation
        let expected_maturity = (base_timestamp + 5000) + (60_u64 * 30 * 24 * 60 * 60); // 60 months
        assert_eq!(loan.maturity_date, expected_maturity);

        // Test with default loan terms
        let mut default_state = GlobalLoanState::new();
        assert_eq!(default_state.loan_terms.interest_rate, 750); // Default 7.5%
        assert_eq!(default_state.loan_terms.term_months, 36); // Default 3 years

        // Test updating loan terms on existing state
        let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 year term
        default_state.set_loan_terms(new_terms);
        assert_eq!(default_state.loan_terms.interest_rate, 500);
        assert_eq!(default_state.loan_terms.term_months, 24);
    }

    /// Helper function to set up KYC'd users for testing
    fn setup_kycd_users(state: &mut GlobalLoanState, borrower_id: [u8; 32], lender_id: [u8; 32]) {
        let base_timestamp = 1_640_995_200; // January 1, 2022

        // Register borrower
        let mut borrower_metadata = MetadataArray::empty();
        borrower_metadata.as_mut_bytes()[0] = UserType::Investor as u8;
        let borrower_reg_tx = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1000,
            metadata: borrower_metadata,
        };
        state.process_transaction(&borrower_reg_tx).unwrap();

        // Register lender
        let mut lender_metadata = MetadataArray::empty();
        lender_metadata.as_mut_bytes()[0] = UserType::Lender as u8;
        let lender_reg_tx = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1100,
            metadata: lender_metadata,
        };
        state.process_transaction(&lender_reg_tx).unwrap();

        // Submit KYC for borrower
        let borrower_kyc_submit_tx = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2000,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&borrower_kyc_submit_tx).unwrap();

        // Submit KYC for lender
        let lender_kyc_submit_tx = LoanTransaction {
            transaction_id: [7u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2100,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&lender_kyc_submit_tx).unwrap();

        // Verify KYC for borrower
        let mut borrower_verify_metadata = MetadataArray::empty();
        borrower_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let borrower_kyc_verify_tx = LoanTransaction {
            transaction_id: [8u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3000,
            metadata: borrower_verify_metadata,
        };
        state.process_transaction(&borrower_kyc_verify_tx).unwrap();

        // Verify KYC for lender
        let mut lender_verify_metadata = MetadataArray::empty();
        lender_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let lender_kyc_verify_tx = LoanTransaction {
            transaction_id: [9u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3100,
            metadata: lender_verify_metadata,
        };
        state.process_transaction(&lender_kyc_verify_tx).unwrap();
    }
    #[test]
    fn test_timestamp_validation_bounds() {
        // Test valid timestamp (within bounds)
        let valid_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1640995200, // January 1, 2022 - valid
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&valid_transaction).is_ok());

        // Test invalid timestamp (too old)
        let old_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000000000, // September 9, 2001 - too old
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&old_transaction).is_err());

        // Test invalid timestamp (too far in future)
        let future_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 2000000000, // May 18, 2033 - too far in future
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&future_transaction).is_err());
    }

    #[test]
    fn test_temporal_ordering_validation() {
        let mut state = GlobalLoanState::new();
        state.last_updated = 2000;

        // Test transaction newer than state (valid)
        let newer_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1640995200, // Valid timestamp after state
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction_with_state(&newer_transaction, &state).is_ok());

        // Test transaction older than state (invalid)
        let older_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000, // Before state timestamp
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction_with_state(&older_transaction, &state).is_err());
    }

    #[test]
    fn test_batch_temporal_ordering() {
        // Test valid batch with properly ordered timestamps
        let transactions = vec![
            LoanTransaction {
                transaction_id: [1u8; 32],
                user_id: [1u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1640995200, // Base time
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1640995300, // 100 seconds later
                metadata: MetadataArray::empty(),
            },
        ];

        let valid_batch = LoanBatch {
            batch_id: [1u8; 32],
            transactions,
            previous_state_root: [0u8; 32],
            created_at: 1640995200,
        };
        assert!(validate_batch_temporal_ordering(&valid_batch).is_ok());

        // Test invalid batch with backwards timestamps
        let invalid_transactions = vec![
            LoanTransaction {
                transaction_id: [1u8; 32],
                user_id: [1u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1640995300, // Later time first
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1640995200, // Earlier time second - invalid!
                metadata: MetadataArray::empty(),
            },
        ];

        let invalid_batch = LoanBatch {
            batch_id: [1u8; 32],
            transactions: invalid_transactions,
            previous_state_root: [0u8; 32],
            created_at: 1640995200,
        };
        assert!(validate_batch_temporal_ordering(&invalid_batch).is_err());
    }

    #[test]
    fn test_process_loan_operations() {
        let previous_balance = 100_000u128; // $1000
        let previous_root = [0u8; 32];
        let batch_timestamp = 1640995200;

        let operations = vec![
            LoanOperation::LoanCreation {
                loan_id: [1u8; 32],
                borrower_id: [2u8; 32],
                lender_id: [3u8; 32],
                principal: 50_000,
                interest_rate: 750,
                term_months: 36,
            },
            LoanOperation::Payment {
                loan_id: [1u8; 32],
                payer_id: [2u8; 32],
                amount: 10_000,
            },
        ];

        let public_values = process_loan_operations(
            previous_balance,
            &operations,
            previous_root,
            batch_timestamp,
        );

        // Check that balance was updated correctly
        assert_eq!(public_values.previousTotalBalance, 100_000);
        assert_eq!(public_values.currentTotalBalance, 160_000); // 100k + 50k loan + 10k payment
        assert_eq!(public_values.transaction_count, 2);
        assert_eq!(public_values.batch_timestamp, batch_timestamp);

        // Check that state root changed (merkle tree was updated)
        assert_ne!(
            public_values.previous_state_root,
            public_values.new_state_root
        );

        // Check that transaction types hash is computed
        let zero_hash: alloy_sol_types::private::FixedBytes<32> = [0u8; 32].into();
        assert_ne!(public_values.transaction_types_hash, zero_hash);
    }

    #[test]
    fn test_loan_operation_nullifiers() {
        let operation1 = LoanOperation::LoanCreation {
            loan_id: [1u8; 32],
            borrower_id: [2u8; 32],
            lender_id: [3u8; 32],
            principal: 50_000,
            interest_rate: 750,
            term_months: 36,
        };

        let operation2 = LoanOperation::Payment {
            loan_id: [1u8; 32],
            payer_id: [2u8; 32],
            amount: 10_000,
        };

        let nullifier1 = operation1.to_nullifier();
        let nullifier2 = operation2.to_nullifier();

        // Nullifiers should be different
        assert_ne!(nullifier1, nullifier2);

        // Same operation should produce same nullifier
        let nullifier1_copy = operation1.to_nullifier();
        assert_eq!(nullifier1, nullifier1_copy);
    }

    #[test]
    fn test_merkle_tree_basic() {
        let mut tree = MerkleTree::from_root([0u8; 32]);
        let initial_root = tree.root();

        // Insert a leaf
        tree.insert([1u8; 32]);
        let new_root = tree.root();

        // Root should change after insertion
        assert_ne!(initial_root, new_root);

        // Insert another leaf
        tree.insert([2u8; 32]);
        let final_root = tree.root();

        // Root should change again
        assert_ne!(new_root, final_root);
    }

    #[test]
    fn test_compute_total_balance() {
        let mut state = GlobalLoanState::new();

        // Initially, total balance should be 0
        assert_eq!(state.compute_total_balance(), 0);

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        // Set up KYC'd users
        setup_kycd_users(&mut state, borrower_id, lender_id);

        // Create multiple loans
        let loan1_id = [10u8; 32];
        let loan2_id = [11u8; 32];
        let loan3_id = [12u8; 32];

        // Loan 1: Active with $100 outstanding balance
        let loan1 = crate::loan::Loan {
            loan_id: loan1_id,
            borrower_id,
            lender_id,
            principal_amount: 10000, // $100
            outstanding_balance: 10000,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::Active,
            originated_at: 1640995200,
            funded_at: 1640995200,
            maturity_date: 1640995200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 2: Originated with $200 outstanding balance
        let loan2 = crate::loan::Loan {
            loan_id: loan2_id,
            borrower_id,
            lender_id,
            principal_amount: 20000, // $200
            outstanding_balance: 20000,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::Originated,
            originated_at: 1640995200,
            funded_at: 0,
            maturity_date: 1640995200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 3: PaidOff (should not count toward balance)
        let loan3 = crate::loan::Loan {
            loan_id: loan3_id,
            borrower_id,
            lender_id,
            principal_amount: 30000, // $300
            outstanding_balance: 0,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::PaidOff,
            originated_at: 1640995200,
            funded_at: 1640995200,
            maturity_date: 1640995200 + (36 * 30 * 24 * 60 * 60),
        };

        state.loans.insert(loan1_id, loan1);
        state.loans.insert(loan2_id, loan2);
        state.loans.insert(loan3_id, loan3);

        // Total balance should be $100 + $200 = $300 (loan3 doesn't count as it's paid off)
        assert_eq!(state.compute_total_balance(), 30000);

        // Make a payment on loan1 to reduce its outstanding balance
        state.loans.get_mut(&loan1_id).unwrap().outstanding_balance = 5000; // $50 remaining

        // Total balance should now be $50 + $200 = $250
        assert_eq!(state.compute_total_balance(), 25000);

        // Pay off loan2
        state.loans.get_mut(&loan2_id).unwrap().outstanding_balance = 0;
        state.loans.get_mut(&loan2_id).unwrap().loan_status = LoanStatus::PaidOff;

        // Total balance should now be just $50
        assert_eq!(state.compute_total_balance(), 5000);
    }
}
