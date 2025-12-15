//! Batch Converter Module
//!
//! This module converts database batch and transaction records to zkVM-compatible
//! `DealBatch` format for proof generation.
//!
//! ## Conversion Strategy
//!
//! - Query `deal_transactions` table for transactions in batch
//! - Convert UUIDs to hashed [u8; 32] format
//! - Convert Decimals to cents (u64)
//! - Build metadata from JSON fields
//! - Construct complete `DealBatch` with proper ordering

use arda_credit_db::DealTransactionDb;
use arda_credit_lib::{hash_uuid, DealBatch, DealTransaction, MetadataArray, TransactionType};
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use tracing::{info, instrument};

/// Convert database batch to zkVM `DealBatch` format
///
/// Queries all transactions for the given batch ID and converts them to
/// zkVM-compatible format with hashed IDs and fixed-point amounts.
///
/// # Errors
///
/// Returns error if:
/// - Database queries fail
/// - No transactions found for batch
/// - Data conversion fails
/// - Invalid transaction types
#[instrument(skip(pool), level = "info")]
pub async fn convert_to_deal_batch(pool: &PgPool, batch_id: i32) -> Result<DealBatch, String> {
    info!("🔄 Converting batch {} to DealBatch format", batch_id);

    // Query batch metadata
    let batch_row = sqlx::query!(
        r#"
        SELECT id, previous_counter_value, final_counter_value, created_at
        FROM proof_batches
        WHERE id = $1
        "#,
        batch_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to query batch {}: {}", batch_id, e))?;

    // Get previous state root from ADS commits
    let state_root_row = sqlx::query!(
        r#"
        SELECT merkle_root
        FROM ads_state_commits
        WHERE batch_id = (
            SELECT MAX(id) FROM proof_batches WHERE id < $1 AND proof_status = 'proven'
        )
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        batch_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to query previous state root: {}", e))?;

    let previous_state_root: [u8; 32] = if let Some(row) = state_root_row {
        row.merkle_root
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid merkle root length".to_string())?
    } else {
        // No previous batches, use zero root
        [0u8; 32]
    };

    // Query all transactions for this batch
    let db_transactions = sqlx::query_as!(
        DealTransactionDb,
        r#"
        SELECT 
            id, transaction_id_hash, user_id_hash, deal_id_hash,
            transaction_type, amount, user_id, deal_id, payment_id,
            included_in_batch_id, metadata, created_at as "created_at!"
        FROM deal_transactions
        WHERE included_in_batch_id = $1
        ORDER BY created_at, id
        "#,
        batch_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query transactions for batch {}: {}", batch_id, e))?;

    if db_transactions.is_empty() {
        return Err(format!("No transactions found for batch {}", batch_id));
    }

    info!(
        "✅ Loaded {} transactions for batch {}",
        db_transactions.len(),
        batch_id
    );

    // Convert database transactions to zkVM transactions
    let mut zkvm_transactions = Vec::new();
    for db_tx in &db_transactions {
        let zkvm_tx = convert_db_transaction_to_zkvm(db_tx)?;
        zkvm_transactions.push(zkvm_tx);
    }

    // ============================================================================
    // SIMPLE GLOBAL TIMESTAMP SORT
    // ============================================================================
    //
    // Since we process from genesis (empty state with last_updated = 0), we only need
    // to sort all transactions by timestamp globally. Natural chronological order:
    // - Processes history in real-world order
    // - Deal lifecycles emerge naturally (origination → payments → closeout)
    // - Collateral conflicts detected at correct historical moment
    // - Works for both historical imports and real-time operations
    //
    // Stable sort preserves database order for transactions with equal timestamps.

    info!("🔄 Sorting transactions in global chronological order");

    // Simple global timestamp sort (stable to preserve DB order for equal timestamps)
    zkvm_transactions.sort_by_key(|tx| tx.created_at);

    info!(
        "✅ Sorted {} transactions in chronological order (oldest to newest)",
        zkvm_transactions.len()
    );

    // Create batch ID hash from batch number
    let batch_id_bytes = batch_id.to_le_bytes();
    let mut batch_id_hash = [0u8; 32];
    batch_id_hash[..4].copy_from_slice(&batch_id_bytes);

    let deal_batch = DealBatch {
        batch_id: batch_id_hash,
        transactions: zkvm_transactions,
        previous_state_root,
        created_at: batch_row
            .created_at
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or_else(|| Utc::now().timestamp() as u64),
    };

    info!(
        "✅ Converted batch {} to DealBatch with {} transactions",
        batch_id,
        deal_batch.transactions.len()
    );

    Ok(deal_batch)
}

/// Convert database transaction to zkVM transaction
fn convert_db_transaction_to_zkvm(db_tx: &DealTransactionDb) -> Result<DealTransaction, String> {
    // Use pre-computed hashes if available, otherwise hash the UUIDs
    let transaction_id =
        if !db_tx.transaction_id_hash.is_empty() && db_tx.transaction_id_hash.len() == 32 {
            db_tx
                .transaction_id_hash
                .as_slice()
                .try_into()
                .map_err(|_| "Invalid transaction_id_hash length".to_string())?
        } else {
            hash_uuid(&db_tx.id)
        };

    let user_id = if !db_tx.user_id_hash.is_empty() && db_tx.user_id_hash.len() == 32 {
        db_tx
            .user_id_hash
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid user_id_hash length".to_string())?
    } else if let Some(uid) = db_tx.user_id {
        hash_uuid(&uid)
    } else {
        return Err("Transaction missing user_id".to_string());
    };

    let deal_id = if let Some(deal_id_hash) = &db_tx.deal_id_hash {
        if !deal_id_hash.is_empty() && deal_id_hash.len() == 32 {
            deal_id_hash
                .as_slice()
                .try_into()
                .map_err(|_| "Invalid deal_id_hash length".to_string())?
        } else if let Some(did) = db_tx.deal_id {
            hash_uuid(&did)
        } else {
            [0u8; 32] // No deal for this transaction
        }
    } else if let Some(did) = db_tx.deal_id {
        hash_uuid(&did)
    } else {
        [0u8; 32] // No deal for this transaction
    };

    // Parse transaction type
    let transaction_type = parse_transaction_type(&db_tx.transaction_type)?;

    // Convert amount from Decimal to cents (u64)
    let amount_cents = if db_tx.amount.is_zero() {
        0
    } else {
        (db_tx.amount * rust_decimal::Decimal::from(100))
            .to_u64()
            .ok_or_else(|| {
                format!(
                    "Failed to convert amount to cents for transaction {}",
                    db_tx.id
                )
            })?
    };

    // Convert metadata JSON to byte array
    let metadata = if let Some(ref metadata_json) = db_tx.metadata {
        // Check if metadata contains base64-encoded data
        if let Some(data_base64) = metadata_json.get("data_base64").and_then(|v| v.as_str()) {
            // Decode base64 to bytes
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data_base64)
                .map_err(|e| format!("Failed to decode metadata base64: {}", e))?;
            MetadataArray::from_bytes(&bytes)
        } else {
            // Fallback: serialize as compact JSON bytes (legacy format)
            let metadata_str = metadata_json.to_string();
            MetadataArray::from_bytes(metadata_str.as_bytes())
        }
    } else {
        MetadataArray::empty()
    };

    // CRITICAL DEBUG: Log EVERY transaction
    tracing::error!(
        "🔬 TX: {:?} - user_id[0..4]={:?}, metadata[0..4]={:?}, metadata_preview={:?}",
        transaction_type,
        &user_id[..4],
        &metadata.as_bytes()[..4],
        db_tx
            .metadata
            .as_ref()
            .map(|m| m.to_string().chars().take(40).collect::<String>())
    );

    Ok(DealTransaction {
        transaction_id,
        user_id,
        deal_id,
        transaction_type,
        amount: amount_cents,
        created_at: db_tx.created_at.timestamp() as u64,
        metadata,
    })
}

/// Parse transaction type string to enum
fn parse_transaction_type(type_str: &str) -> Result<TransactionType, String> {
    match type_str {
        "user_registration" => Ok(TransactionType::UserRegistration),
        "kyc_submission" => Ok(TransactionType::KycSubmission),
        "kyc_verification" => Ok(TransactionType::KycVerification),
        "deal_origination" | "loan_origination" => Ok(TransactionType::DealOrigination),
        "deal_payment" | "loan_payment" => Ok(TransactionType::DealPayment),
        "deal_closeout" | "loan_closeout" => Ok(TransactionType::DealCloseout),
        "user_deposit" => Ok(TransactionType::UserDeposit),
        "user_withdrawal" => Ok(TransactionType::UserWithdrawal),
        "investor_registration" => Ok(TransactionType::InvestorRegistration),
        "admin_registration" => Ok(TransactionType::AdminRegistration),
        "verify_document" => Ok(TransactionType::VerifyDocument),
        "collateral_creation" => Ok(TransactionType::CollateralCreation),
        "collateral_update" => Ok(TransactionType::CollateralUpdate),
        "collateral_deal_association" => Ok(TransactionType::CollateralDealAssociation),
        "collateral_deal_disassociation" => Ok(TransactionType::CollateralDealDisassociation),
        "collateral_detection_request" => Ok(TransactionType::CollateralDetectionRequest),
        "collateral_detection_update" => Ok(TransactionType::CollateralDetectionUpdate),
        "state_regenesis" => Ok(TransactionType::StateRegenesis),
        other => Err(format!("Unknown transaction type: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_transaction_type() {
        assert_eq!(
            parse_transaction_type("user_registration").unwrap(),
            TransactionType::UserRegistration
        );
        assert_eq!(
            parse_transaction_type("deal_origination").unwrap(),
            TransactionType::DealOrigination
        );
        assert_eq!(
            parse_transaction_type("loan_origination").unwrap(),
            TransactionType::DealOrigination
        );
        assert!(parse_transaction_type("invalid_type").is_err());
    }

    #[test]
    fn test_metadata_conversion() {
        let data = b"test metadata";
        let metadata = MetadataArray::from_bytes(data);
        assert_eq!(&metadata.as_bytes()[..data.len()], data);
    }
}
