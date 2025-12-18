/// Tests for proof_batches state roots and transaction membership
use db::test_utils::TestDatabase;
use db::{
    create_batch_from_state_transactions, create_state_transaction, get_batch_by_id,
    store_ads_state_commit,
};

#[tokio::test]
async fn test_batch_records_state_transaction_ids() {
    // Initialize test database (creates a fresh DB and runs migrations)
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to initialize test database");
    let pool = test_db.pool.clone();

    // Create 3 state transactions
    let tx1_id = create_state_transaction(
        &pool,
        "deal_origination",
        &[1, 2, 3],
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create state transaction 1");

    let tx2_id = create_state_transaction(
        &pool,
        "deal_payment",
        &[4, 5, 6],
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create state transaction 2");

    let tx3_id = create_state_transaction(
        &pool,
        "deal_closeout",
        &[7, 8, 9],
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create state transaction 3");

    // Create batch with these transactions
    let batch = create_batch_from_state_transactions(&pool, 10)
        .await
        .expect("Failed to create batch")
        .expect("Expected a batch to be created");

    // Verify batch has correct state_transaction_ids
    assert_eq!(
        batch.state_transaction_ids.len(),
        3,
        "Batch should contain 3 state transactions"
    );

    // Verify the IDs match (order matters - FIFO by created_at, id)
    assert!(
        batch.state_transaction_ids.contains(&tx1_id),
        "Batch should contain tx1"
    );
    assert!(
        batch.state_transaction_ids.contains(&tx2_id),
        "Batch should contain tx2"
    );
    assert!(
        batch.state_transaction_ids.contains(&tx3_id),
        "Batch should contain tx3"
    );

    println!(
        "✅ Batch {} created with {} state transactions",
        batch.id,
        batch.state_transaction_ids.len()
    );
}

#[tokio::test]
async fn test_batch_state_roots_updated_on_ads_commit() {
    // Initialize test database
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to initialize test database");
    let pool = test_db.pool.clone();

    // Create a state transaction
    let _tx_id = create_state_transaction(
        &pool,
        "deal_origination",
        &[1, 2, 3],
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create state transaction");

    // Create batch
    let batch = create_batch_from_state_transactions(&pool, 10)
        .await
        .expect("Failed to create batch")
        .expect("Expected a batch to be created");

    // Before ADS commit, roots should be None
    assert!(
        batch.previous_state_root.is_none(),
        "previous_state_root should be None before ADS commit"
    );
    assert!(
        batch.final_state_root.is_none(),
        "final_state_root should be None before ADS commit"
    );

    // Store ADS commit with a mock merkle root
    let merkle_root = vec![0xAA; 32]; // 32-byte mock root
    store_ads_state_commit(&pool, batch.id, &merkle_root)
        .await
        .expect("Failed to store ADS commit");

    // Fetch batch again
    let batch_after = get_batch_by_id(&pool, batch.id)
        .await
        .expect("Failed to get batch after ADS commit");

    // Verify roots are now populated
    assert!(
        batch_after.previous_state_root.is_some(),
        "previous_state_root should be set after ADS commit"
    );
    assert!(
        batch_after.final_state_root.is_some(),
        "final_state_root should be set after ADS commit"
    );

    // Verify final_state_root matches what we stored
    assert_eq!(
        batch_after.final_state_root.unwrap(),
        merkle_root,
        "final_state_root should match stored merkle root"
    );

    // For the first batch, previous_state_root should be all zeros (genesis)
    let expected_genesis = vec![0u8; 32];
    assert_eq!(
        batch_after.previous_state_root.unwrap(),
        expected_genesis,
        "First batch previous_state_root should be genesis (all zeros)"
    );

    println!(
        "✅ Batch {} state roots updated: prev=genesis, final=0x{}",
        batch_after.id,
        hex::encode(&merkle_root)
    );
}

#[tokio::test]
async fn test_sequential_batches_link_state_roots() {
    // Initialize test database
    let test_db = TestDatabase::new()
        .await
        .expect("Failed to initialize test database");
    let pool = test_db.pool.clone();

    // Create batch 1
    let _tx1_id = create_state_transaction(
        &pool,
        "deal_origination",
        &[1],
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create tx1");

    let batch1 = create_batch_from_state_transactions(&pool, 10)
        .await
        .expect("Failed to create batch1")
        .expect("Expected batch1");

    let merkle_root_1 = vec![0xAA; 32];
    store_ads_state_commit(&pool, batch1.id, &merkle_root_1)
        .await
        .expect("Failed to store ADS commit for batch1");

    // Create batch 2
    let _tx2_id =
        create_state_transaction(&pool, "deal_payment", &[2], None, None, None, None, None)
            .await
            .expect("Failed to create tx2");

    let batch2 = create_batch_from_state_transactions(&pool, 10)
        .await
        .expect("Failed to create batch2")
        .expect("Expected batch2");

    let merkle_root_2 = vec![0xBB; 32];
    store_ads_state_commit(&pool, batch2.id, &merkle_root_2)
        .await
        .expect("Failed to store ADS commit for batch2");

    // Fetch both batches
    let batch1_final = get_batch_by_id(&pool, batch1.id)
        .await
        .expect("Failed to get batch1");
    let batch2_final = get_batch_by_id(&pool, batch2.id)
        .await
        .expect("Failed to get batch2");

    // Verify batch2's previous_state_root equals batch1's final_state_root
    let batch1_final_root = batch1_final
        .final_state_root
        .as_ref()
        .expect("Batch 1 final_state_root should be set");
    let batch2_prev_root = batch2_final
        .previous_state_root
        .as_ref()
        .expect("Batch 2 previous_state_root should be set");

    assert_eq!(
        batch2_prev_root, batch1_final_root,
        "Batch 2's previous_state_root should equal batch 1's final_state_root"
    );

    println!(
        "✅ Sequential batches linked: batch1.final=0x{} → batch2.prev=0x{}",
        hex::encode(batch1_final_root),
        hex::encode(batch2_prev_root)
    );
}
