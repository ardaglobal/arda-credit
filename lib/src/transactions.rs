//! Transaction types and related data structures for the deal system
//!
//! This module consolidates all transaction-related types and definitions
//! for better maintainability and separation of concerns.
//!
//! ## State Transactions vs Financial Transactions
//!
//! **State Transactions (this module):**
//! - Zero-knowledge state transition functions
//! - Stored in `state_transactions` table (consolidated from `deal_transactions` and `incoming_transactions`)
//! - Batched and proven via zkVM
//! - Posted to blockchain for verification
//!
//! **Financial Transactions (separate):**
//! - Actual money movement operations
//! - **Deposits**: User deposits into the system (`account_financial_transactions` with type DEPOSIT)
//! - **Withdrawals**: User withdrawals from the system (`account_financial_transactions` with type WITHDRAWAL)
//! - **Payments**: Money movement between accounts (`deal_payments` table)
//!
//! ## Recent Changes
//! - **Transaction Table Consolidation**: Unified `deal_transactions` and `incoming_transactions` into `state_transactions`
//! - **Terminology Update**: Converted from loan-based to deal-based terminology:
//!   - LoanOrigination → DealOrigination
//!   - LoanPayment → DealPayment
//!   - LoanCloseout → DealCloseout
//! - **Financial Clarity**: Distinguished state transitions from financial transactions (deposits/withdrawals/payments)
//!
//! Legacy loan terminology is supported in FromStr for backward compatibility
//! with existing ZK proof data.

use serde::{Deserialize, Serialize};

// Custom metadata array using Vec for better serde compatibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataArray {
    data: Vec<u8>,
}

impl MetadataArray {
    /// Create empty metadata
    #[must_use]
    pub fn empty() -> Self {
        Self {
            data: vec![0u8; 64],
        }
    }

    /// Create metadata from bytes (truncates if too long, pads if too short)
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut data = vec![0u8; 64];
        let len = bytes.len().min(64);
        data[..len].copy_from_slice(&bytes[..len]);
        Self { data }
    }

    /// Get the inner byte array as a slice
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable access to the underlying data
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// Transaction types for the deal system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionType {
    UserRegistration = 1,
    KycSubmission = 2,
    KycVerification = 3,
    DealOrigination = 4, // was LoanOrigination
    DealPayment = 5,     // was LoanPayment
    DealCloseout = 6,    // was LoanCloseout
    UserDeposit = 7,
    UserWithdrawal = 8,
    // New pool operations
    InvestorRegistration = 9,
    // Admin operations
    AdminRegistration = 11,
    VerifyDocument = 19,
    // Collateral operations
    CollateralCreation = 20,
    CollateralUpdate = 21,
    CollateralDealAssociation = 22,
    CollateralDealDisassociation = 23,
    CollateralDetectionRequest = 24,
    CollateralDetectionUpdate = 25,
    // State management operations
    StateRegenesis = 30,
    // Company operations
    CompanyRegistration = 31,
    CompanyUpdate = 32,
}

impl TransactionType {
    /// Check if transaction type requires KYC
    #[must_use]
    pub const fn requires_kyc(&self) -> bool {
        matches!(
            self,
            Self::DealOrigination     // was LoanOrigination
                | Self::DealPayment   // was LoanPayment
                | Self::DealCloseout  // was LoanCloseout
                | Self::UserWithdrawal
                | Self::VerifyDocument
        )
    }

    /// Check if transaction type is monetary
    #[must_use]
    pub const fn is_monetary(&self) -> bool {
        matches!(
            self,
            Self::DealOrigination | Self::DealPayment | Self::UserDeposit | Self::UserWithdrawal
        )
    }

    /// Check if transaction type requires investor role
    #[must_use]
    pub const fn requires_investor_role(&self) -> bool {
        matches!(self, Self::InvestorRegistration)
    }
}

/// Transaction for batch processing (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealTransaction {
    pub transaction_id: [u8; 32],        // Hashed UUID
    pub user_id: [u8; 32],               // User who initiated the transaction
    pub originator_company_id: [u8; 32], // Originator company (or company for deposits/withdrawals)
    pub investor_company_id: [u8; 32],   // Investor company (or zeros for non-deal transactions)
    pub deal_id: [u8; 32],               // Deal ID (or zeros for non-deal transactions)
    pub transaction_type: TransactionType,
    pub amount: u64,     // 0 for non-monetary transactions
    pub created_at: u64, // Unix timestamp
    // Metadata encoded as fixed-size array for zkVM compatibility
    pub metadata: MetadataArray, // Flexible metadata field
}

impl DealTransaction {
    /// Create a new transaction with minimal fields (company IDs default to zero)
    /// This is a helper for tests and legacy code that don't use company IDs yet
    #[must_use]
    pub const fn new_simple(
        transaction_id: [u8; 32],
        user_id: [u8; 32],
        deal_id: [u8; 32],
        transaction_type: TransactionType,
        amount: u64,
        created_at: u64,
        metadata: MetadataArray,
    ) -> Self {
        Self {
            transaction_id,
            user_id,
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id,
            transaction_type,
            amount,
            created_at,
            metadata,
        }
    }
}

/// Batch of mixed transaction types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealBatch {
    pub batch_id: [u8; 32],
    pub transactions: Vec<DealTransaction>,
    pub previous_state_root: [u8; 32],
    pub created_at: u64,
}

// Note: DealBatchPublicValues is generated by the sol! macro in lib.rs
// This is just a compatibility type for internal use
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDealBatchPublicValues {
    pub previous_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub transaction_count: u32,
    pub batch_timestamp: u64,
    // Hash of transaction types (for transparency without revealing details)
    pub transaction_types_hash: [u8; 32],
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionType::UserRegistration => write!(f, "user_registration"),
            TransactionType::KycSubmission => write!(f, "kyc_submission"),
            TransactionType::KycVerification => write!(f, "kyc_verification"),
            TransactionType::DealOrigination => write!(f, "deal_origination"), // was loan_origination
            TransactionType::DealPayment => write!(f, "deal_payment"),         // was loan_payment
            TransactionType::DealCloseout => write!(f, "deal_closeout"),       // was loan_closeout
            TransactionType::UserDeposit => write!(f, "user_deposit"),
            TransactionType::UserWithdrawal => write!(f, "user_withdrawal"),
            TransactionType::InvestorRegistration => write!(f, "investor_registration"),
            TransactionType::AdminRegistration => write!(f, "admin_registration"),
            TransactionType::VerifyDocument => write!(f, "verify_document"),
            TransactionType::CollateralCreation => write!(f, "collateral_creation"),
            TransactionType::CollateralUpdate => write!(f, "collateral_update"),
            TransactionType::CollateralDealAssociation => write!(f, "collateral_deal_association"),
            TransactionType::CollateralDealDisassociation => {
                write!(f, "collateral_deal_disassociation")
            }
            TransactionType::CollateralDetectionRequest => {
                write!(f, "collateral_detection_request")
            }
            TransactionType::CollateralDetectionUpdate => write!(f, "collateral_detection_update"),
            TransactionType::StateRegenesis => write!(f, "state_regenesis"),
            TransactionType::CompanyRegistration => write!(f, "company_registration"),
            TransactionType::CompanyUpdate => write!(f, "company_update"),
        }
    }
}

impl std::str::FromStr for TransactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user_registration" => Ok(TransactionType::UserRegistration),
            "kyc_submission" => Ok(TransactionType::KycSubmission),
            "kyc_verification" => Ok(TransactionType::KycVerification),
            "deal_origination" => Ok(TransactionType::DealOrigination), // was loan_origination
            "deal_payment" => Ok(TransactionType::DealPayment),         // was loan_payment
            "deal_closeout" => Ok(TransactionType::DealCloseout),       // was loan_closeout
            // Support legacy names for backward compatibility in ZK proofs
            "loan_origination" => Ok(TransactionType::DealOrigination), // legacy support
            "loan_payment" => Ok(TransactionType::DealPayment),         // legacy support
            "loan_closeout" => Ok(TransactionType::DealCloseout),       // legacy support
            "user_deposit" => Ok(TransactionType::UserDeposit),
            "user_withdrawal" => Ok(TransactionType::UserWithdrawal),
            "investor_registration" => Ok(TransactionType::InvestorRegistration),
            "admin_registration" => Ok(TransactionType::AdminRegistration),
            "verify_document" => Ok(TransactionType::VerifyDocument),
            "collateral_creation" => Ok(TransactionType::CollateralCreation),
            "collateral_update" => Ok(TransactionType::CollateralUpdate),
            "collateral_deal_association" => Ok(TransactionType::CollateralDealAssociation),
            "collateral_deal_disassociation" => Ok(TransactionType::CollateralDealDisassociation),
            "collateral_detection_request" => Ok(TransactionType::CollateralDetectionRequest),
            "collateral_detection_update" => Ok(TransactionType::CollateralDetectionUpdate),
            "state_regenesis" => Ok(TransactionType::StateRegenesis),
            "company_registration" => Ok(TransactionType::CompanyRegistration),
            "company_update" => Ok(TransactionType::CompanyUpdate),
            _ => Err(format!("Unknown transaction type: {}", s)),
        }
    }
}

/// Get all deal-related transaction types
#[must_use]
pub fn deal_transaction_types() -> Vec<TransactionType> {
    vec![
        TransactionType::DealOrigination, // was LoanOrigination
        TransactionType::DealPayment,     // was LoanPayment
        TransactionType::DealCloseout,    // was LoanCloseout
    ]
}

/// Check if transaction affects deal state
#[must_use]
pub fn affects_deal_state(transaction_type: TransactionType) -> bool {
    matches!(
        transaction_type,
        TransactionType::DealOrigination
            | TransactionType::DealPayment
            | TransactionType::DealCloseout
    )
}

/// Check if transaction type is deal-related
#[must_use]
pub fn is_deal_transaction(transaction_type: TransactionType) -> bool {
    matches!(
        transaction_type,
        TransactionType::DealOrigination        // was LoanOrigination
        | TransactionType::DealPayment          // was LoanPayment
        | TransactionType::DealCloseout // was LoanCloseout
    )
}

// Backward compatibility type aliases
// These will be removed in a future phase after all references are updated
pub type LoanTransaction = DealTransaction;
pub type LoanBatch = DealBatch;
pub type InternalLoanBatchPublicValues = InternalDealBatchPublicValues;
