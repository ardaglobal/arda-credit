//! Deal management logic for zkVM execution
//!
//! This module contains deal-related types and state transition logic
//! including deal origination, payment processing, and closeout operations.
//!
//! ## Recent Changes
//! Converted from loan-based terminology to deal-based terminology:
//! - Loan → Deal
//! - Borrower → Originator
//! - Lender → Investor
//! - LoanOrigination → DealOrigination
//! - All processing functions updated accordingly

use crate::transactions::{LoanTransaction, TransactionType};
use crate::user::{User, UserError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// LOAN TERMS - Configurable loan parameters
// ============================================================================

/// Configurable loan terms (zkVM compatible)
///
/// This struct allows you to configure loan terms such as interest rate and term length.
/// It provides sensible defaults but allows customization for different loan products.
///
/// # Examples
///
/// ```rust
/// use lib::{LoanTerms, GlobalLoanState};
///
/// // Use default terms (7.5% APR, 36 months)
/// let default_state = GlobalLoanState::new();
///
/// // Create custom terms (12% APR, 60 months)
/// let custom_terms = LoanTerms::new(1200, 60);
/// let custom_state = GlobalLoanState::with_loan_terms(custom_terms);
///
/// // Update terms on existing state
/// let mut state = GlobalLoanState::new();
/// let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 years
/// state.set_loan_terms(new_terms);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanTerms {
    /// Interest rate in basis points (e.g., 750 = 7.5% APR)
    pub interest_rate: u32,
    /// Loan term in months (e.g., 36 = 3 years)
    pub term_months: u16,
}

impl Default for LoanTerms {
    fn default() -> Self {
        Self {
            interest_rate: 750, // Default 7.5% APR (750 basis points)
            term_months: 36,    // Default 3 year term
        }
    }
}

impl LoanTerms {
    /// Create new loan terms with custom values
    #[must_use]
    pub const fn new(interest_rate: u32, term_months: u16) -> Self {
        Self {
            interest_rate,
            term_months,
        }
    }

    /// Calculate maturity date based on origination time and term
    #[must_use]
    pub const fn calculate_maturity_date(&self, originated_at: u64) -> u64 {
        originated_at + (self.term_months as u64 * 30 * 24 * 60 * 60)
    }
}

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

/// Deal record (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deal {
    pub deal_id: [u8; 32],        // Hashed UUID
    pub originator_id: [u8; 32],  // Hashed user ID (was borrower_id)
    pub investor_id: [u8; 32],    // Hashed user ID (was lender_id)
    pub principal_amount: u64,    // Amount in smallest currency unit (cents)
    pub outstanding_balance: u64, // Current balance owed
    pub interest_rate: u32,       // Rate in basis points (7.5% = 750)
    pub term_months: u16,         // Deal term
    pub deal_status: LoanStatus,  // Keeping LoanStatus for compatibility
    pub originated_at: u64,       // Unix timestamp
    pub funded_at: u64,           // Unix timestamp, 0 if not funded
    pub maturity_date: u64,       // Unix timestamp
}

impl Deal {
    /// Check if deal can accept payments
    #[must_use]
    pub const fn can_accept_payment(&self) -> bool {
        matches!(
            self.deal_status,
            LoanStatus::Active | LoanStatus::Originated
        )
    }

    /// Check if deal is fully paid off
    #[must_use]
    pub const fn is_paid_off(&self) -> bool {
        self.outstanding_balance == 0
    }
}

/// Errors specific to deal operations
#[derive(Error, Debug, Clone)]
pub enum DealError {
    #[error("Deal not found: {deal_id:?}")]
    DealNotFound { deal_id: [u8; 32] },

    #[error("Invalid deal state transition: {current_status:?} -> {requested_operation:?}")]
    InvalidDealStateTransition {
        current_status: LoanStatus,
        requested_operation: TransactionType,
    },

    #[error("Insufficient deal balance: requested {amount}, available {balance}")]
    InsufficientDealBalance { amount: u64, balance: u64 },

    #[error("Duplicate deal: {deal_id:?}")]
    DuplicateDeal { deal_id: [u8; 32] },

    #[error("Invalid transaction type for deal operation")]
    InvalidTransactionType,

    #[error("User error: {0}")]
    UserError(#[from] UserError),
}

/// Deal state management
pub struct DealState {
    pub deals: HashMap<[u8; 32], Deal>,
}

impl DealState {
    /// Create a new empty deal state
    #[must_use]
    pub fn new() -> Self {
        Self {
            deals: HashMap::new(),
        }
    }

    /// Process deal origination transaction
    ///
    /// # Errors
    /// Returns various `DealError` variants for invalid transaction type, duplicate deals, invalid users, or KYC failures
    pub fn process_deal_origination(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
        loan_terms: &LoanTerms,
    ) -> Result<(), DealError> {
        if transaction.transaction_type != TransactionType::DealOrigination {
            return Err(DealError::InvalidTransactionType);
        }

        let deal_id = transaction.deal_id;

        // Check for duplicate deal
        if self.deals.contains_key(&deal_id) {
            return Err(DealError::DuplicateDeal { deal_id });
        }

        // Extract company IDs from explicit fields (NOT metadata)
        let originator_company_id = transaction.originator_company_id;
        let investor_company_id = transaction.investor_company_id;

        // Verify initiating user is KYC'd (user who created the deal on behalf of company)
        let initiating_user_id = transaction.user_id;
        if initiating_user_id != [0u8; 32] {
            // Skip if system transaction
            let initiator = users
                .get(&initiating_user_id)
                .ok_or(UserError::UserNotFound {
                    user_id: initiating_user_id,
                })?;

            if !initiator.is_kyc_approved() {
                return Err(DealError::UserError(UserError::KycNotApproved {
                    user_id: initiating_user_id,
                }));
            }
        }

        // NOTE: Company validation (companies exist) is done in deal_processor.rs
        // before calling this function. We just use the company IDs here.

        // Create new deal between companies
        let deal = Deal {
            deal_id,
            originator_id: originator_company_id, // Now company ID, not user ID
            investor_id: investor_company_id,     // Now company ID, not user ID
            principal_amount: transaction.amount,
            outstanding_balance: transaction.amount,
            interest_rate: loan_terms.interest_rate,
            term_months: loan_terms.term_months,
            deal_status: LoanStatus::Originated,
            originated_at: transaction.created_at,
            funded_at: 0,
            maturity_date: loan_terms.calculate_maturity_date(transaction.created_at),
        };

        self.deals.insert(deal_id, deal);
        Ok(())
    }

    /// Process deal payment transaction
    ///
    /// # Errors
    /// Returns various `DealError` variants for invalid transaction type, deal not found, or payment validation failures
    pub fn process_deal_payment(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), DealError> {
        if transaction.transaction_type != TransactionType::DealPayment {
            return Err(DealError::InvalidTransactionType);
        }

        let deal_id = transaction.deal_id; // was loan_id, now deal_id
        let payer_id = transaction.user_id;

        // Verify payer is KYC approved
        let payer = users
            .get(&payer_id)
            .ok_or(UserError::UserNotFound { user_id: payer_id })?;
        if !payer.is_kyc_approved() {
            return Err(DealError::UserError(UserError::KycNotApproved {
                user_id: payer_id,
            }));
        }

        let deal = self
            .deals
            .get_mut(&deal_id)
            .ok_or(DealError::DealNotFound { deal_id })?;

        // Verify deal can accept payments
        if !deal.can_accept_payment() {
            return Err(DealError::InvalidDealStateTransition {
                current_status: deal.deal_status,
                requested_operation: TransactionType::DealPayment,
            });
        }

        let payment_amount = transaction.amount;

        // Check if payment exceeds outstanding balance
        if payment_amount > deal.outstanding_balance {
            return Err(DealError::InsufficientDealBalance {
                amount: payment_amount,
                balance: deal.outstanding_balance,
            });
        }

        // Apply payment
        deal.outstanding_balance -= payment_amount;

        // Update deal status based on remaining balance
        if deal.outstanding_balance == 0 {
            deal.deal_status = LoanStatus::PaidOff;
        } else if deal.deal_status == LoanStatus::Originated {
            // First payment activates the deal
            deal.deal_status = LoanStatus::Active;
            deal.funded_at = transaction.created_at;
        }

        Ok(())
    }

    /// Process deal closeout transaction
    ///
    /// # Errors
    /// Returns various `DealError` variants for invalid transaction type, deal not found, or closeout validation failures
    pub fn process_deal_closeout(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), DealError> {
        if transaction.transaction_type != TransactionType::DealCloseout {
            return Err(DealError::InvalidTransactionType);
        }

        let deal_id = transaction.deal_id; // was loan_id, now deal_id
        let closer_id = transaction.user_id;

        // Verify closer is KYC approved
        let closer = users
            .get(&closer_id)
            .ok_or(UserError::UserNotFound { user_id: closer_id })?;
        if !closer.is_kyc_approved() {
            return Err(DealError::UserError(UserError::KycNotApproved {
                user_id: closer_id,
            }));
        }

        let deal = self
            .deals
            .get_mut(&deal_id)
            .ok_or(DealError::DealNotFound { deal_id })?;

        // Only allow closeout of paid off deals
        match deal.deal_status {
            LoanStatus::PaidOff => {
                deal.deal_status = LoanStatus::Closed;
                Ok(())
            }
            _ => Err(DealError::InvalidDealStateTransition {
                current_status: deal.deal_status,
                requested_operation: TransactionType::DealCloseout,
            }),
        }
    }

    /// Get deal by ID (read-only)
    #[must_use]
    pub fn get_deal(&self, deal_id: &[u8; 32]) -> Option<&Deal> {
        self.deals.get(deal_id)
    }
}

impl Default for DealState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn setup_test_users() -> (HashMap<[u8; 32], User>, [u8; 32], [u8; 32]) {
        let mut users = HashMap::new();
        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];

        let originator = User {
            user_id: originator_id,
            user_type: UserType::Investor,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1_234_567_890,
            kyc_verified_at: 1_234_567_900,
            created_at: 1_234_567_880,
        };

        let investor = User {
            user_id: investor_id,
            user_type: UserType::Originator,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1_234_567_890,
            kyc_verified_at: 1_234_567_900,
            created_at: 1_234_567_880,
        };

        users.insert(originator_id, originator);
        users.insert(investor_id, investor);

        (users, originator_id, investor_id)
    }

    fn create_deal_origination_transaction(
        deal_id: [u8; 32],
        user_id: [u8; 32],
        originator_id: [u8; 32],
        investor_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&originator_id);
        metadata[32..64].copy_from_slice(&investor_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id,
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id, // was loan_id, now deal_id field
            transaction_type: TransactionType::DealOrigination, // was LoanOrigination
            amount,
            created_at: 1_234_567_920,
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_deal_origination() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        let transaction = create_deal_origination_transaction(
            deal_id,
            originator_id, // Originator initiates
            originator_id,
            investor_id,
            10000, // $100.00
        );

        let loan_terms = LoanTerms::default();
        let result = deal_state.process_deal_origination(&transaction, &users, &loan_terms);
        assert!(result.is_ok());

        let deal = deal_state.get_deal(&deal_id).unwrap();
        assert_eq!(deal.principal_amount, 10000);
        assert_eq!(deal.outstanding_balance, 10000);
        assert_eq!(deal.deal_status, LoanStatus::Originated);
    }

    #[test]
    fn test_deal_payment() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        // First originate deal
        let origination = create_deal_origination_transaction(
            deal_id,
            originator_id,
            originator_id,
            investor_id,
            10000,
        );
        let loan_terms = LoanTerms::default(); // Keep LoanTerms for compatibility
        deal_state
            .process_deal_origination(&origination, &users, &loan_terms) // was process_loan_origination
            .unwrap();

        // Then make payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                        // was loan_id, now deal_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 5000,                                   // $50.00 payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = deal_state.process_deal_payment(&payment_transaction, &users); // was process_loan_payment
        assert!(result.is_ok());

        let deal = deal_state.get_deal(&deal_id).unwrap(); // was get_loan
        assert_eq!(deal.outstanding_balance, 5000);
        assert_eq!(deal.deal_status, LoanStatus::Active); // First payment activates deal (keep loan_status field)
    }

    #[test]
    fn test_deal_full_payoff() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        // Originate deal
        let origination = create_deal_origination_transaction(
            deal_id,
            originator_id,
            originator_id,
            investor_id,
            10000,
        );
        let loan_terms = LoanTerms::default(); // Keep LoanTerms for compatibility
        deal_state
            .process_deal_origination(&origination, &users, &loan_terms) // was process_loan_origination
            .unwrap();

        // Full payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                        // was loan_id, now deal_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 10000,                                  // Full payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = deal_state.process_deal_payment(&payment_transaction, &users); // was process_loan_payment
        assert!(result.is_ok());

        let deal = deal_state.get_deal(&deal_id).unwrap(); // was get_loan
        assert_eq!(deal.outstanding_balance, 0);
        assert_eq!(deal.deal_status, LoanStatus::PaidOff); // Keep deal_status field for compatibility
        assert!(deal.is_paid_off());
    }

    #[test]
    fn test_deal_closeout() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        // Setup: Originate and fully pay off deal
        let origination = create_deal_origination_transaction(
            deal_id,
            originator_id,
            originator_id,
            investor_id,
            10000,
        );
        let loan_terms = LoanTerms::default(); // Keep LoanTerms for compatibility
        deal_state
            .process_deal_origination(&origination, &users, &loan_terms) // was process_loan_origination
            .unwrap();

        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                        // was loan_id, now deal_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 10000,                                  // Full payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };
        deal_state
            .process_deal_payment(&payment_transaction, &users) // was process_loan_payment
            .unwrap();

        // Now closeout
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                         // was loan_id, now deal_id field
            transaction_type: TransactionType::DealCloseout, // was LoanCloseout
            amount: 0,
            created_at: 1_234_567_940,
            metadata: MetadataArray::empty(),
        };

        let result = deal_state.process_deal_closeout(&closeout_transaction, &users); // was process_loan_closeout
        assert!(result.is_ok());

        let deal = deal_state.get_deal(&deal_id).unwrap(); // was get_loan
        assert_eq!(deal.deal_status, LoanStatus::Closed); // Keep deal_status field for compatibility
    }

    #[test]
    fn test_deal_closeout_without_payoff_fails() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        // Setup: Originate deal but don't pay it off
        let origination = create_deal_origination_transaction(
            deal_id,
            originator_id,
            originator_id,
            investor_id,
            10000,
        );
        let loan_terms = LoanTerms::default(); // Keep LoanTerms for compatibility
        deal_state
            .process_deal_origination(&origination, &users, &loan_terms) // was process_loan_origination
            .unwrap();

        // Try to closeout without paying off
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                         // was loan_id, now deal_id field
            transaction_type: TransactionType::DealCloseout, // was LoanCloseout
            amount: 0,
            created_at: 1_234_567_940,
            metadata: MetadataArray::empty(),
        };

        let result = deal_state.process_deal_closeout(&closeout_transaction, &users); // was process_loan_closeout
        assert!(matches!(
            result,
            Err(DealError::InvalidDealStateTransition { .. }) // was LoanError::InvalidLoanStateTransition
        ));
    }

    #[test]
    fn test_overpayment_fails() {
        let mut deal_state = DealState::new();
        let (users, originator_id, investor_id) = setup_test_users();
        let deal_id = [10u8; 32];

        // Originate deal
        let origination = create_deal_origination_transaction(
            deal_id,
            originator_id,
            originator_id,
            investor_id,
            10000,
        );
        let loan_terms = LoanTerms::default(); // Keep LoanTerms for compatibility
        deal_state
            .process_deal_origination(&origination, &users, &loan_terms) // was process_loan_origination
            .unwrap();

        // Try to overpay
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id, // was borrower_id
            originator_company_id: originator_id,
            investor_company_id: investor_id,
            deal_id,                                        // was loan_id, now deal_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 15000,                                  // More than outstanding balance
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = deal_state.process_deal_payment(&payment_transaction, &users); // was process_loan_payment
        assert!(matches!(
            result,
            Err(DealError::InsufficientDealBalance { .. }) // was LoanError::InsufficientLoanBalance
        ));
    }
}
