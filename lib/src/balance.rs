//! User balance tracking for loan operations
//!
//! This module provides comprehensive balance tracking that integrates with
//! the existing loan payment processing and IMT batching system. Balance updates
//! are triggered automatically during loan payments and included in the same batches.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during balance operations
#[derive(Error, Debug, Clone)]
pub enum BalanceError {
    #[error("User not found: {user_id:?}")]
    UserNotFound { user_id: [u8; 32] },

    #[error("Insufficient balance: requested {requested}, available {available}")]
    InsufficientBalance { requested: u128, available: u128 },

    #[error("Invalid balance amount: {amount}")]
    InvalidAmount { amount: u128 },

    #[error("Balance operation failed: {reason}")]
    OperationFailed { reason: String },
}

/// Types of balance update operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BalanceUpdateType {
    /// Credit from loan payment received
    CreditFromPayment = 1,
    /// Debit from loan origination (funds lent out)
    DebitFromOrigination = 2,
    /// Credit from loan origination (funds borrowed)
    CreditFromOrigination = 3,
    /// Adjustment for loan interest accrual
    InterestAccrual = 4,
    /// Manual balance adjustment (reconciliation)
    ManualAdjustment = 5,
    /// Credit from user deposit
    CreditFromDeposit = 6,
    /// Debit from user withdrawal
    DebitFromWithdrawal = 7,
}

impl BalanceUpdateType {
    /// Get string representation for logging/display
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CreditFromPayment => "credit_from_payment",
            Self::DebitFromOrigination => "debit_from_origination",
            Self::CreditFromOrigination => "credit_from_origination",
            Self::InterestAccrual => "interest_accrual",
            Self::ManualAdjustment => "manual_adjustment",
            Self::CreditFromDeposit => "credit_from_deposit",
            Self::DebitFromWithdrawal => "debit_from_withdrawal",
        }
    }

    /// Check if this update type increases balance
    #[must_use]
    pub const fn is_credit(&self) -> bool {
        matches!(
            self,
            Self::CreditFromPayment
                | Self::CreditFromOrigination
                | Self::InterestAccrual
                | Self::CreditFromDeposit
        )
    }

    /// Check if this update type decreases balance
    #[must_use]
    pub const fn is_debit(&self) -> bool {
        matches!(self, Self::DebitFromOrigination | Self::DebitFromWithdrawal)
    }
}

/// A single balance update operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceUpdate {
    /// Unique identifier for this balance update
    pub update_id: [u8; 32],
    /// User whose balance is being updated
    pub user_id: [u8; 32],
    /// Related loan ID (if applicable)
    pub loan_id: Option<[u8; 32]>,
    /// Related loan transaction ID that triggered this update
    pub triggering_transaction_id: Option<[u8; 32]>,
    /// Type of balance update
    pub update_type: BalanceUpdateType,
    /// Amount of the balance change (always positive, sign determined by `update_type`)
    pub amount: u128,
    /// User's balance before this update
    pub previous_balance: u128,
    /// User's balance after this update
    pub new_balance: u128,
    /// Timestamp when update was created
    pub created_at: u64,
    /// Additional metadata for the update
    pub metadata: Option<String>,
}

impl BalanceUpdate {
    /// Create a new balance update
    ///
    /// # Errors
    /// Returns `BalanceError::InvalidAmount` if amount is zero
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        update_id: [u8; 32],
        user_id: [u8; 32],
        loan_id: Option<[u8; 32]>,
        triggering_transaction_id: Option<[u8; 32]>,
        update_type: BalanceUpdateType,
        amount: u128,
        previous_balance: u128,
        created_at: u64,
        metadata: Option<String>,
    ) -> Result<Self, BalanceError> {
        if amount == 0 {
            return Err(BalanceError::InvalidAmount { amount });
        }

        let new_balance = if update_type.is_credit() {
            previous_balance.saturating_add(amount)
        } else if update_type.is_debit() {
            if previous_balance < amount {
                return Err(BalanceError::InsufficientBalance {
                    requested: amount,
                    available: previous_balance,
                });
            }
            previous_balance - amount
        } else {
            // Manual adjustment - amount could be positive or negative
            previous_balance.saturating_add(amount)
        };

        Ok(Self {
            update_id,
            user_id,
            loan_id,
            triggering_transaction_id,
            update_type,
            amount,
            previous_balance,
            new_balance,
            created_at,
            metadata,
        })
    }

    /// Get the balance change as a signed integer (positive for credits, negative for debits)
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub const fn signed_change(&self) -> i128 {
        let amount = self.amount as i128;
        match self.update_type {
            BalanceUpdateType::CreditFromPayment
            | BalanceUpdateType::CreditFromOrigination
            | BalanceUpdateType::InterestAccrual
            | BalanceUpdateType::ManualAdjustment
            | BalanceUpdateType::CreditFromDeposit => amount, // Assume positive for manual
            BalanceUpdateType::DebitFromOrigination | BalanceUpdateType::DebitFromWithdrawal => {
                -amount
            }
        }
    }

    /// Convert balance update to nullifier hash for IMT insertion
    /// Uses the `4_000_000_000` prefix as specified
    #[must_use]
    pub fn to_nullifier(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Add the balance update prefix (4_000_000_000)
        hasher.update(4_000_000_000u64.to_le_bytes());

        // Add core balance update data
        hasher.update(self.update_id);
        hasher.update(self.user_id);
        hasher.update([self.update_type as u8]);
        hasher.update(self.amount.to_le_bytes());
        hasher.update(self.previous_balance.to_le_bytes());
        hasher.update(self.new_balance.to_le_bytes());
        hasher.update(self.created_at.to_le_bytes());

        // Add loan ID if present
        if let Some(loan_id) = self.loan_id {
            hasher.update(loan_id);
        }

        // Add triggering transaction ID if present
        if let Some(tx_id) = self.triggering_transaction_id {
            hasher.update(tx_id);
        }

        hasher.finalize().into()
    }
}

/// User balance state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserBalance {
    /// User ID
    pub user_id: [u8; 32],
    /// Current balance
    pub balance: u128,
    /// Last update timestamp
    pub last_updated: u64,
    /// Number of balance updates for this user
    pub update_count: u64,
}

impl UserBalance {
    /// Create a new user balance
    #[must_use]
    pub const fn new(user_id: [u8; 32], initial_balance: u128, timestamp: u64) -> Self {
        Self {
            user_id,
            balance: initial_balance,
            last_updated: timestamp,
            update_count: 0,
        }
    }

    /// Apply a balance update to this user's balance
    ///
    /// # Errors
    /// Returns `BalanceError::OperationFailed` if user ID mismatch or balance underflow
    pub fn apply_update(&mut self, update: &BalanceUpdate) -> Result<(), BalanceError> {
        if update.user_id != self.user_id {
            return Err(BalanceError::OperationFailed {
                reason: "User ID mismatch".to_string(),
            });
        }

        if update.previous_balance != self.balance {
            return Err(BalanceError::OperationFailed {
                reason: format!(
                    "Balance mismatch: expected {}, got {}",
                    self.balance, update.previous_balance
                ),
            });
        }

        self.balance = update.new_balance;
        self.last_updated = update.created_at;
        self.update_count += 1;

        Ok(())
    }
}

/// Global balance state for all users
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BalanceState {
    /// User balances indexed by `user_id`
    pub balances: HashMap<[u8; 32], UserBalance>,
    /// All balance updates (for history tracking)
    pub updates: Vec<BalanceUpdate>,
    /// Last update timestamp
    pub last_updated: u64,
}

impl BalanceState {
    /// Create a new empty balance state
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get user's current balance
    #[must_use]
    pub fn get_balance(&self, user_id: &[u8; 32]) -> u128 {
        self.balances.get(user_id).map_or(0, |b| b.balance)
    }

    /// Get user balance info
    #[must_use]
    pub fn get_user_balance(&self, user_id: &[u8; 32]) -> Option<&UserBalance> {
        self.balances.get(user_id)
    }

    /// Initialize a user's balance if it doesn't exist
    pub fn ensure_user_balance(&mut self, user_id: [u8; 32], timestamp: u64) {
        self.balances
            .entry(user_id)
            .or_insert_with(|| UserBalance::new(user_id, 0, timestamp));
    }

    /// Apply a balance update to the state
    ///
    /// # Errors
    /// Returns `BalanceError::UserNotFound` if user balance cannot be retrieved
    pub fn apply_update(&mut self, update: BalanceUpdate) -> Result<(), BalanceError> {
        // Ensure user balance exists
        self.ensure_user_balance(update.user_id, update.created_at);

        // Get mutable reference to user balance
        let user_balance =
            self.balances
                .get_mut(&update.user_id)
                .ok_or(BalanceError::UserNotFound {
                    user_id: update.user_id,
                })?;

        // Store timestamp before moving update
        let update_timestamp = update.created_at;

        // Apply the update
        user_balance.apply_update(&update)?;

        // Store the update in history
        self.updates.push(update);

        // Update global timestamp
        if update_timestamp > self.last_updated {
            self.last_updated = update_timestamp;
        }

        Ok(())
    }

    /// Create balance updates from loan payment
    /// This is called automatically during loan payment processing
    ///
    /// # Errors
    /// Returns `BalanceError::InvalidAmount` if amount is zero
    pub fn create_payment_updates(
        &self,
        payment_transaction_id: [u8; 32],
        loan_id: [u8; 32],
        _payer_id: [u8; 32],    // Not used in current implementation
        recipient_id: [u8; 32], // Usually the lender
        amount: u128,
        timestamp: u64,
    ) -> Result<Vec<BalanceUpdate>, BalanceError> {
        let mut updates = Vec::new();

        // Credit for recipient (lender receives payment)
        let credit_update = BalanceUpdate::new(
            self.generate_update_id(payment_transaction_id, 1),
            recipient_id,
            Some(loan_id),
            Some(payment_transaction_id),
            BalanceUpdateType::CreditFromPayment,
            amount,
            self.get_balance(&recipient_id),
            timestamp,
            Some(format!(
                "Payment received from loan {:02x}{:02x}{:02x}{:02x}",
                loan_id[0], loan_id[1], loan_id[2], loan_id[3]
            )),
        )?;

        updates.push(credit_update);

        Ok(updates)
    }

    /// Create balance updates from loan origination
    ///
    /// # Errors
    /// Returns `BalanceError::InvalidAmount` if amount is zero
    pub fn create_origination_updates(
        &self,
        origination_transaction_id: [u8; 32],
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u128,
        timestamp: u64,
    ) -> Result<Vec<BalanceUpdate>, BalanceError> {
        let mut updates = Vec::new();

        // Credit for borrower (receives loan funds)
        let borrower_credit = BalanceUpdate::new(
            self.generate_update_id(origination_transaction_id, 1),
            borrower_id,
            Some(loan_id),
            Some(origination_transaction_id),
            BalanceUpdateType::CreditFromOrigination,
            amount,
            self.get_balance(&borrower_id),
            timestamp,
            Some(format!(
                "Loan origination funds received {:02x}{:02x}{:02x}{:02x}",
                loan_id[0], loan_id[1], loan_id[2], loan_id[3]
            )),
        )?;

        // Debit for lender (funds lent out)
        let lender_debit = BalanceUpdate::new(
            self.generate_update_id(origination_transaction_id, 2),
            lender_id,
            Some(loan_id),
            Some(origination_transaction_id),
            BalanceUpdateType::DebitFromOrigination,
            amount,
            self.get_balance(&lender_id),
            timestamp,
            Some(format!(
                "Loan origination funds lent {:02x}{:02x}{:02x}{:02x}",
                loan_id[0], loan_id[1], loan_id[2], loan_id[3]
            )),
        )?;

        updates.push(borrower_credit);
        updates.push(lender_debit);

        Ok(updates)
    }

    /// Create balance update from user deposit
    ///
    /// # Errors
    /// Returns `BalanceError::InvalidAmount` if amount is zero
    pub fn create_deposit_update(
        &self,
        deposit_transaction_id: [u8; 32],
        user_id: [u8; 32],
        wallet_address: [u8; 32], // Wallet address where deposit came from
        amount: u128,
        timestamp: u64,
    ) -> Result<BalanceUpdate, BalanceError> {
        let update = BalanceUpdate::new(
            self.generate_update_id(deposit_transaction_id, 1),
            user_id,
            None, // No loan associated with deposits
            Some(deposit_transaction_id),
            BalanceUpdateType::CreditFromDeposit,
            amount,
            self.get_balance(&user_id),
            timestamp,
            Some(format!(
                "Deposit from wallet {:02x}{:02x}{:02x}{:02x}...",
                wallet_address[0], wallet_address[1], wallet_address[2], wallet_address[3]
            )),
        )?;

        Ok(update)
    }

    /// Create balance update from user withdrawal
    ///
    /// # Errors
    /// Returns `BalanceError::InvalidAmount` if amount is zero
    /// Returns `BalanceError::InsufficientBalance` if user doesn't have enough balance
    pub fn create_withdrawal_update(
        &self,
        withdrawal_transaction_id: [u8; 32],
        user_id: [u8; 32],
        recipient_address: [u8; 32], // Recipient wallet address for withdrawal
        amount: u128,
        timestamp: u64,
    ) -> Result<BalanceUpdate, BalanceError> {
        let current_balance = self.get_balance(&user_id);

        // Check for sufficient balance
        if current_balance < amount {
            return Err(BalanceError::InsufficientBalance {
                requested: amount,
                available: current_balance,
            });
        }

        let update = BalanceUpdate::new(
            self.generate_update_id(withdrawal_transaction_id, 1),
            user_id,
            None, // No loan associated with withdrawals
            Some(withdrawal_transaction_id),
            BalanceUpdateType::DebitFromWithdrawal,
            amount,
            current_balance,
            timestamp,
            Some(format!(
                "Withdrawal to wallet {:02x}{:02x}{:02x}{:02x}...",
                recipient_address[0],
                recipient_address[1],
                recipient_address[2],
                recipient_address[3]
            )),
        )?;

        Ok(update)
    }

    /// Get balance update history for a user
    #[must_use]
    pub fn get_user_update_history(
        &self,
        user_id: &[u8; 32],
        limit: Option<usize>,
    ) -> Vec<&BalanceUpdate> {
        let mut user_updates: Vec<&BalanceUpdate> = self
            .updates
            .iter()
            .filter(|update| update.user_id == *user_id)
            .collect();

        // Sort by timestamp (most recent first)
        user_updates.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        if let Some(limit) = limit {
            user_updates.truncate(limit);
        }

        user_updates
    }

    /// Get balance updates for a specific loan
    #[must_use]
    pub fn get_loan_balance_updates(&self, loan_id: &[u8; 32]) -> Vec<&BalanceUpdate> {
        self.updates
            .iter()
            .filter(|update| update.loan_id.as_ref() == Some(loan_id))
            .collect()
    }

    /// Get total value of all balances
    #[must_use]
    pub fn compute_total_balance(&self) -> u128 {
        self.balances.values().map(|b| b.balance).sum()
    }

    /// Generate a unique update ID based on transaction ID and sequence
    fn generate_update_id(&self, transaction_id: [u8; 32], sequence: u8) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(b"balance_update");
        hasher.update(transaction_id);
        hasher.update([sequence]);
        hasher.update((self.updates.len() as u64).to_le_bytes());

        hasher.finalize().into()
    }
}

/// Balance inclusion proof data for users to verify their balance updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInclusionProof {
    /// The balance update being proven
    pub balance_update: BalanceUpdate,
    /// Merkle path proof elements
    pub merkle_path: Vec<[u8; 32]>,
    /// Merkle root at the time of the update
    pub merkle_root: [u8; 32],
    /// Batch ID that included this balance update
    pub batch_id: [u8; 32],
    /// Position in the Merkle tree
    pub leaf_index: u64,
}

impl BalanceInclusionProof {
    /// Create a new balance inclusion proof
    #[must_use]
    pub const fn new(
        balance_update: BalanceUpdate,
        merkle_path: Vec<[u8; 32]>,
        merkle_root: [u8; 32],
        batch_id: [u8; 32],
        leaf_index: u64,
    ) -> Self {
        Self {
            balance_update,
            merkle_path,
            merkle_root,
            batch_id,
            leaf_index,
        }
    }

    /// Verify that this balance update was included in the specified Merkle tree
    #[must_use]
    pub fn verify(&self) -> bool {
        use sha2::{Digest, Sha256};

        let mut current_hash = self.balance_update.to_nullifier();
        let mut index = self.leaf_index;

        for sibling in &self.merkle_path {
            let mut hasher = Sha256::new();
            if index % 2 == 0 {
                // Current node is left child
                hasher.update(current_hash);
                hasher.update(sibling);
            } else {
                // Current node is right child
                hasher.update(sibling);
                hasher.update(current_hash);
            }
            current_hash = hasher.finalize().into();
            index /= 2;
        }

        current_hash == self.merkle_root
    }

    /// Get a human-readable description of this proof
    #[must_use]
    pub fn description(&self) -> String {
        format!(
            "Balance update proof for user {:02x}{:02x}{:02x}{:02x} - {} {} in batch {:02x}{:02x}{:02x}{:02x}",
            self.balance_update.user_id[0], self.balance_update.user_id[1],
            self.balance_update.user_id[2], self.balance_update.user_id[3],
            self.balance_update.update_type.as_str(),
            self.balance_update.amount,
            self.batch_id[0], self.batch_id[1], self.batch_id[2], self.batch_id[3]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_update_creation() {
        let update_id = [1u8; 32];
        let user_id = [2u8; 32];
        let loan_id = [3u8; 32];
        let tx_id = [4u8; 32];
        let amount = 50000u128; // $500.00
        let previous_balance = 100_000u128; // $1000.00
        let timestamp = 1_640_995_200;

        let update = BalanceUpdate::new(
            update_id,
            user_id,
            Some(loan_id),
            Some(tx_id),
            BalanceUpdateType::CreditFromPayment,
            amount,
            previous_balance,
            timestamp,
            Some("Test payment".to_string()),
        )
        .unwrap();

        assert_eq!(update.user_id, user_id);
        assert_eq!(update.amount, amount);
        assert_eq!(update.previous_balance, previous_balance);
        assert_eq!(update.new_balance, previous_balance + amount);
        assert_eq!(update.update_type, BalanceUpdateType::CreditFromPayment);
    }

    #[test]
    fn test_balance_update_debit() {
        let update_id = [1u8; 32];
        let user_id = [2u8; 32];
        let amount = 30000u128; // $300.00
        let previous_balance = 100_000u128; // $1000.00

        let update = BalanceUpdate::new(
            update_id,
            user_id,
            None,
            None,
            BalanceUpdateType::DebitFromOrigination,
            amount,
            previous_balance,
            1_640_995_200,
            None,
        )
        .unwrap();

        assert_eq!(update.new_balance, previous_balance - amount);
        #[allow(clippy::cast_possible_wrap)]
        let expected_change = -(amount as i128);
        assert_eq!(update.signed_change(), expected_change);
    }

    #[test]
    fn test_insufficient_balance() {
        let result = BalanceUpdate::new(
            [1u8; 32],
            [2u8; 32],
            None,
            None,
            BalanceUpdateType::DebitFromOrigination,
            150_000u128, // More than available
            100_000u128, // Available balance
            1_640_995_200,
            None,
        );

        assert!(matches!(
            result,
            Err(BalanceError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_balance_state_operations() {
        let mut state = BalanceState::new();
        let user_id = [1u8; 32];

        // Initial balance should be 0
        assert_eq!(state.get_balance(&user_id), 0);

        // Create and apply a credit update
        let update = BalanceUpdate::new(
            [10u8; 32],
            user_id,
            None,
            None,
            BalanceUpdateType::CreditFromPayment,
            50000u128,
            0, // Previous balance
            1_640_995_200,
            None,
        )
        .unwrap();

        state.apply_update(update).unwrap();

        // Balance should be updated
        assert_eq!(state.get_balance(&user_id), 50000u128);
        assert_eq!(state.updates.len(), 1);

        let user_balance = state.get_user_balance(&user_id).unwrap();
        assert_eq!(user_balance.update_count, 1);
    }

    #[test]
    fn test_payment_updates_creation() {
        let state = BalanceState::new();
        let payment_tx_id = [1u8; 32];
        let loan_id = [2u8; 32];
        let payer_id = [3u8; 32];
        let recipient_id = [4u8; 32];
        let amount = 25000u128;

        let updates = state
            .create_payment_updates(
                payment_tx_id,
                loan_id,
                payer_id,
                recipient_id,
                amount,
                1_640_995_200,
            )
            .unwrap();

        assert_eq!(updates.len(), 1);

        let credit_update = &updates[0];
        assert_eq!(credit_update.user_id, recipient_id);
        assert_eq!(
            credit_update.update_type,
            BalanceUpdateType::CreditFromPayment
        );
        assert_eq!(credit_update.amount, amount);
        assert_eq!(credit_update.loan_id, Some(loan_id));
    }

    #[test]
    fn test_origination_updates_creation() {
        let mut state = BalanceState::new();
        let origination_tx_id = [1u8; 32];
        let loan_id = [2u8; 32];
        let borrower_id = [3u8; 32];
        let lender_id = [4u8; 32];
        let amount = 100_000u128;

        // First, give the lender sufficient funds to originate the loan
        let initial_funding = BalanceUpdate::new(
            [0u8; 32], // funding_tx_id
            lender_id,
            None, // no loan associated
            None, // no triggering transaction
            BalanceUpdateType::ManualAdjustment,
            amount + 10000, // Give extra funds for safety
            0,              // previous balance is 0
            1_640_995_100,  // timestamp before origination
            Some("Initial funding for test".to_string()),
        )
        .unwrap();

        state.apply_update(initial_funding).unwrap();

        let updates = state
            .create_origination_updates(
                origination_tx_id,
                loan_id,
                borrower_id,
                lender_id,
                amount,
                1_640_995_200,
            )
            .unwrap();

        assert_eq!(updates.len(), 2);

        // First update should be credit for borrower
        let borrower_update = &updates[0];
        assert_eq!(borrower_update.user_id, borrower_id);
        assert_eq!(
            borrower_update.update_type,
            BalanceUpdateType::CreditFromOrigination
        );
        assert_eq!(borrower_update.amount, amount);

        // Second update should be debit for lender
        let lender_update = &updates[1];
        assert_eq!(lender_update.user_id, lender_id);
        assert_eq!(
            lender_update.update_type,
            BalanceUpdateType::DebitFromOrigination
        );
        assert_eq!(lender_update.amount, amount);
    }

    #[test]
    fn test_balance_nullifier_generation() {
        let update = BalanceUpdate::new(
            [1u8; 32],
            [2u8; 32],
            Some([3u8; 32]),
            Some([4u8; 32]),
            BalanceUpdateType::CreditFromPayment,
            50000u128,
            100_000u128,
            1_640_995_200,
            None,
        )
        .unwrap();

        let nullifier1 = update.to_nullifier();
        let nullifier2 = update.to_nullifier();

        // Same update should produce same nullifier
        assert_eq!(nullifier1, nullifier2);

        // Different update should produce different nullifier
        let different_update = BalanceUpdate::new(
            [5u8; 32], // Different ID
            [2u8; 32],
            Some([3u8; 32]),
            Some([4u8; 32]),
            BalanceUpdateType::CreditFromPayment,
            50000u128,
            100_000u128,
            1_640_995_200,
            None,
        )
        .unwrap();

        assert_ne!(nullifier1, different_update.to_nullifier());
    }

    #[test]
    fn test_balance_inclusion_proof_verification() {
        let balance_update = BalanceUpdate::new(
            [1u8; 32],
            [2u8; 32],
            Some([3u8; 32]),
            Some([4u8; 32]),
            BalanceUpdateType::CreditFromPayment,
            50000u128,
            100_000u128,
            1_640_995_200,
            None,
        )
        .unwrap();

        use sha2::{Digest, Sha256};

        // Create a simple merkle path for testing
        let merkle_path = vec![[5u8; 32], [6u8; 32]];
        let leaf_index = 0;

        // Calculate what the root should be with this path
        let mut current_hash = balance_update.to_nullifier();
        let mut index = leaf_index;

        for sibling in &merkle_path {
            let mut hasher = Sha256::new();
            if index % 2 == 0 {
                hasher.update(current_hash);
                hasher.update(sibling);
            } else {
                hasher.update(sibling);
                hasher.update(current_hash);
            }
            current_hash = hasher.finalize().into();
            index /= 2;
        }

        let merkle_root = current_hash;

        let proof = BalanceInclusionProof::new(
            balance_update,
            merkle_path,
            merkle_root,
            [7u8; 32],
            leaf_index,
        );

        // Proof should verify correctly
        assert!(proof.verify());

        // Proof with wrong root should fail
        let wrong_proof = BalanceInclusionProof::new(
            proof.balance_update.clone(),
            proof.merkle_path.clone(),
            [99u8; 32], // Wrong root
            proof.batch_id,
            proof.leaf_index,
        );

        assert!(!wrong_proof.verify());
    }
}
