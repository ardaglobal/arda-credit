//! Integration tests for collateral audit trail functionality
//!
//! Tests the complete lifecycle of collateral transaction recording and audit trail retrieval.

#[cfg(test)]
mod tests {
    use arda_credit_db::{
        create_collateral, get_collateral_transaction_history,
        record_collateral_creation_transaction, record_collateral_deal_association_transaction,
        record_collateral_update_transaction, update_collateral, CreateCollateralRequest,
        UpdateCollateralRequest,
    };
    use sqlx::PgPool;
    use uuid::Uuid;

    /// Helper to initialize test database
    async fn setup_test_db() -> PgPool {
        arda_credit_db::init_db()
            .await
            .expect("Failed to initialize test database")
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_collateral_creation_transaction_recording() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Test Property for Audit Trail".to_string(),
                collateral_type: Some("real_estate".to_string()),
                description: Some("Test property".to_string()),
                address: Some("123 Main St".to_string()),
                custom_id: Some("AUDIT-TEST-001".to_string()),
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Record creation transaction
        let initial_properties = serde_json::json!({
            "name": collateral.name.clone(),
            "collateral_type": collateral.collateral_type.clone(),
            "address": collateral.address.clone(),
        });

        let transaction =
            record_collateral_creation_transaction(&pool, collateral.id, None, initial_properties)
                .await
                .expect("Failed to record creation transaction");

        assert_eq!(transaction.collateral_id, collateral.id);
        assert_eq!(transaction.transaction_type, "collateral_creation");
        assert!(transaction.metadata.is_some());

        // Verify transaction can be retrieved
        let history = get_collateral_transaction_history(&pool, collateral.id, None)
            .await
            .expect("Failed to get transaction history");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].transaction_type, "collateral_creation");
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_collateral_update_transaction_recording() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Original Name".to_string(),
                collateral_type: Some("equipment".to_string()),
                description: Some("Original description".to_string()),
                address: None,
                custom_id: None,
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Record creation transaction
        let initial_properties = serde_json::json!({
            "name": "Original Name",
        });
        record_collateral_creation_transaction(&pool, collateral.id, None, initial_properties)
            .await
            .expect("Failed to record creation transaction");

        // Update collateral
        update_collateral(
            &pool,
            collateral.id,
            UpdateCollateralRequest {
                name: Some("Updated Name".to_string()),
                collateral_type: None,
                description: Some("Updated description".to_string()),
                address: None,
                custom_id: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to update collateral");

        // Record update transaction
        let changes = serde_json::json!({
            "name": {"old": "Original Name", "new": "Updated Name"},
            "description": {"old": "Original description", "new": "Updated description"}
        });

        record_collateral_update_transaction(&pool, collateral.id, None, changes)
            .await
            .expect("Failed to record update transaction");

        // Verify both transactions exist
        let history = get_collateral_transaction_history(&pool, collateral.id, None)
            .await
            .expect("Failed to get transaction history");

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].transaction_type, "collateral_creation");
        assert_eq!(history[1].transaction_type, "collateral_update");

        // Verify change metadata
        let update_metadata = history[1].metadata.as_ref().expect("Missing metadata");
        assert!(update_metadata.get("name").is_some());
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_collateral_deal_association_recording() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Property with Deal".to_string(),
                collateral_type: Some("real_estate".to_string()),
                description: None,
                address: None,
                custom_id: None,
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Simulate deal association (using a random UUID for testing)
        let deal_id = Uuid::new_v4();

        let transaction =
            record_collateral_deal_association_transaction(&pool, collateral.id, deal_id, None)
                .await
                .expect("Failed to record deal association");

        assert_eq!(transaction.transaction_type, "collateral_deal_association");
        assert_eq!(transaction.deal_id, Some(deal_id));

        // Verify transaction history
        let history = get_collateral_transaction_history(&pool, collateral.id, None)
            .await
            .expect("Failed to get transaction history");

        let association_tx = history
            .iter()
            .find(|tx| tx.transaction_type == "collateral_deal_association")
            .expect("Deal association transaction not found");

        assert_eq!(association_tx.deal_id, Some(deal_id));
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_audit_trail_chronological_ordering() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Chronological Test".to_string(),
                collateral_type: Some("inventory".to_string()),
                description: None,
                address: None,
                custom_id: None,
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Record multiple transactions
        record_collateral_creation_transaction(
            &pool,
            collateral.id,
            None,
            serde_json::json!({"name": "Initial"}),
        )
        .await
        .expect("Failed to record creation");

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        record_collateral_update_transaction(
            &pool,
            collateral.id,
            None,
            serde_json::json!({"name": {"old": "Initial", "new": "Updated"}}),
        )
        .await
        .expect("Failed to record update");

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        record_collateral_deal_association_transaction(&pool, collateral.id, Uuid::new_v4(), None)
            .await
            .expect("Failed to record association");

        // Get history
        let history = get_collateral_transaction_history(&pool, collateral.id, None)
            .await
            .expect("Failed to get transaction history");

        // Verify chronological order
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].transaction_type, "collateral_creation");
        assert_eq!(history[1].transaction_type, "collateral_update");
        assert_eq!(history[2].transaction_type, "collateral_deal_association");

        // Verify timestamps are in ascending order
        for i in 1..history.len() {
            assert!(
                history[i].created_at >= history[i - 1].created_at,
                "Transactions not in chronological order"
            );
        }
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_audit_trail_with_limit() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Limit Test".to_string(),
                collateral_type: Some("equipment".to_string()),
                description: None,
                address: None,
                custom_id: None,
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Record 5 transactions
        for i in 0..5 {
            record_collateral_update_transaction(
                &pool,
                collateral.id,
                None,
                serde_json::json!({"update_count": i}),
            )
            .await
            .expect("Failed to record update");

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Test limit
        let history = get_collateral_transaction_history(&pool, collateral.id, Some(3))
            .await
            .expect("Failed to get transaction history");

        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_transaction_metadata_structure() {
        let pool = setup_test_db().await;

        // Create collateral
        let collateral = create_collateral(
            &pool,
            CreateCollateralRequest {
                name: "Metadata Test".to_string(),
                collateral_type: Some("real_estate".to_string()),
                description: None,
                address: None,
                custom_id: None,
                metadata: None,
                created_by: None,
            },
        )
        .await
        .expect("Failed to create collateral");

        // Record creation with detailed metadata
        let initial_properties = serde_json::json!({
            "name": "Metadata Test",
            "collateral_type": "real_estate",
            "additional_info": {
                "size": "5000 sqft",
                "year_built": 2020
            }
        });

        record_collateral_creation_transaction(&pool, collateral.id, None, initial_properties)
            .await
            .expect("Failed to record creation");

        // Verify metadata structure
        let history = get_collateral_transaction_history(&pool, collateral.id, None)
            .await
            .expect("Failed to get transaction history");

        let metadata = history[0].metadata.as_ref().expect("Missing metadata");
        assert!(metadata.get("initial_properties").is_some());

        let props = metadata.get("initial_properties").unwrap();
        assert_eq!(props.get("name").unwrap(), "Metadata Test");
        assert!(props.get("additional_info").is_some());
    }
}
