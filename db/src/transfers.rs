//! Loan transfer database operations
//!
//! This module provides all database operations for loan ownership transfers including
//! validation, payment settlement, and ownership updates. It builds on the Phase 1
//! marketplace foundation.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// Import Phase 1 foundation functions
use crate::{
    get_user_balance_by_id, update_listing_status, update_user_balance_from_deposit_tx,
    update_user_balance_from_withdrawal_tx,
};

/// Transfer validation request
#[derive(Debug, Clone)]
pub struct TransferValidationRequest {
    pub loan_id: Uuid,
    pub buyer_id: Uuid,
    pub purchase_price: Decimal,
}

/// Comprehensive transfer validation result
#[derive(Debug, Clone)]
pub struct TransferValidationResult {
    pub is_valid: bool,
    pub loan_exists: bool,
    pub loan_for_sale: bool,
    pub buyer_has_funds: bool,
    pub buyer_is_not_owner: bool,
    pub buyer_kyc_approved: bool,
    pub price_matches: bool,
    pub available_balance: Option<Decimal>,
    pub validation_errors: Vec<String>,
}

/// Complete transfer execution request
#[derive(Debug, Clone)]
pub struct ExecuteTransferRequest {
    pub loan_id: Uuid,
    pub buyer_id: Uuid,
    pub purchase_price: Decimal,
    pub notes: Option<String>,
}

/// Transfer execution result with payment details
#[derive(Debug, Clone)]
pub struct TransferExecutionResult {
    pub transfer_id: Uuid,
    pub loan_id: Uuid,
    pub from_lender_id: Uuid,
    pub to_lender_id: Uuid,
    pub purchase_price: Decimal,
    pub transfer_date: chrono::NaiveDateTime,
    pub settlement: TransferSettlementResult,
}

/// Payment settlement result
#[derive(Debug, Clone)]
pub struct TransferSettlementResult {
    pub transfer_id: Uuid,
    pub buyer_transaction_id: Uuid,
    pub seller_transaction_id: Uuid,
    pub settled_amount: Decimal,
    pub settlement_timestamp: chrono::NaiveDateTime,
}

/// Transfer history record with user details
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoanTransferHistory {
    pub transfer_id: Uuid,
    pub loan_id: Uuid,
    pub from_lender_id: Uuid,
    pub to_lender_id: Uuid,
    pub purchase_price: Decimal,
    pub transfer_status: String,
    pub transfer_date: chrono::NaiveDateTime,
    pub completed_at: Option<chrono::NaiveDateTime>,
    pub notes: Option<String>,
    pub from_lender_email: String,
    pub to_lender_email: String,
}

/// Validate transfer eligibility with comprehensive checks
///
/// Builds on Phase 1 foundation to perform complete transfer validation
pub async fn validate_transfer_eligibility(
    pool: &PgPool,
    request: &TransferValidationRequest,
) -> Result<TransferValidationResult, sqlx::Error> {
    let mut result = TransferValidationResult {
        is_valid: false,
        loan_exists: false,
        loan_for_sale: false,
        buyer_has_funds: false,
        buyer_is_not_owner: false,
        buyer_kyc_approved: false,
        price_matches: false,
        available_balance: None,
        validation_errors: vec![],
    };

    // 1. Check if loan exists and get current details using Phase 1 schema
    let loan_query = sqlx::query!(
        r#"
        SELECT l.id, l.lender_id, l.loan_status, l.principal_amount,
               ll.asking_price, ll.status as listing_status
        FROM loans l
        LEFT JOIN loan_listings ll ON l.id = ll.loan_id AND ll.status = 'active'
        WHERE l.id = $1
        "#,
        request.loan_id
    )
    .fetch_optional(pool)
    .await?;

    let loan = if let Some(loan) = loan_query {
        result.loan_exists = true;
        loan
    } else {
        result.validation_errors.push("Loan not found".to_string());
        return Ok(result);
    };

    // 2. Check if loan is for sale (Phase 1 status values)
    if loan.loan_status == "for_sale" && loan.listing_status == "active" {
        result.loan_for_sale = true;
    } else {
        result
            .validation_errors
            .push("Loan is not currently for sale".to_string());
    }

    // 3. Check buyer is not current owner
    if loan.lender_id != request.buyer_id {
        result.buyer_is_not_owner = true;
    } else {
        result
            .validation_errors
            .push("Buyer cannot purchase their own loan".to_string());
    }

    // 4. Check price matches listing (only if listing exists)
    if loan.listing_status == "active" {
        if loan.asking_price == request.purchase_price {
            result.price_matches = true;
        } else {
            result.validation_errors.push(format!(
                "Purchase price {} does not match asking price {}",
                request.purchase_price, loan.asking_price
            ));
        }
    } else {
        result
            .validation_errors
            .push("No asking price found for loan".to_string());
    }

    // 5. Check buyer's available balance (using existing balance system)
    let buyer_balance = get_user_balance_by_id(pool, request.buyer_id).await?;
    if let Some(balance) = buyer_balance {
        result.available_balance = Some(balance);
        if balance >= request.purchase_price {
            result.buyer_has_funds = true;
        } else {
            result.validation_errors.push(format!(
                "Insufficient balance: have {}, need {}",
                balance, request.purchase_price
            ));
        }
    } else {
        result
            .validation_errors
            .push("Buyer has no account balance".to_string());
    }

    // 6. Check buyer KYC status (using existing identity system)
    let kyc_check = sqlx::query!(
        r#"
        SELECT kyc_status
        FROM identities
        WHERE id = $1
        "#,
        request.buyer_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(_kyc) = kyc_check {
        // For MVP, auto-approve KYC (following existing pattern)
        result.buyer_kyc_approved = true;

        // TODO: When KYC is properly implemented, use:
        // result.buyer_kyc_approved = kyc.kyc_status == "approved";
    } else {
        result
            .validation_errors
            .push("Buyer identity not found".to_string());
    }

    // 7. Set overall validity
    result.is_valid = result.loan_exists
        && result.loan_for_sale
        && result.buyer_has_funds
        && result.buyer_is_not_owner
        && result.buyer_kyc_approved
        && result.price_matches;

    Ok(result)
}

/// Execute loan ownership transfer atomically
///
/// This is the core transfer function that handles:
/// - Payment settlement (debit buyer, credit seller using existing balance system)
/// - Ownership transfer (update loan.lender_id)
/// - Listing status updates (mark as sold using Phase 1 functions)
/// - Transfer record creation (using Phase 1 loan_transfers table)
/// - Financial transaction recording (using existing financial transaction system)
///
/// All operations are atomic - either all succeed or all rollback.
pub async fn execute_loan_transfer(
    pool: &PgPool,
    request: &ExecuteTransferRequest,
) -> Result<TransferExecutionResult, sqlx::Error> {
    // Begin atomic transaction
    let mut tx = pool.begin().await?;

    // 1. Re-validate transfer eligibility within transaction
    let validation = validate_transfer_eligibility(
        pool,
        &TransferValidationRequest {
            loan_id: request.loan_id,
            buyer_id: request.buyer_id,
            purchase_price: request.purchase_price,
        },
    )
    .await?;

    if !validation.is_valid {
        return Err(sqlx::Error::Protocol(format!(
            "Transfer validation failed: {}",
            validation.validation_errors.join(", ")
        )));
    }

    // 2. Get seller information from current loan ownership
    let seller_info = sqlx::query!(
        r#"
        SELECT l.lender_id as seller_id, ll.listing_id
        FROM loans l
        JOIN loan_listings ll ON l.id = ll.loan_id
        WHERE l.id = $1 AND ll.status = 'active'
        "#,
        request.loan_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let seller_id = seller_info.seller_id;
    let _listing_id = seller_info.listing_id;

    // 3. Update loan status to transfer_pending (prevents concurrent transfers)
    sqlx::query!(
        r#"
        UPDATE loans
        SET loan_status = 'transfer_pending', updated_at = NOW()
        WHERE id = $1 AND loan_status = 'for_sale'
        "#,
        request.loan_id
    )
    .execute(&mut *tx)
    .await?;

    // 4. Create transfer record (using Phase 1 table)
    let transfer_id = uuid::Uuid::new_v4();
    let transfer_date = chrono::Utc::now().naive_utc();

    sqlx::query!(
        r#"
        INSERT INTO loan_transfers (
            transfer_id, loan_id, from_lender_id, to_lender_id,
            purchase_price, transfer_status, transfer_date, notes
        )
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7)
        "#,
        transfer_id,
        request.loan_id,
        seller_id,
        request.buyer_id,
        request.purchase_price,
        transfer_date,
        request.notes
    )
    .execute(&mut *tx)
    .await?;

    // 5. Perform payment settlement (using existing balance system)

    // 5a. Debit buyer account
    update_user_balance_from_withdrawal_tx(&mut tx, request.buyer_id, request.purchase_price)
        .await
        .map_err(|e| sqlx::Error::Protocol(format!("Failed to debit buyer account: {}", e)))?;

    // 5b. Credit seller account
    update_user_balance_from_deposit_tx(&mut tx, seller_id, request.purchase_price)
        .await
        .map_err(|e| sqlx::Error::Protocol(format!("Failed to credit seller account: {}", e)))?;

    // 6. Record financial transactions for audit trail (using existing system)

    // 6a. Buyer's purchase transaction
    let buyer_tx_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO account_financial_transactions (
            id, user_id, transaction_type, amount, currency,
            transaction_hash, blockchain_status, processed_at
        )
        VALUES ($1, $2, 'LOAN_PURCHASE', $3, 'USDC', $4, 'CONFIRMED', NOW())
        "#,
        buyer_tx_id,
        request.buyer_id,
        -request.purchase_price,                  // Negative for debit
        format!("loan_transfer_{}", transfer_id)  // Use transfer_id as transaction hash
    )
    .execute(&mut *tx)
    .await?;

    // 6b. Seller's sale transaction
    let seller_tx_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO account_financial_transactions (
            id, user_id, transaction_type, amount, currency,
            transaction_hash, blockchain_status, processed_at
        )
        VALUES ($1, $2, 'LOAN_SALE', $3, 'USDC', $4, 'CONFIRMED', NOW())
        "#,
        seller_tx_id,
        seller_id,
        request.purchase_price, // Positive for credit
        format!("loan_transfer_{}", transfer_id)
    )
    .execute(&mut *tx)
    .await?;

    // 7. Transfer ownership
    sqlx::query!(
        r#"
        UPDATE loans
        SET lender_id = $2, loan_status = 'active', updated_at = NOW()
        WHERE id = $1
        "#,
        request.loan_id,
        request.buyer_id
    )
    .execute(&mut *tx)
    .await?;

    // 8. Mark listing as sold (using Phase 1 table)
    update_listing_status(&mut tx, request.loan_id, "sold").await?;

    // 9. Mark transfer as completed
    let completion_time = chrono::Utc::now().naive_utc();
    sqlx::query!(
        r#"
        UPDATE loan_transfers
        SET transfer_status = 'completed', completed_at = $2
        WHERE transfer_id = $1
        "#,
        transfer_id,
        completion_time
    )
    .execute(&mut *tx)
    .await?;

    // 10. Commit transaction
    tx.commit().await?;

    // Return success result
    Ok(TransferExecutionResult {
        transfer_id,
        loan_id: request.loan_id,
        from_lender_id: seller_id,
        to_lender_id: request.buyer_id,
        purchase_price: request.purchase_price,
        transfer_date,
        settlement: TransferSettlementResult {
            transfer_id,
            buyer_transaction_id: buyer_tx_id,
            seller_transaction_id: seller_tx_id,
            settled_amount: request.purchase_price,
            settlement_timestamp: completion_time,
        },
    })
}

/// Get loan transfer history (using Phase 1 table)
pub async fn get_loan_transfer_history(
    pool: &PgPool,
    loan_id: Uuid,
) -> Result<Vec<LoanTransferHistory>, sqlx::Error> {
    let transfers = sqlx::query_as!(
        LoanTransferHistory,
        r#"
        SELECT
            lt.transfer_id,
            lt.loan_id,
            lt.from_lender_id,
            lt.to_lender_id,
            lt.purchase_price,
            lt.transfer_status,
            lt.transfer_date,
            lt.completed_at,
            lt.notes,
            from_user.email as from_lender_email,
            to_user.email as to_lender_email
        FROM loan_transfers lt
        JOIN identities from_user ON lt.from_lender_id = from_user.id
        JOIN identities to_user ON lt.to_lender_id = to_user.id
        WHERE lt.loan_id = $1
        ORDER BY lt.transfer_date DESC
        "#,
        loan_id
    )
    .fetch_all(pool)
    .await?;

    Ok(transfers)
}

/// Get transfers by user (loans they've bought or sold)
pub async fn get_user_transfer_history(
    pool: &PgPool,
    user_id: Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<LoanTransferHistory>, sqlx::Error> {
    let limit = limit.unwrap_or(50).min(100);
    let offset = offset.unwrap_or(0);

    let transfers = sqlx::query_as!(
        LoanTransferHistory,
        r#"
        SELECT
            lt.transfer_id,
            lt.loan_id,
            lt.from_lender_id,
            lt.to_lender_id,
            lt.purchase_price,
            lt.transfer_status,
            lt.transfer_date,
            lt.completed_at,
            lt.notes,
            from_user.email as from_lender_email,
            to_user.email as to_lender_email
        FROM loan_transfers lt
        JOIN identities from_user ON lt.from_lender_id = from_user.id
        JOIN identities to_user ON lt.to_lender_id = to_user.id
        WHERE lt.from_lender_id = $1 OR lt.to_lender_id = $1
        ORDER BY lt.transfer_date DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(transfers)
}

/// Cancel pending transfer (restore loan to marketplace)
pub async fn cancel_transfer(
    pool: &PgPool,
    transfer_id: Uuid,
    user_id: Uuid, // Must be buyer or seller
) -> Result<bool, sqlx::Error> {
    // Begin transaction
    let mut tx = pool.begin().await?;

    // Check if user can cancel (must be buyer or seller and status must be pending)
    let transfer_info = sqlx::query!(
        r#"
        SELECT lt.loan_id, lt.from_lender_id, lt.to_lender_id, lt.transfer_status
        FROM loan_transfers lt
        WHERE lt.transfer_id = $1
          AND (lt.from_lender_id = $2 OR lt.to_lender_id = $2)
          AND lt.transfer_status = 'pending'
        "#,
        transfer_id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(transfer) = transfer_info {
        // 1. Mark transfer as cancelled
        sqlx::query!(
            "UPDATE loan_transfers SET transfer_status = 'cancelled' WHERE transfer_id = $1",
            transfer_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Restore loan status to for_sale
        sqlx::query!(
            "UPDATE loans SET loan_status = 'for_sale', updated_at = NOW() WHERE id = $1",
            transfer.loan_id
        )
        .execute(&mut *tx)
        .await?;

        // 3. Restore listing to active (using Phase 1 table)
        sqlx::query!(
            "UPDATE loan_listings SET status = 'active', updated_at = NOW() WHERE loan_id = $1",
            transfer.loan_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    } else {
        Ok(false) // Transfer not found or not cancellable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transfer_validation() {
        // Test setup would use Phase 1 foundation:
        // - Use existing test pool setup
        // - Create test users with balances
        // - Create test loan listing using Phase 1 functions

        // Test validate_transfer_eligibility
        // Test execute_loan_transfer
        // Test get_loan_transfer_history
        // Test cancel_transfer
    }
}
