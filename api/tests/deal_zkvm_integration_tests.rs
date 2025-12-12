//! Integration tests for deal operations with ZKVM transaction recording
//!
//! These tests verify that all deal operations (user registration, deal creation,
//! payments, and closeouts) properly create deal_transactions records that flow
//! through the ZKVM batch processing system.
//!
//! Tests are structured to validate:
//! - API endpoints create deal_transactions records
//! - Transaction types are correct
//! - Metadata is populated appropriately
//! - UUID hashing works correctly
//! - End-to-end deal lifecycle works

use api::services::TransactionRecordingService;
use arda_credit_db::{business::*, *};
use sqlx::PgPool;
use std::str::FromStr;

/// Test user registration creates transaction
#[sqlx::test]
async fn test_user_registration_creates_transaction(pool: PgPool) {
    // Setup test state (similar to other tests)
    let state = create_test_api_state(pool.clone()).await;

    // Create user via API
    let request = api::handlers::users::users::RegisterUserRequest {
        email: "test@example.com".to_string(),
        jurisdiction: Some("US".to_string()),
        invite_code: "TEST123".to_string(), // Assume this exists or create it
    };

    let response = api::handlers::users::users::register_user_endpoint(
        axum::extract::State(state.clone()),
        axum::Json(request),
    )
    .await;

    assert!(response.is_ok(), "User registration should succeed");
    let user_response = response.unwrap().0;

    // Verify user exists
    let user_id = uuid::Uuid::parse_str(&user_response.user_id).unwrap();
    let user = get_identity_by_id(&pool, user_id).await.unwrap().unwrap();
    assert_eq!(user.email, "test@example.com");

    // Verify transaction was recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE user_id = $1",
        user_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].transaction_type, "user_registration");
    assert_eq!(transactions[0].amount, rust_decimal::Decimal::ZERO);
    assert!(transactions[0].included_in_batch_id.is_none()); // Not batched yet
}

/// Test deal creation creates transaction
#[sqlx::test]
async fn test_deal_creation_creates_transaction(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;

    // Create companies first
    let originator = create_test_company(&pool, "Test Originator Corp").await;
    let investor = create_test_company(&pool, "Test Investor Corp").await;

    // Create deal via API
    let request = api::handlers::deals::deals::CreateDealRequest {
        originator_id: originator.id.to_string(),
        originator_name: None,
        investor_id: Some(investor.id.to_string()),
        investor_name: None,
        borrower_id: None,
        borrower_name: None,
        principal_amount: "100000.00".to_string(),
        interest_rate_bps: 500,
        term_days: Some(365),
        loan_status: Some("originated".to_string()),
        metadata: None,
        custom_deal_id: None,
        status: Some("draft".to_string()),
        industry: Some("Technology".to_string()),
        document_ids: None,
        title: Some("Test Deal".to_string()),
        description: Some("Integration test deal".to_string()),
        risk_level: Some("Medium".to_string()),
        asset_class: Some("Corporate Bond".to_string()),
        region: Some("US".to_string()),
        minimum_investment: Some("10000.00".to_string()),
        collateral_id: None,
    };

    let response = api::handlers::deals::deals::create_deal_endpoint(
        axum::extract::State(state.clone()),
        axum::Json(request),
    )
    .await;

    assert!(response.is_ok(), "Deal creation should succeed");
    let deal_response = response.unwrap().0;

    // Verify deal exists
    let deal_id = uuid::Uuid::parse_str(&deal_response.deal_id).unwrap();
    let deal = get_deal_by_id(&pool, deal_id).await.unwrap().unwrap();
    assert_eq!(
        deal.principal_amount,
        rust_decimal::Decimal::from_str("100000.00").unwrap()
    );

    // Verify transaction was recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE deal_id = $1",
        deal_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].transaction_type, "deal_origination");
    assert_eq!(
        transactions[0].amount,
        rust_decimal::Decimal::from_str("100000.00").unwrap()
    );

    // Verify metadata contains originator and investor IDs
    let metadata: serde_json::Value = transactions[0].metadata.as_ref().unwrap().clone();
    assert!(metadata.get("originator_id").is_some());
    assert!(metadata.get("investor_id").is_some());
}

/// Test complete deal lifecycle with transactions
#[sqlx::test]
async fn test_complete_deal_lifecycle_with_transactions(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;

    // 1. Create deal
    let deal_id = create_test_deal_via_api(&state).await;

    // 2. Make partial payment
    let payment1_response = api::handlers::deals::deals::make_payment_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::MakePaymentRequest {
            amount: "50000.00".to_string(),
            payment_type: Some("regular".to_string()),
            notes: Some("First payment".to_string()),
        }),
    )
    .await;

    assert!(payment1_response.is_ok(), "First payment should succeed");
    let payment1 = payment1_response.unwrap().0;
    assert_eq!(payment1.new_outstanding_balance, "50000.00");
    assert_eq!(payment1.deal_status, "active");

    // 3. Make final payment
    let payment2_response = api::handlers::deals::deals::make_payment_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::MakePaymentRequest {
            amount: "50000.00".to_string(),
            payment_type: Some("regular".to_string()),
            notes: Some("Final payment".to_string()),
        }),
    )
    .await;

    assert!(payment2_response.is_ok(), "Final payment should succeed");
    let payment2 = payment2_response.unwrap().0;
    assert_eq!(payment2.new_outstanding_balance, "0.00");
    assert_eq!(payment2.deal_status, "paid_off");

    // 4. Close out deal
    let closeout_response = api::handlers::deals::deals::closeout_deal_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::CloseoutDealRequest {
            notes: Some("Deal completed".to_string()),
            confirm_paid_off: true,
        }),
    )
    .await;

    assert!(closeout_response.is_ok(), "Closeout should succeed");
    let closeout = closeout_response.unwrap().0;
    assert_eq!(closeout.deal_status, "closed");

    // 5. Verify complete transaction history
    let transactions = sqlx::query!(
        "SELECT transaction_type, amount
         FROM deal_transactions
         WHERE deal_id = $1
         ORDER BY created_at ASC",
        deal_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(transactions.len(), 4); // origination + 2 payments + closeout
    assert_eq!(transactions[0].transaction_type, "deal_origination");
    assert_eq!(transactions[1].transaction_type, "deal_payment");
    assert_eq!(transactions[2].transaction_type, "deal_payment");
    assert_eq!(transactions[3].transaction_type, "deal_closeout");

    // Verify amounts
    assert_eq!(
        transactions[0].amount,
        rust_decimal::Decimal::from_str("100000.00").unwrap()
    );
    assert_eq!(
        transactions[1].amount,
        rust_decimal::Decimal::from_str("50000.00").unwrap()
    );
    assert_eq!(
        transactions[2].amount,
        rust_decimal::Decimal::from_str("50000.00").unwrap()
    );
    assert_eq!(transactions[3].amount, rust_decimal::Decimal::ZERO);
}

/// Test payment validation errors
#[sqlx::test]
async fn test_payment_validation_errors(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let deal_id = create_test_deal_via_api(&state).await;

    // Try to pay more than outstanding balance
    let result = api::handlers::deals::deals::make_payment_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::MakePaymentRequest {
            amount: "200000.00".to_string(), // Deal is only $100k
            payment_type: None,
            notes: None,
        }),
    )
    .await;

    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(message.contains("exceeds outstanding balance"));

    // Verify no transaction was recorded
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM deal_transactions
         WHERE deal_id = $1 AND transaction_type = 'deal_payment'",
        deal_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count.count, Some(0));
}

/// Test closeout validation errors
#[sqlx::test]
async fn test_closeout_validation_errors(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let deal_id = create_test_deal_via_api(&state).await;

    // Try to close deal that isn't paid off
    let result = api::handlers::deals::deals::closeout_deal_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::CloseoutDealRequest {
            notes: None,
            confirm_paid_off: true,
        }),
    )
    .await;

    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(message.contains("Must be 'paid_off'"));
}

/// Test payment on closed deal fails
#[sqlx::test]
async fn test_payment_on_closed_deal_fails(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;

    // Create and close deal
    let deal_id = create_and_close_test_deal_via_api(&state).await;

    // Try to make payment on closed deal
    let result = api::handlers::deals::deals::make_payment_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::MakePaymentRequest {
            amount: "1000.00".to_string(),
            payment_type: None,
            notes: None,
        }),
    )
    .await;

    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(message.contains("Cannot make payment on deal with status"));
}

/// Test transaction recording service directly
#[sqlx::test]
async fn test_transaction_recording_service(pool: PgPool) {
    let service = TransactionRecordingService::new(pool.clone());

    let user_id = uuid::Uuid::new_v4();
    let deal_id = uuid::Uuid::new_v4();

    // Test UserRegistration
    let tx_id = service
        .record_transaction(
            arda_credit_lib::TransactionType::UserRegistration,
            user_id,
            None,
            rust_decimal::Decimal::ZERO,
            Some(serde_json::json!({
                "email": "test@example.com",
                "user_type": "investor"
            })),
        )
        .await
        .unwrap();

    // Verify transaction was created
    let record = sqlx::query!("SELECT * FROM deal_transactions WHERE id = $1", tx_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(record.transaction_type, "user_registration");
    assert_eq!(record.user_id, Some(user_id));
    assert_eq!(record.deal_id, None);
    assert_eq!(record.amount, rust_decimal::Decimal::ZERO);

    // Test DealOrigination
    let tx_id2 = service
        .record_transaction(
            arda_credit_lib::TransactionType::DealOrigination,
            user_id,
            Some(deal_id),
            rust_decimal::Decimal::from(100000),
            Some(serde_json::json!({
                "originator_id": user_id.to_string(),
                "investor_id": uuid::Uuid::new_v4().to_string(),
                "interest_rate_bps": 500
            })),
        )
        .await
        .unwrap();

    let record2 = sqlx::query!("SELECT * FROM deal_transactions WHERE id = $1", tx_id2)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(record2.transaction_type, "deal_origination");
    assert_eq!(record2.user_id, Some(user_id));
    assert_eq!(record2.deal_id, Some(deal_id));
    assert_eq!(record2.amount, rust_decimal::Decimal::from(100000));
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create test API state for integration tests
async fn create_test_api_state(pool: PgPool) -> api::api::router::ApiState {
    use api::api::router::{ApiConfig, ApiState};
    use api::services::{MetricsCache, MetricsCacheConfig, TransactionRecordingService};
    use arda_credit_db::ads::{AdsConfig, IndexedMerkleTreeADS};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = ApiConfig::default();

    // Create ADS service with default config
    let ads_config = AdsConfig::default();
    let ads_service = Arc::new(RwLock::new(
        IndexedMerkleTreeADS::new(pool.clone(), ads_config)
            .await
            .unwrap(),
    ));

    let session_service = Arc::new(api::services::session_service::SessionService::new(
        pool.clone(),
    ));
    let transaction_recording = Arc::new(TransactionRecordingService::new(pool.clone()));

    // Create metrics cache config
    let metrics_config = MetricsCacheConfig {
        ttl: std::time::Duration::from_secs(config.metrics_cache_ttl_seconds),
        max_entries: config.metrics_cache_max_entries,
        enable_stats: true,
    };
    let metrics_cache = Arc::new(MetricsCache::new(metrics_config));

    let withdrawal_service =
        api::services::withdrawal_service::WithdrawalTransactionService::new(pool.clone());
    let balance_service =
        api::services::balance_service::BalanceVerificationService::new(pool.clone());

    ApiState {
        pool,
        config,
        batch_processor: None,
        ads_service,
        session_service,
        magic_link_service: None,
        team_invitation_service: None,
        withdrawal_service,
        balance_service,
        withdrawal_executor: None,
        metrics_handle: None,
        email_rate_limiter: None,
        metrics_cache,
        transaction_recording,
        event_publisher: None,
    }
}

/// Create a test company for integration tests
async fn create_test_company(pool: &PgPool, name: &str) -> Company {
    let request = CreateCompanyRequest {
        name: name.to_string(),
        description: Some(format!("Test company: {}", name)),
        metadata: None,
    };
    create_company(pool, &request).await.unwrap()
}

/// Create a test deal via API for integration tests
async fn create_test_deal_via_api(state: &api::api::router::ApiState) -> uuid::Uuid {
    let pool = &state.pool;

    // Create companies
    let originator = create_test_company(pool, "Test Originator Corp").await;
    let investor = create_test_company(pool, "Test Investor Corp").await;

    // Create deal via API
    let request = api::handlers::deals::deals::CreateDealRequest {
        originator_id: originator.id.to_string(),
        originator_name: None,
        investor_id: Some(investor.id.to_string()),
        investor_name: None,
        borrower_id: None,
        borrower_name: None,
        principal_amount: "100000.00".to_string(),
        interest_rate_bps: 500,
        term_days: Some(365),
        loan_status: Some("originated".to_string()),
        metadata: None,
        custom_deal_id: None,
        status: Some("draft".to_string()),
        industry: Some("Technology".to_string()),
        document_ids: None,
        title: Some("Test Deal".to_string()),
        description: Some("Integration test deal".to_string()),
        risk_level: Some("Medium".to_string()),
        asset_class: Some("Corporate Bond".to_string()),
        region: Some("US".to_string()),
        minimum_investment: Some("10000.00".to_string()),
        collateral_id: None,
    };

    let response = api::handlers::deals::deals::create_deal_endpoint(
        axum::extract::State(state.clone()),
        axum::Json(request),
    )
    .await
    .expect("Deal creation should succeed");

    uuid::Uuid::parse_str(&response.0.deal_id).unwrap()
}

/// Create and close a test deal via API for integration tests
async fn create_and_close_test_deal_via_api(state: &api::api::router::ApiState) -> uuid::Uuid {
    let deal_id = create_test_deal_via_api(state).await;

    // Make full payment
    let _ = api::handlers::deals::deals::make_payment_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::MakePaymentRequest {
            amount: "100000.00".to_string(),
            payment_type: Some("regular".to_string()),
            notes: Some("Full payment".to_string()),
        }),
    )
    .await
    .expect("Full payment should succeed");

    // Close deal
    let _ = api::handlers::deals::deals::closeout_deal_endpoint(
        axum::extract::State(state.clone()),
        axum::extract::Path(deal_id.to_string()),
        axum::Json(api::handlers::deals::deals::CloseoutDealRequest {
            notes: Some("Completed".to_string()),
            confirm_paid_off: true,
        }),
    )
    .await
    .expect("Closeout should succeed");

    deal_id
}
