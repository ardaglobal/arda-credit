//! User management logic for zkVM execution
//!
//! This module contains user-related types and state transition logic
//! including user registration and KYC operations.

use crate::transactions::{LoanTransaction, TransactionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

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

impl User {
    /// Check if user is KYC approved
    #[must_use]
    pub const fn is_kyc_approved(&self) -> bool {
        matches!(self.kyc_status, KycStatus::Approved)
    }
}

/// Errors specific to user operations
#[derive(Error, Debug, Clone)]
pub enum UserError {
    #[error("User not found: {user_id:?}")]
    UserNotFound { user_id: [u8; 32] },

    #[error("KYC not approved for user: {user_id:?}")]
    KycNotApproved { user_id: [u8; 32] },

    #[error("Duplicate user registration: {user_id:?}")]
    DuplicateUser { user_id: [u8; 32] },

    #[error("Invalid transaction type for user operation")]
    InvalidTransactionType,
}

/// User state management
pub struct UserState {
    pub users: HashMap<[u8; 32], User>,
}

impl UserState {
    /// Create a new empty user state
    #[must_use]
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
        }
    }

    /// Process user registration transaction
    ///
    /// # Errors
    /// Returns `UserError` if transaction type is invalid, user already exists,
    /// or metadata contains invalid user type.
    pub fn process_user_registration(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), UserError> {
        if transaction.transaction_type != TransactionType::UserRegistration {
            return Err(UserError::InvalidTransactionType);
        }

        let user_id = transaction.user_id;

        // Check for duplicate registration
        if self.users.contains_key(&user_id) {
            return Err(UserError::DuplicateUser { user_id });
        }

        // Decode user data from metadata
        let user_type = match transaction.metadata.as_bytes()[0] {
            1 => UserType::Investor,
            2 => UserType::Lender,
            _ => return Err(UserError::InvalidTransactionType),
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

    /// Process KYC submission transaction
    ///
    /// # Errors
    /// Returns `UserError` if transaction type is invalid or user not found.
    pub fn process_kyc_submission(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), UserError> {
        if transaction.transaction_type != TransactionType::KycSubmission {
            return Err(UserError::InvalidTransactionType);
        }

        let user_id = transaction.user_id;

        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(UserError::UserNotFound { user_id })?;

        // Update KYC status to pending
        user.kyc_status = KycStatus::Pending;
        user.kyc_submitted_at = transaction.created_at;

        Ok(())
    }

    /// Process KYC verification transaction (admin operation)
    ///
    /// # Errors
    /// Returns `UserError` if transaction type is invalid or user not found.
    pub fn process_kyc_verification(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), UserError> {
        if transaction.transaction_type != TransactionType::KycVerification {
            return Err(UserError::InvalidTransactionType);
        }

        let user_id = transaction.user_id;

        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(UserError::UserNotFound { user_id })?;

        // Extract approval/rejection from metadata
        let approval = transaction.metadata.as_bytes()[0] == 1;

        if approval {
            user.kyc_status = KycStatus::Approved;
            user.kyc_verified_at = transaction.created_at;
        } else {
            user.kyc_status = KycStatus::Rejected;
        }

        Ok(())
    }

    /// Get user by ID (read-only)
    #[must_use]
    pub fn get_user(&self, user_id: &[u8; 32]) -> Option<&User> {
        self.users.get(user_id)
    }

    /// Check if user exists and is KYC approved
    ///
    /// # Errors
    /// Returns `UserError::UserNotFound` if user does not exist.
    pub fn is_user_kyc_approved(&self, user_id: &[u8; 32]) -> Result<bool, UserError> {
        let user = self
            .users
            .get(user_id)
            .ok_or(UserError::UserNotFound { user_id: *user_id })?;
        Ok(user.is_kyc_approved())
    }
}

impl Default for UserState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;

    fn create_test_user_registration_transaction(
        user_id: [u8; 32],
        user_type: u8,
    ) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32], // Should be all zeros for user registration
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_234_567_890,
            metadata: MetadataArray::from_bytes(&[user_type]),
        }
    }

    #[test]
    fn test_user_registration() {
        let mut user_state = UserState::new();
        let user_id = [1u8; 32];
        let transaction = create_test_user_registration_transaction(user_id, 1); // Investor

        let result = user_state.process_user_registration(&transaction);
        assert!(result.is_ok());

        let user = user_state.get_user(&user_id).unwrap();
        assert_eq!(user.user_type, UserType::Investor);
        assert_eq!(user.kyc_status, KycStatus::NotSubmitted);
    }

    #[test]
    fn test_duplicate_user_registration() {
        let mut user_state = UserState::new();
        let user_id = [1u8; 32];
        let transaction = create_test_user_registration_transaction(user_id, 1);

        // First registration should succeed
        assert!(user_state.process_user_registration(&transaction).is_ok());

        // Second registration should fail
        let result = user_state.process_user_registration(&transaction);
        assert!(matches!(result, Err(UserError::DuplicateUser { .. })));
    }

    #[test]
    fn test_kyc_submission() {
        let mut user_state = UserState::new();
        let user_id = [1u8; 32];

        // First register user
        let registration_transaction = create_test_user_registration_transaction(user_id, 1);
        user_state
            .process_user_registration(&registration_transaction)
            .unwrap();

        // Then submit KYC
        let kyc_transaction = LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: 1_234_567_900,
            metadata: MetadataArray::empty(),
        };

        let result = user_state.process_kyc_submission(&kyc_transaction);
        assert!(result.is_ok());

        let user = user_state.get_user(&user_id).unwrap();
        assert_eq!(user.kyc_status, KycStatus::Pending);
        assert_eq!(user.kyc_submitted_at, 1_234_567_900);
    }

    #[test]
    fn test_kyc_verification_approval() {
        let mut user_state = UserState::new();
        let user_id = [1u8; 32];

        // Setup: Register user and submit KYC
        let registration_transaction = create_test_user_registration_transaction(user_id, 1);
        user_state
            .process_user_registration(&registration_transaction)
            .unwrap();

        let kyc_submission = LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: 1_234_567_900,
            metadata: MetadataArray::empty(),
        };
        user_state.process_kyc_submission(&kyc_submission).unwrap();

        // Verify KYC (approval)
        let kyc_verification = LoanTransaction {
            transaction_id: [3u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1_234_567_910,
            metadata: MetadataArray::from_bytes(&[1u8]), // 1 = approved
        };

        let result = user_state.process_kyc_verification(&kyc_verification);
        assert!(result.is_ok());

        let user = user_state.get_user(&user_id).unwrap();
        assert_eq!(user.kyc_status, KycStatus::Approved);
        assert_eq!(user.kyc_verified_at, 1_234_567_910);
        assert!(user.is_kyc_approved());
    }
}
