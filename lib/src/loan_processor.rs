//! Core loan processing logic for zkVM execution
//!
//! This module contains the core state transition logic that will run inside
//! the SP1 zkVM. It processes batches of mixed transaction types while maintaining
//! privacy guarantees.

use crate::loan_types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during loan processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("User not found: {user_id:?}")]
    UserNotFound { user_id: [u8; 32] },

    #[error("Loan not found: {loan_id:?}")]
    LoanNotFound { loan_id: [u8; 32] },

    #[error("KYC not approved for user: {user_id:?}")]
    KycNotApproved { user_id: [u8; 32] },

    #[error("Invalid loan state transition: {current_status:?} -> {requested_operation:?}")]
    InvalidLoanStateTransition {
        current_status: LoanStatus,
        requested_operation: TransactionType,
    },

    #[error("Insufficient loan balance: requested {amount}, available {balance}")]
    InsufficientLoanBalance { amount: u64, balance: u64 },

    #[error("Invalid transaction type for operation")]
    InvalidTransactionType,

    #[error("Duplicate user registration: {user_id:?}")]
    DuplicateUser { user_id: [u8; 32] },

    #[error("Duplicate loan: {loan_id:?}")]
    DuplicateLoan { loan_id: [u8; 32] },
}

/// Global state for loan processing (maintains user and loan state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,
    
    /// All loans in the system
    pub loans: HashMap<[u8; 32], Loan>,
    
    /// Transaction counter for ordering
    pub transaction_count: u64,
    
    /// Last update timestamp
    pub last_updated: u64,
}

impl Default for LoanState {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            transaction_count: 0,
            last_updated: 0,
        }
    }
}

impl LoanState {
    /// Create a new empty loan state
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a single transaction and update state
    pub fn process_transaction(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        match transaction.transaction_type {
            TransactionType::UserRegistration => {
                self.process_user_registration(transaction)
            }
            TransactionType::KycSubmission => {
                self.process_kyc_submission(transaction)
            }
            TransactionType::KycVerification => {
                self.process_kyc_verification(transaction)
            }
            TransactionType::LoanOrigination => {
                self.process_loan_origination(transaction)
            }
            TransactionType::LoanPayment => {
                self.process_loan_payment(transaction)
            }
            TransactionType::LoanCloseout => {
                self.process_loan_closeout(transaction)
            }
        }?;

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

    /// State Transition 1: User Registration
    fn process_user_registration(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let user_id = transaction.user_id;

        // Check for duplicate registration
        if self.users.contains_key(&user_id) {
            return Err(ProcessingError::DuplicateUser { user_id });
        }

        // Decode user data from metadata
        let user_type = match transaction.metadata.as_bytes()[0] {
            1 => UserType::Investor,
            2 => UserType::Lender,
            _ => return Err(ProcessingError::InvalidTransactionType),
        };

        // Create new user
        let user = User {
            user_id,
            user_type,
            kyc_status: KycStatus::NotSubmitted,
            kyc_submitted_at: 0,
            kyc_verified_at: 0,
            created_at: transaction.created_at,
        };

        self.users.insert(user_id, user);
        Ok(())
    }

    /// State Transition 2: KYC Submission
    fn process_kyc_submission(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let user_id = transaction.user_id;

        let user = self.users.get_mut(&user_id)
            .ok_or(ProcessingError::UserNotFound { user_id })?;

        // Update KYC status to pending
        user.kyc_status = KycStatus::Pending;
        user.kyc_submitted_at = transaction.created_at;

        Ok(())
    }

    /// State Transition 3: KYC Verification (admin operation)
    fn process_kyc_verification(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let user_id = transaction.user_id;

        let user = self.users.get_mut(&user_id)
            .ok_or(ProcessingError::UserNotFound { user_id })?;

        // Decode verification result from metadata
        let approved = transaction.metadata.as_bytes()[0] == 1;

        user.kyc_status = if approved {
            KycStatus::Approved
        } else {
            KycStatus::Rejected
        };
        user.kyc_verified_at = transaction.created_at;

        Ok(())
    }

    /// State Transition 4: Loan Origination
    fn process_loan_origination(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let loan_id = transaction.loan_id;
        let _initiator_id = transaction.user_id;

        // Check for duplicate loan
        if self.loans.contains_key(&loan_id) {
            return Err(ProcessingError::DuplicateLoan { loan_id });
        }

        // Decode loan participants from metadata (borrower_id and lender_id)
        let metadata = transaction.metadata.as_bytes();
        let mut borrower_id = [0u8; 32];
        let mut lender_id = [0u8; 32];
        borrower_id.copy_from_slice(&metadata[0..32]);
        lender_id.copy_from_slice(&metadata[32..64]);

        // Verify both parties are KYC'd
        let borrower = self.users.get(&borrower_id)
            .ok_or(ProcessingError::UserNotFound { user_id: borrower_id })?;
        let lender = self.users.get(&lender_id)
            .ok_or(ProcessingError::UserNotFound { user_id: lender_id })?;

        if !borrower.is_kyc_approved() {
            return Err(ProcessingError::KycNotApproved { user_id: borrower_id });
        }
        if !lender.is_kyc_approved() {
            return Err(ProcessingError::KycNotApproved { user_id: lender_id });
        }

        // Create new loan
        let loan = Loan {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: transaction.amount,
            outstanding_balance: transaction.amount,
            interest_rate: 750, // Default 7.5% APR (750 basis points)
            term_months: 36,    // Default 3 year term
            loan_status: LoanStatus::Originated,
            originated_at: transaction.created_at,
            funded_at: 0,
            maturity_date: transaction.created_at + (36 * 30 * 24 * 60 * 60), // 36 months
        };

        self.loans.insert(loan_id, loan);
        Ok(())
    }

    /// State Transition 5: Loan Payment
    fn process_loan_payment(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let loan_id = transaction.loan_id;
        let payment_amount = transaction.amount;

        let loan = self.loans.get_mut(&loan_id)
            .ok_or(ProcessingError::LoanNotFound { loan_id })?;

        // Verify loan can accept payments
        if !loan.can_accept_payment() {
            return Err(ProcessingError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanPayment,
            });
        }

        // Apply payment
        if payment_amount >= loan.outstanding_balance {
            // Full payoff
            loan.outstanding_balance = 0;
            loan.loan_status = LoanStatus::PaidOff;
        } else {
            // Partial payment
            loan.outstanding_balance -= payment_amount;
            if loan.loan_status == LoanStatus::Originated {
                loan.loan_status = LoanStatus::Active;
                loan.funded_at = transaction.created_at;
            }
        }

        Ok(())
    }

    /// State Transition 6: Loan Closeout
    fn process_loan_closeout(&mut self, transaction: &LoanTransaction) -> Result<(), ProcessingError> {
        let loan_id = transaction.loan_id;

        let loan = self.loans.get_mut(&loan_id)
            .ok_or(ProcessingError::LoanNotFound { loan_id })?;

        // Verify loan can be closed
        match loan.loan_status {
            LoanStatus::PaidOff => {
                loan.loan_status = LoanStatus::Closed;
                Ok(())
            }
            _ => Err(ProcessingError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanCloseout,
            })
        }
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
    initial_state: LoanState,
    batch: &LoanBatch,
) -> Result<LoanState, ProcessingError> {
    let mut state = initial_state;
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
        TransactionType::UserRegistration | TransactionType::KycSubmission | TransactionType::KycVerification => {
            if transaction.loan_id != zero_loan_id {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_registration() {
        let mut state = LoanState::new();
        let user_id = [1u8; 32];
        
        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = 1; // Investor
        
        let transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000,
            metadata,
        };

        state.process_transaction(&transaction).unwrap();
        
        let user = state.get_user(&user_id).unwrap();
        assert_eq!(user.user_type, UserType::Investor);
        assert_eq!(user.kyc_status, KycStatus::NotSubmitted);
    }

    #[test]
    fn test_kyc_flow() {
        let mut state = LoanState::new();
        let user_id = [1u8; 32];

        // Register user first
        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = 2; // Lender

        let reg_tx = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000,
            metadata,
        };
        state.process_transaction(&reg_tx).unwrap();

        // Submit KYC
        let kyc_submit_tx = LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: 2000,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&kyc_submit_tx).unwrap();

        // Verify KYC
        let mut verify_metadata = MetadataArray::empty();
        verify_metadata.as_mut_bytes()[0] = 1; // Approved

        let kyc_verify_tx = LoanTransaction {
            transaction_id: [3u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 3000,
            metadata: verify_metadata,
        };
        state.process_transaction(&kyc_verify_tx).unwrap();

        let user = state.get_user(&user_id).unwrap();
        assert_eq!(user.kyc_status, KycStatus::Approved);
        assert!(user.is_kyc_approved());
    }
}
