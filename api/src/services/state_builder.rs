//! State Builder Module
//!
//! This module builds `GlobalLoanState` from PostgreSQL database records
//! to prepare complete state structures for zkVM proof generation.
//!
//! ## Conversion Strategy
//!
//! - Database stores native format (UUIDs, Decimals, full strings)
//! - zkVM needs hashed IDs ([u8; 32]), timestamps (u64), and simplified enums
//! - This module bridges the gap by querying DB and converting to zkVM format
//!
//! ## State Loading Modes
//!
//! - **FromGenesis:** Empty state (users only) for first batch or after re-genesis
//! - **Incremental:** Load state from latest proven batch and process only new transactions

use arda_credit_db::User as DbUser;
use arda_credit_lib::{hash_uuid, BalanceState, GlobalLoanState, LoanTerms, User};

#[cfg(test)]
use arda_credit_lib::{KycStatus, UserType};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, instrument, warn};

/// Mode for loading state - determines whether to start from genesis or load from snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateLoadMode {
    /// Start from empty state (users only) - used for first batch or after re-genesis
    FromGenesis,
    /// Load state from latest proven batch - used for incremental processing
    Incremental,
}

/// Build complete `GlobalLoanState` from database with specified mode
///
/// # Arguments
///
/// - `pool`: Database connection pool
/// - `mode`: StateLoadMode - FromGenesis or Incremental
///
/// # Errors
///
/// Returns error if:
/// - Database queries fail
/// - Data conversion fails
/// - Invalid data found in database
#[instrument(skip(pool), level = "info")]
pub async fn build_global_loan_state_with_mode(
    pool: &PgPool,
    mode: StateLoadMode,
) -> Result<GlobalLoanState, String> {
    match mode {
        StateLoadMode::FromGenesis => {
            info!("🔄 Building state from genesis (empty state with users only)");
            build_empty_state_with_users(pool).await
        }
        StateLoadMode::Incremental => {
            info!("📊 Loading state incrementally from latest proven batch");
            load_latest_state_snapshot(pool).await
        }
    }
}

/// Build empty state from genesis (users and companies) - for MVP and re-genesis
///
/// This is the current MVP implementation that was previously the main function.
/// Now called explicitly when processing from genesis.
#[instrument(skip(pool), level = "info")]
async fn build_empty_state_with_users(pool: &PgPool) -> Result<GlobalLoanState, String> {
    info!("🔨 Building empty GlobalLoanState with users and companies");

    // Query all users (for observability only in FromGenesis mode)
    let db_users = sqlx::query_as!(
        DbUser,
        r#"
        SELECT
            id, user_type, email, kyc_status,
            kyc_submitted_at, kyc_verified_at,
            kyc_investor_type as "kyc_investor_type: _",
            created_at as "created_at!", updated_at as "updated_at!",
            last_login_at, email_verified, email_verification_token,
            first_name, last_name, jurisdiction, wallet_address
        FROM users
        ORDER BY created_at
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query users: {}", e))?;

    info!("✅ Loaded {} users from database", db_users.len());

    // IMPORTANT (FromGenesis):
    // We MUST start from an empty user map and let UserRegistration transactions create users.
    // Otherwise we preload "current DB state" and then replay UserRegistration, causing
    // DuplicateUser inside the zkVM (confirmed by debug.ndjson overlap evidence).
    let users: HashMap<[u8; 32], User> = HashMap::new();

    // Keep a small sample of DB user hashes for debugging/observability (not inserted into state).
    let mut user_id_prefixes: Vec<String> = Vec::new();
    for db_user in &db_users {
        if user_id_prefixes.len() < 5 {
            let user_id_hash = hash_uuid(&db_user.id);
            user_id_prefixes.push(format!("0x{}", hex::encode(&user_id_hash[..8])));
        }
    }

    // For genesis mode, companies will be created via CompanyRegistration transactions
    // Don't pre-load them to avoid duplicate registration errors
    info!(
        "📋 Genesis mode: Companies will be created via CompanyRegistration transactions in batch"
    );
    let companies = HashMap::new();

    // Build empty balance state (balances will be created via transactions)
    let balance_state = BalanceState::new();

    // ============================================================================
    // MVP: RETURN EMPTY STATE (USERS ONLY)
    // ============================================================================
    //
    // For historical imports and current MVP, we process ALL transactions from genesis.
    // This means:
    // - Users: Loaded (needed for KYC validation)
    // - Deals: Empty (will be built by zkVM from transactions)
    // - Collateral: Empty (will be built by zkVM from transactions)
    // - last_updated: 0 (allows all transactions to be processed)
    //
    // This ensures:
    // 1. Transactions can be sorted by deal in chronological order
    // 2. Each deal's lifecycle is coherent (origin → payments → close)
    // 3. Collateral conflicts detected at correct historical moment
    // 4. Works for both historical portfolio imports AND real-time operations

    let state = GlobalLoanState {
        users,
        companies,
        loans: HashMap::new(),               // Empty - built from transactions
        collateral: HashMap::new(),          // Empty - built from transactions
        active_associations: HashMap::new(), // Empty - built from transactions
        association_history: HashMap::new(), // Empty - built from transactions
        balance_state,
        loan_terms: LoanTerms::default(),
        transaction_count: 0,
        last_updated: 0, // Allow all transactions to be processed
    };

    info!(
        "✅ GlobalLoanState built (MVP mode): {} users, {} companies, 0 deals, 0 collateral",
        state.users.len(),
        state.companies.len()
    );

    Ok(state)
}

/// Build `GlobalLoanState` from database (backward compatibility wrapper)
///
/// Defaults to FromGenesis mode for MVP. Use `build_global_loan_state_with_mode` for explicit control.
#[instrument(skip(pool), level = "info")]
pub async fn build_global_loan_state(pool: &PgPool) -> Result<GlobalLoanState, String> {
    build_global_loan_state_with_mode(pool, StateLoadMode::FromGenesis).await
}

/// Load state snapshot from latest proven batch
///
/// Reconstructs `GlobalLoanState` by replaying all transactions up to the latest
/// non-superseded proven batch. If no proven batches exist, returns empty state.
#[instrument(skip(pool), level = "info")]
async fn load_latest_state_snapshot(pool: &PgPool) -> Result<GlobalLoanState, String> {
    // Find latest proven, non-superseded batch
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
    .map_err(|e| format!("Failed to query latest proven batch: {}", e))?;

    match latest_batch {
        Some(batch) => {
            info!("📥 Loading state from latest proven batch: {}", batch.id);
            load_state_up_to_batch(pool, batch.id).await
        }
        None => {
            info!("📭 No proven batches found - starting from empty state");
            build_empty_state_with_users(pool).await
        }
    }
}

/// Reconstruct state by replaying all transactions up to specified batch
///
/// This replays all included transactions from genesis to rebuild the exact state
/// at the end of the specified batch.
#[instrument(skip(pool), level = "info")]
async fn load_state_up_to_batch(pool: &PgPool, batch_id: i32) -> Result<GlobalLoanState, String> {
    use arda_credit_lib::DealTransaction;
    use bincode;

    // Get all transactions up to and including this batch using StateTransaction
    let state_transactions = sqlx::query_as!(
        arda_credit_db::StateTransaction,
        r#"
        SELECT
            id,
            transaction_type,
            transaction_data,
            user_id,
            deal_id,
            originator_company_id,
            investor_company_id,
            collateral_id,
            included_in_batch_id,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM state_transactions
        WHERE included_in_batch_id IS NOT NULL
          AND included_in_batch_id <= $1
        ORDER BY included_in_batch_id ASC, created_at ASC
        "#,
        batch_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load transactions for batch replay: {}", e))?;

    info!(
        "🔄 Replaying {} transactions to reconstruct state at batch {}",
        state_transactions.len(),
        batch_id
    );

    // Deserialize transaction_data to get DealTransaction for each
    let mut deal_transactions = Vec::new();
    for state_tx in &state_transactions {
        match bincode::serde::decode_from_slice::<DealTransaction, _>(
            &state_tx.transaction_data,
            bincode::config::standard(),
        ) {
            Ok((deal_tx, _)) => deal_transactions.push(deal_tx),
            Err(e) => {
                warn!("Failed to deserialize transaction {}: {}", state_tx.id, e);
                // Continue with other transactions
            }
        }
    }

    // Start with empty state
    let state = build_empty_state_with_users(pool).await?;

    // Replay all transactions to rebuild state
    // NOTE: This is a simplified version - full implementation would call zkVM processor
    // For now, we'll just warn and return empty state
    warn!(
        "⚠️  State replay not yet fully implemented - {} transactions would be replayed",
        deal_transactions.len()
    );
    warn!("⚠️  For MVP, falling back to empty state. TODO: Implement full replay logic");

    Ok(state)
}

/// Convert database User to zkVM User
#[cfg(test)]
fn convert_db_user_to_zkvm(db_user: &DbUser) -> Result<User, String> {
    let user_id_hash = hash_uuid(&db_user.id);

    let user_type = match db_user.user_type.as_str() {
        "investor" => UserType::Investor,
        "originator" => UserType::Originator,
        "admin" => UserType::Admin,
        other => return Err(format!("Unknown user type: {}", other)),
    };

    let kyc_status = match db_user.kyc_status.as_str() {
        "not_submitted" => KycStatus::NotSubmitted,
        "pending" => KycStatus::Pending,
        "approved" => KycStatus::Approved,
        "rejected" => KycStatus::Rejected,
        other => return Err(format!("Unknown KYC status: {}", other)),
    };

    Ok(User {
        user_id: user_id_hash,
        user_type,
        kyc_status,
        kyc_submitted_at: db_user
            .kyc_submitted_at
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or(0),
        kyc_verified_at: db_user
            .kyc_verified_at
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or(0),
        created_at: db_user.created_at.timestamp() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_user_type_conversion() {
        let db_user = DbUser {
            id: uuid::Uuid::new_v4(),
            user_type: "investor".to_string(),
            email: "test@example.com".to_string(),
            kyc_status: "approved".to_string(),
            kyc_submitted_at: Some(Utc::now()),
            kyc_verified_at: Some(Utc::now()),
            kyc_investor_type: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
            email_verified: Some(true),
            email_verification_token: None,
            first_name: None,
            last_name: None,
            jurisdiction: None,
            wallet_address: None,
        };

        let result = convert_db_user_to_zkvm(&db_user);
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.user_type, UserType::Investor);
        assert_eq!(user.kyc_status, KycStatus::Approved);
    }
}
