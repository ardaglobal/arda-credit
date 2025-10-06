//! Deal transfer database operations
//!
//! This module handles deal ownership transfers between investors.
//! Converted from loan transfer system to deal transfer system.
//!
//! Core functionality:
//! - Deal ownership validation
//! - Transfer record creation and management
//! - Transfer history tracking
//! - Ownership change execution

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// Import Phase 1 foundation functions
use crate::{
    get_user_balance_by_id, update_deal_listing_status, update_user_balance_from_deposit_tx,
    update_user_balance_from_withdrawal_tx,
};

/// Transfer validation request
#[derive(Debug, Clone)]
pub struct TransferValidationRequest {
    pub deal_id: Uuid, // was loan_id, now deal_id
    pub buyer_id: Uuid,
    pub purchase_price: Decimal,
}

/// Comprehensive transfer validation result
#[derive(Debug, Clone)]
pub struct TransferValidationResult {
    pub is_valid: bool,
    pub deal_exists: bool,
    pub deal_for_sale: bool,
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
    pub deal_id: Uuid, // was loan_id, now deal_id
    pub buyer_id: Uuid,
    pub purchase_price: Decimal,
    pub notes: Option<String>,
}

/// Transfer execution result with payment details
#[derive(Debug, Clone)]
pub struct TransferExecutionResult {
    pub transfer_id: Uuid,
    pub deal_id: Uuid,          // was loan_id, now deal_id
    pub from_investor_id: Uuid, // was from_lender_id
    pub to_investor_id: Uuid,   // was to_lender_id
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
pub struct DealTransferHistory {
    pub transfer_id: Uuid,
    pub deal_id: Uuid,
    pub from_investor_id: Uuid,
    pub to_investor_id: Uuid,
    pub purchase_price: Decimal,
    pub transfer_status: String,
    pub transfer_date: chrono::NaiveDateTime,
    pub completed_at: Option<chrono::NaiveDateTime>,
    pub notes: Option<String>,
    pub from_investor_email: String,
    pub to_investor_email: String,
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
        deal_exists: false,
        deal_for_sale: false,
        buyer_has_funds: false,
        buyer_is_not_owner: false,
        buyer_kyc_approved: false,
        price_matches: false,
        available_balance: None,
        validation_errors: vec![],
    };

    // 1. Check if deal exists and get current details using Phase 1 schema
    let deal_query = sqlx::query!(
        r#"
        SELECT d.id, d.investor_id, d.deal_status, d.principal_amount,
               dl.asking_price, dl.status as listing_status
        FROM deals d
        LEFT JOIN deal_listings dl ON d.id = dl.deal_id AND dl.status = 'active'
        WHERE d.id = $1
        "#,
        request.deal_id
    )
    .fetch_optional(pool)
    .await?;

    let deal = if let Some(deal) = deal_query {
        result.deal_exists = true;
        deal
    } else {
        result.validation_errors.push("Deal not found".to_string());
        return Ok(result);
    };

    // 2. Check if deal is for sale (Phase 1 status values)
    if deal.deal_status == "for_sale" && deal.listing_status == "active" {
        result.deal_for_sale = true;
    } else {
        result
            .validation_errors
            .push("Deal is not currently for sale".to_string());
    }

    // 3. Check buyer is not current owner
    if deal.investor_id != Some(request.buyer_id) {
        result.buyer_is_not_owner = true;
    } else {
        result
            .validation_errors
            .push("Buyer cannot purchase their own deal".to_string());
    }

    // 4. Check price matches listing (only if listing exists)
    if deal.listing_status == "active" {
        if deal.asking_price == request.purchase_price {
            result.price_matches = true;
        } else {
            result.validation_errors.push(format!(
                "Purchase price {} does not match asking price {}",
                request.purchase_price, deal.asking_price
            ));
        }
    } else {
        result
            .validation_errors
            .push("No asking price found for deal".to_string());
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
        FROM users
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
    result.is_valid = result.deal_exists
        && result.deal_for_sale
        && result.buyer_has_funds
        && result.buyer_is_not_owner
        && result.buyer_kyc_approved
        && result.price_matches;

    Ok(result)
}

/// Execute deal ownership transfer atomically
///
/// This is the core transfer function that handles:
/// - Payment settlement (debit buyer, credit seller using existing balance system)
/// - Ownership transfer (update deal.investor_id)
/// - Listing status updates (mark as sold using Phase 1 functions)
/// - Transfer record creation (using Phase 1 deal_transfers table)
/// - Financial transaction recording (using existing financial transaction system)
///
/// All operations are atomic - either all succeed or all rollback.
pub async fn execute_deal_transfer(
    pool: &PgPool,
    request: &ExecuteTransferRequest,
) -> Result<TransferExecutionResult, sqlx::Error> {
    // Begin atomic transaction
    let mut tx = pool.begin().await?;

    // 1. Re-validate transfer eligibility within transaction
    let validation = validate_transfer_eligibility(
        pool,
        &TransferValidationRequest {
            deal_id: request.deal_id,
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

    // 2. Get seller information from current deal ownership
    let seller_info = sqlx::query!(
        r#"
        SELECT d.investor_id as seller_id, dl.listing_id
        FROM deals d
        JOIN deal_listings dl ON d.id = dl.deal_id
        WHERE d.id = $1 AND dl.status = 'active'
        "#,
        request.deal_id
    )
    .fetch_one(&mut *tx)
    .await?;

    // Ensure deal has an investor assigned (can't transfer draft deals)
    let seller_id = seller_info.seller_id.ok_or_else(|| {
        sqlx::Error::Protocol(
            "Cannot transfer deal without assigned investor (draft deals cannot be sold)"
                .to_string(),
        )
    })?;
    let _listing_id = seller_info.listing_id;

    // 3. Update deal marketplace status to transfer_pending (prevents concurrent transfers)
    sqlx::query!(
        r#"
        UPDATE deals
        SET status = 'transfer_pending', updated_at = NOW()
        WHERE id = $1 AND status = 'for_sale'
        "#,
        request.deal_id
    )
    .execute(&mut *tx)
    .await?;

    // 4. Create transfer record (using Phase 1 table)
    let transfer_id = uuid::Uuid::new_v4();
    let transfer_date = chrono::Utc::now().naive_utc();

    sqlx::query!(
        r#"
        INSERT INTO deal_transfers (
            transfer_id, deal_id, from_investor_id, to_investor_id,
            purchase_price, transfer_status, transfer_date, notes
        )
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7)
        "#,
        transfer_id,
        request.deal_id,
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
        VALUES ($1, $2, 'DEAL_PURCHASE', $3, 'USDC', $4, 'CONFIRMED', NOW())
        "#,
        buyer_tx_id,
        request.buyer_id,
        -request.purchase_price,                  // Negative for debit
        format!("deal_transfer_{}", transfer_id)  // Use transfer_id as transaction hash
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
        VALUES ($1, $2, 'DEAL_SALE', $3, 'USDC', $4, 'CONFIRMED', NOW())
        "#,
        seller_tx_id,
        seller_id,
        request.purchase_price, // Positive for credit
        format!("deal_transfer_{}", transfer_id)
    )
    .execute(&mut *tx)
    .await?;

    // 7. Transfer ownership
    sqlx::query!(
        r#"
        UPDATE deals
        SET investor_id = $2, deal_status = 'active', updated_at = NOW()
        WHERE id = $1
        "#,
        request.deal_id,
        request.buyer_id
    )
    .execute(&mut *tx)
    .await?;

    // 8. Mark listing as sold (using Phase 1 table)
    update_deal_listing_status(&mut tx, request.deal_id, "sold").await?; // was update_listing_status, request.loan_id

    // 9. Mark transfer as completed
    let completion_time = chrono::Utc::now().naive_utc();
    sqlx::query!(
        r#"
        UPDATE deal_transfers
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
        deal_id: request.deal_id,
        from_investor_id: seller_id,      // was from_lender_id
        to_investor_id: request.buyer_id, // was to_lender_id
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

/// Get deal transfer history (using Phase 1 table)
pub async fn get_deal_transfer_history(
    pool: &PgPool,
    deal_id: Uuid,
) -> Result<Vec<DealTransferHistory>, sqlx::Error> {
    let transfers = sqlx::query_as!(
        DealTransferHistory,
        r#"
        SELECT
            dt.transfer_id,
            dt.deal_id,
            dt.from_investor_id,
            dt.to_investor_id,
            dt.purchase_price,
            dt.transfer_status,
            dt.transfer_date,
            dt.completed_at,
            dt.notes,
            from_user.email as from_investor_email,
            to_user.email as to_investor_email
        FROM deal_transfers dt
        JOIN users from_user ON dt.from_investor_id = from_user.id
        JOIN users to_user ON dt.to_investor_id = to_user.id
        WHERE dt.deal_id = $1
        ORDER BY dt.transfer_date DESC
        "#,
        deal_id
    )
    .fetch_all(pool)
    .await?;

    Ok(transfers)
}

/// Get transfers by user (deals they've bought or sold)
pub async fn get_user_transfer_history(
    pool: &PgPool,
    user_id: Uuid,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<DealTransferHistory>, sqlx::Error> {
    let limit = limit.unwrap_or(50).min(100);
    let offset = offset.unwrap_or(0);

    let transfers = sqlx::query_as!(
        DealTransferHistory,
        r#"
        SELECT
            dt.transfer_id,
            dt.deal_id,
            dt.from_investor_id,
            dt.to_investor_id,
            dt.purchase_price,
            dt.transfer_status,
            dt.transfer_date,
            dt.completed_at,
            dt.notes,
            from_user.email as from_investor_email,
            to_user.email as to_investor_email
        FROM deal_transfers dt
        JOIN users from_user ON dt.from_investor_id = from_user.id
        JOIN users to_user ON dt.to_investor_id = to_user.id
        WHERE dt.from_investor_id = $1 OR dt.to_investor_id = $1
        ORDER BY dt.transfer_date DESC
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

/// Cancel pending transfer (restore deal to marketplace)
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
        SELECT dt.deal_id, dt.from_investor_id, dt.to_investor_id, dt.transfer_status
        FROM deal_transfers dt
        WHERE dt.transfer_id = $1
          AND (dt.from_investor_id = $2 OR dt.to_investor_id = $2)
          AND dt.transfer_status = 'pending'
        "#,
        transfer_id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(transfer) = transfer_info {
        // 1. Mark transfer as cancelled
        sqlx::query!(
            "UPDATE deal_transfers SET transfer_status = 'cancelled' WHERE transfer_id = $1",
            transfer_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Restore deal marketplace status to for_sale
        sqlx::query!(
            "UPDATE deals SET status = 'for_sale', updated_at = NOW() WHERE id = $1",
            transfer.deal_id
        )
        .execute(&mut *tx)
        .await?;

        // 3. Restore listing to active (using Phase 1 table)
        sqlx::query!(
            "UPDATE deal_listings SET status = 'active', updated_at = NOW() WHERE deal_id = $1",
            transfer.deal_id
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
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn test_transfer_validation() {
        // Test setup would use Phase 1 foundation:
        // - Use existing test pool setup
        // - Create test users with balances
        // - Create test deal listing using Phase 1 functions

        // Test validate_transfer_eligibility
        // Test execute_deal_transfer
        // Test get_deal_transfer_history
        // Test cancel_transfer
    }
}
