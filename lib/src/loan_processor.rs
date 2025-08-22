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
pub struct LoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,

    /// All loans in the system
    pub loans: HashMap<[u8; 32], Loan>,

    /// Configurable loan terms
    pub loan_terms: LoanTerms,

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
            loan_terms: LoanTerms::default(),
            transaction_count: 0,
            last_updated: 0,
        }
    }
}

impl LoanState {
    /// Create a new empty loan state with default loan terms
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new loan state with custom loan terms
    pub fn with_loan_terms(loan_terms: LoanTerms) -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
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
            TransactionType::UserRegistration => self.process_user_registration(transaction),
            TransactionType::KycSubmission => self.process_kyc_submission(transaction),
            TransactionType::KycVerification => self.process_kyc_verification(transaction),
            TransactionType::LoanOrigination => self.process_loan_origination(transaction),
            TransactionType::LoanPayment => self.process_loan_payment(transaction),
            TransactionType::LoanCloseout => self.process_loan_closeout(transaction),
        }?;

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

    /// State Transition 1: User Registration
    fn process_user_registration(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
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
    fn process_kyc_submission(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        let user_id = transaction.user_id;

        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(ProcessingError::UserNotFound { user_id })?;

        // Update KYC status to pending
        user.kyc_status = KycStatus::Pending;
        user.kyc_submitted_at = transaction.created_at;

        Ok(())
    }

    /// State Transition 3: KYC Verification (admin operation)
    fn process_kyc_verification(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        let user_id = transaction.user_id;

        let user = self
            .users
            .get_mut(&user_id)
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
    fn process_loan_origination(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
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
        let borrower = self
            .users
            .get(&borrower_id)
            .ok_or(ProcessingError::UserNotFound {
                user_id: borrower_id,
            })?;
        let lender = self
            .users
            .get(&lender_id)
            .ok_or(ProcessingError::UserNotFound { user_id: lender_id })?;

        if !borrower.is_kyc_approved() {
            return Err(ProcessingError::KycNotApproved {
                user_id: borrower_id,
            });
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
            interest_rate: self.loan_terms.interest_rate,
            term_months: self.loan_terms.term_months,
            loan_status: LoanStatus::Originated,
            originated_at: transaction.created_at,
            funded_at: 0,
            maturity_date: self
                .loan_terms
                .calculate_maturity_date(transaction.created_at),
        };

        self.loans.insert(loan_id, loan);
        Ok(())
    }

    /// State Transition 5: Loan Payment
    fn process_loan_payment(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        let loan_id = transaction.loan_id;
        let payment_amount = transaction.amount;

        let loan = self
            .loans
            .get_mut(&loan_id)
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
    fn process_loan_closeout(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        let loan_id = transaction.loan_id;

        let loan = self
            .loans
            .get_mut(&loan_id)
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
            }),
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

    // Validate timestamp bounds for production safety
    validate_timestamp_bounds(transaction.created_at)?;

    Ok(())
}

/// Validate a loan transaction with state context for temporal ordering
pub fn validate_transaction_with_state(
    transaction: &LoanTransaction,
    current_state: &LoanState,
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
            created_at: 1640995200, // January 1, 2022 - valid timestamp
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
            created_at: 1640995200, // January 1, 2022
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
            created_at: 1640995300, // 100 seconds later
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
            created_at: 1640995400, // 200 seconds after start
            metadata: verify_metadata,
        };
        state.process_transaction(&kyc_verify_tx).unwrap();

        let user = state.get_user(&user_id).unwrap();
        assert_eq!(user.kyc_status, KycStatus::Approved);
        assert!(user.is_kyc_approved());
    }

    #[test]
    fn test_configurable_loan_terms() {
        // Test with custom loan terms
        let custom_terms = LoanTerms::new(1200, 60); // 12% APR, 5 year term
        let mut state = LoanState::with_loan_terms(custom_terms.clone());

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [3u8; 32];

        // Register and KYC both users
        setup_kycd_users(&mut state, borrower_id, lender_id);

        // Create loan origination transaction
        let mut loan_metadata = MetadataArray::empty();
        loan_metadata.as_mut_bytes()[0..32].copy_from_slice(&borrower_id);
        loan_metadata.as_mut_bytes()[32..64].copy_from_slice(&lender_id);

        let loan_tx = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id: lender_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount: 10_000_00, // $10,000 in cents
            created_at: 5000,
            metadata: loan_metadata,
        };

        state.process_transaction(&loan_tx).unwrap();

        // Verify loan uses custom terms
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.interest_rate, 1200); // Custom 12% APR
        assert_eq!(loan.term_months, 60); // Custom 5 year term
        assert_eq!(loan.principal_amount, 10_000_00);
        assert_eq!(loan.outstanding_balance, 10_000_00);

        // Verify maturity date calculation
        let expected_maturity = 5000 + (60_u64 * 30 * 24 * 60 * 60); // 60 months
        assert_eq!(loan.maturity_date, expected_maturity);

        // Test with default loan terms
        let mut default_state = LoanState::new();
        assert_eq!(default_state.loan_terms.interest_rate, 750); // Default 7.5%
        assert_eq!(default_state.loan_terms.term_months, 36); // Default 3 years

        // Test updating loan terms on existing state
        let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 year term
        default_state.set_loan_terms(new_terms);
        assert_eq!(default_state.loan_terms.interest_rate, 500);
        assert_eq!(default_state.loan_terms.term_months, 24);
    }

    /// Helper function to set up KYC'd users for testing
    fn setup_kycd_users(state: &mut LoanState, borrower_id: [u8; 32], lender_id: [u8; 32]) {
        // Register borrower
        let mut borrower_metadata = MetadataArray::empty();
        borrower_metadata.as_mut_bytes()[0] = UserType::Investor as u8;
        let borrower_reg_tx = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000,
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
            created_at: 1100,
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
            created_at: 2000,
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
            created_at: 2100,
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
            created_at: 3000,
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
            created_at: 3100,
            metadata: lender_verify_metadata,
        };
        state.process_transaction(&lender_kyc_verify_tx).unwrap();
    }
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
        let mut state = LoanState::new();
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
}
