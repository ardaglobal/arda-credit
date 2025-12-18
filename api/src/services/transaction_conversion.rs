//! Transaction Conversion Utilities
//!
//! This module provides conversion functions between database transaction formats
//! and zkVM transaction formats.

use anyhow::{anyhow, Context, Result};
use arda_credit_db::DealTransactionDb;
use arda_credit_lib::transactions::{DealTransaction, MetadataArray, TransactionType};
use rust_decimal::prelude::{ToPrimitive, *};
use std::str::FromStr;
use tracing::{debug, warn};

/// Convert DealTransactionDb (database format) to DealTransaction (zkVM format)
///
/// This function bridges the gap between the database layer and the zkVM layer,
/// converting rich database transaction records into the fixed-size format
/// required for zero-knowledge proof generation.
///
/// # Arguments
/// * `tx` - Database transaction record from state_transactions table
///
/// # Returns
/// DealTransaction struct suitable for zkVM processing
///
/// # Errors
/// Returns error if:
/// - Hash lengths are invalid (must be 32 bytes)
/// - Transaction type cannot be parsed
/// - Amount cannot be converted to u64
/// - Metadata serialization fails
pub fn deal_transaction_to_zkvm(tx: &DealTransactionDb) -> Result<DealTransaction> {
    // Convert transaction_id_hash (BYTEA) to [u8; 32]
    let transaction_id: [u8; 32] = tx
        .transaction_id_hash
        .as_slice()
        .try_into()
        .context("Invalid transaction_id_hash length - expected 32 bytes")?;

    // Convert user_id_hash (BYTEA) to [u8; 32]
    let user_id: [u8; 32] = tx
        .user_id_hash
        .as_slice()
        .try_into()
        .context("Invalid user_id_hash length - expected 32 bytes")?;

    // Convert deal_id_hash (optional BYTEA) to [u8; 32]
    // Use zeros if not present (for non-deal transactions like UserRegistration)
    let deal_id = match &tx.deal_id_hash {
        Some(hash) => {
            let result: Result<[u8; 32], _> = hash.as_slice().try_into();
            match result {
                Ok(arr) => arr,
                Err(_) => {
                    warn!(
                        "Invalid deal_id_hash length for transaction {}, using zeros",
                        tx.id
                    );
                    [0u8; 32]
                }
            }
        }
        None => [0u8; 32],
    };

    // Parse transaction type from string
    let transaction_type = TransactionType::from_str(&tx.transaction_type).map_err(|e| {
        anyhow!(
            "Failed to parse transaction type '{}': {}",
            tx.transaction_type,
            e
        )
    })?;

    // Convert amount from Decimal to u64
    // The database uses Decimal(18,6) but zkVM uses u64 (representing smallest units)
    let amount = tx
        .amount
        .to_u64()
        .ok_or_else(|| anyhow!("Amount out of u64 range: {}", tx.amount))?;

    // Convert timestamp to u64
    let created_at = tx.created_at.timestamp() as u64;

    // Convert metadata from JSON to fixed-size byte array
    // zkVM requires MetadataArray (64 bytes) for deterministic proof generation
    let metadata = if let Some(json) = &tx.metadata {
        // Serialize JSON to bytes
        let json_bytes = serde_json::to_vec(json).context("Failed to serialize metadata JSON")?;

        debug!(
            "Converting metadata for tx {}: {} bytes -> MetadataArray",
            tx.id,
            json_bytes.len()
        );

        // MetadataArray will truncate if > 64 bytes or pad if < 64 bytes
        MetadataArray::from_bytes(&json_bytes)
    } else {
        // No metadata - use empty array
        MetadataArray::empty()
    };

    Ok(DealTransaction {
        transaction_id,
        user_id,
        originator_company_id: [0u8; 32], // Not used in this conversion path
        investor_company_id: [0u8; 32],   // Not used in this conversion path
        deal_id,
        transaction_type,
        amount,
        created_at,
        metadata,
    })
}

/// Convert a batch of database transactions to zkVM format
///
/// Convenience function for batch conversion with detailed error reporting.
///
/// # Arguments
/// * `db_transactions` - Vector of database transaction records
///
/// # Returns
/// Vector of DealTransaction structs ready for zkVM batch processing
///
/// # Errors
/// Returns error if any transaction conversion fails
pub fn batch_convert_to_zkvm(
    db_transactions: &[DealTransactionDb],
) -> Result<Vec<DealTransaction>> {
    debug!(
        "Converting {} database transactions to zkVM format",
        db_transactions.len()
    );

    let mut zkvm_transactions = Vec::with_capacity(db_transactions.len());

    for (idx, tx) in db_transactions.iter().enumerate() {
        match deal_transaction_to_zkvm(tx) {
            Ok(zkvm_tx) => zkvm_transactions.push(zkvm_tx),
            Err(e) => {
                return Err(anyhow!(
                    "Failed to convert transaction at index {}, id {}: {}",
                    idx,
                    tx.id,
                    e
                ));
            }
        }
    }

    debug!(
        "Successfully converted {} transactions to zkVM format",
        zkvm_transactions.len()
    );

    Ok(zkvm_transactions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn test_deal_transaction_conversion() {
        // Create a mock database transaction
        let tx_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();

        // Hash the UUIDs (in real system, done by TransactionRecordingService)
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(tx_id.as_bytes());
        let tx_hash: Vec<u8> = hasher.finalize().to_vec();

        let mut hasher = Sha256::new();
        hasher.update(user_id.as_bytes());
        let user_hash: Vec<u8> = hasher.finalize().to_vec();

        let mut hasher = Sha256::new();
        hasher.update(deal_id.as_bytes());
        let deal_hash: Vec<u8> = hasher.finalize().to_vec();

        let db_tx = DealTransactionDb {
            id: tx_id,
            transaction_id_hash: tx_hash,
            user_id_hash: user_hash,
            deal_id_hash: Some(deal_hash),
            transaction_type: "deal_payment".to_string(),
            amount: Decimal::from(50000),
            user_id: Some(user_id),
            deal_id: Some(deal_id),
            payment_id: None,
            included_in_batch_id: None,
            metadata: Some(serde_json::json!({"payment_type": "regular"})),
            created_at: Utc::now(),
        };

        // Convert to zkVM format
        let result = deal_transaction_to_zkvm(&db_tx);
        assert!(result.is_ok(), "Conversion should succeed");

        let zkvm_tx = result.unwrap();
        assert_eq!(zkvm_tx.transaction_type, TransactionType::DealPayment);
        assert_eq!(zkvm_tx.amount, 50000);
    }

    #[test]
    fn test_transaction_without_deal_id() {
        // Test UserRegistration (no deal_id)
        let tx_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(tx_id.as_bytes());
        let tx_hash: Vec<u8> = hasher.finalize().to_vec();

        let mut hasher = Sha256::new();
        hasher.update(user_id.as_bytes());
        let user_hash: Vec<u8> = hasher.finalize().to_vec();

        let db_tx = DealTransactionDb {
            id: tx_id,
            transaction_id_hash: tx_hash,
            user_id_hash: user_hash,
            deal_id_hash: None, // No deal for user registration
            transaction_type: "user_registration".to_string(),
            amount: Decimal::ZERO,
            user_id: Some(user_id),
            deal_id: None,
            payment_id: None,
            included_in_batch_id: None,
            metadata: Some(serde_json::json!({"email": "test@example.com"})),
            created_at: Utc::now(),
        };

        let result = deal_transaction_to_zkvm(&db_tx);
        assert!(
            result.is_ok(),
            "Conversion should succeed for non-deal transaction"
        );

        let zkvm_tx = result.unwrap();
        assert_eq!(zkvm_tx.transaction_type, TransactionType::UserRegistration);
        assert_eq!(zkvm_tx.deal_id, [0u8; 32]); // Should be zeros
    }
}
