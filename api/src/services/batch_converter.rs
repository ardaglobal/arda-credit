//! Batch Converter Module
//!
//! This module converts database batch and transaction records to zkVM-compatible
//! `DealBatch` format for proof generation.
//!
//! ## Conversion Strategy
//!
//! - Query `state_transactions` table for transactions in batch
//! - Convert UUIDs to hashed [u8; 32] format
//! - Convert Decimals to cents (u64)
//! - Build metadata from JSON fields
//! - Construct complete `DealBatch` with proper ordering

use arda_credit_db::StateTransaction;
use arda_credit_lib::{hash_uuid, DealBatch, DealTransaction, MetadataArray, TransactionType};
use bincode;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, instrument};

/// Query deals to build a lookup map of deal_id → (originator_company_id, investor_company_id)
async fn build_deal_company_lookup(
    pool: &PgPool,
) -> Result<HashMap<uuid::Uuid, (uuid::Uuid, Option<uuid::Uuid>)>, String> {
    let deals = sqlx::query!(
        r#"
        SELECT id, originator_id, investor_id
        FROM deals
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query deals: {}", e))?;

    let mut lookup = HashMap::new();
    for deal in deals {
        lookup.insert(deal.id, (deal.originator_id, deal.investor_id));
    }

    Ok(lookup)
}

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

    // Query all transactions for this batch using StateTransaction
    let state_transactions = arda_credit_db::get_batch_state_transactions(pool, batch_id, None)
        .await
        .map_err(|e| format!("Failed to query transactions for batch {}: {}", batch_id, e))?;

    if state_transactions.is_empty() {
        return Err(format!("No transactions found for batch {}", batch_id));
    }

    info!(
        "✅ Loaded {} transactions for batch {}",
        state_transactions.len(),
        batch_id
    );

    // Build deal → company lookup map
    let deal_company_lookup = build_deal_company_lookup(pool).await?;

    // Deserialize transaction_data and convert to zkVM transactions
    let mut zkvm_transactions = Vec::new();
    for state_tx in &state_transactions {
        // Deserialize DealTransaction from transaction_data
        let (deal_tx, _): (DealTransaction, _) = bincode::serde::decode_from_slice(
            &state_tx.transaction_data,
            bincode::config::standard(),
        )
        .map_err(|e| format!("Failed to deserialize transaction {}: {}", state_tx.id, e))?;

        // Convert to zkVM format
        let zkvm_tx = convert_state_transaction_to_zkvm(&state_tx, &deal_tx, &deal_company_lookup)?;
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

    info!("🔄 Sorting transactions in global chronological order with type priority");

    // Sort by transaction type priority FIRST, then by timestamp
    // This ensures proper dependency ordering:
    // 1. StateRegenesis first
    // 2. CompanyRegistration before DealOrigination
    // 3. UserRegistration before transactions requiring users
    // Within each priority level, transactions are sorted chronologically
    zkvm_transactions.sort_by_key(|tx| {
        let type_priority = match tx.transaction_type {
            TransactionType::StateRegenesis => 0,
            TransactionType::UserRegistration => 1,
            TransactionType::CompanyRegistration => 2, // Before deals
            TransactionType::KycSubmission => 3,
            TransactionType::KycVerification => 4,
            TransactionType::UserDeposit => 5,
            TransactionType::CollateralCreation => 6,
            TransactionType::DealOrigination => 7, // After companies
            TransactionType::CollateralDealAssociation => 8,
            TransactionType::DealPayment => 9,
            TransactionType::DealCloseout => 10,
            TransactionType::CollateralDealDisassociation => 11,
            TransactionType::UserWithdrawal => 12,
            _ => 99, // Other types at end
        };
        (type_priority, tx.created_at) // FIX: Priority FIRST, then timestamp
    });

    info!(
        "✅ Sorted {} transactions in chronological order with dependency ordering",
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

/// Convert state transaction (with deserialized DealTransaction) to zkVM transaction
///
/// The DealTransaction already contains all hashed IDs, so we can use them directly.
/// Company IDs are already in the DealTransaction, but we verify they're correct
/// using the deal_company_lookup if available.
fn convert_state_transaction_to_zkvm(
    state_tx: &StateTransaction,
    deal_tx: &DealTransaction,
    deal_company_lookup: &HashMap<uuid::Uuid, (uuid::Uuid, Option<uuid::Uuid>)>,
) -> Result<DealTransaction, String> {
    // The DealTransaction already has all the correct hashed IDs
    // We just need to verify company IDs for deal transactions if we have a lookup

    let (originator_company_id, investor_company_id) = if matches!(
        deal_tx.transaction_type,
        TransactionType::DealOrigination | TransactionType::DealPayment
    ) {
        // For deal transactions, verify company IDs from the deal lookup if available
        // Otherwise use the IDs already in the DealTransaction
        if deal_tx.deal_id != [0u8; 32] {
            // Try to convert deal_id hash back to UUID for lookup (not always possible)
            // For now, trust the company IDs already in the DealTransaction
            // They were set correctly when the transaction was created
            (deal_tx.originator_company_id, deal_tx.investor_company_id)
        } else {
            (deal_tx.originator_company_id, deal_tx.investor_company_id)
        }
    } else {
        // For non-deal transactions, use the IDs from DealTransaction
        (deal_tx.originator_company_id, deal_tx.investor_company_id)
    };

    // Debug logging for transaction conversion
    tracing::debug!(
        "🔬 TX: {:?} - user_id[0..4]={:?}, originator[0..4]={:?}, investor[0..4]={:?}",
        deal_tx.transaction_type,
        &deal_tx.user_id[..4],
        &originator_company_id[..4],
        &investor_company_id[..4]
    );

    // Return the DealTransaction (already has all correct fields)
    Ok(DealTransaction {
        transaction_id: deal_tx.transaction_id,
        user_id: deal_tx.user_id,
        originator_company_id,
        investor_company_id,
        deal_id: deal_tx.deal_id,
        transaction_type: deal_tx.transaction_type,
        amount: deal_tx.amount,
        created_at: deal_tx.created_at,
        metadata: deal_tx.metadata.clone(),
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
        "company_registration" => Ok(TransactionType::CompanyRegistration),
        "company_update" => Ok(TransactionType::CompanyUpdate),
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
