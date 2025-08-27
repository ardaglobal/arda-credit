use chrono::Utc;
use db::error::DbError;
use db::merkle_tree::*;
use sqlx::PgPool;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_loan_aware_imt_operations(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        // Test loan operation insertion
        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let operation_amount = rust_decimal::Decimal::new(10000, 2); // $100.00

        let result = imt
            .insert_loan_operation(
                12345,
                "loan_origination".to_string(),
                Some(loan_id),
                Some(user_id),
                Some(operation_amount),
                Some(serde_json::json!({
                    "interest_rate": "5.0",
                    "term_months": 12
                })),
                None, // batch_id
            )
            .await?;

        assert!(result.new_root != result.old_root);
        assert_eq!(
            result.nullifier.loan_operation_type,
            Some("loan_origination".to_string())
        );
        assert_eq!(result.nullifier.loan_id, Some(loan_id));

        Ok(())
    }

    #[sqlx::test]
    async fn test_loan_operation_history_queries(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Insert multiple loan operations
        let operations = vec![
            ("loan_origination", 10000),
            ("loan_payment", 1000),
            ("loan_payment", 1500),
        ];

        for (i, (op_type, amount)) in operations.iter().enumerate() {
            imt.insert_loan_operation(
                10000 + i as i64,
                op_type.to_string(),
                Some(loan_id),
                Some(user_id),
                Some(rust_decimal::Decimal::new(*amount, 2)),
                Some(serde_json::json!({})),
                None,
            )
            .await?;
        }

        // Test loan operation history retrieval
        let history = imt.get_loan_operation_history(loan_id, Some(10)).await?;
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].operation_type, "loan_origination");

        // Test user loan operations retrieval
        let user_operations = imt
            .get_user_loan_operations(user_id, None, Some(10))
            .await?;
        assert_eq!(user_operations.len(), 3);

        // Test user loan operations with type filter
        let payments = imt
            .get_user_loan_operations(user_id, Some("loan_payment".to_string()), Some(10))
            .await?;
        assert_eq!(payments.len(), 2);

        Ok(())
    }

    #[sqlx::test]
    async fn test_loan_statistics_and_views(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        let loan_id1 = Uuid::new_v4();
        let loan_id2 = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Insert loan operations for statistics testing
        let operations = vec![
            (loan_id1, "loan_origination", 50000),
            (loan_id1, "loan_payment", 5000),
            (loan_id2, "loan_origination", 30000),
            (loan_id2, "loan_payment", 3000),
        ];

        for (i, (loan_id, op_type, amount)) in operations.iter().enumerate() {
            imt.insert_loan_operation(
                20000 + i as i64,
                op_type.to_string(),
                Some(*loan_id),
                Some(user_id),
                Some(rust_decimal::Decimal::new(*amount, 2)),
                Some(serde_json::json!({})),
                None,
            )
            .await?;
        }

        // Test IMT loan statistics
        let stats = imt.get_imt_loan_stats().await?;
        if let Some(stats) = stats {
            assert!(stats.total_loan_nullifiers.unwrap_or(0) >= 4);
            assert!(stats.total_loan_originations.unwrap_or(0) >= 2);
            assert!(stats.total_loan_payments.unwrap_or(0) >= 2);
            assert!(stats.unique_loans.unwrap_or(0) >= 2);
        }

        // Test refresh loan statistics
        imt.refresh_loan_statistics().await?;

        Ok(())
    }

    #[sqlx::test]
    async fn test_loan_audit_trail_integration(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let amount = rust_decimal::Decimal::new(25000, 2);

        // Insert loan operation (should trigger audit trail)
        imt.insert_loan_operation(
            30000,
            "loan_origination".to_string(),
            Some(loan_id),
            Some(user_id),
            Some(amount),
            Some(serde_json::json!({
                "source": "test_integration"
            })),
            None,
        )
        .await?;

        // Test loan audit trail retrieval
        let audit_trail = imt.get_loan_audit_trail(loan_id, Some(10)).await?;
        assert!(!audit_trail.is_empty());

        let first_event = &audit_trail[0];
        assert_eq!(
            first_event.loan_operation_type,
            Some("loan_origination".to_string())
        );
        assert_eq!(first_event.operation_amount, Some(amount));

        // Test user audit trail
        let user_audit = imt
            .get_user_loan_audit_trail(user_id, None, Some(10))
            .await?;
        assert!(!user_audit.is_empty());

        // Test comprehensive audit view
        let comprehensive = imt
            .get_loan_audit_comprehensive(Some(loan_id), Some(10))
            .await?;
        assert!(!comprehensive.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn test_amount_range_queries(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Insert operations with different amounts
        let operations = vec![
            ("loan_origination", 50000), // $500.00
            ("loan_payment", 10000),     // $100.00
            ("loan_payment", 25000),     // $250.00
        ];

        for (i, (op_type, amount)) in operations.iter().enumerate() {
            imt.insert_loan_operation(
                40000 + i as i64,
                op_type.to_string(),
                Some(loan_id),
                Some(user_id),
                Some(rust_decimal::Decimal::new(*amount, 2)),
                Some(serde_json::json!({})),
                None,
            )
            .await?;
        }

        // Test amount range query
        let min_amount = rust_decimal::Decimal::new(15000, 2); // $150.00
        let max_amount = rust_decimal::Decimal::new(60000, 2); // $600.00

        let range_results = imt
            .get_loan_operations_by_amount_range(min_amount, max_amount, None)
            .await?;

        // Should find loan_origination ($500) and one payment ($250)
        assert_eq!(range_results.len(), 2);

        // Test with operation type filter
        let payment_range = imt
            .get_loan_operations_by_amount_range(
                min_amount,
                max_amount,
                Some("loan_payment".to_string()),
            )
            .await?;

        // Should find only the $250 payment
        assert_eq!(payment_range.len(), 1);
        assert_eq!(
            payment_range[0].operation_amount,
            Some(rust_decimal::Decimal::new(25000, 2))
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_batch_processing_integration(pool: PgPool) -> Result<(), DbError> {
        let imt = IndexedMerkleTree::new(pool);

        let loan_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Insert unbatched loan operations
        let operations = vec![("loan_origination", 100000), ("loan_payment", 10000)];

        for (i, (op_type, amount)) in operations.iter().enumerate() {
            imt.insert_loan_operation(
                50000 + i as i64,
                op_type.to_string(),
                Some(loan_id),
                Some(user_id),
                Some(rust_decimal::Decimal::new(*amount, 2)),
                Some(serde_json::json!({})),
                None, // No batch_id initially
            )
            .await?;
        }

        // Get unbatched operations
        let unbatched = imt.get_unbatched_loan_operations(Some(10)).await?;
        assert_eq!(unbatched.len(), 2);

        // Mark operations as batched
        let nullifier_ids: Vec<i64> = unbatched.iter().map(|n| n.id).collect();
        let batch_id = 1;

        let affected_rows = imt
            .mark_loan_operations_batched(&nullifier_ids, batch_id)
            .await?;
        assert_eq!(affected_rows, 2);

        // Verify no more unbatched operations
        let remaining_unbatched = imt.get_unbatched_loan_operations(Some(10)).await?;
        assert!(remaining_unbatched.is_empty());

        // Test batch summary
        let batch_summary = imt.get_batch_loan_summary(batch_id).await?;
        if let Some(summary) = batch_summary {
            assert_eq!(summary.total_loan_operations, Some(2));
            assert_eq!(summary.loan_originations, Some(1));
            assert_eq!(summary.loan_payments, Some(1));
            assert_eq!(summary.unique_loans_affected, Some(1));
        }

        Ok(())
    }
}
