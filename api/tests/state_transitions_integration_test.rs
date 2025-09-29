//! State Transitions System Integration Tests
//!
//! Comprehensive tests to validate the enhanced state transition system:
//! - Automatic state transition recording via database triggers
//! - ZK circuit integration and state validation
//! - State history tracking and querying
//! - Database-ZK circuit alignment checking

use arda_credit_api::state_transitions_service::{
    CreateStateTransitionRequest, StateTransitionsService, StateValidationStatus, TransactionType,
};
use arda_credit_api::zk_circuit_mapper::ZkCircuitMapper;
use arda_credit_db::db::{Company, CompanyType, Deal, User};
use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// TEST SETUP
// ============================================================================

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/arda_credit_db".to_string()
    });

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations to ensure state_transitions table exists
    sqlx::migrate!("../db/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn cleanup_test_data(pool: &PgPool, test_id: &str) {
    // Clean up test data to avoid conflicts
    sqlx::query!(
        "DELETE FROM state_transitions WHERE metadata->>'test_id' = $1",
        test_id
    )
    .execute(pool)
    .await
    .expect("Failed to clean up test state transitions");

    sqlx::query!(
        "DELETE FROM deals WHERE title LIKE $1",
        format!("Test Deal {}", test_id)
    )
    .execute(pool)
    .await
    .expect("Failed to clean up test deals");

    sqlx::query!(
        "DELETE FROM users WHERE email LIKE $1",
        format!("test-{}%", test_id)
    )
    .execute(pool)
    .await
    .expect("Failed to clean up test users");

    sqlx::query!(
        "DELETE FROM companies WHERE name LIKE $1",
        format!("Test Company {}", test_id)
    )
    .execute(pool)
    .await
    .expect("Failed to clean up test companies");
}

// ============================================================================
// AUTOMATIC STATE TRANSITION RECORDING TESTS
// ============================================================================

#[sqlx::test]
async fn test_automatic_user_registration_state_transition(pool: PgPool) {
    let test_id = "user_reg_001";
    cleanup_test_data(&pool, test_id).await;

    let service = StateTransitionsService::new(pool.clone());

    // Create a user - this should automatically trigger state transition recording
    let user_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();

    // First create a company
    sqlx::query!(
        r#"
        INSERT INTO companies (id, name, type, kyb_status, created_at)
        VALUES ($1, $2, 'investor', 'not_submitted', NOW())
        "#,
        company_id,
        format!("Test Company {}", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test company");

    // Create user - should trigger state transition
    sqlx::query!(
        r#"
        INSERT INTO users (id, company_id, email, first_name, last_name, kyc_status, created_at)
        VALUES ($1, $2, $3, 'Test', 'User', 'not_submitted', NOW())
        "#,
        user_id,
        company_id,
        format!("test-{}@example.com", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test user");

    // Wait a moment for trigger to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check that state transition was created
    let transitions = sqlx::query!(
        r#"
        SELECT transition_id, transaction_type, user_id, after_state
        FROM state_transitions
        WHERE user_id = $1 AND transaction_type = 'user_registration'
        "#,
        user_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch state transitions");

    assert_eq!(
        transitions.len(),
        1,
        "Should have exactly one user registration transition"
    );

    let transition = &transitions[0];
    assert_eq!(transition.user_id, Some(user_id));
    assert!(
        transition.after_state.is_some(),
        "Should have captured after state"
    );

    // Verify the after_state contains user data
    let after_state = transition.after_state.as_ref().unwrap();
    assert_eq!(
        after_state["email"],
        format!("test-{}@example.com", test_id)
    );
    assert_eq!(after_state["kyc_status"], "not_submitted");

    cleanup_test_data(&pool, test_id).await;
}

#[sqlx::test]
async fn test_automatic_deal_origination_state_transition(pool: PgPool) {
    let test_id = "deal_orig_001";
    cleanup_test_data(&pool, test_id).await;

    // Setup users first
    let originator_id = Uuid::new_v4();
    let investor_id = Uuid::new_v4();
    let deal_id = Uuid::new_v4();

    // Create users (these will also generate state transitions)
    sqlx::query!(
        r#"
        INSERT INTO users (id, email, first_name, last_name, kyc_status, created_at)
        VALUES ($1, $2, 'Test', 'Originator', 'not_submitted', NOW()),
               ($3, $4, 'Test', 'Investor', 'not_submitted', NOW())
        "#,
        originator_id,
        format!("test-originator-{}@example.com", test_id),
        investor_id,
        format!("test-investor-{}@example.com", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test users");

    // Create deal - should trigger deal_origination state transition
    sqlx::query!(
        r#"
        INSERT INTO deals (id, originator_id, investor_id, principal_amount, outstanding_balance,
                          interest_rate_bps, status, title, created_at)
        VALUES ($1, $2, $3, $4, $4, 750, 'draft', $5, NOW())
        "#,
        deal_id,
        originator_id,
        investor_id,
        Decimal::from_str("50000.00").unwrap(),
        format!("Test Deal {}", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test deal");

    // Wait for triggers to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check for deal origination transition
    let deal_transitions = sqlx::query!(
        r#"
        SELECT transition_id, transaction_type, deal_id, after_state
        FROM state_transitions
        WHERE deal_id = $1 AND transaction_type = 'deal_origination'
        "#,
        deal_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch deal transitions");

    assert_eq!(
        deal_transitions.len(),
        1,
        "Should have exactly one deal origination transition"
    );

    let transition = &deal_transitions[0];
    assert_eq!(transition.deal_id, Some(deal_id));
    assert!(
        transition.after_state.is_some(),
        "Should have captured deal after state"
    );

    // Verify deal state was captured
    let after_state = transition.after_state.as_ref().unwrap();
    assert_eq!(after_state["status"], "draft");
    assert_eq!(after_state["principal_amount"], "50000.00");

    cleanup_test_data(&pool, test_id).await;
}

#[sqlx::test]
async fn test_deal_status_change_state_transition(pool: PgPool) {
    let test_id = "deal_status_001";
    cleanup_test_data(&pool, test_id).await;

    // Setup test deal
    let originator_id = Uuid::new_v4();
    let investor_id = Uuid::new_v4();
    let deal_id = Uuid::new_v4();

    // Create users
    sqlx::query!(
        r#"
        INSERT INTO users (id, email, first_name, last_name, kyc_status)
        VALUES ($1, $2, 'Test', 'Originator', 'not_submitted'),
               ($3, $4, 'Test', 'Investor', 'not_submitted')
        "#,
        originator_id,
        format!("test-originator-{}@example.com", test_id),
        investor_id,
        format!("test-investor-{}@example.com", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test users");

    // Create deal in draft status
    sqlx::query!(
        r#"
        INSERT INTO deals (id, originator_id, investor_id, principal_amount, outstanding_balance,
                          interest_rate_bps, status, title)
        VALUES ($1, $2, $3, $4, $4, 750, 'draft', $5)
        "#,
        deal_id,
        originator_id,
        investor_id,
        Decimal::from_str("50000.00").unwrap(),
        format!("Test Deal {}", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test deal");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Update deal status - should trigger deal_status_change transition
    sqlx::query!(
        r#"
        UPDATE deals
        SET status = 'active', updated_at = NOW()
        WHERE id = $1
        "#,
        deal_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update deal status");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check for status change transition
    let transitions = sqlx::query!(
        r#"
        SELECT transition_id, transaction_type, before_state, after_state
        FROM state_transitions
        WHERE deal_id = $1 AND transaction_type = 'deal_status_change'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        deal_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch state transitions");

    assert_eq!(
        transitions.len(),
        1,
        "Should have exactly one status change transition"
    );

    let transition = &transitions[0];
    assert!(
        transition.before_state.is_some(),
        "Should have before state"
    );
    assert!(transition.after_state.is_some(), "Should have after state");

    // Verify state change
    let before_state = transition.before_state.as_ref().unwrap();
    let after_state = transition.after_state.as_ref().unwrap();

    assert_eq!(before_state["status"], "draft");
    assert_eq!(after_state["status"], "active");

    cleanup_test_data(&pool, test_id).await;
}

// ============================================================================
// STATE HISTORY AND QUERYING TESTS
// ============================================================================

#[sqlx::test]
async fn test_entity_state_history_retrieval(pool: PgPool) {
    let test_id = "history_001";
    cleanup_test_data(&pool, test_id).await;

    let service = StateTransitionsService::new(pool.clone());

    // Create test user with multiple state changes
    let user_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO users (id, email, first_name, last_name, kyc_status)
        VALUES ($1, $2, 'Test', 'User', 'not_submitted')
        "#,
        user_id,
        format!("test-{}@example.com", test_id)
    )
    .execute(&pool)
    .await
    .expect("Failed to create test user");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Update KYC status
    sqlx::query!(
        r#"
        UPDATE users
        SET kyc_status = 'pending', updated_at = NOW()
        WHERE id = $1
        "#,
        user_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update KYC status");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Approve KYC
    sqlx::query!(
        r#"
        UPDATE users
        SET kyc_status = 'approved', kyc_verified_at = NOW(), updated_at = NOW()
        WHERE id = $1
        "#,
        user_id
    )
    .execute(&pool)
    .await
    .expect("Failed to approve KYC");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Get state history
    let history = service
        .get_entity_history("user", user_id, Some(10))
        .await
        .expect("Failed to get entity history");

    // Should have at least 3 transitions: registration + 2 KYC updates
    assert!(
        history.len() >= 3,
        "Should have at least 3 state transitions"
    );

    // Verify transitions are in chronological order (newest first)
    let mut previous_time = None;
    for entry in &history {
        if let Some(prev) = previous_time {
            assert!(
                entry.created_at <= prev,
                "History should be in descending chronological order"
            );
        }
        previous_time = Some(entry.created_at);
    }

    cleanup_test_data(&pool, test_id).await;
}

// ============================================================================
// STATE VALIDATION AND ALIGNMENT TESTS
// ============================================================================

#[sqlx::test]
async fn test_state_alignment_checking(pool: PgPool) {
    let service = StateTransitionsService::new(pool.clone());

    // Run state alignment check
    let alignment_results = service
        .check_state_alignment()
        .await
        .expect("Failed to check state alignment");

    // Should return at least the basic checks
    assert!(
        !alignment_results.is_empty(),
        "Should have alignment check results"
    );

    // All checks should at least not fail (PASS or WARN are acceptable)
    for check in &alignment_results {
        assert_ne!(
            check.status, "FAIL",
            "No critical alignment failures should exist: {}",
            check.details
        );
    }
}

#[sqlx::test]
async fn test_transition_validation(pool: PgPool) {
    let test_id = "validation_001";
    cleanup_test_data(&pool, test_id).await;

    let service = StateTransitionsService::new(pool.clone());

    // Create a manual state transition for testing
    let request = CreateStateTransitionRequest {
        transaction_type: TransactionType::UserRegistration,
        user_id: Some(Uuid::new_v4()),
        deal_id: None,
        company_id: None,
        payment_id: None,
        amount: None,
        before_state: None,
        after_state: Some(json!({
            "id": "test-user-id",
            "status": "registered",
            "kyc_status": "not_submitted"
        })),
        metadata: Some(json!({
            "test_id": test_id,
            "manual_creation": true
        })),
        parent_transition_id: None,
    };

    let transition_id = service
        .create_transition(request)
        .await
        .expect("Failed to create test transition");

    // Validate the transition
    let validation_result = service
        .validate_transition(transition_id, None)
        .await
        .expect("Failed to validate transition");

    assert_eq!(validation_result.transition_id, transition_id);
    // For this simple test, validation should pass
    assert!(
        validation_result.is_valid,
        "Validation should pass for simple transition"
    );

    cleanup_test_data(&pool, test_id).await;
}

// ============================================================================
// ZK CIRCUIT INTEGRATION TESTS
// ============================================================================

#[sqlx::test]
async fn test_zk_circuit_mapper_validation(pool: PgPool) {
    let mapper = ZkCircuitMapper::new(pool.clone());

    // Test circuit-database alignment validation
    let alignment_issues = mapper
        .validate_circuit_database_alignment()
        .await
        .expect("Failed to validate circuit alignment");

    // In a well-functioning system, there should be no critical alignment issues
    // Some warnings might be acceptable depending on system state
    let critical_issues: Vec<_> = alignment_issues
        .iter()
        .filter(|issue| issue.contains("FAIL") || issue.contains("critical"))
        .collect();

    if !critical_issues.is_empty() {
        panic!("Found critical alignment issues: {:?}", critical_issues);
    }

    println!(
        "Circuit alignment validation passed. Issues found: {:?}",
        alignment_issues
    );
}

// ============================================================================
// PERFORMANCE AND SCALABILITY TESTS
// ============================================================================

#[sqlx::test]
async fn test_state_transitions_performance(pool: PgPool) {
    let test_id = "perf_001";
    cleanup_test_data(&pool, test_id).await;

    let service = StateTransitionsService::new(pool.clone());

    let start_time = std::time::Instant::now();

    // Create multiple state transitions rapidly
    let mut transition_ids = Vec::new();
    for i in 0..100 {
        let request = CreateStateTransitionRequest {
            transaction_type: TransactionType::BalanceUpdate,
            user_id: Some(Uuid::new_v4()),
            deal_id: None,
            company_id: None,
            payment_id: None,
            amount: Some(Decimal::from(i)),
            before_state: Some(json!({"balance": i - 1})),
            after_state: Some(json!({"balance": i})),
            metadata: Some(json!({
                "test_id": test_id,
                "sequence": i
            })),
            parent_transition_id: None,
        };

        let transition_id = service
            .create_transition(request)
            .await
            .expect("Failed to create transition");
        transition_ids.push(transition_id);
    }

    let creation_time = start_time.elapsed();
    println!("Created 100 state transitions in {:?}", creation_time);

    // Test querying performance
    let query_start = std::time::Instant::now();
    let stats = service
        .get_transition_stats()
        .await
        .expect("Failed to get stats");
    let query_time = query_start.elapsed();

    println!("Retrieved statistics in {:?}", query_time);

    // Performance assertions
    assert!(
        creation_time.as_millis() < 5000,
        "Should create 100 transitions in under 5 seconds"
    );
    assert!(
        query_time.as_millis() < 1000,
        "Should retrieve stats in under 1 second"
    );
    assert!(
        stats.get("total").unwrap_or(&0) >= &100,
        "Should have at least 100 total transitions"
    );

    cleanup_test_data(&pool, test_id).await;
}

// ============================================================================
// ERROR HANDLING AND EDGE CASES
// ============================================================================

#[sqlx::test]
async fn test_invalid_transition_handling(pool: PgPool) {
    let service = StateTransitionsService::new(pool.clone());

    // Test validation of transition that should fail
    let invalid_request = CreateStateTransitionRequest {
        transaction_type: TransactionType::DealOrigination,
        user_id: None, // Missing required user_id for deal origination
        deal_id: Some(Uuid::new_v4()),
        company_id: None,
        payment_id: None,
        amount: Some(Decimal::ZERO),
        before_state: Some(json!({"status": "active"})),
        after_state: Some(json!({"status": "active"})), // No state change
        metadata: None,
        parent_transition_id: None,
    };

    // This should succeed in creation but fail validation
    let transition_id = service
        .create_transition(invalid_request)
        .await
        .expect("Transition creation should succeed");

    let validation_result = service
        .validate_transition(transition_id, None)
        .await
        .expect("Validation should complete even for invalid transitions");

    assert!(
        !validation_result.is_valid,
        "Invalid transition should not validate"
    );
    assert!(
        validation_result.error_details.is_some(),
        "Should have error details for invalid transition"
    );
}

#[sqlx::test]
async fn test_concurrent_state_transitions(pool: PgPool) {
    let test_id = "concurrent_001";
    cleanup_test_data(&pool, test_id).await;

    let service = StateTransitionsService::new(pool.clone());

    // Create concurrent state transitions for the same entity
    let user_id = Uuid::new_v4();
    let mut handles = Vec::new();

    for i in 0..10 {
        let service_clone = service.clone();
        let user_id_clone = user_id;
        let test_id_clone = test_id.to_string();

        let handle = tokio::spawn(async move {
            let request = CreateStateTransitionRequest {
                transaction_type: TransactionType::BalanceUpdate,
                user_id: Some(user_id_clone),
                deal_id: None,
                company_id: None,
                payment_id: None,
                amount: Some(Decimal::from(i)),
                before_state: Some(json!({"balance": i})),
                after_state: Some(json!({"balance": i + 1})),
                metadata: Some(json!({
                    "test_id": test_id_clone,
                    "concurrent_index": i
                })),
                parent_transition_id: None,
            };

            service_clone.create_transition(request).await
        });

        handles.push(handle);
    }

    // Wait for all concurrent operations to complete
    let results = futures::future::join_all(handles).await;

    // All operations should succeed
    let mut successful_transitions = 0;
    for result in results {
        match result {
            Ok(Ok(_)) => successful_transitions += 1,
            Ok(Err(e)) => panic!("State transition creation failed: {:?}", e),
            Err(e) => panic!("Task failed: {:?}", e),
        }
    }

    assert_eq!(
        successful_transitions, 10,
        "All concurrent transitions should succeed"
    );

    // Verify all transitions were recorded
    let history = service
        .get_entity_history("user", user_id, Some(20))
        .await
        .expect("Failed to get entity history");

    assert_eq!(
        history.len(),
        10,
        "Should have recorded all 10 concurrent transitions"
    );

    cleanup_test_data(&pool, test_id).await;
}
