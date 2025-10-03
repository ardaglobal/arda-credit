//! Consolidated balance, deposit, and withdrawal database functions
//!
//! This module provides clean, account-centric functions for managing balances,
//! deposits, and withdrawals with automatic trigger-based balance updates.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::{Account, Balance, Deposit, Withdrawal};
use crate::error::{DbError, DbResult};

// ============================================================================
// ACCOUNT FUNCTIONS
// ============================================================================

/// Get account by ID
pub async fn get_account_by_id(pool: &PgPool, account_id: Uuid) -> DbResult<Option<Account>> {
    let account = sqlx::query_as!(
        Account,
        r#"
        SELECT
            account_id, org_id, user_id, account_label, account_number,
            account_type, currency, status, opened_at, closed_at,
            created_at, updated_at
        FROM accounts
        WHERE account_id = $1
        "#,
        account_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(account)
}

/// Get account by Ethereum address (account_number)
pub async fn get_account_by_address(pool: &PgPool, address: &str) -> DbResult<Option<Account>> {
    let account = sqlx::query_as!(
        Account,
        r#"
        SELECT
            account_id, org_id, user_id, account_label, account_number,
            account_type, currency, status, opened_at, closed_at,
            created_at, updated_at
        FROM accounts
        WHERE account_number = $1
        "#,
        address
    )
    .fetch_optional(pool)
    .await?;

    Ok(account)
}

/// Get all accounts for a user
pub async fn get_accounts_by_user_id(pool: &PgPool, user_id: Uuid) -> DbResult<Vec<Account>> {
    let accounts = sqlx::query_as!(
        Account,
        r#"
        SELECT
            account_id, org_id, user_id, account_label, account_number,
            account_type, currency, status, opened_at, closed_at,
            created_at, updated_at
        FROM accounts
        WHERE user_id = $1
        ORDER BY created_at
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(accounts)
}

/// Get all accounts for an organization
pub async fn get_accounts_by_org_id(pool: &PgPool, org_id: Uuid) -> DbResult<Vec<Account>> {
    let accounts = sqlx::query_as!(
        Account,
        r#"
        SELECT
            account_id, org_id, user_id, account_label, account_number,
            account_type, currency, status, opened_at, closed_at,
            created_at, updated_at
        FROM accounts
        WHERE org_id = $1
        ORDER BY created_at
        "#,
        org_id
    )
    .fetch_all(pool)
    .await?;

    Ok(accounts)
}

// ============================================================================
// BALANCE FUNCTIONS
// ============================================================================

/// Get balance for a specific account and currency
pub async fn get_balance(
    pool: &PgPool,
    account_id: Uuid,
    currency: &str,
) -> DbResult<Option<Balance>> {
    let balance = sqlx::query_as!(
        Balance,
        r#"
        SELECT
            account_id, currency, available_balance, locked_balance,
            total_deposited, total_withdrawn, deposit_count, withdrawal_count,
            created_at, updated_at,
            first_deposit_at, last_deposit_at, first_withdrawal_at, last_withdrawal_at
        FROM balances
        WHERE account_id = $1 AND currency = $2
        "#,
        account_id,
        currency
    )
    .fetch_optional(pool)
    .await?;

    Ok(balance)
}

/// Get all balances for an account (all currencies)
pub async fn get_balances_for_account(pool: &PgPool, account_id: Uuid) -> DbResult<Vec<Balance>> {
    let balances = sqlx::query_as!(
        Balance,
        r#"
        SELECT
            account_id, currency, available_balance, locked_balance,
            total_deposited, total_withdrawn, deposit_count, withdrawal_count,
            created_at, updated_at,
            first_deposit_at, last_deposit_at, first_withdrawal_at, last_withdrawal_at
        FROM balances
        WHERE account_id = $1
        ORDER BY currency
        "#,
        account_id
    )
    .fetch_all(pool)
    .await?;

    Ok(balances)
}

/// Get all balances for a user (across all their accounts)
pub async fn get_balances_by_user_id(pool: &PgPool, user_id: Uuid) -> DbResult<Vec<Balance>> {
    let balances = sqlx::query_as!(
        Balance,
        r#"
        SELECT
            b.account_id, b.currency, b.available_balance, b.locked_balance,
            b.total_deposited, b.total_withdrawn, b.deposit_count, b.withdrawal_count,
            b.created_at, b.updated_at,
            b.first_deposit_at, b.last_deposit_at, b.first_withdrawal_at, b.last_withdrawal_at
        FROM balances b
        JOIN accounts a ON b.account_id = a.account_id
        WHERE a.user_id = $1
        ORDER BY b.currency, a.created_at
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(balances)
}

/// Get all balances for an organization
pub async fn get_balances_by_org_id(pool: &PgPool, org_id: Uuid) -> DbResult<Vec<Balance>> {
    let balances = sqlx::query_as!(
        Balance,
        r#"
        SELECT
            b.account_id, b.currency, b.available_balance, b.locked_balance,
            b.total_deposited, b.total_withdrawn, b.deposit_count, b.withdrawal_count,
            b.created_at, b.updated_at,
            b.first_deposit_at, b.last_deposit_at, b.first_withdrawal_at, b.last_withdrawal_at
        FROM balances b
        JOIN accounts a ON b.account_id = a.account_id
        WHERE a.org_id = $1
        ORDER BY b.currency, a.created_at
        "#,
        org_id
    )
    .fetch_all(pool)
    .await?;

    Ok(balances)
}

// ============================================================================
// DEPOSIT FUNCTIONS
// ============================================================================

/// Create a new deposit record from smart contract event
/// Looks up account_id by address, creates account if needed
pub async fn create_deposit(
    pool: &PgPool,
    address: &str,
    amount: Decimal,
    currency: &str,
    transaction_hash: &str,
    event_transaction_hash: &str,
    block_number: i64,
    event_timestamp: i64,
    log_index: i64,
    event_signature: &str,
) -> DbResult<Deposit> {
    // Look up account by address
    let account = get_account_by_address(pool, address).await?;

    let account_id = if let Some(acc) = account {
        acc.account_id
    } else {
        // Account doesn't exist - create orphan account for this address
        // This handles cases where deposit arrives before user links wallet
        let new_account = sqlx::query!(
            r#"
            INSERT INTO accounts (account_label, account_number, account_type, currency, status)
            VALUES ($1, $2, 'OPERATING', $3, 'ACTIVE')
            RETURNING account_id
            "#,
            format!("Unlinked Account ({}...)", &address[..10]),
            address,
            currency
        )
        .fetch_one(pool)
        .await?;

        // Create balance record for new account
        sqlx::query!(
            r#"
            INSERT INTO balances (account_id, currency)
            VALUES ($1, $2)
            "#,
            new_account.account_id,
            currency
        )
        .execute(pool)
        .await?;

        new_account.account_id
    };

    let deposit = sqlx::query_as!(
        Deposit,
        r#"
        INSERT INTO deposits (
            account_id, address, amount, currency, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, log_index, event_signature
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (transaction_hash, log_index)
        DO UPDATE SET updated_at = NOW()
        RETURNING
            id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, block_timestamp, log_index, event_signature,
            status, confirmation_count, required_confirmations, error_message,
            processing_details, created_at, updated_at, processed_at
        "#,
        account_id,
        address,
        amount,
        currency,
        transaction_hash,
        event_transaction_hash,
        block_number,
        event_timestamp,
        log_index,
        event_signature
    )
    .fetch_one(pool)
    .await?;

    Ok(deposit)
}

/// Update deposit status and confirmation count
pub async fn update_deposit_status(
    pool: &PgPool,
    deposit_id: i32,
    status: &str,
    confirmation_count: i32,
    block_timestamp: Option<DateTime<Utc>>,
) -> DbResult<Deposit> {
    let should_set_processed = status == "processed" || status == "confirmed";

    let deposit = sqlx::query_as!(
        Deposit,
        r#"
        UPDATE deposits
        SET status = $2, confirmation_count = $3, block_timestamp = $4,
            processed_at = CASE WHEN $5 THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, block_timestamp, log_index, event_signature,
            status, confirmation_count, required_confirmations, error_message,
            processing_details, created_at, updated_at, processed_at
        "#,
        deposit_id,
        status,
        confirmation_count,
        block_timestamp,
        should_set_processed
    )
    .fetch_one(pool)
    .await?;

    Ok(deposit)
}

/// Get all deposits for an account
pub async fn get_deposits_by_account_id(
    pool: &PgPool,
    account_id: Uuid,
    currency: Option<&str>,
) -> DbResult<Vec<Deposit>> {
    let deposits = if let Some(curr) = currency {
        sqlx::query_as!(
            Deposit,
            r#"
            SELECT
                id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
                block_number, event_timestamp, block_timestamp, log_index, event_signature,
                status, confirmation_count, required_confirmations, error_message,
                processing_details, created_at, updated_at, processed_at
            FROM deposits
            WHERE account_id = $1 AND currency = $2
            ORDER BY created_at DESC
            "#,
            account_id,
            curr
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            Deposit,
            r#"
            SELECT
                id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
                block_number, event_timestamp, block_timestamp, log_index, event_signature,
                status, confirmation_count, required_confirmations, error_message,
                processing_details, created_at, updated_at, processed_at
            FROM deposits
            WHERE account_id = $1
            ORDER BY created_at DESC
            "#,
            account_id
        )
        .fetch_all(pool)
        .await?
    };

    Ok(deposits)
}

/// Get deposit by transaction hash
pub async fn get_deposit_by_tx_hash(pool: &PgPool, tx_hash: &str) -> DbResult<Option<Deposit>> {
    let deposit = sqlx::query_as!(
        Deposit,
        r#"
        SELECT
            id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, block_timestamp, log_index, event_signature,
            status, confirmation_count, required_confirmations, error_message,
            processing_details, created_at, updated_at, processed_at
        FROM deposits
        WHERE transaction_hash = $1 OR event_transaction_hash = $1
        "#,
        tx_hash
    )
    .fetch_optional(pool)
    .await?;

    Ok(deposit)
}

/// Get pending deposits that need confirmation checking
pub async fn get_pending_deposits(pool: &PgPool, limit: Option<i32>) -> DbResult<Vec<Deposit>> {
    let deposits = sqlx::query_as!(
        Deposit,
        r#"
        SELECT
            id, account_id, address, amount, currency, transaction_hash, event_transaction_hash,
            block_number, event_timestamp, block_timestamp, log_index, event_signature,
            status, confirmation_count, required_confirmations, error_message,
            processing_details, created_at, updated_at, processed_at
        FROM deposits
        WHERE status = 'pending'
        ORDER BY block_number ASC
        LIMIT $1
        "#,
        limit.unwrap_or(100) as i64
    )
    .fetch_all(pool)
    .await?;

    Ok(deposits)
}

// ============================================================================
// WITHDRAWAL FUNCTIONS
// ============================================================================

/// Create a new withdrawal request (user-initiated)
/// Looks up account by address
pub async fn create_withdrawal_request(
    pool: &PgPool,
    address: &str,
    recipient_address: &str,
    amount: Decimal,
    currency: &str,
    requested_by_user_id: Uuid,
    kyc_status_at_request: &str,
    available_balance_at_request: Decimal,
) -> DbResult<Withdrawal> {
    // Look up account by address
    let account = get_account_by_address(pool, address)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("No account found for address {}", address)))?;

    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        INSERT INTO withdrawals (
            account_id, address, recipient_address, amount, currency, requested_by_user_id,
            request_source, state, kyc_status_at_request, available_balance_at_request
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'user', 'pending', $7, $8)
        RETURNING
            id, account_id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        account.account_id,
        address,
        recipient_address,
        amount,
        currency,
        requested_by_user_id,
        kyc_status_at_request,
        available_balance_at_request
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Create withdrawal from smart contract event (direct contract interaction)
/// Looks up account by address
pub async fn create_withdrawal_from_event(
    pool: &PgPool,
    address: &str,
    recipient_address: &str,
    amount: Decimal,
    currency: &str,
    transaction_hash: &str,
    event_transaction_hash: &str,
    block_number: i64,
    event_timestamp: i64,
    log_index: i64,
    event_signature: &str,
) -> DbResult<Withdrawal> {
    // Look up account by address
    let account = get_account_by_address(pool, address)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("No account found for address {}", address)))?;

    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        INSERT INTO withdrawals (
            account_id, address, recipient_address, amount, currency, request_source,
            state, transaction_hash, event_transaction_hash, block_number,
            event_timestamp, log_index, event_signature
        )
        VALUES ($1, $2, $3, $4, $5, 'contract', 'submitted', $6, $7, $8, $9, $10, $11)
        ON CONFLICT DO NOTHING
        RETURNING
            id, account_id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        account.account_id,
        address,
        recipient_address,
        amount,
        currency,
        transaction_hash,
        event_transaction_hash,
        block_number,
        event_timestamp,
        log_index,
        event_signature
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Get withdrawal by ID
pub async fn get_withdrawal_by_id(pool: &PgPool, withdrawal_id: i32) -> DbResult<Withdrawal> {
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        SELECT
            id, account_id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        FROM withdrawals
        WHERE id = $1
        "#,
        withdrawal_id
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Get all withdrawals for an account
pub async fn get_withdrawals_by_account_id(
    pool: &PgPool,
    account_id: Uuid,
    currency: Option<&str>,
) -> DbResult<Vec<Withdrawal>> {
    let withdrawals = if let Some(curr) = currency {
        sqlx::query_as!(
            Withdrawal,
            r#"
            SELECT
                id, account_id, address, recipient_address, amount, currency,
                requested_by_user_id, request_source, state,
                kyc_validated, balance_validated, kyc_status_at_request,
                available_balance_at_request, validation_errors,
                transaction_hash, event_transaction_hash, block_number,
                event_timestamp, block_timestamp, log_index, event_signature,
                confirmation_count, required_confirmations, error_message,
                admin_notes, rejection_reason, processing_history,
                created_at, updated_at, validated_at, approved_at,
                submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
            FROM withdrawals
            WHERE account_id = $1 AND currency = $2
            ORDER BY created_at DESC
            "#,
            account_id,
            curr
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            Withdrawal,
            r#"
            SELECT
                id, account_id, address, recipient_address, amount, currency,
                requested_by_user_id, request_source, state,
                kyc_validated, balance_validated, kyc_status_at_request,
                available_balance_at_request, validation_errors,
                transaction_hash, event_transaction_hash, block_number,
                event_timestamp, block_timestamp, log_index, event_signature,
                confirmation_count, required_confirmations, error_message,
                admin_notes, rejection_reason, processing_history,
                created_at, updated_at, validated_at, approved_at,
                submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
            FROM withdrawals
            WHERE account_id = $1
            ORDER BY created_at DESC
            "#,
            account_id
        )
        .fetch_all(pool)
        .await?
    };

    Ok(withdrawals)
}

/// Update withdrawal state
pub async fn update_withdrawal_state(
    pool: &PgPool,
    withdrawal_id: i32,
    new_state: &str,
    error_message: Option<&str>,
) -> DbResult<Withdrawal> {
    let now = Utc::now();

    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            state = $2,
            error_message = $3,
            updated_at = NOW(),
            failed_at = CASE WHEN $2 = 'failed' THEN $4 ELSE failed_at END,
            cancelled_at = CASE WHEN $2 = 'cancelled' THEN $4 ELSE cancelled_at END
        WHERE id = $1
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        new_state,
        error_message,
        now
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Update withdrawal validation results
pub async fn update_withdrawal_validation(
    pool: &PgPool,
    withdrawal_id: i32,
    kyc_validated: bool,
    balance_validated: bool,
    validation_errors: Option<serde_json::Value>,
) -> DbResult<Withdrawal> {
    let new_state = if kyc_validated && balance_validated {
        "validated"
    } else {
        "pending"
    };

    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            kyc_validated = $2,
            balance_validated = $3,
            validation_errors = $4,
            state = $5,
            validated_at = CASE WHEN $2 AND $3 THEN NOW() ELSE validated_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        kyc_validated,
        balance_validated,
        validation_errors,
        new_state
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Approve withdrawal (admin action)
pub async fn approve_withdrawal(
    pool: &PgPool,
    withdrawal_id: i32,
    admin_notes: Option<&str>,
) -> DbResult<Withdrawal> {
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            state = 'approved',
            admin_notes = $2,
            approved_at = NOW(),
            updated_at = NOW()
        WHERE id = $1 AND state = 'validated'
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        admin_notes
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Reject withdrawal (admin action)
pub async fn reject_withdrawal(
    pool: &PgPool,
    withdrawal_id: i32,
    rejection_reason: &str,
    admin_notes: Option<&str>,
) -> DbResult<Withdrawal> {
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            state = 'failed',
            rejection_reason = $2,
            admin_notes = $3,
            failed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1 AND state IN ('pending', 'validated')
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        rejection_reason,
        admin_notes
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Mark withdrawal as submitted to blockchain
pub async fn mark_withdrawal_submitted(
    pool: &PgPool,
    withdrawal_id: i32,
    transaction_hash: &str,
    block_number: Option<i64>,
) -> DbResult<Withdrawal> {
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            state = 'submitted',
            transaction_hash = $2,
            block_number = $3,
            submitted_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        transaction_hash,
        block_number
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Update withdrawal confirmation count
pub async fn update_withdrawal_confirmations(
    pool: &PgPool,
    withdrawal_id: i32,
    confirmation_count: i32,
) -> DbResult<Withdrawal> {
    // If confirmations >= required, mark as confirmed
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            confirmation_count = $2,
            state = CASE
                WHEN $2 >= required_confirmations AND state = 'submitted' THEN 'confirmed'
                ELSE state
            END,
            confirmed_at = CASE
                WHEN $2 >= required_confirmations AND confirmed_at IS NULL THEN NOW()
                ELSE confirmed_at
            END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id,
        confirmation_count
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Get pending withdrawals that need processing
pub async fn get_pending_withdrawals(
    pool: &PgPool,
    state_filter: Option<&str>,
    limit: Option<i32>,
) -> DbResult<Vec<Withdrawal>> {
    let withdrawals = if let Some(state) = state_filter {
        sqlx::query_as!(
            Withdrawal,
            r#"
            SELECT
                id, address, recipient_address, amount, currency,
                requested_by_user_id, request_source, state,
                kyc_validated, balance_validated, kyc_status_at_request,
                available_balance_at_request, validation_errors,
                transaction_hash, event_transaction_hash, block_number,
                event_timestamp, block_timestamp, log_index, event_signature,
                confirmation_count, required_confirmations, error_message,
                admin_notes, rejection_reason, processing_history,
                created_at, updated_at, validated_at, approved_at,
                submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
            FROM withdrawals
            WHERE state = $1
            ORDER BY created_at ASC
            LIMIT $2
            "#,
            state,
            limit.unwrap_or(100) as i64
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            Withdrawal,
            r#"
            SELECT
                id, address, recipient_address, amount, currency,
                requested_by_user_id, request_source, state,
                kyc_validated, balance_validated, kyc_status_at_request,
                available_balance_at_request, validation_errors,
                transaction_hash, event_transaction_hash, block_number,
                event_timestamp, block_timestamp, log_index, event_signature,
                confirmation_count, required_confirmations, error_message,
                admin_notes, rejection_reason, processing_history,
                created_at, updated_at, validated_at, approved_at,
                submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
            FROM withdrawals
            WHERE state IN ('pending', 'validated', 'approved', 'submitted')
            ORDER BY created_at ASC
            LIMIT $1
            "#,
            limit.unwrap_or(100) as i64
        )
        .fetch_all(pool)
        .await?
    };

    Ok(withdrawals)
}

/// Cancel a withdrawal (only if not yet submitted)
pub async fn cancel_withdrawal(pool: &PgPool, withdrawal_id: i32) -> DbResult<Withdrawal> {
    let withdrawal = sqlx::query_as!(
        Withdrawal,
        r#"
        UPDATE withdrawals
        SET
            state = 'cancelled',
            cancelled_at = NOW(),
            updated_at = NOW()
        WHERE id = $1 AND state IN ('pending', 'validated')
        RETURNING
            id, address, recipient_address, amount, currency,
            requested_by_user_id, request_source, state,
            kyc_validated, balance_validated, kyc_status_at_request,
            available_balance_at_request, validation_errors,
            transaction_hash, event_transaction_hash, block_number,
            event_timestamp, block_timestamp, log_index, event_signature,
            confirmation_count, required_confirmations, error_message,
            admin_notes, rejection_reason, processing_history,
            created_at, updated_at, validated_at, approved_at,
            submitted_at, confirmed_at, processed_at, failed_at, cancelled_at
        "#,
        withdrawal_id
    )
    .fetch_one(pool)
    .await?;

    Ok(withdrawal)
}

/// Add entry to withdrawal processing history
pub async fn add_withdrawal_history_entry(
    pool: &PgPool,
    withdrawal_id: i32,
    event_type: &str,
    old_state: &str,
    new_state: &str,
    success: bool,
    message: Option<&str>,
    details: Option<&serde_json::Value>,
) -> DbResult<()> {
    sqlx::query!(
        r#"
        UPDATE withdrawals
        SET processing_history = COALESCE(processing_history, '[]'::jsonb) || jsonb_build_object(
            'event_type', $2::text,
            'old_state', $3::text,
            'new_state', $4::text,
            'success', $5::boolean,
            'message', $6::text,
            'details', $7::jsonb,
            'timestamp', NOW()
        )::jsonb
        WHERE id = $1
        "#,
        withdrawal_id,
        event_type,
        old_state,
        new_state,
        success,
        message,
        details
    )
    .execute(pool)
    .await?;

    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get available balance for an account (shorthand function)
pub async fn get_available_balance(
    pool: &PgPool,
    account_id: Uuid,
    currency: &str,
) -> DbResult<Decimal> {
    let result = sqlx::query!(
        r#"
        SELECT available_balance
        FROM balances
        WHERE account_id = $1 AND currency = $2
        "#,
        account_id,
        currency
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.available_balance).unwrap_or(Decimal::ZERO))
}

/// Get total balance (available + locked) for an account
pub async fn get_total_balance(
    pool: &PgPool,
    account_id: Uuid,
    currency: &str,
) -> DbResult<Decimal> {
    let result = sqlx::query!(
        r#"
        SELECT available_balance + locked_balance as total
        FROM balances
        WHERE account_id = $1 AND currency = $2
        "#,
        account_id,
        currency
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.total).unwrap_or(Decimal::ZERO))
}

/// Check if account can perform operation (validates account status)
pub async fn validate_account_active(
    pool: &PgPool,
    account_id: Uuid,
    operation: &str,
) -> DbResult<()> {
    let account = sqlx::query!(
        r#"
        SELECT status
        FROM accounts
        WHERE account_id = $1
        "#,
        account_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("Account {} not found", account_id)))?;

    match account.status.as_str() {
        "ACTIVE" => Ok(()),
        "SUSPENDED" => Err(DbError::InvalidState(format!(
            "Account {} is SUSPENDED and cannot perform {} operation",
            account_id, operation
        ))),
        "INACTIVE" => Err(DbError::InvalidState(format!(
            "Account {} is INACTIVE and cannot perform {} operation",
            account_id, operation
        ))),
        _ => Err(DbError::InvalidState(format!(
            "Account {} has invalid status: {}",
            account_id, account.status
        ))),
    }
}
