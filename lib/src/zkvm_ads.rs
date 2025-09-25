//! zkVM-compatible Authenticated Data Structure operations
//!
//! This module provides Merkle tree operations that can run inside SP1 zkVM
//! while maintaining consistency with the off-chain ADS system.
//!
//! CRITICAL: All logic in this module MUST exactly match the database implementation
//! to ensure state root consistency between off-chain processing and ZK proofs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Re-export transaction prefixes constants to ensure consistency
pub mod transaction_prefixes {
    /// Loan origination transactions: 1,000,000,000 - 1,999,999,999
    pub const LOAN_ORIGINATION: i64 = 1_000_000_000;

    /// Loan closeout transactions: 2,000,000,000 - 2,999,999,999
    pub const LOAN_CLOSEOUT: i64 = 2_000_000_000;

    /// Default/legacy transactions (for backward compatibility): 0 - 999,999,999
    pub const LEGACY_DEFAULT: i64 = 0;

    /// Maximum value within a prefix range (999,999,999)
    pub const PREFIX_RANGE_SIZE: i64 = 999_999_999;
}

/// zkVM-compatible ADS state for Indexed Merkle Tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkVMAds {
    /// Current tree height (should be 32 for compatibility)
    pub tree_height: u32,

    /// Current nullifiers in the tree (sorted by value)
    pub nullifiers: Vec<ZkVMNullifier>,

    /// Total number of nullifiers
    pub nullifier_count: u64,

    /// Next available tree index
    pub next_available_index: u64,
}

/// zkVM-compatible nullifier structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkVMNullifier {
    pub value: i64,
    pub tree_index: u64,
    pub next_index: Option<u64>, // Points to next higher nullifier
    pub next_value: i64,         // Value of next higher nullifier (0 = max)
}

/// Result of a state transition
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub nullifiers_inserted: Vec<i64>,
}

impl ZkVMAds {
    /// Create new empty ADS state
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tree_height: 32,
            nullifiers: Vec::new(),
            nullifier_count: 0,
            next_available_index: 0,
        }
    }

    /// Process transactions and return state transition
    ///
    /// # Errors
    /// Returns error string if nullifier insertion fails due to duplicates or other issues.
    pub fn process_transactions(
        &mut self,
        transactions: &[crate::LoanTransaction],
    ) -> Result<StateTransition, String> {
        let old_root = self.compute_merkle_root();

        // Convert transactions to nullifiers and insert them
        let mut inserted_nullifiers = Vec::new();
        for transaction in transactions {
            let nullifier_value = self.transaction_to_nullifier(transaction);
            self.insert_nullifier(nullifier_value)?;
            inserted_nullifiers.push(nullifier_value);
        }

        let new_root = self.compute_merkle_root();

        Ok(StateTransition {
            old_root,
            new_root,
            nullifiers_inserted: inserted_nullifiers,
        })
    }

    /// Convert transaction to nullifier value
    /// CRITICAL: This must match exactly the logic in `db/src/background_processor.rs`
    #[must_use]
    pub fn transaction_to_nullifier(&self, transaction: &crate::LoanTransaction) -> i64 {
        Self::transaction_to_nullifier_with_prefix(
            transaction,
            transaction_prefixes::LEGACY_DEFAULT,
        )
    }

    /// Convert transaction to nullifier with specific prefix
    /// CRITICAL: This must match exactly `db/src/background_processor.rs::transaction_to_nullifier_with_prefix`
    fn transaction_to_nullifier_with_prefix(
        transaction: &crate::LoanTransaction,
        prefix: i64,
    ) -> i64 {
        // Create a deterministic nullifier from the transaction data using blake3
        // This ensures the same transaction always produces the same nullifier across deployments
        let mut hasher = blake3::Hasher::new();

        // IMPORTANT: For zkVM compatibility, we need to derive transaction fields from LoanTransaction
        // In the database version, this uses IncomingTransaction with amount and id fields
        // We need to derive equivalent values from LoanTransaction

        // Hash transaction amount (convert from u64 to i32 for compatibility)
        let amount_i32 = i32::try_from(transaction.amount).unwrap_or(i32::MAX);
        hasher.update(&amount_i32.to_le_bytes());

        // Hash a unique identifier derived from transaction_id
        // Convert the transaction_id bytes to an i64 for compatibility
        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&transaction.transaction_id[0..8]);
        let transaction_id = i64::from_le_bytes(id_bytes);
        hasher.update(&transaction_id.to_le_bytes());

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

        // Convert to positive value within prefix range
        // Take modulo to fit within the prefix range (999,999,999), then add 1 to avoid zero
        let base_nullifier = i64::try_from(
            hash_u64 % u64::try_from(transaction_prefixes::PREFIX_RANGE_SIZE).unwrap_or(u64::MAX),
        )
        .unwrap_or(i64::MAX)
        .wrapping_add(1);

        // Apply prefix for namespace isolation
        prefix + base_nullifier
    }

    /// Insert nullifier into indexed Merkle tree
    fn insert_nullifier(&mut self, nullifier: i64) -> Result<(), String> {
        // Check for duplicate nullifier
        if self.nullifiers.iter().any(|n| n.value == nullifier) {
            return Err(format!("Nullifier {nullifier} already exists"));
        }

        // Find insertion point using binary search
        let insertion_index = self.find_insertion_index(nullifier);

        // Create new nullifier entry
        let new_nullifier = ZkVMNullifier {
            value: nullifier,
            tree_index: self.next_available_index,
            next_index: None, // Will be set during insertion
            next_value: 0,    // Will be set during insertion
        };

        // Update tree structure (this is the complex part)
        self.update_tree_structure(insertion_index, new_nullifier);

        self.nullifier_count += 1;
        self.next_available_index += 1;

        Ok(())
    }

    /// Find where to insert nullifier to maintain sorted order
    fn find_insertion_index(&self, nullifier: i64) -> usize {
        // Binary search implementation
        self.nullifiers
            .binary_search_by_key(&nullifier, |n| n.value)
            .unwrap_or_else(|pos| pos)
    }

    /// Update tree structure after nullifier insertion
    /// This implements the indexed Merkle tree insertion logic
    fn update_tree_structure(&mut self, insertion_index: usize, mut nullifier: ZkVMNullifier) {
        // Handle empty tree case
        if self.nullifiers.is_empty() {
            nullifier.next_value = 0; // 0 means this is the maximum value
            nullifier.next_index = None;
            self.nullifiers.push(nullifier);
            return;
        }

        // Set next pointers for the new nullifier
        if insertion_index < self.nullifiers.len() {
            // There's a next nullifier
            let next_nullifier = &self.nullifiers[insertion_index];
            nullifier.next_value = next_nullifier.value;
            nullifier.next_index = Some(next_nullifier.tree_index);
        } else {
            // This is the new maximum
            nullifier.next_value = 0;
            nullifier.next_index = None;
        }

        // Update the previous nullifier's next pointer if it exists
        if insertion_index > 0 {
            let prev_index = insertion_index - 1;
            self.nullifiers[prev_index].next_value = nullifier.value;
            self.nullifiers[prev_index].next_index = Some(nullifier.tree_index);
        }

        // Insert the new nullifier at the correct position
        self.nullifiers.insert(insertion_index, nullifier);
    }

    /// Compute current Merkle root
    #[must_use]
    pub fn compute_merkle_root(&self) -> [u8; 32] {
        if self.nullifiers.is_empty() {
            return [0u8; 32]; // Empty tree root
        }

        // Build Merkle tree from current nullifier set
        let leaves: Vec<[u8; 32]> = self.nullifiers.iter().map(Self::hash_nullifier).collect();

        Self::compute_merkle_root_from_leaves(&leaves)
    }

    /// Hash a nullifier for Merkle tree leaf
    /// This creates a leaf node hash from nullifier data
    fn hash_nullifier(nullifier: &ZkVMNullifier) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(nullifier.value.to_le_bytes());
        hasher.update(nullifier.tree_index.to_le_bytes());
        hasher.update(nullifier.next_value.to_le_bytes());
        if let Some(next_idx) = nullifier.next_index {
            hasher.update(next_idx.to_le_bytes());
        } else {
            hasher.update([0u8; 8]); // Pad for None case
        }
        hasher.finalize().into()
    }

    /// Compute Merkle root from leaf hashes using standard binary tree construction
    fn compute_merkle_root_from_leaves(leaves: &[[u8; 32]]) -> [u8; 32] {
        if leaves.is_empty() {
            return [0u8; 32];
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut current_level = leaves.to_vec();

        // Build tree level by level until we reach the root
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            // Process pairs of nodes
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(chunk[0], chunk[1])
                } else {
                    // Odd number of nodes, hash with zero padding
                    Self::hash_pair(chunk[0], [0u8; 32])
                };
                next_level.push(hash);
            }

            current_level = next_level;
        }

        current_level[0]
    }

    /// Hash two nodes together for parent node
    fn hash_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    /// Get the current root hash
    #[must_use]
    pub fn get_root(&self) -> [u8; 32] {
        self.compute_merkle_root()
    }

    /// Get current state as vector for serialization
    #[must_use]
    pub fn get_nullifiers(&self) -> &[ZkVMNullifier] {
        &self.nullifiers
    }
}

impl Default for ZkVMAds {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoanTransaction, MetadataArray, TransactionType};

    fn create_test_transaction(id: u8, amount: u64) -> LoanTransaction {
        let mut transaction_id = [0u8; 32];
        transaction_id[0] = id;

        LoanTransaction {
            transaction_id,
            user_id: [1u8; 32],
            deal_id: [2u8; 32], // was loan_id field
            transaction_type: TransactionType::DealPayment,  // was LoanPayment
            amount,
            created_at: 1_000_000_000, // Fixed timestamp for deterministic testing
            metadata: MetadataArray::empty(),
        }
    }

    #[test]
    fn test_nullifier_generation_deterministic() {
        let ads = ZkVMAds::new();
        let transaction = create_test_transaction(1, 15);

        let nullifier1 = ads.transaction_to_nullifier(&transaction);
        let nullifier2 = ads.transaction_to_nullifier(&transaction);

        assert_eq!(nullifier1, nullifier2, "Nullifier should be deterministic");
        assert!(nullifier1 > 0, "Nullifier should be positive");
    }

    #[test]
    fn test_empty_tree_root() {
        let ads = ZkVMAds::new();
        let root = ads.compute_merkle_root();
        assert_eq!(root, [0u8; 32], "Empty tree should have zero root");
    }

    #[test]
    fn test_single_nullifier_insertion() {
        let mut ads = ZkVMAds::new();
        let transaction = create_test_transaction(1, 100);

        let result = ads.process_transactions(&[transaction]);
        assert!(
            result.is_ok(),
            "Single transaction processing should succeed"
        );

        let transition = result.unwrap();
        assert_eq!(transition.old_root, [0u8; 32], "Old root should be zero");
        assert_ne!(
            transition.new_root, [0u8; 32],
            "New root should be non-zero"
        );
        assert_eq!(
            transition.nullifiers_inserted.len(),
            1,
            "Should insert one nullifier"
        );
    }

    #[test]
    fn test_multiple_nullifier_insertion() {
        let mut ads = ZkVMAds::new();
        let transactions = vec![
            create_test_transaction(1, 100),
            create_test_transaction(2, 200),
            create_test_transaction(3, 300),
        ];

        let result = ads.process_transactions(&transactions);
        assert!(
            result.is_ok(),
            "Multiple transaction processing should succeed"
        );

        let transition = result.unwrap();
        assert_eq!(
            transition.nullifiers_inserted.len(),
            3,
            "Should insert three nullifiers"
        );

        // Check that nullifiers are sorted
        let nullifiers = ads.get_nullifiers();
        assert_eq!(nullifiers.len(), 3, "Should have three nullifiers");
        for i in 1..nullifiers.len() {
            assert!(
                nullifiers[i - 1].value < nullifiers[i].value,
                "Nullifiers should be sorted"
            );
        }
    }

    #[test]
    fn test_duplicate_nullifier_rejection() {
        let mut ads = ZkVMAds::new();
        let transaction = create_test_transaction(1, 100);

        // First insertion should succeed
        let result1 = ads.process_transactions(&[transaction.clone()]);
        assert!(result1.is_ok(), "First insertion should succeed");

        // Second insertion of same transaction should fail
        let result2 = ads.process_transactions(&[transaction]);
        assert!(result2.is_err(), "Duplicate insertion should fail");
    }

    #[test]
    fn test_tree_structure_consistency() {
        let mut ads = ZkVMAds::new();
        let transactions = vec![
            create_test_transaction(3, 300), // Insert out of order to test sorting
            create_test_transaction(1, 100),
            create_test_transaction(2, 200),
        ];

        let result = ads.process_transactions(&transactions);
        assert!(result.is_ok(), "Processing should succeed");

        // Verify the linked list structure
        let nullifiers = ads.get_nullifiers();
        for i in 0..nullifiers.len() {
            let nullifier = &nullifiers[i];

            if i < nullifiers.len() - 1 {
                // Not the last element - should point to next
                let next = &nullifiers[i + 1];
                assert_eq!(nullifier.next_value, next.value, "Next value should match");
                assert_eq!(
                    nullifier.next_index,
                    Some(next.tree_index),
                    "Next index should match"
                );
            } else {
                // Last element - should have next_value = 0 and next_index = None
                assert_eq!(
                    nullifier.next_value, 0,
                    "Last nullifier should have next_value = 0"
                );
                assert_eq!(
                    nullifier.next_index, None,
                    "Last nullifier should have next_index = None"
                );
            }
        }
    }
}
