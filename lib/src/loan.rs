//! Loan management logic for zkVM execution
//!
//! This module contains loan-related types and state transition logic
//! including loan origination, payment processing, and closeout operations.

use crate::transactions::{LoanTransaction, TransactionType};
use crate::user::{User, UserError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Loan status throughout lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LoanStatus {
    Originated = 1, // Created, not funded
    Active = 2,     // Funded and active
    PaidOff = 3,    // Fully repaid
    Closed = 4,     // Closed/settled
    Defaulted = 5,  // In default
}

/// Loan record (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loan {
    pub loan_id: [u8; 32],        // Hashed UUID
    pub borrower_id: [u8; 32],    // Hashed user ID
    pub lender_id: [u8; 32],      // Hashed user ID
    pub principal_amount: u64,    // Amount in smallest currency unit (cents)
    pub outstanding_balance: u64, // Current balance owed
    pub interest_rate: u32,       // Rate in basis points (7.5% = 750)
    pub term_months: u16,         // Loan term
    pub loan_status: LoanStatus,
    pub originated_at: u64, // Unix timestamp
    pub funded_at: u64,     // Unix timestamp, 0 if not funded
    pub maturity_date: u64, // Unix timestamp
}

impl Loan {
    /// Check if loan can accept payments
    pub const fn can_accept_payment(&self) -> bool {
        matches!(
            self.loan_status,
            LoanStatus::Active | LoanStatus::Originated
        )
    }

    /// Check if loan is fully paid off
    pub const fn is_paid_off(&self) -> bool {
        self.outstanding_balance == 0
    }
}

/// Errors specific to loan operations
#[derive(Error, Debug, Clone)]
pub enum LoanError {
    #[error("Loan not found: {loan_id:?}")]
    LoanNotFound { loan_id: [u8; 32] },

    #[error("Invalid loan state transition: {current_status:?} -> {requested_operation:?}")]
    InvalidLoanStateTransition {
        current_status: LoanStatus,
        requested_operation: TransactionType,
    },

    #[error("Insufficient loan balance: requested {amount}, available {balance}")]
    InsufficientLoanBalance { amount: u64, balance: u64 },

    #[error("Duplicate loan: {loan_id:?}")]
    DuplicateLoan { loan_id: [u8; 32] },

    #[error("Invalid transaction type for loan operation")]
    InvalidTransactionType,

    #[error("User error: {0}")]
    UserError(#[from] UserError),
}

/// Loan state management
pub struct LoanState {
    pub loans: HashMap<[u8; 32], Loan>,
}

impl LoanState {
    /// Create a new empty loan state
    pub fn new() -> Self {
        Self {
            loans: HashMap::new(),
        }
    }

    /// Process loan origination transaction
    pub fn process_loan_origination(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanOrigination {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        let _initiator_id = transaction.user_id;

        // Check for duplicate loan
        if self.loans.contains_key(&loan_id) {
            return Err(LoanError::DuplicateLoan { loan_id });
        }

        // Decode loan participants from metadata (borrower_id and lender_id)
        let metadata = transaction.metadata.as_bytes();
        let mut borrower_id = [0u8; 32];
        let mut lender_id = [0u8; 32];
        borrower_id.copy_from_slice(&metadata[0..32]);
        lender_id.copy_from_slice(&metadata[32..64]);

        // Verify both parties are KYC'd
        let borrower = users
            .get(&borrower_id)
            .ok_or(UserError::UserNotFound {
                user_id: borrower_id,
            })?;
        let lender = users
            .get(&lender_id)
            .ok_or(UserError::UserNotFound { user_id: lender_id })?;

        if !borrower.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved {
                user_id: borrower_id,
            }));
        }
        if !lender.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved { user_id: lender_id }));
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

    /// Process loan payment transaction
    pub fn process_loan_payment(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanPayment {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        let payer_id = transaction.user_id;

        // Verify payer is KYC approved
        let payer = users
            .get(&payer_id)
            .ok_or(UserError::UserNotFound { user_id: payer_id })?;
        if !payer.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved { user_id: payer_id }));
        }

        let loan = self
            .loans
            .get_mut(&loan_id)
            .ok_or(LoanError::LoanNotFound { loan_id })?;

        // Verify loan can accept payments
        if !loan.can_accept_payment() {
            return Err(LoanError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanPayment,
            });
        }

        let payment_amount = transaction.amount;

        // Check if payment exceeds outstanding balance
        if payment_amount > loan.outstanding_balance {
            return Err(LoanError::InsufficientLoanBalance {
                amount: payment_amount,
                balance: loan.outstanding_balance,
            });
        }

        // Apply payment
        loan.outstanding_balance -= payment_amount;

        // Update loan status based on remaining balance
        if loan.outstanding_balance == 0 {
            loan.loan_status = LoanStatus::PaidOff;
        } else if loan.loan_status == LoanStatus::Originated {
            // First payment activates the loan
            loan.loan_status = LoanStatus::Active;
            loan.funded_at = transaction.created_at;
        }

        Ok(())
    }

    /// Process loan closeout transaction
    pub fn process_loan_closeout(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanCloseout {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        let closer_id = transaction.user_id;

        // Verify closer is KYC approved
        let closer = users
            .get(&closer_id)
            .ok_or(UserError::UserNotFound { user_id: closer_id })?;
        if !closer.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved { user_id: closer_id }));
        }

        let loan = self
            .loans
            .get_mut(&loan_id)
            .ok_or(LoanError::LoanNotFound { loan_id })?;

        // Only allow closeout of paid off loans
        match loan.loan_status {
            LoanStatus::PaidOff => {
                loan.loan_status = LoanStatus::Closed;
                Ok(())
            }
            _ => Err(LoanError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanCloseout,
            }),
        }
    }

    /// Get loan by ID (read-only)
    pub fn get_loan(&self, loan_id: &[u8; 32]) -> Option<&Loan> {
        self.loans.get(loan_id)
    }
}

impl Default for LoanState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;
    use crate::user::{UserType, KycStatus};

    fn setup_test_users() -> (HashMap<[u8; 32], User>, [u8; 32], [u8; 32]) {
        let mut users = HashMap::new();
        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        let borrower = User {
            user_id: borrower_id,
            user_type: UserType::Investor,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1234567890,
            kyc_verified_at: 1234567900,
            created_at: 1234567880,
        };

        let lender = User {
            user_id: lender_id,
            user_type: UserType::Lender,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1234567890,
            kyc_verified_at: 1234567900,
            created_at: 1234567880,
        };

        users.insert(borrower_id, borrower);
        users.insert(lender_id, lender);

        (users, borrower_id, lender_id)
    }

    fn create_loan_origination_transaction(
        loan_id: [u8; 32],
        user_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&borrower_id);
        metadata[32..64].copy_from_slice(&lender_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount,
            created_at: 1234567920,
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_loan_origination() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        let transaction = create_loan_origination_transaction(
            loan_id,
            borrower_id, // Borrower initiates
            borrower_id,
            lender_id,
            10000, // $100.00
        );

        let result = loan_state.process_loan_origination(&transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.principal_amount, 10000);
        assert_eq!(loan.outstanding_balance, 10000);
        assert_eq!(loan.loan_status, LoanStatus::Originated);
    }

    #[test]
    fn test_loan_payment() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // First originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        loan_state.process_loan_origination(&origination, &users).unwrap();

        // Then make payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 5000, // $50.00 payment
            created_at: 1234567930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 5000);
        assert_eq!(loan.loan_status, LoanStatus::Active); // First payment activates loan
    }

    #[test]
    fn test_loan_full_payoff() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        loan_state.process_loan_origination(&origination, &users).unwrap();

        // Full payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 10000, // Full payment
            created_at: 1234567930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 0);
        assert_eq!(loan.loan_status, LoanStatus::PaidOff);
        assert!(loan.is_paid_off());
    }

    #[test]
    fn test_loan_closeout() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Setup: Originate and fully pay off loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        loan_state.process_loan_origination(&origination, &users).unwrap();

        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 10000, // Full payment
            created_at: 1234567930,
            metadata: MetadataArray::empty(),
        };
        loan_state.process_loan_payment(&payment_transaction, &users).unwrap();

        // Now closeout
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1234567940,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_closeout(&closeout_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Closed);
    }

    #[test]
    fn test_loan_closeout_without_payoff_fails() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Setup: Originate loan but don't pay it off
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        loan_state.process_loan_origination(&origination, &users).unwrap();

        // Try to closeout without paying off
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1234567940,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_closeout(&closeout_transaction, &users);
        assert!(matches!(result, Err(LoanError::InvalidLoanStateTransition { .. })));
    }

    #[test]
    fn test_overpayment_fails() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        loan_state.process_loan_origination(&origination, &users).unwrap();

        // Try to overpay
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 15000, // More than outstanding balance
            created_at: 1234567930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(matches!(result, Err(LoanError::InsufficientLoanBalance { .. })));
    }
}