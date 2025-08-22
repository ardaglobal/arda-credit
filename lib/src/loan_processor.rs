//! Core loan processing logic for zkVM execution
//!
//! This module contains the core state transition logic that will run inside
//! the SP1 zkVM. It processes batches of mixed transaction types while maintaining
//! privacy guarantees.

use crate::loan::{LoanError, Loan};
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

    #[error("Invalid transaction type for operation")]
    InvalidTransactionType,
}

/// Global state for loan processing (maintains user and loan state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,

    /// All loans in the system
    pub loans: HashMap<[u8; 32], Loan>,

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
            transaction_count: 0,
            last_updated: 0,
        }
    }
}

impl GlobalLoanState {
    /// Create a new empty loan state
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a single transaction and update state
    pub fn process_transaction(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        match transaction.transaction_type {
            TransactionType::UserRegistration => {
                let mut user_state = UserState { users: self.users.clone() };
                user_state.process_user_registration(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycSubmission => {
                let mut user_state = UserState { users: self.users.clone() };
                user_state.process_kyc_submission(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycVerification => {
                let mut user_state = UserState { users: self.users.clone() };
                user_state.process_kyc_verification(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::LoanOrigination => {
                let mut loan_state = crate::loan::LoanState { loans: self.loans.clone() };
                loan_state.process_loan_origination(transaction, &self.users)?;
                self.loans = loan_state.loans;
            }
            TransactionType::LoanPayment => {
                let mut loan_state = crate::loan::LoanState { loans: self.loans.clone() };
                loan_state.process_loan_payment(transaction, &self.users)?;
                self.loans = loan_state.loans;
            }
            TransactionType::LoanCloseout => {
                let mut loan_state = crate::loan::LoanState { loans: self.loans.clone() };
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
}

/// Process a complete loan batch and return the state transition
pub fn process_loan_batch(
    initial_state: GlobalLoanState,
    batch: &LoanBatch,
) -> Result<GlobalLoanState, ProcessingError> {
    let mut state = initial_state;
    state.process_batch(batch)?;
    Ok(state)
}

// Re-export the old LoanState name for backward compatibility
pub type LoanState = GlobalLoanState;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loan::LoanStatus;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus};

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1234567890,
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
            created_at: 1234567900,
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
            created_at: 1234567910,
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

        // 3. Loan origination
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        state.process_transaction(&loan_origination).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Originated);
        assert_eq!(loan.principal_amount, 50000);

        // 4. Loan payment
        let payment = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 25000, // Partial payment
            created_at: 1234567920,
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
            amount: 25000, // Final payment
            created_at: 1234567930,
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
            created_at: 1234567940,
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
        
        assert!(matches!(result, Err(ProcessingError::LoanError(LoanError::UserError(UserError::KycNotApproved { .. })))));
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
            created_at: 1234567800,
        };

        let result = state.process_batch(&batch);
        assert!(result.is_ok());
        
        assert_eq!(state.users.len(), 2);
        assert_eq!(state.transaction_count, 4);
        
        // Verify both users are KYC approved
        assert!(state.get_user(&borrower_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&lender_id).unwrap().is_kyc_approved());
    }
}
