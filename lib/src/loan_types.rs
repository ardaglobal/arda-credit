//! Legacy loan types module - DEPRECATED
//!
//! Most types have been moved to specialized modules:
//! - transactions.rs: TransactionType, LoanTransaction, LoanBatch
//! - user.rs: User, UserType, KycStatus
//! - loan.rs: Loan, LoanStatus
//!
//! This module now only contains helper functions and LoanTerms for backward compatibility.

use serde::{Deserialize, Serialize};

// ============================================================================
// LOAN TERMS - Still used by GlobalLoanState
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
    pub fn new(interest_rate: u32, term_months: u16) -> Self {
        Self {
            interest_rate,
            term_months,
        }
    }

    /// Calculate maturity date based on origination time and term
    pub fn calculate_maturity_date(&self, originated_at: u64) -> u64 {
        originated_at + (self.term_months as u64 * 30 * 24 * 60 * 60)
    }
}

// ============================================================================
// CONVERSION HELPERS (for API layer)
// ============================================================================

/// Hash a UUID to a 32-byte array for privacy
pub fn hash_uuid(uuid: &uuid::Uuid) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(uuid.as_bytes());
    hasher.finalize().into()
}

/// Convert decimal amount to u64 (cents)
pub fn decimal_to_cents(amount: rust_decimal::Decimal) -> u64 {
    use rust_decimal::prelude::ToPrimitive;
    (amount * rust_decimal::Decimal::from(100))
        .to_u64()
        .unwrap_or(0)
}

/// Convert u64 cents back to decimal
pub fn cents_to_decimal(cents: u64) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from(cents) / rust_decimal::Decimal::from(100)
}

// ============================================================================
// DEPRECATED TYPE RE-EXPORTS (for backward compatibility)
// ============================================================================
//
// These re-exports maintain compatibility with existing code.
// New code should import directly from the specific modules.

pub use crate::loan::{Loan, LoanStatus};
pub use crate::transactions::{
    InternalLoanBatchPublicValues, LoanBatch, LoanTransaction, MetadataArray, TransactionType,
};
pub use crate::user::{KycStatus, User, UserType};
