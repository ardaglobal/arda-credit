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

use arda_credit_db::{
    business::company::Company as DbCompany, Collateral as DbCollateral, Deal as DbDeal,
    DealTransactionDb, User as DbUser,
};
use arda_credit_lib::{
    hash_uuid, BalanceState, Collateral, CollateralAssociation, Company, CompanyBalance,
    CompanyStatus, Deal, GlobalLoanState, KycStatus, LoanStatus, LoanTerms, User, UserBalance,
    UserType,
};
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

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

    // Query all companies
    let db_companies = sqlx::query_as!(
        DbCompany,
        r#"
        SELECT 
            id, name, description, 
            status as "status: arda_credit_db::business::company::CompanyStatus",
            metadata as "metadata!", 
            created_at as "created_at!", 
            updated_at as "updated_at!"
        FROM companies
        ORDER BY created_at
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query companies: {}", e))?;

    info!("✅ Loaded {} companies from database", db_companies.len());

    // Convert DB companies to zkVM companies
    let mut companies = HashMap::new();
    for db_company in &db_companies {
        let company = convert_db_company_to_zkvm(db_company)?;
        companies.insert(company.company_id, company);
    }

    // Build BalanceState from company balances
    let balance_state = build_balance_state_from_companies(pool, &db_companies).await?;

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

    return Ok(state);

    // ============================================================================
    // FUTURE: STATE SNAPSHOTS (COMMENTED OUT FOR NOW)
    // ============================================================================
    //
    // The code below will be used when we implement state snapshots for
    // incremental processing. For now, we always start from empty state.
    //
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
        companies: HashMap::new(), // Will be built from company registration transactions
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
        "✅ GlobalLoanState built: {} users, {} companies, {} deals, {} collateral",
        state.users.len(),
        state.companies.len(),
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
    // Get all transactions up to and including this batch
    let transactions = sqlx::query_as!(
        DealTransactionDb,
        r#"
        SELECT 
            id, transaction_id_hash, user_id_hash, deal_id_hash,
            transaction_type, amount, user_id, deal_id, payment_id,
            included_in_batch_id, metadata, created_at as "created_at!"
        FROM deal_transactions 
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
        transactions.len(),
        batch_id
    );

    // Start with empty state
    let mut state = build_empty_state_with_users(pool).await?;

    // Replay all transactions to rebuild state
    // NOTE: This is a simplified version - full implementation would call zkVM processor
    // For now, we'll just warn and return empty state
    warn!(
        "⚠️  State replay not yet fully implemented - {} transactions would be replayed",
        transactions.len()
    );
    warn!("⚠️  For MVP, falling back to empty state. TODO: Implement full replay logic");

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

/// Convert database Company to zkVM Company
fn convert_db_company_to_zkvm(db_company: &DbCompany) -> Result<Company, String> {
    let company_id_hash = hash_uuid(&db_company.id);

    let status = match db_company.status {
        arda_credit_db::business::company::CompanyStatus::Active => CompanyStatus::Active,
        arda_credit_db::business::company::CompanyStatus::Suspended => CompanyStatus::Suspended,
        arda_credit_db::business::company::CompanyStatus::Inactive => CompanyStatus::Inactive,
    };

    Ok(Company {
        company_id: company_id_hash,
        created_at: db_company.created_at.timestamp() as u64,
        status,
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

/// Build BalanceState from database company balances
async fn build_balance_state_from_companies(
    pool: &PgPool,
    db_companies: &[DbCompany],
) -> Result<BalanceState, String> {
    let mut balance_state = BalanceState::new();

    // For each company, query their balance from account_financial_transactions
    // Note: Companies get balances via deposits made by users with access to company wallets
    for db_company in db_companies {
        let company_id_hash = hash_uuid(&db_company.id);

        // Query company's net balance from account_financial_transactions
        // We join through company_memberships to link users to companies
        let balance_row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(
                CASE 
                    WHEN aft.transaction_type IN ('DEPOSIT', 'deposit', 'credit_line_repayment') THEN aft.amount
                    WHEN aft.transaction_type IN ('WITHDRAWAL', 'withdrawal', 'credit_line_drawdown') THEN -aft.amount
                    ELSE 0
                END
            ), 0) as "net_balance!"
            FROM account_financial_transactions aft
            INNER JOIN company_memberships cm ON aft.user_id = cm.user_id
            WHERE cm.company_id = $1 AND cm.status = 'active'
            "#,
            db_company.id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            format!(
                "Failed to query balance for company {}: {}",
                db_company.id, e
            )
        })?;

        // Convert Decimal to cents (u128 for CompanyBalance)
        let balance_cents = (balance_row.net_balance * rust_decimal::Decimal::from(100))
            .to_u128()
            .ok_or_else(|| {
                format!(
                    "Failed to convert balance to cents for company {}",
                    db_company.id
                )
            })?;

        // Create CompanyBalance for this company
        let company_balance = CompanyBalance {
            company_id: company_id_hash,
            balance: balance_cents,
            last_updated: Utc::now().timestamp() as u64,
            update_count: 0, // Could track this if needed
        };

        balance_state
            .balances
            .insert(company_id_hash, company_balance);
    }

    debug!(
        "Built BalanceState with {} company balances",
        balance_state.balances.len()
    );

    Ok(balance_state)
}

/// Legacy function for backwards compatibility
async fn build_balance_state(pool: &PgPool, db_users: &[DbUser]) -> Result<BalanceState, String> {
    // For legacy code that still calls this, return empty balance state
    // All balances should be tracked at company level now
    warn!("⚠️  build_balance_state(db_users) is deprecated - use build_balance_state_from_companies instead");
    Ok(BalanceState::new())
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
