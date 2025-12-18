/// Integration tests for batch API endpoints with state roots and contract fields
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

// Note: This test assumes you have integration test helpers set up
// If not, you may need to adapt the setup to your test infrastructure

#[tokio::test]
#[ignore] // Remove this when you have test infrastructure ready
async fn test_batch_api_returns_state_roots() {
    // This is a template - adapt to your test setup
    // Typically you'd:
    // 1. Initialize test app/server
    // 2. Create state transactions via API or directly in DB
    // 3. Trigger batch creation
    // 4. Call GET /api/v1/state/batches
    // 5. Assert response contains state roots and correct transaction_count

    // Example structure (adapt to your setup):
    /*
    let app = test_app().await;

    // Create test transactions...

    // Trigger batch creation
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/batch/trigger")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Get batches
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/state/batches")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert structure
    assert!(json["batches"].is_array());
    let batch = &json["batches"][0];

    // NEW: Assert state roots are present
    assert!(batch["previous_state_root"].is_string());
    assert!(batch["final_state_root"].is_string());

    // Assert roots are 0x-prefixed hex
    let prev_root = batch["previous_state_root"].as_str().unwrap();
    let final_root = batch["final_state_root"].as_str().unwrap();
    assert!(prev_root.starts_with("0x"));
    assert!(final_root.starts_with("0x"));
    assert_eq!(prev_root.len(), 66); // 0x + 64 hex chars = 32 bytes
    assert_eq!(final_root.len(), 66);

    // Assert transaction_count > 0
    let tx_count = batch["transaction_count"].as_u64().unwrap();
    assert!(tx_count > 0);

    // OLD FIELDS REMOVED: previous_counter_value, final_counter_value
    assert!(batch.get("previous_counter_value").is_none());
    assert!(batch.get("final_counter_value").is_none());

    println!("✅ Batch API returns state roots and transaction count correctly");
    */

    // For now, just document the expected behavior
    println!("📋 Test template created - adapt to your integration test infrastructure");
    println!("   Expected assertions:");
    println!("   - Response contains previous_state_root (0x-prefixed, 66 chars)");
    println!("   - Response contains final_state_root (0x-prefixed, 66 chars)");
    println!("   - Response contains transaction_count > 0");
    println!("   - Response does NOT contain previous_counter_value or final_counter_value");
    println!("   - Response contains contract posting fields (posted_to_contract, etc.)");
}

#[tokio::test]
#[ignore] // Remove this when you have test infrastructure ready
async fn test_batch_api_contract_fields_match_db() {
    // This test verifies that contract posting fields returned by the API
    // match what's actually in the database

    println!("📋 Test template for contract field matching:");
    println!("   1. Create and prove a batch");
    println!("   2. Mark it as posted to contract with tx hash");
    println!("   3. Call GET /api/v1/state/batches");
    println!("   4. Assert posted_to_contract=true");
    println!("   5. Assert posted_to_contract_at is not null");
    println!("   6. Assert contract_tx_hash matches what was set");
}

#[tokio::test]
#[ignore] // Remove this when you have test infrastructure ready
async fn test_batch_detail_endpoint_includes_roots() {
    // Test GET /api/v1/state/batches/{id} includes state roots

    println!("📋 Test template for batch detail endpoint:");
    println!("   1. Create a batch with ADS commit");
    println!("   2. Call GET /api/v1/state/batches/{{id}}");
    println!("   3. Assert response includes previous_state_root and final_state_root");
    println!("   4. Assert contract_prev_merkle_root and contract_new_merkle_root if available");
    println!("   5. Assert transaction_count matches state_transaction_ids length");
}
