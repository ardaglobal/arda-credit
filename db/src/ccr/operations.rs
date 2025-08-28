//! CCR Operations Module
//!
//! This module provides prefix-based nullifier generation for different CCR operation types
//! to prevent nullifier collisions in the Indexed Merkle Tree.
//!
//! Each operation type has a unique prefix that ensures nullifiers for different operation
//! types never conflict, even if they hash to the same base value.

use blake3;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Operation type prefixes for nullifier generation
/// These large prefixes ensure no collisions between operation types
const LOAN_CREATION_PREFIX: i64 = 1_000_000_000;
const LOAN_FUNDING_PREFIX: i64 = 2_000_000_000;
const PAYMENT_PREFIX: i64 = 3_000_000_000;
const BALANCE_UPDATE_PREFIX: i64 = 4_000_000_000;

/// CCR Operation types that map to existing loan system operations
/// Each variant corresponds to a different type of operation in the credit system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CcrOperation {
    /// Loan origination/creation operation
    LoanCreation {
        loan_id: Uuid,
        borrower_id: Uuid,
        lender_id: Uuid,
        principal_amount: i64, // Amount in smallest currency unit (e.g., cents)
        created_at: DateTime<Utc>,
    },
    /// Loan funding operation (when funds are disbursed)
    LoanFunding {
        loan_id: Uuid,
        funder_id: Uuid,
        amount: i64,
        created_at: DateTime<Utc>,
    },
    /// Payment operation (loan payment or repayment)
    Payment {
        payment_id: Uuid,
        loan_id: Uuid,
        payer_id: Uuid,
        amount: i64,
        created_at: DateTime<Utc>,
    },
    /// Balance update operation (account balance changes)
    BalanceUpdate {
        balance_id: Uuid,
        identity_id: Uuid,
        amount_delta: i64, // Change in balance (can be negative)
        new_balance: i64,
        created_at: DateTime<Utc>,
    },
}

impl CcrOperation {
    /// Generate a deterministic nullifier for this operation
    ///
    /// This follows the same blake3-based approach as the existing `transaction_to_nullifier`
    /// method in `api/src/unified_batch_service.rs` but adds operation-type prefixes to
    /// prevent collisions between different operation types.
    ///
    /// # Returns
    ///
    /// A positive i64 nullifier value compatible with the IndexedMerkleTree requirements
    pub fn to_nullifier(&self) -> i64 {
        let mut hasher = blake3::Hasher::new();

        // Hash common fields and variant-specific data
        match self {
            CcrOperation::LoanCreation {
                loan_id,
                borrower_id,
                lender_id,
                principal_amount,
                created_at,
            } => {
                hasher.update(loan_id.as_bytes());
                hasher.update(borrower_id.as_bytes());
                hasher.update(lender_id.as_bytes());
                hasher.update(&principal_amount.to_le_bytes());
                hasher.update(&created_at.timestamp().to_le_bytes());
            }
            CcrOperation::LoanFunding {
                loan_id,
                funder_id,
                amount,
                created_at,
            } => {
                hasher.update(loan_id.as_bytes());
                hasher.update(funder_id.as_bytes());
                hasher.update(&amount.to_le_bytes());
                hasher.update(&created_at.timestamp().to_le_bytes());
            }
            CcrOperation::Payment {
                payment_id,
                loan_id,
                payer_id,
                amount,
                created_at,
            } => {
                hasher.update(payment_id.as_bytes());
                hasher.update(loan_id.as_bytes());
                hasher.update(payer_id.as_bytes());
                hasher.update(&amount.to_le_bytes());
                hasher.update(&created_at.timestamp().to_le_bytes());
            }
            CcrOperation::BalanceUpdate {
                balance_id,
                identity_id,
                amount_delta,
                new_balance,
                created_at,
            } => {
                hasher.update(balance_id.as_bytes());
                hasher.update(identity_id.as_bytes());
                hasher.update(&amount_delta.to_le_bytes());
                hasher.update(&new_balance.to_le_bytes());
                hasher.update(&created_at.timestamp().to_le_bytes());
            }
        }

        // Get the first 8 bytes of the digest as a u64 (little-endian)
        let digest = hasher.finalize();
        let hash_bytes = digest.as_bytes();
        let hash_u64 = u64::from_le_bytes([
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
            hash_bytes[4],
            hash_bytes[5],
            hash_bytes[6],
            hash_bytes[7],
        ]);

        // Convert to positive i64 in a smaller range to leave room for the prefix
        // We use a smaller modulo to ensure prefix + hash doesn't overflow i64
        let base_nullifier = (hash_u64 % 999_999_999) as i64 + 1;

        // Add the operation-type prefix to prevent collisions
        let prefix = self.get_prefix();
        prefix + base_nullifier
    }

    /// Get the prefix for this operation type
    fn get_prefix(&self) -> i64 {
        match self {
            CcrOperation::LoanCreation { .. } => LOAN_CREATION_PREFIX,
            CcrOperation::LoanFunding { .. } => LOAN_FUNDING_PREFIX,
            CcrOperation::Payment { .. } => PAYMENT_PREFIX,
            CcrOperation::BalanceUpdate { .. } => BALANCE_UPDATE_PREFIX,
        }
    }

    /// Get a human-readable operation type name
    pub fn operation_type(&self) -> &'static str {
        match self {
            CcrOperation::LoanCreation { .. } => "loan_creation",
            CcrOperation::LoanFunding { .. } => "loan_funding",
            CcrOperation::Payment { .. } => "payment",
            CcrOperation::BalanceUpdate { .. } => "balance_update",
        }
    }

    /// Get the primary identifier for this operation (e.g., loan_id, payment_id, etc.)
    pub fn primary_id(&self) -> Uuid {
        match self {
            CcrOperation::LoanCreation { loan_id, .. } => *loan_id,
            CcrOperation::LoanFunding { loan_id, .. } => *loan_id,
            CcrOperation::Payment { payment_id, .. } => *payment_id,
            CcrOperation::BalanceUpdate { balance_id, .. } => *balance_id,
        }
    }

    /// Get the timestamp when this operation was created
    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            CcrOperation::LoanCreation { created_at, .. } => *created_at,
            CcrOperation::LoanFunding { created_at, .. } => *created_at,
            CcrOperation::Payment { created_at, .. } => *created_at,
            CcrOperation::BalanceUpdate { created_at, .. } => *created_at,
        }
    }

    /// Get the amount associated with this operation (if applicable)
    pub fn amount(&self) -> Option<i64> {
        match self {
            CcrOperation::LoanCreation {
                principal_amount, ..
            } => Some(*principal_amount),
            CcrOperation::LoanFunding { amount, .. } => Some(*amount),
            CcrOperation::Payment { amount, .. } => Some(*amount),
            CcrOperation::BalanceUpdate { amount_delta, .. } => Some(*amount_delta),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_nullifier_generation_deterministic() {
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let created_at = Utc::now();

        let operation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: 100_000,
            created_at,
        };

        // Same operation should generate same nullifier
        let nullifier1 = operation.to_nullifier();
        let nullifier2 = operation.to_nullifier();
        assert_eq!(nullifier1, nullifier2);
    }

    #[test]
    fn test_nullifiers_are_positive() {
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let payer_id = Uuid::new_v4();
        let balance_id = Uuid::new_v4();
        let identity_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();
        let created_at = Utc::now();

        let operations = vec![
            CcrOperation::LoanCreation {
                loan_id,
                borrower_id,
                lender_id,
                principal_amount: 100_000,
                created_at,
            },
            CcrOperation::LoanFunding {
                loan_id,
                funder_id: lender_id,
                amount: 100_000,
                created_at,
            },
            CcrOperation::Payment {
                payment_id,
                loan_id,
                payer_id,
                amount: 5000,
                created_at,
            },
            CcrOperation::BalanceUpdate {
                balance_id,
                identity_id,
                amount_delta: -5000,
                new_balance: 95000,
                created_at,
            },
        ];

        for operation in operations {
            let nullifier = operation.to_nullifier();
            assert!(nullifier > 0, "Nullifier should be positive: {}", nullifier);
            assert!(nullifier <= i64::MAX, "Nullifier should not overflow i64");
        }
    }

    #[test]
    fn test_different_operation_types_different_prefixes() {
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();
        let balance_id = Uuid::new_v4();
        let created_at = Utc::now();

        let loan_creation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: 100_000,
            created_at,
        };

        let loan_funding = CcrOperation::LoanFunding {
            loan_id,
            funder_id: lender_id,
            amount: 100_000,
            created_at,
        };

        let payment = CcrOperation::Payment {
            payment_id,
            loan_id,
            payer_id: borrower_id,
            amount: 5000,
            created_at,
        };

        let balance_update = CcrOperation::BalanceUpdate {
            balance_id,
            identity_id: borrower_id,
            amount_delta: -5000,
            new_balance: 95000,
            created_at,
        };

        let nullifiers = vec![
            loan_creation.to_nullifier(),
            loan_funding.to_nullifier(),
            payment.to_nullifier(),
            balance_update.to_nullifier(),
        ];

        // Verify all nullifiers are different
        for i in 0..nullifiers.len() {
            for j in (i + 1)..nullifiers.len() {
                assert_ne!(
                    nullifiers[i], nullifiers[j],
                    "Nullifiers should be different: {} vs {}",
                    nullifiers[i], nullifiers[j]
                );
            }
        }

        // Verify prefixes are correctly applied
        assert!(loan_creation.to_nullifier() >= LOAN_CREATION_PREFIX);
        assert!(loan_creation.to_nullifier() < LOAN_CREATION_PREFIX + 1_000_000_000);

        assert!(loan_funding.to_nullifier() >= LOAN_FUNDING_PREFIX);
        assert!(loan_funding.to_nullifier() < LOAN_FUNDING_PREFIX + 1_000_000_000);

        assert!(payment.to_nullifier() >= PAYMENT_PREFIX);
        assert!(payment.to_nullifier() < PAYMENT_PREFIX + 1_000_000_000);

        assert!(balance_update.to_nullifier() >= BALANCE_UPDATE_PREFIX);
        assert!(balance_update.to_nullifier() < BALANCE_UPDATE_PREFIX + 1_000_000_000);
    }

    #[test]
    fn test_same_data_different_operations_different_nullifiers() {
        // Even with the same underlying data (like loan_id), different operation types
        // should produce different nullifiers due to prefixes
        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let created_at = Utc::now();
        let amount = 100_000;

        let loan_creation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id: user_id,
            lender_id: user_id, // Same user for simplicity
            principal_amount: amount,
            created_at,
        };

        let loan_funding = CcrOperation::LoanFunding {
            loan_id,
            funder_id: user_id,
            amount,
            created_at,
        };

        assert_ne!(
            loan_creation.to_nullifier(),
            loan_funding.to_nullifier(),
            "Same data in different operation types should produce different nullifiers"
        );
    }

    #[test]
    fn test_operation_metadata_methods() {
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let created_at = Utc::now();

        let operation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: 100_000,
            created_at,
        };

        assert_eq!(operation.operation_type(), "loan_creation");
        assert_eq!(operation.primary_id(), loan_id);
        assert_eq!(operation.created_at(), created_at);
        assert_eq!(operation.amount(), Some(100_000));
    }

    #[test]
    fn test_prefix_constants() {
        // Ensure prefixes are properly spaced to avoid collisions
        assert_eq!(LOAN_CREATION_PREFIX, 1_000_000_000);
        assert_eq!(LOAN_FUNDING_PREFIX, 2_000_000_000);
        assert_eq!(PAYMENT_PREFIX, 3_000_000_000);
        assert_eq!(BALANCE_UPDATE_PREFIX, 4_000_000_000);

        // Verify no overlap in prefix ranges
        let prefixes = [
            LOAN_CREATION_PREFIX,
            LOAN_FUNDING_PREFIX,
            PAYMENT_PREFIX,
            BALANCE_UPDATE_PREFIX,
        ];

        for (i, &prefix1) in prefixes.iter().enumerate() {
            for &prefix2 in prefixes.iter().skip(i + 1) {
                assert!(
                    (prefix1 + 999_999_999) < prefix2,
                    "Prefix ranges should not overlap: {} and {}",
                    prefix1,
                    prefix2
                );
            }
        }
    }

    #[test]
    fn test_nullifier_collision_resistance() {
        // Generate a large number of operations and verify no collisions
        use std::collections::HashSet;

        let mut nullifiers = HashSet::new();
        let base_time = Utc::now();

        for i in 0..1000 {
            let loan_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();
            let created_at = base_time + chrono::Duration::seconds(i);

            let operations = vec![
                CcrOperation::LoanCreation {
                    loan_id,
                    borrower_id: user_id,
                    lender_id: Uuid::new_v4(),
                    principal_amount: i,
                    created_at,
                },
                CcrOperation::Payment {
                    payment_id: Uuid::new_v4(),
                    loan_id,
                    payer_id: user_id,
                    amount: i,
                    created_at,
                },
            ];

            for operation in operations {
                let nullifier = operation.to_nullifier();
                assert!(
                    nullifiers.insert(nullifier),
                    "Nullifier collision detected: {}",
                    nullifier
                );
            }
        }
    }
}
