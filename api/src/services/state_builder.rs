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
//! ## State Caching
//!
//! Initial implementation rebuilds state from database per batch (simple, correct).
//! Future optimization: cache state in memory if size is manageable (< 100MB).

use arda_credit_db::{Collateral as DbCollateral, Deal as DbDeal, User as DbUser};
use arda_credit_lib::{
    hash_uuid, BalanceState, Collateral, CollateralAssociation, Deal, GlobalLoanState, KycStatus,
    LoanStatus, LoanTerms, User, UserBalance, UserType,
};
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

/// Build complete `GlobalLoanState` from database
///
/// Queries all users, deals, and collateral from the database and converts them
/// to zkVM-compatible format. This state is then used for proof generation.
///
/// # Errors
///
/// Returns error if:
/// - Database queries fail
/// - Data conversion fails
/// - Invalid data found in database
#[instrument(skip(pool), level = "info")]
pub async fn build_global_loan_state(pool: &PgPool) -> Result<GlobalLoanState, String> {
    info!("🔨 Building GlobalLoanState from database");

    // Query all users
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

    // Convert DB users to zkVM users
    let mut users = HashMap::new();
    for db_user in &db_users {
        let user = convert_db_user_to_zkvm(db_user)?;
        users.insert(user.user_id, user);
    }

    // Query all deals (borrower_legal_name is in DbDeal but not in deals table - will be NULL for now)
    let db_deals = sqlx::query_as!(
        DbDeal,
        r#"
        SELECT 
            id, originator_id, investor_id, borrower_id, 
            NULL::text as "borrower_legal_name?",
            principal_amount, outstanding_balance, interest_rate_bps,
            deal_status as "deal_status!", originated_at as "originated_at!", maturity_date,
            created_at as "created_at!", updated_at as "updated_at!", custom_deal_id, status as "status!", industry, document_ids,
            title, description, risk_level, asset_class, region, minimum_investment,
            payment_frequency, collateral_type, collateral_value, ltv_ratio,
            credit_score, dscr, state_transition_hash, proof_included as "proof_included!", collateral_id
        FROM deals
        ORDER BY originated_at
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query deals: {}", e))?;

    info!("✅ Loaded {} deals from database", db_deals.len());

    // Convert DB deals to zkVM deals
    let mut deals = HashMap::new();
    for db_deal in &db_deals {
        let deal = convert_db_deal_to_zkvm(db_deal)?;
        deals.insert(deal.deal_id, deal);
    }

    // Query all collateral
    let db_collateral = sqlx::query_as!(
        DbCollateral,
        r#"
        SELECT 
            id, name, collateral_type,
            description, address, custom_id, double_pledged,
            has_potential_duplicates,
            created_at as "created_at!", updated_at as "updated_at!", created_by, metadata,
            has_detection as "has_detection!", detection_count as "detection_count!", last_detection_at, detection_status
        FROM collateral
        ORDER BY created_at
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query collateral: {}", e))?;

    info!(
        "✅ Loaded {} collateral items from database",
        db_collateral.len()
    );

    // Convert DB collateral to zkVM collateral
    let mut collateral = HashMap::new();
    for db_coll in &db_collateral {
        let coll = convert_db_collateral_to_zkvm(db_coll)?;
        collateral.insert(coll.collateral_id.clone(), coll);
    }

    // Build active associations from deals.collateral_id
    // Note: The schema uses a single collateral_id per deal, not a many-to-many table
    let mut active_associations: HashMap<Vec<u8>, Vec<Vec<u8>>> = HashMap::new();
    let mut association_history = HashMap::new();

    // Build associations from deals with collateral_id
    for db_deal in &db_deals {
        if let Some(collateral_id) = db_deal.collateral_id {
            let coll_id_hash = hash_uuid(&collateral_id);
            let deal_id_hash = hash_uuid(&db_deal.id);

            // Add to active associations
            active_associations
                .entry(coll_id_hash.to_vec())
                .or_default()
                .push(deal_id_hash.to_vec());

            // Add to association history
            let zkvm_assoc = CollateralAssociation {
                collateral_id: coll_id_hash.to_vec(),
                deal_id: deal_id_hash.to_vec(),
                associated_at: db_deal.created_at.timestamp() as u64,
                disassociated_at: None, // No disassociation tracking in current schema
                is_active: true,
            };

            // Create compound key: collateral_id + deal_id
            let mut key = coll_id_hash.to_vec();
            key.extend_from_slice(&deal_id_hash);

            association_history.insert(key, zkvm_assoc);
        }
    }

    info!(
        "✅ Built {} active collateral associations from deals",
        active_associations.len()
    );

    // Build BalanceState from user cash balances
    let balance_state = build_balance_state(pool, &db_users).await?;

    let state = GlobalLoanState {
        users,
        loans: deals,
        collateral,
        active_associations,
        association_history,
        balance_state,
        loan_terms: LoanTerms::default(),
        transaction_count: 0, // Will be updated by zkVM
        last_updated: 0,      // Set to 0 to allow all transactions to be processed
    };

    info!(
        "✅ GlobalLoanState built: {} users, {} deals, {} collateral",
        state.users.len(),
        state.loans.len(),
        state.collateral.len()
    );

    // Log state size estimate
    let estimated_size_kb = estimate_state_size(&state);
    info!(
        "📊 Estimated state size: {:.2} KB ({:.2} MB)",
        estimated_size_kb,
        estimated_size_kb / 1024.0
    );

    Ok(state)
}

/// Convert database User to zkVM User
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

/// Convert database Deal to zkVM Deal
fn convert_db_deal_to_zkvm(db_deal: &DbDeal) -> Result<Deal, String> {
    let deal_id_hash = hash_uuid(&db_deal.id);
    let originator_id_hash = hash_uuid(&db_deal.originator_id);

    // Investor ID might be None for draft deals
    let investor_id_hash = db_deal
        .investor_id
        .as_ref()
        .map(hash_uuid)
        .unwrap_or([0u8; 32]); // Use zero array if no investor

    let loan_status = match db_deal.deal_status.as_str() {
        "originated" => LoanStatus::Originated,
        "active" => LoanStatus::Active,
        "paid_off" => LoanStatus::PaidOff,
        "closed" => LoanStatus::Closed,
        "defaulted" => LoanStatus::Defaulted,
        other => return Err(format!("Unknown deal status: {}", other)),
    };

    // Convert Decimal to cents (u64 for lib::Deal)
    let principal_cents = (db_deal.principal_amount * rust_decimal::Decimal::from(100))
        .to_u64()
        .ok_or_else(|| "Failed to convert principal_amount to cents".to_string())?;

    let outstanding_cents = (db_deal.outstanding_balance * rust_decimal::Decimal::from(100))
        .to_u64()
        .ok_or_else(|| "Failed to convert outstanding_balance to cents".to_string())?;

    Ok(Deal {
        deal_id: deal_id_hash,
        originator_id: originator_id_hash,
        investor_id: investor_id_hash,
        principal_amount: principal_cents,
        outstanding_balance: outstanding_cents,
        interest_rate: db_deal.interest_rate_bps as u32,
        term_months: 12, // Default term, could be computed from maturity_date if needed
        deal_status: loan_status,
        originated_at: db_deal.originated_at.timestamp() as u64,
        funded_at: db_deal.originated_at.timestamp() as u64, // Assume funded at origination
        maturity_date: db_deal
            .maturity_date
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or(0),
    })
}

/// Convert database Collateral to zkVM Collateral
fn convert_db_collateral_to_zkvm(db_coll: &DbCollateral) -> Result<Collateral, String> {
    let coll_id_hash = hash_uuid(&db_coll.id);

    Ok(Collateral {
        collateral_id: coll_id_hash.to_vec(),
        created_at: db_coll.created_at.timestamp() as u64,
        updated_at: db_coll.updated_at.timestamp() as u64,
        collateral_type: db_coll.collateral_type.clone().unwrap_or_default(),
    })
}

/// Build BalanceState from database user cash balances
async fn build_balance_state(pool: &PgPool, db_users: &[DbUser]) -> Result<BalanceState, String> {
    let mut balance_state = BalanceState::new();

    // For each user, query their balance transactions and compute current balance
    for db_user in db_users {
        let user_id_hash = hash_uuid(&db_user.id);

        // Query user's net balance from account_financial_transactions
        let balance_row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(
                CASE 
                    WHEN transaction_type IN ('deposit', 'credit_line_repayment') THEN amount
                    WHEN transaction_type IN ('withdrawal', 'credit_line_drawdown') THEN -amount
                    ELSE 0
                END
            ), 0) as "net_balance!"
            FROM account_financial_transactions
            WHERE user_id = $1
            "#,
            db_user.id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to query balance for user {}: {}", db_user.id, e))?;

        // Convert Decimal to cents (u128 for UserBalance)
        let balance_cents = (balance_row.net_balance * rust_decimal::Decimal::from(100))
            .to_u128()
            .ok_or_else(|| format!("Failed to convert balance to cents for user {}", db_user.id))?;

        // Create UserBalance for this user
        let user_balance = UserBalance {
            user_id: user_id_hash,
            balance: balance_cents,
            last_updated: Utc::now().timestamp() as u64,
            update_count: 0, // Could track this if needed
        };

        balance_state.balances.insert(user_id_hash, user_balance);
    }

    debug!(
        "Built BalanceState with {} user balances",
        balance_state.balances.len()
    );

    Ok(balance_state)
}

/// Estimate the memory size of GlobalLoanState in KB
fn estimate_state_size(state: &GlobalLoanState) -> f64 {
    // Rough estimates per structure:
    // User: ~100 bytes
    // Deal: ~200 bytes
    // Collateral: ~100 bytes
    // Association: ~100 bytes
    // UserBalance: ~100 bytes

    let users_kb = (state.users.len() * 100) as f64 / 1024.0;
    let deals_kb = (state.loans.len() * 200) as f64 / 1024.0;
    let collateral_kb = (state.collateral.len() * 100) as f64 / 1024.0;
    let associations_kb = (state.active_associations.len() * 100) as f64 / 1024.0;
    let balances_kb = (state.balance_state.balances.len() * 100) as f64 / 1024.0;

    users_kb + deals_kb + collateral_kb + associations_kb + balances_kb
}

#[cfg(test)]
mod tests {
    use super::*;

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
