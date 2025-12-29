// ! Re-Genesis API Handlers
//!
//! This module handles state re-genesis operations for the proof system.
//! Re-genesis resets the proof state to zero, marks old proofs as superseded,
//! and allows reprocessing of all transactions from genesis.
//!
//! ## Use Cases
//!
//! - Historical portfolio imports via CSV
//! - Populate script execution
//! - Manual admin-triggered state reset

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, instrument};
use uuid::Uuid;

/// Request to trigger re-genesis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RegenesisRequest {
    /// Reason for re-genesis (e.g., "historical_portfolio_import", "populate_script")
    pub reason: String,
    /// User ID of the person/system triggering re-genesis
    pub initiated_by_user_id: Uuid,
    /// Optional: Additional context about the re-genesis
    pub notes: Option<String>,
}

/// Response from re-genesis operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RegenesisResponse {
    /// ID of the re-genesis transaction created
    pub regenesis_tx_id: Uuid,
    /// Number of proofs marked as superseded
    pub proofs_superseded: i64,
    /// Number of transactions reset to pending
    pub transactions_reset: i64,
    /// Summary message
    pub message: String,
}

/// Trigger re-genesis: reset state to zero, mark old proofs as superseded
///
/// This endpoint:
/// 1. Creates a StateRegenesis transaction
/// 2. Marks all existing proven proofs as superseded
/// 3. Resets ALL transactions to pending (clears included_in_batch_id)
/// 4. Allows reprocessing from genesis with new historical data
#[cfg_attr(
    feature = "utoipa",
    utoipa::path(
        post,
        path = "/api/v1/batches/trigger-regenesis",
        request_body = RegenesisRequest,
        responses(
            (status = 200, description = "Re-genesis triggered successfully", body = RegenesisResponse),
            (status = 500, description = "Re-genesis failed")
        ),
        tag = "batches"
    )
)]
#[instrument(skip(pool), level = "info")]
pub async fn trigger_regenesis(
    pool: &PgPool,
    request: RegenesisRequest,
) -> Result<RegenesisResponse, String> {
    info!(
        "🔄 Re-genesis triggered by user {}: {}",
        request.initiated_by_user_id, request.reason
    );

    // 1. Create re-genesis transaction
    let regenesis_tx_id = create_regenesis_transaction(
        pool,
        &request.reason,
        request.initiated_by_user_id,
        request.notes.as_deref(),
    )
    .await?;

    // 2. Mark all existing proven/ready proofs as superseded
    let proofs_superseded = supersede_all_proofs(pool, regenesis_tx_id).await?;

    // 3. Reset all transactions to pending
    let transactions_reset = reset_transactions_to_pending(pool).await?;

    info!(
        "✅ Re-genesis completed: {} proofs superseded, {} transactions reset",
        proofs_superseded, transactions_reset
    );

    Ok(RegenesisResponse {
        regenesis_tx_id,
        proofs_superseded,
        transactions_reset,
        message: format!(
            "Re-genesis successful: {} proofs superseded, {} transactions will be reprocessed",
            proofs_superseded, transactions_reset
        ),
    })
}

/// Create a StateRegenesis transaction in the database
async fn create_regenesis_transaction(
    pool: &PgPool,
    reason: &str,
    initiated_by_user_id: Uuid,
    notes: Option<&str>,
) -> Result<Uuid, String> {
    let tx_id = Uuid::new_v4();
    let metadata = serde_json::json!({
        "reason": reason,
        "initiated_by": initiated_by_user_id.to_string(),
        "notes": notes,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // Hash the transaction ID and user ID for zkVM compatibility
    let tx_id_hash = arda_credit_lib::hash_uuid(&tx_id);
    let user_id_hash = arda_credit_lib::hash_uuid(&initiated_by_user_id);

    // Create DealTransaction for state_regenesis
    use arda_credit_lib::{DealTransaction, MetadataArray, TransactionType};

    let deal_transaction = DealTransaction {
        transaction_id: tx_id_hash,
        user_id: user_id_hash,
        originator_company_id: [0u8; 32],
        investor_company_id: [0u8; 32],
        deal_id: [0u8; 32], // NULL for state_regenesis
        transaction_type: TransactionType::StateRegenesis,
        amount: 0,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metadata: MetadataArray::from_bytes(
            &serde_json::to_string(&metadata)
                .unwrap_or_default()
                .into_bytes(),
        ),
    };

    // Serialize transaction data
    let transaction_data =
        bincode::serde::encode_to_vec(&deal_transaction, bincode::config::standard())
            .map_err(|e| format!("Failed to serialize transaction: {}", e))?;

    // Insert into state_transactions
    arda_credit_db::create_state_transaction(
        pool,
        "state_regenesis",
        &transaction_data,
        Some(initiated_by_user_id),
        None, // No deal_id for state_regenesis
        None,
        None,
        None, // No collateral_id
    )
    .await
    .map_err(|e| format!("Failed to insert re-genesis transaction: {}", e))?;

    info!("📝 Created re-genesis transaction: {}", tx_id);
    Ok(tx_id)
}

/// Mark all non-superseded proven proofs as superseded
async fn supersede_all_proofs(pool: &PgPool, regenesis_tx_id: Uuid) -> Result<i64, String> {
    let result = sqlx::query!(
        r#"
        UPDATE proof_batches 
        SET proof_status = 'superseded',
            superseded_at = NOW(),
            superseded_by_tx = $1
        WHERE proof_status IN ('proven', 'ready')
          AND superseded_at IS NULL
        "#,
        regenesis_tx_id
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to update proof statuses: {}", e))?;

    info!("🔄 Superseded {} proofs", result.rows_affected());
    Ok(result.rows_affected() as i64)
}

/// Reset all transactions to pending (clear included_in_batch_id)
async fn reset_transactions_to_pending(pool: &PgPool) -> Result<i64, String> {
    let result = sqlx::query!(
        r#"
        UPDATE state_transactions 
        SET included_in_batch_id = NULL
        WHERE included_in_batch_id IS NOT NULL
        "#
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to reset transactions: {}", e))?;

    info!(
        "🔄 Reset {} transactions to pending",
        result.rows_affected()
    );
    Ok(result.rows_affected() as i64)
}

/// Get current processing mode (genesis vs incremental)
#[cfg_attr(
    feature = "utoipa",
    utoipa::path(
        get,
        path = "/api/v1/batches/state-mode",
        responses(
            (status = 200, description = "Current state mode", body = StateModeResponse),
        ),
        tag = "batches"
    )
)]
pub async fn get_state_mode(pool: &PgPool) -> Result<StateModeResponse, String> {
    determine_state_mode(pool).await
}

/// Response showing current state mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StateModeResponse {
    /// Current processing mode: "genesis" or "incremental"
    pub mode: String,
    /// ID of latest proven batch (if any)
    pub latest_proven_batch: Option<i32>,
    /// Total pending transactions
    pub pending_transactions: i64,
}

/// Determine current state mode based on database state
pub async fn determine_state_mode(pool: &PgPool) -> Result<StateModeResponse, String> {
    // Check if there are any non-superseded proven batches
    let latest_batch = sqlx::query!(
        r#"
        SELECT id 
        FROM proof_batches 
        WHERE proof_status = 'proven' 
          AND superseded_at IS NULL
        ORDER BY id DESC 
        LIMIT 1
        "#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to query proven batches: {}", e))?;

    // Count pending transactions
    let pending_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM state_transactions WHERE included_in_batch_id IS NULL"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to count pending transactions: {}", e))?
    .unwrap_or(0);

    let mode = if latest_batch.is_some() {
        "incremental"
    } else {
        "genesis"
    };

    Ok(StateModeResponse {
        mode: mode.to_string(),
        latest_proven_batch: latest_batch.map(|b| b.id),
        pending_transactions: pending_count,
    })
}
