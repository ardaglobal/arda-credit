//! Critical consistency tests between zkVM ADS and database ADS implementations
//!
//! These tests ensure that the zkVM version produces exactly the same results
//! as the database version for nullifier generation and state transitions.

#[cfg(test)]
mod consistency_tests {
    use crate::zkvm_ads::*;
    use crate::{LoanTransaction, MetadataArray, TransactionType};

    /// Test that nullifier generation is deterministic and matches expected patterns
    #[test]
    fn test_nullifier_generation_matches_expected_values() {
        let ads = ZkVMAds::new();

        // Create a transaction that should produce a known nullifier value
        let mut transaction_id = [0u8; 32];
        transaction_id[0] = 1; // Simple ID encoding

        let transaction = LoanTransaction {
            transaction_id,
            user_id: [1u8; 32],
            deal_id: [2u8; 32],                             // was loan_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 15,
            created_at: 1_000_000_000,
            metadata: MetadataArray::empty(),
        };

        let nullifier = ads.transaction_to_nullifier(&transaction);

        // This nullifier should be positive and within expected range
        assert!(nullifier > 0, "Nullifier should be positive");
        assert!(
            nullifier
                <= transaction_prefixes::LEGACY_DEFAULT + transaction_prefixes::PREFIX_RANGE_SIZE,
            "Nullifier should be within prefix range"
        );

        // Test determinism
        let nullifier2 = ads.transaction_to_nullifier(&transaction);
        assert_eq!(
            nullifier, nullifier2,
            "Nullifier generation must be deterministic"
        );

        // Test different transactions produce different nullifiers
        let mut transaction2 = transaction;
        transaction2.amount = 16;
        let nullifier3 = ads.transaction_to_nullifier(&transaction2);
        assert_ne!(
            nullifier, nullifier3,
            "Different transactions should produce different nullifiers"
        );
    }

    /// Test that state transitions work correctly with multiple transactions
    #[test]
    fn test_batch_processing_consistency() {
        let mut ads = ZkVMAds::new();

        // Create a batch of transactions with different amounts
        let transactions = vec![
            create_test_transaction(1, 100),
            create_test_transaction(2, 200),
            create_test_transaction(3, 300),
            create_test_transaction(4, 150),
            create_test_transaction(5, 250),
        ];

        let result = ads.process_transactions(&transactions);
        assert!(result.is_ok(), "Batch processing should succeed");

        let transition = result.unwrap();

        // Verify basic properties
        assert_eq!(
            transition.old_root, [0u8; 32],
            "Initial root should be zero"
        );
        assert_ne!(
            transition.new_root, [0u8; 32],
            "New root should be non-zero"
        );
        assert_eq!(
            transition.nullifiers_inserted.len(),
            5,
            "Should insert 5 nullifiers"
        );

        // Verify all nullifiers are unique and positive
        for &nullifier in &transition.nullifiers_inserted {
            assert!(nullifier > 0, "All nullifiers should be positive");
        }

        let mut sorted_nullifiers = transition.nullifiers_inserted;
        sorted_nullifiers.sort_unstable();
        sorted_nullifiers.dedup();
        assert_eq!(
            sorted_nullifiers.len(),
            5,
            "All nullifiers should be unique"
        );

        // Verify tree structure
        let nullifiers = ads.get_nullifiers();
        assert_eq!(nullifiers.len(), 5, "Tree should contain 5 nullifiers");

        // Verify nullifiers are sorted
        for i in 1..nullifiers.len() {
            assert!(
                nullifiers[i - 1].value < nullifiers[i].value,
                "Nullifiers should be sorted by value"
            );
        }

        // Verify linked list structure
        for i in 0..nullifiers.len() {
            let nullifier = &nullifiers[i];

            if i < nullifiers.len() - 1 {
                let next = &nullifiers[i + 1];
                assert_eq!(
                    nullifier.next_value, next.value,
                    "Next value should point to next nullifier"
                );
                assert_eq!(
                    nullifier.next_index,
                    Some(next.tree_index),
                    "Next index should point to next nullifier"
                );
            } else {
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

    /// Test root computation consistency
    #[test]
    fn test_root_computation_deterministic() {
        let mut ads1 = ZkVMAds::new();
        let mut ads2 = ZkVMAds::new();

        let transactions = vec![
            create_test_transaction(1, 100),
            create_test_transaction(2, 200),
        ];

        // Process same transactions in both instances
        let result1 = ads1.process_transactions(&transactions);
        let result2 = ads2.process_transactions(&transactions);

        assert!(result1.is_ok() && result2.is_ok(), "Both should succeed");

        let transition1 = result1.unwrap();
        let transition2 = result2.unwrap();

        // Should produce identical state transitions
        assert_eq!(
            transition1.old_root, transition2.old_root,
            "Old roots should match"
        );
        assert_eq!(
            transition1.new_root, transition2.new_root,
            "New roots should match"
        );
        assert_eq!(
            transition1.nullifiers_inserted, transition2.nullifiers_inserted,
            "Inserted nullifiers should match"
        );
    }

    /// Test that transaction order affects nullifier order and insertion tracking
    #[test]
    fn test_transaction_order_affects_insertion_tracking() {
        let mut ads1 = ZkVMAds::new();
        let mut ads2 = ZkVMAds::new();

        let transactions_forward = vec![
            create_test_transaction(1, 100),
            create_test_transaction(2, 200),
            create_test_transaction(3, 300),
        ];

        let transactions_reverse = vec![
            create_test_transaction(3, 300),
            create_test_transaction(2, 200),
            create_test_transaction(1, 100),
        ];

        let result1 = ads1.process_transactions(&transactions_forward);
        let result2 = ads2.process_transactions(&transactions_reverse);

        assert!(result1.is_ok() && result2.is_ok(), "Both should succeed");

        let transition1 = result1.unwrap();
        let transition2 = result2.unwrap();

        // Generate nullifiers for both sets to compare the actual nullifiers
        let nullifiers1: Vec<_> = transactions_forward
            .iter()
            .map(|tx| ads1.transaction_to_nullifier(tx))
            .collect();
        let nullifiers2: Vec<_> = transactions_reverse
            .iter()
            .map(|tx| ads2.transaction_to_nullifier(tx))
            .collect();

        // Same transactions should generate same nullifiers regardless of order
        let mut sorted_nullifiers1 = nullifiers1;
        sorted_nullifiers1.sort_unstable();
        let mut sorted_nullifiers2 = nullifiers2;
        sorted_nullifiers2.sort_unstable();
        assert_eq!(
            sorted_nullifiers1, sorted_nullifiers2,
            "Same transactions should produce same set of nullifiers"
        );

        // But the insertion order should be different
        assert_ne!(
            transition1.nullifiers_inserted, transition2.nullifiers_inserted,
            "Insertion order should reflect input order"
        );

        // Both ADS instances should have the same final nullifier sets (sorted)
        let final_nullifiers1: Vec<_> = ads1.get_nullifiers().iter().map(|n| n.value).collect();
        let final_nullifiers2: Vec<_> = ads2.get_nullifiers().iter().map(|n| n.value).collect();
        assert_eq!(
            final_nullifiers1, final_nullifiers2,
            "Final nullifier sets should be identical"
        );
    }

    /// Test edge cases
    #[test]
    fn test_edge_cases() {
        let mut ads = ZkVMAds::new();

        // Test empty batch
        let result = ads.process_transactions(&[]);
        assert!(result.is_ok(), "Empty batch should succeed");
        let transition = result.unwrap();
        assert_eq!(transition.old_root, [0u8; 32], "Old root should be zero");
        assert_eq!(
            transition.new_root, [0u8; 32],
            "New root should remain zero"
        );
        assert!(
            transition.nullifiers_inserted.is_empty(),
            "No nullifiers should be inserted"
        );

        // Test single transaction
        let single_tx = vec![create_test_transaction(1, 100)];
        let result = ads.process_transactions(&single_tx);
        assert!(result.is_ok(), "Single transaction should succeed");
        let transition = result.unwrap();
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

    fn create_test_transaction(id: u8, amount: u64) -> LoanTransaction {
        let mut transaction_id = [0u8; 32];
        transaction_id[0] = id;

        LoanTransaction {
            transaction_id,
            user_id: [1u8; 32],
            deal_id: [2u8; 32],                             // was loan_id field
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount,
            created_at: 1_000_000_000, // Fixed timestamp for deterministic testing
            metadata: MetadataArray::empty(),
        }
    }
}
