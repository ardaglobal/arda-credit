//! Loan management data structures for zkVM compatibility
//!
//! These types are designed to be serializable and work within the SP1 zkVM
//! while supporting the full loan lifecycle operations.

use serde::{Deserialize, Serialize};

// Custom metadata array using Vec for better serde compatibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataArray {
    data: Vec<u8>,
}

impl MetadataArray {
    /// Create empty metadata
    pub fn empty() -> Self {
        Self {
            data: vec![0u8; 64],
        }
    }

    /// Create metadata from bytes (truncates if too long, pads if too short)
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut data = vec![0u8; 64];
        let len = bytes.len().min(64);
        data[..len].copy_from_slice(&bytes[..len]);
        Self { data }
    }

    /// Get the inner byte array as a slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable access to the underlying data
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

// ============================================================================
// CORE ENUMS - zkVM compatible
// ============================================================================

/// User types in the loan system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum UserType {
    Investor = 1,
    Lender = 2,
}

/// KYC status for compliance tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KycStatus {
    NotSubmitted = 0,
    Pending = 1,
    Approved = 2,
    Rejected = 3,
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

/// Transaction types for the loan system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionType {
    UserRegistration = 1,
    KycSubmission = 2,
    KycVerification = 3,
    LoanOrigination = 4,
    LoanPayment = 5,
    LoanCloseout = 6,
}

// ============================================================================
// CORE STRUCTS - zkVM compatible (simple field types)
// ============================================================================

/// Configurable loan terms (zkVM compatible)
/// 
/// This struct allows you to configure loan terms such as interest rate and term length.
/// It provides sensible defaults but allows customization for different loan products.
/// 
/// # Examples
/// 
/// ```rust
/// use lib::{LoanTerms, LoanState};
/// 
/// // Use default terms (7.5% APR, 36 months)
/// let default_state = LoanState::new();
/// 
/// // Create custom terms (12% APR, 60 months)
/// let custom_terms = LoanTerms::new(1200, 60);
/// let custom_state = LoanState::with_loan_terms(custom_terms);
/// 
/// // Update terms on existing state
/// let mut state = LoanState::new();
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

/// User account (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub user_id: [u8; 32], // Hashed UUID for privacy
    pub user_type: UserType,
    pub kyc_status: KycStatus,
    pub kyc_submitted_at: u64, // Unix timestamp, 0 if not submitted
    pub kyc_verified_at: u64,  // Unix timestamp, 0 if not verified
    pub created_at: u64,       // Unix timestamp
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

/// Transaction for batch processing (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanTransaction {
    pub transaction_id: [u8; 32], // Hashed UUID
    pub user_id: [u8; 32],        // Transaction initiator
    pub loan_id: [u8; 32],        // All zeros for non-loan transactions
    pub transaction_type: TransactionType,
    pub amount: u64,     // 0 for non-monetary transactions
    pub created_at: u64, // Unix timestamp
    // Metadata encoded as fixed-size array for zkVM compatibility
    pub metadata: MetadataArray, // Flexible metadata field
}

/// Batch of mixed transaction types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanBatch {
    pub batch_id: [u8; 32],
    pub transactions: Vec<LoanTransaction>,
    pub previous_state_root: [u8; 32],
    pub created_at: u64,
}

// Note: LoanBatchPublicValues is generated by the sol! macro in lib.rs
// This is just a compatibility type for internal use
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalLoanBatchPublicValues {
    pub previous_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub transaction_count: u32,
    pub batch_timestamp: u64,
    // Hash of transaction types (for transparency without revealing details)
    pub transaction_types_hash: [u8; 32],
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

impl User {
    /// Check if user is KYC approved
    pub const fn is_kyc_approved(&self) -> bool {
        matches!(self.kyc_status, KycStatus::Approved)
    }
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

impl TransactionType {
    /// Check if transaction type requires KYC
    pub const fn requires_kyc(&self) -> bool {
        matches!(
            self,
            Self::LoanOrigination | Self::LoanPayment | Self::LoanCloseout
        )
    }

    /// Check if transaction type is monetary
    pub const fn is_monetary(&self) -> bool {
        matches!(self, Self::LoanOrigination | Self::LoanPayment)
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
