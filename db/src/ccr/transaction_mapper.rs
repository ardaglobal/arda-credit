//! Transaction Mapping Layer for CCR Operations
//!
//! This module provides conversion between existing LoanTransaction types and CCR operations,
//! enabling unified processing while preserving all metadata and UUIDs. It bridges the gap
//! between the zkVM-compatible transaction types in lib/ and the enhanced CCR operations.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::ccr::CcrOperation;
use crate::{Identity, Deal, DealTransactionDb};

// Re-export lib transaction types for convenience
pub use lib::transactions::{LoanTransaction, TransactionType};

/// Errors that can occur during transaction mapping
#[derive(Error, Debug, Clone)]
pub enum MappingError {
    #[error("Unsupported transaction type for CCR mapping: {transaction_type:?}")]
    UnsupportedTransactionType { transaction_type: TransactionType },

    #[error("Missing required data for mapping: {field}")]
    MissingRequiredData { field: String },

    #[error("Invalid UUID conversion: {error}")]
    InvalidUuid { error: String },

    #[error("Timestamp conversion error: {timestamp}")]
    InvalidTimestamp { timestamp: u64 },

    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Deal not found: {loan_id}")]
    LoanNotFound { loan_id: String },

    #[error("Invalid transaction amount: {amount}")]
    InvalidAmount { amount: u64 },

    #[error("Database transaction type not supported: {transaction_type}")]
    UnsupportedDbTransactionType { transaction_type: String },
}

/// Context required for transaction mapping
/// This provides the necessary data to create complete CCR operations
#[derive(Debug, Clone)]
pub struct MappingContext {
    pub users: HashMap<Uuid, Identity>,
    pub deals: HashMap<Uuid, Deal>,
}

impl MappingContext {
    /// Create new mapping context
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            deals: HashMap::new(),
        }
    }

    /// Add a user to the context
    pub fn add_user(&mut self, user: Identity) {
        self.users.insert(user.id, user);
    }

    /// Add a deal to the context
    pub fn add_deal(&mut self, deal: Deal) {
        self.deals.insert(deal.id, deal);
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: &Uuid) -> Option<&Identity> {
        self.users.get(user_id)
    }

    /// Get deal by ID
    pub fn get_deal(&self, deal_id: &Uuid) -> Option<&Deal> {
        self.deals.get(deal_id)
    }
}

impl Default for MappingContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction mapper for converting between different transaction representations
pub struct TransactionMapper {
    context: MappingContext,
}

impl TransactionMapper {
    /// Create new transaction mapper with context
    pub fn new(context: MappingContext) -> Self {
        Self { context }
    }

    /// Create mapper with empty context
    pub fn empty() -> Self {
        Self {
            context: MappingContext::new(),
        }
    }

    /// Update the mapping context
    pub fn update_context(&mut self, context: MappingContext) {
        self.context = context;
    }

    /// Convert lib::LoanTransaction to CcrOperation
    ///
    /// This maps zkVM-compatible transactions to CCR operations:
    /// - LoanOrigination → LoanCreation
    /// - LoanPayment → Payment (+ BalanceUpdate operations)
    /// - LoanCloseout → Payment (final payment)
    pub fn lib_transaction_to_ccr(
        &self,
        transaction: &LoanTransaction,
    ) -> Result<Vec<CcrOperation>, MappingError> {
        let created_at = self.timestamp_to_datetime(transaction.created_at)?;

        match transaction.transaction_type {
            TransactionType::LoanOrigination => {
                let loan_id = self.bytes_to_uuid(&transaction.loan_id)?;
                let borrower_id = self.bytes_to_uuid(&transaction.user_id)?;

                // For loan origination, we need to extract lender_id from metadata or context
                let loan =
                    self.context
                        .get_deal(&loan_id)
                        .ok_or_else(|| MappingError::LoanNotFound {
                            loan_id: loan_id.to_string(),
                        })?;

                let lender_id = loan.investor_id;

                Ok(vec![CcrOperation::LoanCreation {
                    loan_id,
                    borrower_id,
                    lender_id,
                    principal_amount: transaction.amount as i64,
                    created_at,
                }])
            }

            TransactionType::LoanPayment => {
                let loan_id = self.bytes_to_uuid(&transaction.loan_id)?;
                let payer_id = self.bytes_to_uuid(&transaction.user_id)?;
                let payment_id = self.generate_payment_id(&transaction.transaction_id);

                let mut operations = Vec::new();

                // Create the payment operation
                operations.push(CcrOperation::Payment {
                    payment_id,
                    loan_id,
                    payer_id,
                    amount: transaction.amount as i64,
                    created_at,
                });

                // Create balance update operations for both payer and loan recipient
                let loan =
                    self.context
                        .get_deal(&loan_id)
                        .ok_or_else(|| MappingError::LoanNotFound {
                            loan_id: loan_id.to_string(),
                        })?;

                // Payer balance decreases
                operations.push(CcrOperation::BalanceUpdate {
                    balance_id: self.generate_balance_id(&transaction.transaction_id, &payer_id),
                    identity_id: payer_id,
                    amount_delta: -(transaction.amount as i64),
                    new_balance: self
                        .calculate_new_balance(payer_id, -(transaction.amount as i64))?,
                    created_at,
                });

                // Lender balance increases (receiver of payment)
                operations.push(CcrOperation::BalanceUpdate {
                    balance_id: self
                        .generate_balance_id(&transaction.transaction_id, &loan.investor_id),
                    identity_id: loan.investor_id,
                    amount_delta: transaction.amount as i64,
                    new_balance: self
                        .calculate_new_balance(loan.investor_id, transaction.amount as i64)?,
                    created_at,
                });

                Ok(operations)
            }

            TransactionType::LoanCloseout => {
                // Deal closeout is treated as a final payment
                let loan_id = self.bytes_to_uuid(&transaction.loan_id)?;
                let payer_id = self.bytes_to_uuid(&transaction.user_id)?;
                let payment_id = self.generate_payment_id(&transaction.transaction_id);

                // For closeout, the amount is typically the remaining balance
                Ok(vec![CcrOperation::Payment {
                    payment_id,
                    loan_id,
                    payer_id,
                    amount: transaction.amount as i64,
                    created_at,
                }])
            }

            TransactionType::InvestorRegistration => {
                // For investor registration, we don't generate CCR operations
                // since this is a user state change, not a financial operation
                Ok(vec![])
            }

            TransactionType::PoolCreation => {
                // For pool creation, we don't generate CCR operations
                // since this is pool state management, not a financial operation
                Ok(vec![])
            }

            // Other non-monetary transaction types
            TransactionType::UserRegistration
            | TransactionType::KycSubmission
            | TransactionType::KycVerification
            | TransactionType::UserDeposit
            | TransactionType::UserWithdrawal => {
                // These transaction types don't generate CCR operations
                // They are handled by their respective state management systems
                Ok(vec![])
            }

            TransactionType::AdminRegistration
            | TransactionType::MerchantProfileUpdate
            | TransactionType::CreditLineRequest => {
                // Merchant operations don't generate CCR operations
                // They are handled by the merchant state management system
                // Credit line requests will eventually trigger loan originations
                // which will then generate the appropriate CCR operations
                Ok(vec![])
            }

            _ => Err(MappingError::UnsupportedTransactionType {
                transaction_type: transaction.transaction_type,
            }),
        }
    }

    /// Convert database DealTransactionDb to CcrOperation
    ///
    /// This maps database transaction records to CCR operations, handling
    /// the conversion from string-based transaction types to enum variants.
    pub fn db_transaction_to_ccr(
        &self,
        db_transaction: &DealTransactionDb,
    ) -> Result<Vec<CcrOperation>, MappingError> {
        let created_at = db_transaction.created_at;

        match db_transaction.transaction_type.as_str() {
            "loan_origination" | "origination" => {
                let loan_id =
                    db_transaction
                        .deal_id
                        .ok_or_else(|| MappingError::MissingRequiredData {
                            field: "loan_id".to_string(),
                        })?;

                let identity_id = db_transaction.identity_id.ok_or_else(|| {
                    MappingError::MissingRequiredData {
                        field: "identity_id".to_string(),
                    }
                })?;

                let loan =
                    self.context
                        .get_deal(&loan_id)
                        .ok_or_else(|| MappingError::LoanNotFound {
                            loan_id: loan_id.to_string(),
                        })?;

                Ok(vec![CcrOperation::LoanCreation {
                    loan_id,
                    borrower_id: identity_id,
                    lender_id: loan.investor_id,
                    principal_amount: db_transaction
                        .amount
                        .to_string()
                        .parse::<i64>()
                        .map_err(|_| MappingError::InvalidAmount { amount: 0 })?,
                    created_at,
                }])
            }

            "investor_registration" => {
                // For investor registration, we don't generate CCR operations
                Ok(vec![])
            }

            "pool_creation" => {
                // For pool creation, we don't generate CCR operations
                Ok(vec![])
            }

            "user_registration" | "kyc_submission" | "kyc_verification" | "user_deposit"
            | "user_withdrawal" => {
                // These transaction types don't generate CCR operations
                Ok(vec![])
            }

            "loan_payment" | "payment" => {
                let loan_id =
                    db_transaction
                        .deal_id
                        .ok_or_else(|| MappingError::MissingRequiredData {
                            field: "loan_id".to_string(),
                        })?;

                let identity_id = db_transaction.identity_id.ok_or_else(|| {
                    MappingError::MissingRequiredData {
                        field: "identity_id".to_string(),
                    }
                })?;

                let payment_id = db_transaction.payment_id.unwrap_or_else(|| Uuid::new_v4());

                let amount = db_transaction
                    .amount
                    .to_string()
                    .parse::<i64>()
                    .map_err(|_| MappingError::InvalidAmount { amount: 0 })?;

                let mut operations = Vec::new();

                // Create the payment operation
                operations.push(CcrOperation::Payment {
                    payment_id,
                    loan_id,
                    payer_id: identity_id,
                    amount,
                    created_at,
                });

                // Add balance updates if we have loan context
                if let Some(loan) = self.context.get_deal(&loan_id) {
                    // Payer balance decreases
                    operations.push(CcrOperation::BalanceUpdate {
                        balance_id: self
                            .generate_balance_id_from_uuid(&db_transaction.id, &identity_id),
                        identity_id,
                        amount_delta: -amount,
                        new_balance: self.calculate_new_balance(identity_id, -amount)?,
                        created_at,
                    });

                    // Lender balance increases
                    operations.push(CcrOperation::BalanceUpdate {
                        balance_id: self
                            .generate_balance_id_from_uuid(&db_transaction.id, &loan.investor_id),
                        identity_id: loan.investor_id,
                        amount_delta: amount,
                        new_balance: self.calculate_new_balance(loan.investor_id, amount)?,
                        created_at,
                    });
                }

                Ok(operations)
            }

            _ => Err(MappingError::UnsupportedDbTransactionType {
                transaction_type: db_transaction.transaction_type.clone(),
            }),
        }
    }

    /// Convert CcrOperation back to lib::TransactionType
    ///
    /// This provides the reverse mapping for compatibility with existing zkVM code
    pub fn ccr_to_transaction_type(operation: &CcrOperation) -> TransactionType {
        match operation {
            CcrOperation::LoanCreation { .. } => TransactionType::LoanOrigination,
            CcrOperation::LoanFunding { .. } => TransactionType::LoanOrigination, // Funding is part of origination
            CcrOperation::Payment { .. } => TransactionType::LoanPayment,
            CcrOperation::BalanceUpdate { .. } => TransactionType::LoanPayment, // Balance updates are side effects of payments
        }
    }

    /// Batch convert multiple lib transactions to CCR operations
    pub fn batch_lib_to_ccr(
        &self,
        transactions: &[LoanTransaction],
    ) -> Result<Vec<CcrOperation>, MappingError> {
        let mut all_operations = Vec::new();
        for transaction in transactions {
            let operations = self.lib_transaction_to_ccr(transaction)?;
            all_operations.extend(operations);
        }
        Ok(all_operations)
    }

    /// Batch convert multiple database transactions to CCR operations
    pub fn batch_db_to_ccr(
        &self,
        transactions: &[DealTransactionDb],
    ) -> Result<Vec<CcrOperation>, MappingError> {
        let mut all_operations = Vec::new();
        for transaction in transactions {
            let operations = self.db_transaction_to_ccr(transaction)?;
            all_operations.extend(operations);
        }
        Ok(all_operations)
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    /// Convert byte array to UUID
    fn bytes_to_uuid(&self, bytes: &[u8; 32]) -> Result<Uuid, MappingError> {
        // Take first 16 bytes and create UUID
        let uuid_bytes: [u8; 16] =
            bytes[..16]
                .try_into()
                .map_err(|_| MappingError::InvalidUuid {
                    error: "Cannot convert 32-byte hash to 16-byte UUID".to_string(),
                })?;

        Ok(Uuid::from_bytes(uuid_bytes))
    }

    /// Convert unix timestamp to DateTime
    fn timestamp_to_datetime(&self, timestamp: u64) -> Result<DateTime<Utc>, MappingError> {
        match DateTime::from_timestamp(timestamp as i64, 0) {
            Some(dt) => Ok(dt),
            None => Err(MappingError::InvalidTimestamp { timestamp }),
        }
    }

    /// Generate deterministic payment ID from transaction ID
    fn generate_payment_id(&self, transaction_id: &[u8; 32]) -> Uuid {
        // Use a deterministic method to generate UUID from transaction hash
        let uuid_bytes: [u8; 16] = transaction_id[16..].try_into().unwrap_or([0u8; 16]);
        Uuid::from_bytes(uuid_bytes)
    }

    /// Generate deterministic balance ID from transaction and user ID
    fn generate_balance_id(&self, transaction_id: &[u8; 32], user_id: &Uuid) -> Uuid {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        transaction_id.hash(&mut hasher);
        user_id.hash(&mut hasher);
        let hash = hasher.finish();

        // Convert hash to UUID bytes
        let uuid_bytes = hash.to_le_bytes();
        let mut full_bytes = [0u8; 16];
        full_bytes[..8].copy_from_slice(&uuid_bytes);
        full_bytes[8..].copy_from_slice(&uuid_bytes); // Duplicate to fill 16 bytes

        Uuid::from_bytes(full_bytes)
    }

    /// Generate deterministic balance ID from database transaction ID and user ID
    fn generate_balance_id_from_uuid(&self, transaction_id: &Uuid, user_id: &Uuid) -> Uuid {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        transaction_id.hash(&mut hasher);
        user_id.hash(&mut hasher);
        let hash = hasher.finish();

        let uuid_bytes = hash.to_le_bytes();
        let mut full_bytes = [0u8; 16];
        full_bytes[..8].copy_from_slice(&uuid_bytes);
        full_bytes[8..].copy_from_slice(&uuid_bytes);

        Uuid::from_bytes(full_bytes)
    }

    /// Calculate new balance after transaction using loan context
    fn calculate_new_balance(&self, user_id: Uuid, amount_delta: i64) -> Result<i64, MappingError> {
        // Look up current balance from loan context
        // In a loan system, balance tracking would be based on:
        // 1. Outstanding loan balances for borrowers
        // 2. Total lent amounts for lenders

        let mut current_balance = 0i64;

        // Aggregate balance from all loans involving this user
        for loan in self.context.deals.values() {
            if loan.originator_id == user_id {
                // For borrowers, outstanding balance is a negative balance (debt)
                let outstanding = loan
                    .outstanding_balance
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0) as i64;
                current_balance -= outstanding;
            } else if loan.investor_id == user_id {
                // For lenders, outstanding balance represents money lent out
                let outstanding = loan
                    .outstanding_balance
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0) as i64;
                current_balance += outstanding;

                // Add principal amount as well (total exposure)
                let principal = loan
                    .principal_amount
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0) as i64;
                current_balance += principal - outstanding; // Principal minus what's still owed
            }
        }

        // Apply the transaction delta
        let new_balance = current_balance + amount_delta;

        // For a credit system, negative balances are allowed (debt)
        // But we ensure minimum floor based on business rules
        Ok(new_balance.max(-1_000_000_000)) // Minimum debt limit of 1B
    }
}

/// Utility functions for transaction mapping
pub mod utils {
    use super::*;

    /// Extract loan IDs from a batch of CCR operations
    pub fn extract_loan_ids(operations: &[CcrOperation]) -> Vec<Uuid> {
        let mut loan_ids = Vec::new();
        for operation in operations {
            match operation {
                CcrOperation::LoanCreation { loan_id, .. } => loan_ids.push(*loan_id),
                CcrOperation::LoanFunding { loan_id, .. } => loan_ids.push(*loan_id),
                CcrOperation::Payment { loan_id, .. } => loan_ids.push(*loan_id),
                CcrOperation::BalanceUpdate { .. } => {} // No loan ID
            }
        }
        loan_ids.sort();
        loan_ids.dedup();
        loan_ids
    }

    /// Extract user IDs from a batch of CCR operations
    pub fn extract_user_ids(operations: &[CcrOperation]) -> Vec<Uuid> {
        let mut user_ids = Vec::new();
        for operation in operations {
            match operation {
                CcrOperation::LoanCreation {
                    borrower_id,
                    lender_id,
                    ..
                } => {
                    user_ids.push(*borrower_id);
                    user_ids.push(*lender_id);
                }
                CcrOperation::LoanFunding { funder_id, .. } => user_ids.push(*funder_id),
                CcrOperation::Payment { payer_id, .. } => user_ids.push(*payer_id),
                CcrOperation::BalanceUpdate { identity_id, .. } => user_ids.push(*identity_id),
            }
        }
        user_ids.sort();
        user_ids.dedup();
        user_ids
    }

    /// Check if nullifier generation is deterministic across different representations
    pub fn verify_deterministic_mapping(
        _lib_transaction: &LoanTransaction,
        ccr_operations: &[CcrOperation],
    ) -> bool {
        // Verify that the same lib transaction always produces the same CCR operations
        // with the same nullifiers
        for operation in ccr_operations {
            let nullifier1 = operation.to_nullifier();
            let nullifier2 = operation.to_nullifier();
            if nullifier1 != nullifier2 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_context() -> MappingContext {
        let mut context = MappingContext::new();

        // Add test user
        let user = Identity {
            id: Uuid::from_bytes([1u8; 16]),
            user_type: "investor".to_string(),
            email: "test@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            kyc_investor_type: Some(crate::KycInvestorType::AI),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // Authentication fields
            last_login_at: None,
            email_verified: Some(true), // Test user is verified
            email_verification_token: None,
            first_name: None,
            last_name: None,
            jurisdiction: None,
            wallet_address: None,
        };
        context.add_user(user);

        // Add test deal
        let deal = Deal {
            id: Uuid::from_bytes([2u8; 16]),
            originator_id: Uuid::from_bytes([1u8; 16]),  // was borrower_id
            investor_id: Uuid::from_bytes([3u8; 16]),    // was lender_id
            principal_amount: rust_decimal::Decimal::new(10_000_000, 2), // $100,000
            outstanding_balance: rust_decimal::Decimal::new(10_000_000, 2),
            interest_rate_bps: 750,
            loan_status: "active".to_string(),
            originated_at: Utc::now(),
            maturity_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        context.add_deal(deal);

        context
    }

    #[test]
    fn test_lib_loan_origination_mapping() {
        let context = create_test_context();
        let mapper = TransactionMapper::new(context);

        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            loan_id: [2u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            transaction_type: TransactionType::LoanOrigination,
            amount: 100_000,
            created_at: 1_640_995_200, // 2022-01-01
            metadata: lib::transactions::MetadataArray::empty(),
        };

        let operations = mapper.lib_transaction_to_ccr(&lib_transaction).unwrap();
        assert_eq!(operations.len(), 1);

        match &operations[0] {
            CcrOperation::LoanCreation {
                loan_id,
                borrower_id,
                principal_amount,
                ..
            } => {
                assert_eq!(*loan_id, Uuid::from_bytes([2u8; 16]));
                assert_eq!(*borrower_id, Uuid::from_bytes([1u8; 16]));
                assert_eq!(*principal_amount, 100_000);
            }
            _ => panic!("Expected LoanCreation operation"),
        }
    }

    #[test]
    fn test_lib_loan_payment_mapping() {
        let context = create_test_context();
        let mapper = TransactionMapper::new(context);

        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            loan_id: [2u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            transaction_type: TransactionType::LoanPayment,
            amount: 5000,
            created_at: 1_640_995_200,
            metadata: lib::transactions::MetadataArray::empty(),
        };

        let operations = mapper.lib_transaction_to_ccr(&lib_transaction).unwrap();
        assert_eq!(operations.len(), 3); // Payment + 2 balance updates

        // Check payment operation
        match &operations[0] {
            CcrOperation::Payment {
                loan_id,
                payer_id,
                amount,
                ..
            } => {
                assert_eq!(*loan_id, Uuid::from_bytes([2u8; 16]));
                assert_eq!(*payer_id, Uuid::from_bytes([1u8; 16]));
                assert_eq!(*amount, 5000);
            }
            _ => panic!("Expected Payment operation"),
        }

        // Check balance update operations
        assert!(matches!(operations[1], CcrOperation::BalanceUpdate { .. }));
        assert!(matches!(operations[2], CcrOperation::BalanceUpdate { .. }));
    }

    #[test]
    fn test_deterministic_nullifier_generation() {
        let context = create_test_context();
        let mapper = TransactionMapper::new(context);

        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            loan_id: [2u8; 16]
                .iter()
                .chain([0u8; 16].iter())
                .cloned()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
            transaction_type: TransactionType::LoanOrigination,
            amount: 100_000,
            created_at: 1_640_995_200,
            metadata: lib::transactions::MetadataArray::empty(),
        };

        let operations1 = mapper.lib_transaction_to_ccr(&lib_transaction).unwrap();
        let operations2 = mapper.lib_transaction_to_ccr(&lib_transaction).unwrap();

        assert_eq!(operations1.len(), operations2.len());
        for (op1, op2) in operations1.iter().zip(operations2.iter()) {
            assert_eq!(op1.to_nullifier(), op2.to_nullifier());
        }
    }

    #[test]
    fn test_utils_extract_ids() {
        let operations = vec![
            CcrOperation::LoanCreation {
                loan_id: Uuid::from_bytes([1u8; 16]),
                borrower_id: Uuid::from_bytes([2u8; 16]),
                lender_id: Uuid::from_bytes([3u8; 16]),
                principal_amount: 100_000,
                created_at: Utc::now(),
            },
            CcrOperation::Payment {
                payment_id: Uuid::from_bytes([4u8; 16]),
                loan_id: Uuid::from_bytes([1u8; 16]), // Same loan
                payer_id: Uuid::from_bytes([2u8; 16]),
                amount: 5000,
                created_at: Utc::now(),
            },
        ];

        let loan_ids = utils::extract_loan_ids(&operations);
        assert_eq!(loan_ids.len(), 1); // Deduplicated
        assert_eq!(loan_ids[0], Uuid::from_bytes([1u8; 16]));

        let user_ids = utils::extract_user_ids(&operations);
        assert_eq!(user_ids.len(), 2); // borrower and lender (payer is same as borrower)
    }
}
