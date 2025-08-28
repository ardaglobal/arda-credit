#[cfg(test)]
mod tests {
    use crate::vapp_integration::{
        Environment, MockComplianceService, MockNotificationService, MockProofService,
        MockSettlementService, ProofType, VAppAdsIntegration, VAppConfig,
    };
    use crate::{
        ads_service::AuthenticatedDataStructure, AdsConfig, AdsError, AdsServiceFactory,
        AuditEventType, InsertionProof, MerkleProof, StateTransition,
    };
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio::time::{timeout, Duration};
    use tracing::{info, warn};
    use tracing_test::traced_test;

    // ============================================================================
    // ADS SERVICE LAYER TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_ads_service_initialization(pool: PgPool) {
        info!("🧪 Testing ADS service initialization");

        let config = AdsConfig::default();
        let factory = AdsServiceFactory::with_config(pool, config);

        let ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS service creation should succeed");

        // Test health check
        assert!(ads
            .health_check()
            .await
            .expect("Health check should succeed"));

        // Test initial metrics
        let metrics = ads.get_metrics().await.expect("Should get initial metrics");
        assert_eq!(metrics.operations_total, 0);
        assert_eq!(metrics.insertions_total, 0);
        assert_eq!(metrics.proofs_generated, 0);

        info!("✅ ADS service initialization test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_nullifier_insertion_with_audit_trail(pool: PgPool) {
        info!("🧪 Testing nullifier insertion with audit trail");

        let config = AdsConfig {
            audit_enabled: true,
            metrics_enabled: true,
            ..Default::default()
        };

        let factory = AdsServiceFactory::with_config(pool, config);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let nullifier = 12345;

        // Insert nullifier
        let transition = ads
            .insert(nullifier)
            .await
            .expect("Insertion should succeed");

        // Verify state transition properties
        assert_ne!(transition.old_root, transition.new_root);
        assert_eq!(transition.nullifier_value, nullifier);
        assert!(!transition.id.is_empty());
        assert!(transition.gas_estimate > 0);
        assert!(!transition.witnesses.is_empty());

        // Verify audit trail was created
        let audit_trail = ads
            .get_audit_trail(nullifier)
            .await
            .expect("Audit trail should exist");

        assert_eq!(audit_trail.nullifier_value, nullifier);
        assert!(!audit_trail.operation_history.is_empty());
        assert!(audit_trail.compliance_status.is_compliant);

        // Check first audit event
        let first_event = &audit_trail.operation_history[0];
        assert!(matches!(first_event.event_type, AuditEventType::Insertion));
        assert_eq!(first_event.root_before, transition.old_root);
        assert_eq!(first_event.root_after, transition.new_root);

        info!("✅ Nullifier insertion with audit trail test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_membership_proof_generation(pool: PgPool) {
        info!("🧪 Testing membership proof generation");

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let nullifier = 54321;

        // Insert nullifier first
        let transition = ads
            .insert(nullifier)
            .await
            .expect("Insertion should succeed");

        // Generate membership proof
        let membership_proof = ads
            .prove_membership(nullifier)
            .await
            .expect("Membership proof should succeed");

        // Verify proof properties
        assert_eq!(membership_proof.nullifier_value, nullifier);
        assert_eq!(membership_proof.root_hash, transition.new_root);
        assert!(membership_proof.tree_index >= 0);
        assert_eq!(membership_proof.merkle_proof.siblings.len(), 32); // 32-level tree

        // Verify the proof is valid by checking basic properties
        assert_eq!(membership_proof.merkle_proof.siblings.len(), 32);
        assert_eq!(membership_proof.merkle_proof.path_indices.len(), 32);
        assert_ne!(membership_proof.root_hash, [0u8; 32]);

        info!("✅ Membership proof generation test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_non_membership_proof_generation(pool: PgPool) {
        info!("🧪 Testing non-membership proof generation");

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Insert some nullifiers to create a proper tree structure
        let existing_nullifiers = vec![100, 300, 500];
        for nullifier in &existing_nullifiers {
            ads.insert(*nullifier)
                .await
                .expect("Insertion should succeed");
        }

        // Try to prove non-membership of a value that should be in the gap
        let non_existent_nullifier = 200; // Between 100 and 300

        let non_membership_proof = ads
            .prove_non_membership(non_existent_nullifier)
            .await
            .expect("Non-membership proof should succeed");

        // Verify proof properties
        assert_eq!(non_membership_proof.queried_value, non_existent_nullifier);
        assert_eq!(non_membership_proof.low_nullifier.value, 100);
        assert_eq!(non_membership_proof.low_nullifier.next_value, 300);
        assert!(non_membership_proof.range_proof.valid);
        assert_eq!(non_membership_proof.range_proof.lower_bound, 100);
        assert_eq!(non_membership_proof.range_proof.upper_bound, 300);

        info!("✅ Non-membership proof generation test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_state_commitment_generation(pool: PgPool) {
        info!("🧪 Testing state commitment generation");

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Insert multiple nullifiers
        let nullifiers = vec![111, 222, 333];
        for nullifier in &nullifiers {
            ads.insert(*nullifier)
                .await
                .expect("Insertion should succeed");
        }

        // Get state commitment
        let commitment = ads
            .get_state_commitment()
            .await
            .expect("State commitment should succeed");

        // Verify commitment properties
        assert_eq!(commitment.nullifier_count, nullifiers.len() as u64);
        assert_eq!(commitment.tree_height, 32);
        assert_ne!(commitment.root_hash, [0u8; 32]);
        assert_ne!(commitment.commitment_hash, [0u8; 32]);
        assert!(!commitment.settlement_data.contract_address.is_empty());
        assert!(commitment.settlement_data.chain_id > 0);

        info!("✅ State commitment generation test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_state_transition_verification(pool: PgPool) {
        info!("🧪 Testing state transition verification");

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let nullifier = 999;

        // Insert nullifier and get transition
        let transition = ads
            .insert(nullifier)
            .await
            .expect("Insertion should succeed");

        // Verify the transition
        let is_valid = ads
            .verify_state_transition(&transition)
            .await
            .expect("Verification should succeed");

        assert!(is_valid, "Valid transition should verify");

        // Test invalid transition (same roots)
        let mut invalid_transition = transition.clone();
        invalid_transition.old_root = invalid_transition.new_root;

        let verify_result = ads.verify_state_transition(&invalid_transition).await;
        assert!(
            verify_result.is_err() || !verify_result.unwrap(),
            "Invalid transition should not verify"
        );

        info!("✅ State transition verification test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_batch_operations(pool: PgPool) {
        info!("🧪 Testing batch operations");

        let config = AdsConfig {
            batch_size_limit: 10,
            ..Default::default()
        };

        let factory = AdsServiceFactory::with_config(pool, config);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let nullifiers = vec![1001, 1002, 1003, 1004, 1005];

        // Batch insert
        let transitions = ads
            .batch_insert(&nullifiers)
            .await
            .expect("Batch insert should succeed");

        assert_eq!(transitions.len(), nullifiers.len());

        // Verify each nullifier was inserted
        for (&nullifier, transition) in nullifiers.iter().zip(transitions.iter()) {
            assert_eq!(transition.nullifier_value, nullifier);

            // Verify we can generate membership proof
            let membership_proof = ads
                .prove_membership(nullifier)
                .await
                .expect("Membership proof should succeed");
            assert_eq!(membership_proof.nullifier_value, nullifier);
        }

        // Test batch size limit
        let large_batch: Vec<i64> = (2000..2020).collect();
        let result = ads.batch_insert(&large_batch).await;
        assert!(result.is_err(), "Batch over limit should fail");

        info!("✅ Batch operations test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_concurrent_operations(pool: PgPool) {
        info!("🧪 Testing concurrent operations");

        let factory = AdsServiceFactory::new(pool);
        let ads = Arc::new(RwLock::new(
            factory
                .create_indexed_merkle_tree()
                .await
                .expect("ADS creation should succeed"),
        ));

        let mut handles = vec![];
        let base_nullifier = 3000;
        let concurrent_ops = 10;

        // Spawn concurrent insertion tasks
        for i in 0..concurrent_ops {
            let ads_clone = ads.clone();
            let nullifier = base_nullifier + i;

            let handle = tokio::spawn(async move {
                let mut ads_guard = ads_clone.write().await;
                ads_guard.insert(nullifier).await
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut successful_insertions = 0;
        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => successful_insertions += 1,
                Ok(Err(e)) => warn!("Concurrent insertion failed: {:?}", e),
                Err(e) => warn!("Task failed: {:?}", e),
            }
        }

        assert!(
            successful_insertions > 0,
            "At least some insertions should succeed"
        );

        // Verify final state consistency
        let ads_guard = ads.read().await;
        let is_valid = ads_guard
            .health_check()
            .await
            .expect("Health check should work");
        assert!(is_valid);

        info!(
            "✅ Concurrent operations test passed ({} successful)",
            successful_insertions
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_performance_metrics(pool: PgPool) {
        info!("🧪 Testing performance metrics");

        let config = AdsConfig {
            metrics_enabled: true,
            ..Default::default()
        };

        let factory = AdsServiceFactory::with_config(pool, config);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Perform various operations to generate metrics
        let nullifiers = vec![4001, 4002, 4003];

        for nullifier in &nullifiers {
            ads.insert(*nullifier)
                .await
                .expect("Insertion should succeed");

            ads.prove_membership(*nullifier)
                .await
                .expect("Membership proof should succeed");
        }

        // Check metrics
        let metrics = ads.get_metrics().await.expect("Should get metrics");

        assert_eq!(metrics.insertions_total, nullifiers.len() as u64);
        assert_eq!(metrics.proofs_generated, nullifiers.len() as u64);
        assert!(metrics.operations_total >= nullifiers.len() as u64 * 2);
        assert!(metrics.avg_insertion_time_ms > 0.0);
        assert!(metrics.avg_proof_time_ms > 0.0);

        // Test metrics reset
        ads.reset_metrics()
            .await
            .expect("Metrics reset should succeed");

        let reset_metrics = ads.get_metrics().await.expect("Should get reset metrics");

        assert_eq!(reset_metrics.operations_total, 0);
        assert_eq!(reset_metrics.insertions_total, 0);
        assert_eq!(reset_metrics.proofs_generated, 0);

        info!("✅ Performance metrics test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_error_handling(pool: PgPool) {
        info!("🧪 Testing error handling");

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let nullifier = 5000;

        // Insert nullifier first
        ads.insert(nullifier)
            .await
            .expect("First insertion should succeed");

        // Try to insert same nullifier again
        let duplicate_result = ads.insert(nullifier).await;
        assert!(duplicate_result.is_err());
        if let Err(AdsError::NullifierExists(value)) = duplicate_result {
            assert_eq!(value, nullifier);
        } else {
            panic!("Expected NullifierExists error");
        }

        // Try to prove membership of non-existent nullifier
        let non_existent = 5001;
        let membership_result = ads.prove_membership(non_existent).await;
        assert!(membership_result.is_err());

        // Try invalid state transition verification
        let invalid_transition = StateTransition {
            id: "invalid".to_string(),
            old_root: [0u8; 32],
            new_root: [0u8; 32], // Same as old_root
            nullifier_value: 0,  // Invalid value
            insertion_proof: InsertionProof {
                low_nullifier_proof: MerkleProof {
                    leaf_index: 0,
                    leaf_hash: [0u8; 32],
                    siblings: vec![],
                    path_indices: vec![],
                },
                new_nullifier_proof: MerkleProof {
                    leaf_index: 0,
                    leaf_hash: [0u8; 32],
                    siblings: vec![],
                    path_indices: vec![],
                },
                low_nullifier_before: crate::merkle_tree::LowNullifier {
                    value: 0,
                    next_index: None,
                    next_value: 0,
                    tree_index: 0,
                },
                low_nullifier_after: crate::merkle_tree::LowNullifier {
                    value: 0,
                    next_index: None,
                    next_value: 0,
                    tree_index: 0,
                },
            },
            block_height: 0,
            timestamp: chrono::Utc::now(),
            gas_estimate: 0,
            witnesses: vec![],
        };

        let verify_result = ads.verify_state_transition(&invalid_transition).await;
        assert!(
            verify_result.is_err(),
            "Invalid transition should be rejected"
        );

        info!("✅ Error handling test passed");
    }

    // ============================================================================
    // VAPP INTEGRATION TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_integration_initialization(pool: PgPool) {
        info!("🧪 Testing vApp integration initialization");

        let config = VAppConfig {
            environment: Environment::Development,
            compliance_checks_enabled: true,
            auto_proof_generation: true,
            ..Default::default()
        };

        let settlement_service = Arc::new(MockSettlementService);
        let proof_service = Arc::new(MockProofService);
        let compliance_service = Arc::new(MockComplianceService);
        let notification_service = Arc::new(MockNotificationService);

        let integration = VAppAdsIntegration::new(
            pool,
            config,
            settlement_service,
            proof_service,
            compliance_service,
            notification_service,
        )
        .await
        .expect("vApp integration should initialize");

        // Test health check
        assert!(integration
            .health_check()
            .await
            .expect("Health check should succeed"));

        // Test metrics
        let metrics = integration.get_metrics().await.expect("Should get metrics");
        assert_eq!(metrics.operations_total, 0);

        info!("✅ vApp integration initialization test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_nullifier_insertion_workflow(pool: PgPool) {
        info!("🧪 Testing vApp nullifier insertion workflow");

        let config = VAppConfig {
            compliance_checks_enabled: true,
            auto_proof_generation: true,
            settlement_enabled: false, // Disable for test
            ..Default::default()
        };

        let integration = create_test_vapp_integration(pool, config).await;

        let nullifier = 6000;

        // Process nullifier insertion
        let response = integration
            .process_nullifier_insertion(nullifier)
            .await
            .expect("Nullifier insertion should succeed");

        // Verify response properties
        assert_eq!(response.state_transition.nullifier_value, nullifier);
        assert_ne!(
            response.state_transition.old_root,
            response.state_transition.new_root
        );
        assert!(response.compliance_result.is_valid);
        assert!(response.zk_proof.is_some());
        assert!(response.processing_time_ms > 0);
        assert!(!response.transaction_id.is_empty());

        info!("✅ vApp nullifier insertion workflow test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_proof_generation_workflow(pool: PgPool) {
        info!("🧪 Testing vApp proof generation workflow");

        let config = VAppConfig {
            auto_proof_generation: true,
            ..Default::default()
        };

        let integration = create_test_vapp_integration(pool, config).await;

        let nullifier = 7000;

        // Insert nullifier first
        integration
            .process_nullifier_insertion(nullifier)
            .await
            .expect("Insertion should succeed");

        // Test membership proof
        let membership_response = integration
            .verify_nullifier_presence(nullifier)
            .await
            .expect("Membership proof should succeed");

        assert!(matches!(
            membership_response.proof_type,
            ProofType::Membership
        ));
        assert!(membership_response.membership_proof.is_some());
        assert!(membership_response.verification_status);
        assert!(membership_response.zk_proof.is_some());

        // Test non-membership proof for different value
        let non_existent_nullifier = 7001;
        let non_membership_response = integration
            .verify_nullifier_absence(non_existent_nullifier)
            .await
            .expect("Non-membership proof should succeed");

        assert!(matches!(
            non_membership_response.proof_type,
            ProofType::NonMembership
        ));
        assert!(non_membership_response.non_membership_proof.is_some());
        assert!(non_membership_response.verification_status);

        info!("✅ vApp proof generation workflow test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_batch_processing_workflow(pool: PgPool) {
        info!("🧪 Testing vApp batch processing workflow");

        let config = VAppConfig {
            batch_processing_enabled: true,
            ..Default::default()
        };

        let integration = create_test_vapp_integration(pool, config).await;

        let nullifiers = vec![8001, 8002, 8003, 8004, 8005];

        // Process batch
        let batch_response = integration
            .process_batch_insertions(&nullifiers)
            .await
            .expect("Batch processing should succeed");

        assert_eq!(batch_response.total_operations, nullifiers.len());
        assert_eq!(batch_response.successful_operations, nullifiers.len());
        assert!(batch_response.failed_operations.is_empty());
        assert!(batch_response.combined_state_transition.is_some());
        assert!(batch_response.processing_time_ms > 0);
        assert!(!batch_response.batch_id.is_empty());

        // Verify all nullifiers were inserted by checking membership
        for nullifier in &nullifiers {
            let membership_response = integration
                .verify_nullifier_presence(*nullifier)
                .await
                .expect("Membership verification should succeed");
            assert!(membership_response.verification_status);
        }

        info!("✅ vApp batch processing workflow test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_state_commitment_workflow(pool: PgPool) {
        info!("🧪 Testing vApp state commitment workflow");

        let integration = create_test_vapp_integration(pool, VAppConfig::default()).await;

        // Insert some nullifiers
        let nullifiers = vec![9001, 9002, 9003];
        for nullifier in &nullifiers {
            integration
                .process_nullifier_insertion(*nullifier)
                .await
                .expect("Insertion should succeed");
        }

        // Get state commitment
        let commitment = integration
            .get_current_state_commitment()
            .await
            .expect("State commitment should succeed");

        assert_eq!(commitment.nullifier_count, nullifiers.len() as u64);
        assert_eq!(commitment.tree_height, 32);
        assert_ne!(commitment.root_hash, [0u8; 32]);
        assert!(!commitment.settlement_data.contract_address.is_empty());

        info!("✅ vApp state commitment workflow test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_error_scenarios(pool: PgPool) {
        info!("🧪 Testing vApp error scenarios");

        let integration = create_test_vapp_integration(pool.clone(), VAppConfig::default()).await;

        let nullifier = 10000;

        // Insert nullifier first
        integration
            .process_nullifier_insertion(nullifier)
            .await
            .expect("First insertion should succeed");

        // Try to insert same nullifier again
        let duplicate_result = integration.process_nullifier_insertion(nullifier).await;
        assert!(duplicate_result.is_err(), "Duplicate insertion should fail");

        // Try to prove membership of non-existent nullifier
        let non_existent = 10001;
        let membership_result = integration.verify_nullifier_presence(non_existent).await;
        assert!(
            membership_result.is_err(),
            "Membership proof of non-existent should fail"
        );

        // Test batch processing with disabled config
        let config_no_batch = VAppConfig {
            batch_processing_enabled: false,
            ..Default::default()
        };
        let integration_no_batch = create_test_vapp_integration(pool, config_no_batch).await;

        let batch_nullifiers = vec![10002, 10003];
        let batch_result = integration_no_batch
            .process_batch_insertions(&batch_nullifiers)
            .await;
        assert!(batch_result.is_err(), "Batch processing should be disabled");

        info!("✅ vApp error scenarios test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_performance_under_load(pool: PgPool) {
        info!("🧪 Testing vApp performance under load");

        let integration =
            Arc::new(create_test_vapp_integration(pool.clone(), VAppConfig::default()).await);

        let start_time = std::time::Instant::now();
        let load_nullifiers = 50;
        let base_nullifier = 11000;

        // Process multiple insertions concurrently
        let mut handles = vec![];
        for i in 0..load_nullifiers {
            let integration_clone = integration.clone();
            let nullifier = base_nullifier + i;

            // Use timeout to prevent hanging
            let handle = tokio::spawn(async move {
                timeout(
                    Duration::from_secs(10),
                    integration_clone.process_nullifier_insertion(nullifier),
                )
                .await
            });

            handles.push(handle);
        }

        let mut successful_operations = 0;
        for handle in handles {
            match handle.await {
                Ok(Ok(Ok(_))) => successful_operations += 1,
                Ok(Ok(Err(e))) => warn!("Operation failed: {:?}", e),
                Ok(Err(e)) => warn!("Operation timed out: {:?}", e),
                Err(e) => warn!("Task panicked: {:?}", e),
            }
        }

        let total_time = start_time.elapsed();
        let avg_time_per_op = total_time.as_millis() / successful_operations as u128;

        info!("📊 Load Test Results:");
        info!(
            "  - Successful operations: {}/{}",
            successful_operations, load_nullifiers
        );
        info!("  - Total time: {:?}", total_time);
        info!("  - Average time per operation: {}ms", avg_time_per_op);

        assert!(
            successful_operations > load_nullifiers / 2,
            "At least half of operations should succeed"
        );
        assert!(
            avg_time_per_op < 1000,
            "Average operation time should be under 1 second"
        );

        // Check final metrics
        let metrics = integration.get_metrics().await.expect("Should get metrics");

        assert!(metrics.operations_total > 0);
        assert!(metrics.avg_insertion_time_ms > 0.0);

        info!("✅ vApp performance under load test passed");
    }

    // ============================================================================
    // CCR OPERATIONS INTEGRATION TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_ccr_operation_insertion(pool: PgPool) {
        info!("🧪 Testing CCR operation insertion");

        use crate::ccr::CcrOperation;
        use chrono::Utc;
        use uuid::Uuid;

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Create a CCR loan creation operation
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let created_at = Utc::now();

        let ccr_operation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: 100000,
            created_at,
        };

        // Insert CCR operation
        let transition = ads
            .insert_ccr_operation(&ccr_operation)
            .await
            .expect("CCR operation insertion should succeed");

        // Verify the state transition has CCR metadata
        assert!(transition.ccr_metadata.is_some());
        let ccr_metadata = transition.ccr_metadata.unwrap();
        assert_eq!(ccr_metadata.operation_type, "loan_creation");
        assert_eq!(ccr_metadata.operation_id, loan_id.to_string());
        assert_eq!(ccr_metadata.amount, Some(100000));
        assert_eq!(ccr_metadata.user_ids.len(), 2); // borrower and lender

        // Verify nullifier is in expected prefix range
        let loan_creation_prefix = 1_000_000_000;
        assert!(
            transition.nullifier_value >= loan_creation_prefix,
            "Nullifier should have loan creation prefix"
        );
        assert!(
            transition.nullifier_value < loan_creation_prefix + 1_000_000_000,
            "Nullifier should be within prefix range"
        );

        info!("✅ CCR operation insertion test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_ccr_batch_operations(pool: PgPool) {
        info!("🧪 Testing CCR batch operations");

        use crate::ccr::CcrOperation;
        use chrono::Utc;
        use uuid::Uuid;

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let created_at = Utc::now();
        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();
        let balance_id = Uuid::new_v4();

        // Create multiple CCR operations
        let ccr_operations = vec![
            CcrOperation::LoanCreation {
                loan_id,
                borrower_id,
                lender_id,
                principal_amount: 100000,
                created_at,
            },
            CcrOperation::LoanFunding {
                loan_id,
                funder_id: lender_id,
                amount: 100000,
                created_at,
            },
            CcrOperation::Payment {
                payment_id,
                loan_id,
                payer_id: borrower_id,
                amount: 5000,
                created_at,
            },
            CcrOperation::BalanceUpdate {
                balance_id,
                identity_id: borrower_id,
                amount_delta: -5000,
                new_balance: 95000,
                created_at,
            },
        ];

        // Batch insert CCR operations
        let transitions = ads
            .batch_insert_ccr_operations(&ccr_operations)
            .await
            .expect("CCR batch insertion should succeed");

        assert_eq!(transitions.len(), 4);

        // Verify each transition has correct CCR metadata and prefix
        let expected_prefixes = [
            1_000_000_000, // LoanCreation
            2_000_000_000, // LoanFunding
            3_000_000_000, // Payment
            4_000_000_000, // BalanceUpdate
        ];

        let expected_types = ["loan_creation", "loan_funding", "payment", "balance_update"];

        for (i, transition) in transitions.iter().enumerate() {
            assert!(transition.ccr_metadata.is_some());
            let ccr_metadata = transition.ccr_metadata.as_ref().unwrap();
            assert_eq!(ccr_metadata.operation_type, expected_types[i]);

            // Verify prefix ranges
            assert!(
                transition.nullifier_value >= expected_prefixes[i],
                "Transition {} should have correct prefix", i
            );
            assert!(
                transition.nullifier_value < expected_prefixes[i] + 1_000_000_000,
                "Transition {} should be within prefix range", i
            );
        }

        // Verify all nullifiers are different
        let mut nullifiers = transitions.iter().map(|t| t.nullifier_value).collect::<Vec<_>>();
        nullifiers.sort();
        for i in 0..nullifiers.len()-1 {
            assert_ne!(nullifiers[i], nullifiers[i+1], "All nullifiers should be unique");
        }

        info!("✅ CCR batch operations test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_ccr_audit_trail(pool: PgPool) {
        info!("🧪 Testing CCR audit trail");

        use crate::ccr::CcrOperation;
        use chrono::Utc;
        use uuid::Uuid;

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let loan_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let created_at = Utc::now();

        let ccr_operation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: 100000,
            created_at,
        };

        // Insert CCR operation
        let transition = ads
            .insert_ccr_operation(&ccr_operation)
            .await
            .expect("CCR operation insertion should succeed");

        // Get audit trail for the operation
        let audit_events = ads
            .get_ccr_audit_trail(&loan_id.to_string())
            .await
            .expect("Should retrieve audit trail");

        assert!(!audit_events.is_empty(), "Should have audit events");
        let audit_event = &audit_events[0];
        assert_eq!(audit_event.event_type, crate::AuditEventType::CcrOperation);
        assert_eq!(audit_event.root_before, transition.old_root);
        assert_eq!(audit_event.root_after, transition.new_root);

        // Verify metadata contains CCR-specific information
        let metadata = &audit_event.metadata;
        assert_eq!(metadata["operation_type"], "loan_creation");
        assert_eq!(metadata["operation_id"], loan_id.to_string());
        assert_eq!(metadata["nullifier_value"], transition.nullifier_value);

        info!("✅ CCR audit trail test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_ccr_backward_compatibility(pool: PgPool) {
        info!("🧪 Testing CCR backward compatibility with existing operations");

        use crate::ccr::CcrOperation;
        use chrono::Utc;
        use uuid::Uuid;

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Insert a regular nullifier (existing functionality)
        let regular_nullifier = 123456;
        let regular_transition = ads
            .insert(regular_nullifier)
            .await
            .expect("Regular insertion should succeed");

        // Verify regular transition has no CCR metadata
        assert!(regular_transition.ccr_metadata.is_none());

        // Insert a CCR operation
        let loan_id = Uuid::new_v4();
        let ccr_operation = CcrOperation::LoanCreation {
            loan_id,
            borrower_id: Uuid::new_v4(),
            lender_id: Uuid::new_v4(),
            principal_amount: 100000,
            created_at: Utc::now(),
        };

        let ccr_transition = ads
            .insert_ccr_operation(&ccr_operation)
            .await
            .expect("CCR operation insertion should succeed");

        // Verify CCR transition has metadata
        assert!(ccr_transition.ccr_metadata.is_some());

        // Verify both nullifiers are properly inserted in the tree
        let membership_proof_regular = ads
            .prove_membership(regular_nullifier)
            .await
            .expect("Should prove membership for regular nullifier");

        let membership_proof_ccr = ads
            .prove_membership(ccr_transition.nullifier_value)
            .await
            .expect("Should prove membership for CCR nullifier");

        // Both should have valid proofs
        assert_eq!(membership_proof_regular.nullifier_value, regular_nullifier);
        assert_eq!(membership_proof_ccr.nullifier_value, ccr_transition.nullifier_value);

        // Health check should still pass
        assert!(ads.health_check().await.expect("Health check should succeed"));

        info!("✅ CCR backward compatibility test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_transaction_mapper_lib_to_ccr(pool: PgPool) {
        info!("🧪 Testing transaction mapper lib to CCR conversion");

        use crate::ccr::{MappingContext, TransactionMapper};
        use crate::{Identity, Loan};
        use lib::transactions::{LoanTransaction, TransactionType, MetadataArray};
        use chrono::Utc;
        use uuid::Uuid;

        let mut context = MappingContext::new();
        
        // Add test entities to context
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let loan_id = Uuid::new_v4();

        let borrower = Identity {
            id: borrower_id,
            user_type: "investor".to_string(),
            email: "borrower@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // Authentication fields
            password_hash: None,
            last_login_at: None,
            email_verified: Some(true),
            email_verification_token: None,
            password_reset_token: None,
            password_reset_expires_at: None,
        };

        let loan = Loan {
            id: loan_id,
            borrower_id,
            lender_id,
            principal_amount: rust_decimal::Decimal::new(10000000, 2),
            outstanding_balance: rust_decimal::Decimal::new(10000000, 2),
            interest_rate_bps: 750,
            loan_status: "active".to_string(),
            originated_at: Utc::now(),
            maturity_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        context.add_user(borrower);
        context.add_loan(loan);

        let mapper = TransactionMapper::new(context);

        // Test loan origination mapping
        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: borrower_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            loan_id: loan_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            transaction_type: TransactionType::LoanOrigination,
            amount: 100000,
            created_at: 1640995200, // 2022-01-01
            metadata: MetadataArray::empty(),
        };

        let ccr_operations = mapper.lib_transaction_to_ccr(&lib_transaction)
            .expect("Should convert lib transaction to CCR");

        assert_eq!(ccr_operations.len(), 1);
        match &ccr_operations[0] {
            crate::CcrOperation::LoanCreation { loan_id: ccr_loan_id, borrower_id: ccr_borrower_id, principal_amount, .. } => {
                // Note: UUID conversion from bytes truncates to 16 bytes, so we compare the first 16 bytes
                assert_eq!(ccr_loan_id.as_bytes(), &loan_id.as_bytes()[..16]);
                assert_eq!(ccr_borrower_id.as_bytes(), &borrower_id.as_bytes()[..16]);
                assert_eq!(*principal_amount, 100000);
            }
            _ => panic!("Expected LoanCreation operation"),
        }

        info!("✅ Transaction mapper lib to CCR conversion test passed");
    }

    #[traced_test] 
    #[sqlx::test]
    async fn test_transaction_mapper_db_to_ccr(pool: PgPool) {
        info!("🧪 Testing transaction mapper database to CCR conversion");

        use crate::ccr::{MappingContext, TransactionMapper};
        use crate::{Identity, Loan, LoanTransactionDb};
        use chrono::Utc;
        use uuid::Uuid;

        let mut context = MappingContext::new();
        
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let loan_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();

        // Add entities to context
        let borrower = Identity {
            id: borrower_id,
            user_type: "investor".to_string(),
            email: "borrower@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // Authentication fields
            password_hash: None,
            last_login_at: None,
            email_verified: Some(true),
            email_verification_token: None,
            password_reset_token: None,
            password_reset_expires_at: None,
        };

        let loan = Loan {
            id: loan_id,
            borrower_id,
            lender_id,
            principal_amount: rust_decimal::Decimal::new(10000000, 2),
            outstanding_balance: rust_decimal::Decimal::new(9500000, 2),
            interest_rate_bps: 750,
            loan_status: "active".to_string(),
            originated_at: Utc::now(),
            maturity_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        context.add_user(borrower);
        context.add_loan(loan);

        let mapper = TransactionMapper::new(context);

        // Test database payment transaction mapping
        let db_transaction = LoanTransactionDb {
            id: Uuid::new_v4(),
            transaction_id_hash: vec![1u8; 32],
            user_id_hash: borrower_id.as_bytes().to_vec(),
            loan_id_hash: Some(loan_id.as_bytes().to_vec()),
            transaction_type: "loan_payment".to_string(),
            amount: rust_decimal::Decimal::new(500000, 2), // $5,000
            identity_id: Some(borrower_id),
            loan_id: Some(loan_id),
            payment_id: Some(payment_id),
            included_in_batch_id: None,
            metadata: None,
            created_at: Utc::now(),
        };

        let ccr_operations = mapper.db_transaction_to_ccr(&db_transaction)
            .expect("Should convert db transaction to CCR");

        assert_eq!(ccr_operations.len(), 3); // Payment + 2 balance updates

        // Verify payment operation
        match &ccr_operations[0] {
            crate::CcrOperation::Payment { payment_id: ccr_payment_id, loan_id: ccr_loan_id, amount, .. } => {
                assert_eq!(*ccr_payment_id, payment_id);
                assert_eq!(*ccr_loan_id, loan_id);
                assert_eq!(*amount, 500000);
            }
            _ => panic!("Expected Payment operation"),
        }

        // Verify balance update operations
        assert!(matches!(ccr_operations[1], crate::CcrOperation::BalanceUpdate { .. }));
        assert!(matches!(ccr_operations[2], crate::CcrOperation::BalanceUpdate { .. }));

        info!("✅ Transaction mapper database to CCR conversion test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_transaction_mapper_consistency(pool: PgPool) {
        info!("🧪 Testing transaction mapper consistency and deterministic nullifiers");

        use crate::ccr::{MappingContext, TransactionMapper};
        use crate::{Identity, Loan};
        use lib::transactions::{LoanTransaction, TransactionType, MetadataArray};
        use chrono::Utc;
        use uuid::Uuid;

        let mut context = MappingContext::new();
        
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let loan_id = Uuid::new_v4();

        let borrower = Identity {
            id: borrower_id,
            user_type: "investor".to_string(),
            email: "borrower@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // Authentication fields
            password_hash: None,
            last_login_at: None,
            email_verified: Some(true),
            email_verification_token: None,
            password_reset_token: None,
            password_reset_expires_at: None,
        };

        let loan = Loan {
            id: loan_id,
            borrower_id,
            lender_id,
            principal_amount: rust_decimal::Decimal::new(10000000, 2),
            outstanding_balance: rust_decimal::Decimal::new(10000000, 2),
            interest_rate_bps: 750,
            loan_status: "active".to_string(),
            originated_at: Utc::now(),
            maturity_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        context.add_user(borrower);
        context.add_loan(loan);

        let mapper = TransactionMapper::new(context);

        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: borrower_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            loan_id: loan_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            transaction_type: TransactionType::LoanPayment,
            amount: 5000,
            created_at: 1640995200,
            metadata: MetadataArray::empty(),
        };

        // Convert the same transaction multiple times
        let ccr_operations1 = mapper.lib_transaction_to_ccr(&lib_transaction)
            .expect("First conversion should succeed");
        let ccr_operations2 = mapper.lib_transaction_to_ccr(&lib_transaction)
            .expect("Second conversion should succeed");

        assert_eq!(ccr_operations1.len(), ccr_operations2.len());

        // Verify deterministic nullifier generation
        for (op1, op2) in ccr_operations1.iter().zip(ccr_operations2.iter()) {
            let nullifier1 = op1.to_nullifier();
            let nullifier2 = op2.to_nullifier();
            assert_eq!(nullifier1, nullifier2, "Nullifiers should be deterministic");
        }

        // Verify all operations have correct prefix ranges
        for operation in &ccr_operations1 {
            let nullifier = operation.to_nullifier();
            match operation {
                crate::CcrOperation::LoanCreation { .. } => {
                    assert!(nullifier >= 1_000_000_000 && nullifier < 2_000_000_000, 
                            "LoanCreation nullifier should have correct prefix");
                }
                crate::CcrOperation::LoanFunding { .. } => {
                    assert!(nullifier >= 2_000_000_000 && nullifier < 3_000_000_000, 
                            "LoanFunding nullifier should have correct prefix");
                }
                crate::CcrOperation::Payment { .. } => {
                    assert!(nullifier >= 3_000_000_000 && nullifier < 4_000_000_000, 
                            "Payment nullifier should have correct prefix");
                }
                crate::CcrOperation::BalanceUpdate { .. } => {
                    assert!(nullifier >= 4_000_000_000 && nullifier < 5_000_000_000, 
                            "BalanceUpdate nullifier should have correct prefix");
                }
            }
        }

        info!("✅ Transaction mapper consistency test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_transaction_mapper_integration_with_ads(pool: PgPool) {
        info!("🧪 Testing transaction mapper integration with ADS service");

        use crate::ccr::{MappingContext, TransactionMapper};
        use crate::{Identity, Loan};
        use lib::transactions::{LoanTransaction, TransactionType, MetadataArray};
        use chrono::Utc;
        use uuid::Uuid;

        // Create ADS service
        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        // Create mapping context
        let mut context = MappingContext::new();
        
        let borrower_id = Uuid::new_v4();
        let lender_id = Uuid::new_v4();
        let loan_id = Uuid::new_v4();

        let borrower = Identity {
            id: borrower_id,
            user_type: "investor".to_string(),
            email: "borrower@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // Authentication fields
            password_hash: None,
            last_login_at: None,
            email_verified: Some(true),
            email_verification_token: None,
            password_reset_token: None,
            password_reset_expires_at: None,
        };

        let loan = Loan {
            id: loan_id,
            borrower_id,
            lender_id,
            principal_amount: rust_decimal::Decimal::new(10000000, 2),
            outstanding_balance: rust_decimal::Decimal::new(10000000, 2),
            interest_rate_bps: 750,
            loan_status: "active".to_string(),
            originated_at: Utc::now(),
            maturity_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        context.add_user(borrower);
        context.add_loan(loan);

        let mapper = TransactionMapper::new(context);

        // Create lib transaction
        let lib_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: borrower_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            loan_id: loan_id.as_bytes().iter().chain([0u8; 16].iter()).cloned().collect::<Vec<u8>>().try_into().unwrap(),
            transaction_type: TransactionType::LoanPayment,
            amount: 5000,
            created_at: 1640995200,
            metadata: MetadataArray::empty(),
        };

        // Map to CCR operations
        let ccr_operations = mapper.lib_transaction_to_ccr(&lib_transaction)
            .expect("Should convert to CCR operations");

        // Process through ADS service
        let transitions = ads.batch_insert_ccr_operations(&ccr_operations)
            .await
            .expect("Should process CCR operations through ADS");

        assert_eq!(transitions.len(), ccr_operations.len());

        // Verify all transitions have CCR metadata
        for transition in &transitions {
            assert!(transition.ccr_metadata.is_some(), "All transitions should have CCR metadata");
            let metadata = transition.ccr_metadata.as_ref().unwrap();
            assert!(!metadata.operation_type.is_empty(), "Operation type should not be empty");
        }

        // Verify membership proofs work for mapped operations
        for transition in &transitions {
            let membership_proof = ads.prove_membership(transition.nullifier_value)
                .await
                .expect("Should prove membership for mapped operation");
            
            assert_eq!(membership_proof.nullifier_value, transition.nullifier_value);
        }

        info!("✅ Transaction mapper integration with ADS test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_ccr_collision_resistance(pool: PgPool) {
        info!("🧪 Testing CCR operation collision resistance");

        use crate::ccr::CcrOperation;
        use chrono::Utc;
        use uuid::Uuid;
        use std::collections::HashSet;

        let factory = AdsServiceFactory::new(pool);
        let mut ads = factory
            .create_indexed_merkle_tree()
            .await
            .expect("ADS creation should succeed");

        let mut inserted_nullifiers = HashSet::new();
        let created_at = Utc::now();

        // Insert multiple operations of different types with potentially similar data
        for i in 0..50 {
            let base_uuid = Uuid::new_v4();
            
            let operations = vec![
                CcrOperation::LoanCreation {
                    loan_id: base_uuid,
                    borrower_id: base_uuid,
                    lender_id: base_uuid,
                    principal_amount: i,
                    created_at,
                },
                CcrOperation::Payment {
                    payment_id: base_uuid,
                    loan_id: base_uuid,
                    payer_id: base_uuid,
                    amount: i,
                    created_at,
                },
            ];

            for operation in operations {
                let transition = ads
                    .insert_ccr_operation(&operation)
                    .await
                    .expect("CCR operation insertion should succeed");

                // Verify no collisions
                assert!(
                    inserted_nullifiers.insert(transition.nullifier_value),
                    "Nullifier collision detected: {}",
                    transition.nullifier_value
                );
            }
        }

        assert_eq!(inserted_nullifiers.len(), 100); // 50 * 2 operations
        info!("✅ CCR collision resistance test passed");
    }

    // ============================================================================
    // HELPER FUNCTIONS
    // ============================================================================

    async fn create_test_vapp_integration(pool: PgPool, config: VAppConfig) -> VAppAdsIntegration {
        let settlement_service = Arc::new(MockSettlementService);
        let proof_service = Arc::new(MockProofService);
        let compliance_service = Arc::new(MockComplianceService);
        let notification_service = Arc::new(MockNotificationService);

        VAppAdsIntegration::new(
            pool,
            config,
            settlement_service,
            proof_service,
            compliance_service,
            notification_service,
        )
        .await
        .expect("vApp integration creation should succeed")
    }
}
