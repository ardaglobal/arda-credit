# 🔄 **Worker B: Transfer Engine & Payment (Phase 2 Parallel)**

## 🎯 **Your Mission - Phase 2**

You are implementing the **Transfer Engine** for loan trading. The database foundation is already complete from Phase 1. Your work enables users to purchase loans with proper payment settlement and ownership transfer. You own the transfer business logic, financial integration, and transfer CLI.

**Timeline**: 2.5 days parallel development with Worker A  
**Prerequisites**: Database foundation completed (Phase 1)  
**Your Domain**: Transfer operations, payment settlement, financial transactions

---

## ✅ **Phase 1 Dependencies (COMPLETED ✅)**

From the Database Foundation phase, you now have:
- ✅ Database migration `060_add_loan_trading_foundation.sql` applied
- ✅ Database tables: `loan_listings` and `loan_transfers` implemented
- ✅ Loan status enum with `for_sale` and `transfer_pending` values
- ✅ Core marketplace functions in `db/src/marketplace.rs` ready to use
- ✅ Schema validated and tested

**⚠️ IMPORTANT - Schema Reference Update:**
- **Use `loans.id`** (not `loans.loan_id` as originally planned)
- The existing database functions use the correct schema
- All your transfer implementations should follow the existing database functions

**Available Database Functions** (from Phase 1):
- `get_loan_listing_by_loan_id()` - Validate loan is for sale
- `update_listing_status()` - Mark listing as sold
- `validate_loan_for_transfer()` - Basic transfer validation
- `create_loan_listing()`, `remove_loan_listing()` - Listing management
- Plus existing: `get_user_balance_by_id()`, `update_user_balance_*()` functions

---

## 📋 **Phase 2 Task Checklist**

### **Day 1: Transfer Business Logic**
- [ ] Implement transfer database functions
- [ ] Build transfer validation logic  
- [ ] Test atomic transfer execution

### **Day 2: API & Financial Integration**
- [ ] Build transfer API handlers with payment settlement
- [ ] Add transfer routes to infrastructure
- [ ] Test financial transaction integration

### **Day 2.5: CLI & Integration Prep**
- [ ] Implement transfer CLI commands
- [ ] Coordinate infrastructure files with Worker A
- [ ] Prepare for Phase 3 integration

---

## 🗄️ **Database Implementation (Priority 1)**

### **File: `db/src/transfers.rs`** (New File)

Build on the Phase 1 foundation with comprehensive transfer operations:

```rust
//! Loan transfer database operations
//!
//! This module provides all database operations for loan ownership transfers including
//! validation, payment settlement, and ownership updates. It builds on the Phase 1
//! marketplace foundation.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

// Import Phase 1 foundation functions
use crate::{
    get_user_balance_by_id, update_user_balance_from_withdrawal_tx,
    update_user_balance_from_deposit_tx, get_loan_listing_by_loan_id
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
    pub transfer_date: DateTime<Utc>,
    pub settlement: TransferSettlementResult,
}

/// Payment settlement result
#[derive(Debug, Clone)]
pub struct TransferSettlementResult {
    pub transfer_id: Uuid,
    pub buyer_transaction_id: Uuid,
    pub seller_transaction_id: Uuid,
    pub settled_amount: Decimal,
    pub settlement_timestamp: DateTime<Utc>,
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
    pub transfer_date: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
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
        SELECT l.loan_id, l.lender_id, l.loan_status, l.principal_amount,
               ll.asking_price, ll.status as listing_status
        FROM loans l
        LEFT JOIN loan_listings ll ON l.loan_id = ll.loan_id AND ll.status = 'active'
        WHERE l.loan_id = $1
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
    if loan.loan_status == "for_sale" && loan.listing_status.is_some() {
        result.loan_for_sale = true;
    } else {
        result.validation_errors.push("Loan is not currently for sale".to_string());
    }
    
    // 3. Check buyer is not current owner
    if loan.lender_id != request.buyer_id {
        result.buyer_is_not_owner = true;
    } else {
        result.validation_errors.push("Buyer cannot purchase their own loan".to_string());
    }
    
    // 4. Check price matches listing
    if let Some(asking_price) = loan.asking_price {
        if asking_price == request.purchase_price {
            result.price_matches = true;
        } else {
            result.validation_errors.push(format!(
                "Purchase price {} does not match asking price {}", 
                request.purchase_price, asking_price
            ));
        }
    } else {
        result.validation_errors.push("No asking price found for loan".to_string());
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
        result.validation_errors.push("Buyer has no account balance".to_string());
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
        result.validation_errors.push("Buyer identity not found".to_string());
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
    let validation = validate_transfer_eligibility(pool, &TransferValidationRequest {
        loan_id: request.loan_id,
        buyer_id: request.buyer_id,
        purchase_price: request.purchase_price,
    }).await?;
    
    if !validation.is_valid {
        return Err(sqlx::Error::Database(Box::new(sqlx::postgres::PgDatabaseError::new(
            sqlx::postgres::PgSeverity::Error,
            "23514".to_string(),
            format!("Transfer validation failed: {}", validation.validation_errors.join(", ")),
        ))));
    }
    
    // 2. Get seller information from current loan ownership
    let seller_info = sqlx::query!(
        r#"
        SELECT l.lender_id as seller_id, ll.listing_id
        FROM loans l
        JOIN loan_listings ll ON l.loan_id = ll.loan_id 
        WHERE l.loan_id = $1 AND ll.status = 'active'
        "#,
        request.loan_id
    )
    .fetch_one(&mut *tx)
    .await?;
    
    let seller_id = seller_info.seller_id;
    let listing_id = seller_info.listing_id;
    
    // 3. Update loan status to transfer_pending (prevents concurrent transfers)
    sqlx::query!(
        r#"
        UPDATE loans 
        SET loan_status = 'transfer_pending', updated_at = NOW()
        WHERE loan_id = $1 AND loan_status = 'for_sale'
        "#,
        request.loan_id
    )
    .execute(&mut *tx)
    .await?;
    
    // 4. Create transfer record (using Phase 1 table)
    let transfer_id = uuid::Uuid::new_v4();
    let transfer_date = chrono::Utc::now();
    
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
        .map_err(|e| {
            sqlx::Error::Database(Box::new(sqlx::postgres::PgDatabaseError::new(
                sqlx::postgres::PgSeverity::Error,
                "23514".to_string(),
                format!("Failed to debit buyer account: {}", e),
            )))
        })?;
    
    // 5b. Credit seller account
    update_user_balance_from_deposit_tx(&mut tx, seller_id, request.purchase_price)
        .await
        .map_err(|e| {
            sqlx::Error::Database(Box::new(sqlx::postgres::PgDatabaseError::new(
                sqlx::postgres::PgSeverity::Error,
                "23514".to_string(),
                format!("Failed to credit seller account: {}", e),
            )))
        })?;
    
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
        -request.purchase_price, // Negative for debit
        format!("loan_transfer_{}", transfer_id) // Use transfer_id as transaction hash
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
        WHERE loan_id = $1
        "#,
        request.loan_id,
        request.buyer_id
    )
    .execute(&mut *tx)
    .await?;
    
    // 8. Mark listing as sold (using Phase 1 table)
    sqlx::query!(
        r#"
        UPDATE loan_listings
        SET status = 'sold', updated_at = NOW()
        WHERE listing_id = $1
        "#,
        listing_id
    )
    .execute(&mut *tx)
    .await?;
    
    // 9. Mark transfer as completed
    let completion_time = chrono::Utc::now();
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
            "UPDATE loans SET loan_status = 'for_sale', updated_at = NOW() WHERE loan_id = $1",
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
```

**Add to `db/src/lib.rs`**:
```rust
// Add to existing mod declarations
pub mod transfers;

// Add to existing use declarations  
pub use transfers::*;
```

---

## 🌐 **API Implementation (Priority 2)**

### **File: `api/src/transfer_handlers.rs`** (New File)

Implement transfer API endpoints with payment integration:

```rust
//! Transfer API handlers for loan trading
//!
//! This module contains all loan transfer-related API endpoints including:
//! - Execute loan ownership transfers with payment settlement
//! - View transfer history for loans and users
//! - Cancel pending transfers
//! - Transfer validation and eligibility checking

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use metrics;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use utoipa::{IntoParams, ToSchema};

use crate::types::ApiState;
use arda_credit_db::{
    execute_loan_transfer, get_loan_transfer_history, get_user_transfer_history,
    cancel_transfer, validate_transfer_eligibility, ExecuteTransferRequest,
    TransferValidationRequest, LoanTransferHistory
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to transfer loan ownership
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferRequest {
    pub new_lender_id: String,          // UUID of the buyer
    pub purchase_price: String,         // Decimal as string (e.g., "50000.00")
    pub transfer_notes: Option<String>, // Optional transfer notes
}

/// Response after successful loan transfer
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferResponse {
    pub transfer_id: String,            // UUID of the transfer record
    pub loan_id: String,                // UUID of the transferred loan
    pub previous_lender_id: String,     // UUID of the seller
    pub new_lender_id: String,          // UUID of the buyer
    pub purchase_price: String,         // Amount paid for the loan
    pub transfer_date: DateTime<Utc>,   // When transfer occurred
    pub updated_loan: TransferredLoanDetails, // Updated loan details
    pub payment_settlement: PaymentSettlement, // Payment details
}

/// Updated loan details after transfer
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransferredLoanDetails {
    pub loan_id: String,
    pub new_owner_id: String,           // new lender_id
    pub loan_status: String,            // "active"
    pub last_updated: DateTime<Utc>,
}

/// Payment settlement details
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentSettlement {
    pub buyer_transaction_id: String,   // Financial transaction for buyer
    pub seller_transaction_id: String,  // Financial transaction for seller
    pub settled_amount: String,         // Amount settled
    pub settlement_timestamp: DateTime<Utc>,
}

/// Transfer history item
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransferHistoryItem {
    pub transfer_id: String,
    pub loan_id: String,
    pub from_lender_id: String,
    pub to_lender_id: String,
    pub from_lender_email: String,
    pub to_lender_email: String,
    pub purchase_price: String,
    pub transfer_status: String,        // pending, completed, failed, cancelled
    pub transfer_date: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Response for loan transfer history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferHistoryResponse {
    pub loan_id: String,
    pub transfers: Vec<TransferHistoryItem>,
    pub total_transfers: i32,
}

/// Response for user transfer history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserTransferHistoryResponse {
    pub user_id: String,
    pub transfers: Vec<TransferHistoryItem>,
    pub total_count: i64,
    pub has_more: bool,
}

/// Query parameters for transfer history
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TransferHistoryQuery {
    pub limit: Option<i64>,     // Pagination limit (default: 50, max: 100)
    pub offset: Option<i64>,    // Pagination offset (default: 0)
}

/// Response for transfer cancellation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CancelTransferResponse {
    pub transfer_id: String,
    pub cancelled_at: DateTime<Utc>,
    pub status: String,         // "cancelled"
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Parse decimal from string with validation
fn parse_decimal(s: &str, field_name: &str) -> Result<Decimal, (StatusCode, String)> {
    s.parse::<Decimal>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid {} format: {}", field_name, s),
        )
    })
}

/// Convert database transfer history to API response format
fn transfer_history_to_api(history: LoanTransferHistory) -> TransferHistoryItem {
    TransferHistoryItem {
        transfer_id: history.transfer_id.to_string(),
        loan_id: history.loan_id.to_string(),
        from_lender_id: history.from_lender_id.to_string(),
        to_lender_id: history.to_lender_id.to_string(),
        from_lender_email: history.from_lender_email,
        to_lender_email: history.to_lender_email,
        purchase_price: history.purchase_price.to_string(),
        transfer_status: history.transfer_status,
        transfer_date: history.transfer_date,
        completed_at: history.completed_at,
        notes: history.notes,
    }
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Transfer loan ownership with payment settlement
#[utoipa::path(
    post,
    path = "/api/v1/loan/{loan_id}/transfer",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan to transfer")
    ),
    request_body = LoanTransferRequest,
    responses(
        (status = 200, description = "Loan transferred successfully", body = LoanTransferResponse),
        (status = 400, description = "Invalid request parameters or insufficient funds"),
        (status = 401, description = "Unauthorized - user not authenticated"),
        (status = 403, description = "Forbidden - user not KYC approved"),
        (status = 404, description = "Loan not found or not for sale"),
        (status = 409, description = "Transfer conflict (already in progress, etc.)"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn transfer_loan_ownership_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    Json(request): Json<LoanTransferRequest>,
) -> Result<Json<LoanTransferResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    // Record metrics
    metrics::counter!("loan_transfer_requests_total").increment(1);
    
    info!(
        "🔄 TRANSFER: Processing loan transfer - loan={}, buyer={}, price={}",
        loan_id_str, request.new_lender_id, request.purchase_price
    );
    
    // Parse and validate input
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;
    
    let buyer_id = Uuid::parse_str(&request.new_lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid buyer ID format".to_string(),
        )
    })?;
    
    let purchase_price = parse_decimal(&request.purchase_price, "purchase_price")?;
    
    // Validate purchase price is positive
    if purchase_price <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Purchase price must be positive".to_string(),
        ));
    }
    
    // Create transfer request
    let transfer_request = ExecuteTransferRequest {
        loan_id,
        buyer_id,
        purchase_price,
        notes: request.transfer_notes,
    };
    
    // Execute transfer using your database function
    let transfer_result = execute_loan_transfer(&state.pool, &transfer_request)
        .await
        .map_err(|e| {
            error!("❌ Failed to execute loan transfer: {}", e);
            
            // Map specific database errors to HTTP status codes
            match e {
                sqlx::Error::Database(ref db_err) => {
                    let code = db_err.code().map(|c| c.as_ref()).unwrap_or("");
                    match code {
                        "23514" => {
                            metrics::counter!("transfer_errors_total", "type" => "validation").increment(1);
                            if db_err.message().contains("validation failed") {
                                (StatusCode::BAD_REQUEST, db_err.message().to_string())
                            } else if db_err.message().contains("insufficient") {
                                (StatusCode::BAD_REQUEST, "Insufficient balance for transfer".to_string())
                            } else {
                                (StatusCode::CONFLICT, "Transfer validation failed".to_string())
                            }
                        },
                        "23505" => {
                            metrics::counter!("transfer_errors_total", "type" => "conflict").increment(1);
                            (StatusCode::CONFLICT, "Transfer already in progress".to_string())
                        },
                        _ => {
                            metrics::counter!("transfer_errors_total", "type" => "database").increment(1);
                            (StatusCode::INTERNAL_SERVER_ERROR, "Database error during transfer".to_string())
                        }
                    }
                },
                _ => {
                    metrics::counter!("transfer_errors_total", "type" => "unknown").increment(1);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to execute transfer".to_string())
                }
            }
        })?;
    
    let response = LoanTransferResponse {
        transfer_id: transfer_result.transfer_id.to_string(),
        loan_id: transfer_result.loan_id.to_string(),
        previous_lender_id: transfer_result.from_lender_id.to_string(),
        new_lender_id: transfer_result.to_lender_id.to_string(),
        purchase_price: transfer_result.purchase_price.to_string(),
        transfer_date: transfer_result.transfer_date,
        updated_loan: TransferredLoanDetails {
            loan_id: transfer_result.loan_id.to_string(),
            new_owner_id: transfer_result.to_lender_id.to_string(),
            loan_status: "active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        payment_settlement: PaymentSettlement {
            buyer_transaction_id: transfer_result.settlement.buyer_transaction_id.to_string(),
            seller_transaction_id: transfer_result.settlement.seller_transaction_id.to_string(),
            settled_amount: transfer_result.settlement.settled_amount.to_string(),
            settlement_timestamp: transfer_result.settlement.settlement_timestamp,
        },
    };
    
    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("loan_transfer_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("loan_transfers_completed_total").increment(1);
    metrics::histogram!("loan_transfer_amount_usdc").record(transfer_result.purchase_price.to_f64().unwrap_or(0.0));
    
    info!(
        "✅ TRANSFER: Loan {} transferred successfully from {} to {} for ${} ({}ms)",
        loan_id,
        transfer_result.from_lender_id,
        transfer_result.to_lender_id,
        transfer_result.purchase_price,
        duration.as_millis()
    );
    
    Ok(Json(response))
}

/// Get loan transfer history
#[utoipa::path(
    get,
    path = "/api/v1/loan/{loan_id}/transfers",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan")
    ),
    responses(
        (status = 200, description = "Transfer history retrieved successfully", body = LoanTransferHistoryResponse),
        (status = 400, description = "Invalid loan ID format"),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_loan_transfer_history_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
) -> Result<Json<LoanTransferHistoryResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    info!("📋 TRANSFER: Getting transfer history for loan {}", loan_id_str);
    
    // Parse loan ID
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;
    
    // Get transfer history using your database function
    let history = get_loan_transfer_history(&state.pool, loan_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get loan transfer history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve transfer history".to_string(),
            )
        })?;
    
    let transfers: Vec<TransferHistoryItem> = history
        .into_iter()
        .map(transfer_history_to_api)
        .collect();
    
    let response = LoanTransferHistoryResponse {
        loan_id: loan_id.to_string(),
        total_transfers: transfers.len() as i32,
        transfers,
    };
    
    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("transfer_history_query_duration_seconds").record(duration.as_secs_f64());
    
    info!(
        "✅ TRANSFER: Retrieved {} transfers for loan {} ({}ms)",
        response.total_transfers,
        loan_id,
        duration.as_millis()
    );
    
    Ok(Json(response))
}

/// Get user transfer history (loans bought or sold)
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/transfers",
    params(
        ("user_id" = String, Path, description = "UUID of the user"),
        TransferHistoryQuery
    ),
    responses(
        (status = 200, description = "User transfer history retrieved successfully", body = UserTransferHistoryResponse),
        (status = 400, description = "Invalid user ID format or query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_transfer_history_endpoint(
    State(state): State<ApiState>,
    Path(user_id_str): Path<String>,
    Query(query): Query<TransferHistoryQuery>,
) -> Result<Json<UserTransferHistoryResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    info!("👤 TRANSFER: Getting transfer history for user {}", user_id_str);
    
    // Parse user ID
    let user_id = Uuid::parse_str(&user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;
    
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    
    // Get user transfer history using your database function
    let history = get_user_transfer_history(&state.pool, user_id, Some(limit), Some(offset))
        .await
        .map_err(|e| {
            error!("❌ Failed to get user transfer history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user transfer history".to_string(),
            )
        })?;
    
    let transfers: Vec<TransferHistoryItem> = history
        .into_iter()
        .map(transfer_history_to_api)
        .collect();
    
    // Calculate pagination
    let has_more = transfers.len() as i64 == limit;
    
    let response = UserTransferHistoryResponse {
        user_id: user_id.to_string(),
        total_count: transfers.len() as i64,
        has_more,
        transfers,
    };
    
    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("user_transfer_history_query_duration_seconds").record(duration.as_secs_f64());
    
    info!(
        "✅ TRANSFER: Retrieved {} transfers for user {} ({}ms)",
        response.transfers.len(),
        user_id,
        duration.as_millis()
    );
    
    Ok(Json(response))
}

/// Validate transfer eligibility (utility endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/loan/{loan_id}/transfer/validate",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan")
    ),
    request_body = LoanTransferRequest,
    responses(
        (status = 200, description = "Transfer validation result", body = serde_json::Value),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn validate_transfer_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    Json(request): Json<LoanTransferRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🔍 TRANSFER: Validating transfer for loan {}", loan_id_str);
    
    // Parse inputs
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;
    
    let buyer_id = Uuid::parse_str(&request.new_lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid buyer ID format".to_string(),
        )
    })?;
    
    let purchase_price = parse_decimal(&request.purchase_price, "purchase_price")?;
    
    // Validate transfer using your database function
    let validation = validate_transfer_eligibility(&state.pool, &TransferValidationRequest {
        loan_id,
        buyer_id,
        purchase_price,
    })
    .await
    .map_err(|e| {
        error!("❌ Failed to validate transfer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to validate transfer".to_string(),
        )
    })?;
    
    // Return validation result as JSON
    let response = serde_json::json!({
        "is_valid": validation.is_valid,
        "loan_exists": validation.loan_exists,
        "loan_for_sale": validation.loan_for_sale,
        "buyer_has_funds": validation.buyer_has_funds,
        "buyer_is_not_owner": validation.buyer_is_not_owner,
        "buyer_kyc_approved": validation.buyer_kyc_approved,
        "price_matches": validation.price_matches,
        "available_balance": validation.available_balance.map(|b| b.to_string()),
        "validation_errors": validation.validation_errors,
        "timestamp": chrono::Utc::now()
    });
    
    Ok(Json(response))
}
```

---

## 💻 **CLI Implementation (Priority 3)**

### **File: `cli/src/transfers.rs`** (New File)

Create CLI commands for transfer operations:

```rust
//! Transfer CLI commands for loan trading
//!
//! This module provides command-line interface for loan transfer operations including
//! executing transfers, viewing history, and managing transfer lifecycle.

use clap::{Args, Subcommand};
use reqwest::Client;
use serde_json::json;
use std::error::Error;
use uuid::Uuid;

use crate::cli::{get_api_base_url, handle_api_error};

/// Transfer management commands
#[derive(Debug, Subcommand)]
pub enum TransferCommands {
    /// Execute loan ownership transfer
    Execute(ExecuteTransferArgs),
    
    /// View transfer history for a loan
    LoanHistory(LoanTransferHistoryArgs),
    
    /// View transfer history for a user
    UserHistory(UserTransferHistoryArgs),
    
    /// Validate transfer before executing
    Validate(ValidateTransferArgs),
}

/// Arguments for executing transfer
#[derive(Debug, Args)]
pub struct ExecuteTransferArgs {
    /// UUID of the loan to transfer
    #[arg(long)]
    pub loan_id: String,
    
    /// UUID of the buyer (new owner)
    #[arg(long)]
    pub buyer_id: String,
    
    /// Purchase price in USDC
    #[arg(long)]
    pub purchase_price: String,
    
    /// Optional transfer notes
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for loan transfer history
#[derive(Debug, Args)]
pub struct LoanTransferHistoryArgs {
    /// UUID of the loan
    #[arg(long)]
    pub loan_id: String,
    
    /// Show verbose output with all details
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for user transfer history
#[derive(Debug, Args)]
pub struct UserTransferHistoryArgs {
    /// UUID of the user
    #[arg(long)]
    pub user_id: String,
    
    /// Maximum number of results to return
    #[arg(long, default_value = "20")]
    pub limit: u32,
    
    /// Number of results to skip (for pagination)
    #[arg(long, default_value = "0")]
    pub offset: u32,
    
    /// Show verbose output with all details
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for validating transfer
#[derive(Debug, Args)]
pub struct ValidateTransferArgs {
    /// UUID of the loan to transfer
    #[arg(long)]
    pub loan_id: String,
    
    /// UUID of the buyer (new owner)
    #[arg(long)]
    pub buyer_id: String,
    
    /// Purchase price in USDC
    #[arg(long)]
    pub purchase_price: String,
}

/// Handle transfer commands
pub async fn handle_transfer_command(
    command: TransferCommands,
) -> Result<(), Box<dyn Error>> {
    match command {
        TransferCommands::Execute(args) => execute_transfer(args).await,
        TransferCommands::LoanHistory(args) => loan_transfer_history(args).await,
        TransferCommands::UserHistory(args) => user_transfer_history(args).await,
        TransferCommands::Validate(args) => validate_transfer(args).await,
    }
}

/// Execute loan ownership transfer
async fn execute_transfer(args: ExecuteTransferArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate input formats
    Uuid::parse_str(&args.loan_id).map_err(|_| "Invalid loan ID format")?;
    Uuid::parse_str(&args.buyer_id).map_err(|_| "Invalid buyer ID format")?;
    args.purchase_price.parse::<f64>().map_err(|_| "Invalid purchase price format")?;
    
    println!("🔄 Executing loan transfer...");
    println!("   📋 Loan ID: {}", args.loan_id);
    println!("   🧑 Buyer ID: {}", args.buyer_id);
    println!("   💰 Purchase Price: ${}", args.purchase_price);
    
    if let Some(ref notes) = args.notes {
        println!("   📝 Notes: {}", notes);
    }
    
    let request_body = json!({
        "new_lender_id": args.buyer_id,
        "purchase_price": args.purchase_price,
        "transfer_notes": args.notes
    });
    
    // Call your transfer API endpoint
    let response = client
        .post(&format!("{}/api/v1/loan/{}/transfer", base_url, args.loan_id))
        .json(&request_body)
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    // Parse response
    let transfer_id = result["transfer_id"].as_str().unwrap_or("unknown");
    let transfer_date = result["transfer_date"].as_str().unwrap_or("unknown");
    let previous_owner = result["previous_lender_id"].as_str().unwrap_or("unknown");
    let new_owner = result["new_lender_id"].as_str().unwrap_or("unknown");
    let settlement = &result["payment_settlement"];
    
    println!("");
    println!("✅ Loan transfer completed successfully!");
    println!("   🆔 Transfer ID: {}", transfer_id);
    println!("   📅 Transfer Date: {}", transfer_date);
    println!("   👥 Ownership: {} → {}", previous_owner, new_owner);
    println!("   💰 Amount: ${}", args.purchase_price);
    
    if let Some(buyer_tx) = settlement["buyer_transaction_id"].as_str() {
        println!("   🧾 Buyer Transaction: {}", buyer_tx);
    }
    if let Some(seller_tx) = settlement["seller_transaction_id"].as_str() {
        println!("   🧾 Seller Transaction: {}", seller_tx);
    }
    
    let updated_loan = &result["updated_loan"];
    if let Some(status) = updated_loan["loan_status"].as_str() {
        println!("   🔘 Loan Status: {}", status);
    }
    
    Ok(())
}

/// Validate transfer before executing
async fn validate_transfer(args: ValidateTransferArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate input formats
    Uuid::parse_str(&args.loan_id).map_err(|_| "Invalid loan ID format")?;
    Uuid::parse_str(&args.buyer_id).map_err(|_| "Invalid buyer ID format")?;
    args.purchase_price.parse::<f64>().map_err(|_| "Invalid purchase price format")?;
    
    println!("🔍 Validating transfer...");
    println!("   📋 Loan ID: {}", args.loan_id);
    println!("   🧑 Buyer ID: {}", args.buyer_id);
    println!("   💰 Purchase Price: ${}", args.purchase_price);
    
    let request_body = json!({
        "new_lender_id": args.buyer_id,
        "purchase_price": args.purchase_price
    });
    
    // Call your validation API endpoint
    let response = client
        .post(&format!("{}/api/v1/loan/{}/transfer/validate", base_url, args.loan_id))
        .json(&request_body)
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    println!("");
    
    // Parse validation results
    let is_valid = result["is_valid"].as_bool().unwrap_or(false);
    let loan_exists = result["loan_exists"].as_bool().unwrap_or(false);
    let loan_for_sale = result["loan_for_sale"].as_bool().unwrap_or(false);
    let buyer_has_funds = result["buyer_has_funds"].as_bool().unwrap_or(false);
    let buyer_is_not_owner = result["buyer_is_not_owner"].as_bool().unwrap_or(false);
    let buyer_kyc_approved = result["buyer_kyc_approved"].as_bool().unwrap_or(false);
    let price_matches = result["price_matches"].as_bool().unwrap_or(false);
    
    if is_valid {
        println!("✅ Transfer validation: PASSED");
        println!("   🎉 This transfer can be executed successfully!");
    } else {
        println!("❌ Transfer validation: FAILED");
        
        // Show detailed validation results
        println!("   📋 Loan exists: {}", if loan_exists { "✅" } else { "❌" });
        println!("   🛒 Loan for sale: {}", if loan_for_sale { "✅" } else { "❌" });
        println!("   💰 Buyer has funds: {}", if buyer_has_funds { "✅" } else { "❌" });
        println!("   👤 Buyer != owner: {}", if buyer_is_not_owner { "✅" } else { "❌" });
        println!("   🆔 Buyer KYC approved: {}", if buyer_kyc_approved { "✅" } else { "❌" });
        println!("   💲 Price matches: {}", if price_matches { "✅" } else { "❌" });
        
        if let Some(balance) = result["available_balance"].as_str() {
            println!("   💳 Available balance: ${}", balance);
        }
    }
    
    // Show validation errors if any
    if let Some(errors) = result["validation_errors"].as_array() {
        if !errors.is_empty() {
            println!("");
            println!("❌ Validation errors:");
            for error in errors {
                if let Some(error_str) = error.as_str() {
                    println!("   • {}", error_str);
                }
            }
        }
    }
    
    Ok(())
}

/// View loan transfer history
async fn loan_transfer_history(args: LoanTransferHistoryArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate loan ID
    Uuid::parse_str(&args.loan_id).map_err(|_| "Invalid loan ID format")?;
    
    println!("📋 Getting transfer history for loan {}...", args.loan_id);
    
    // Call your transfer history API endpoint
    let response = client
        .get(&format!("{}/api/v1/loan/{}/transfers", base_url, args.loan_id))
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    let transfers = result["transfers"].as_array().unwrap_or(&vec![]);
    let total_transfers = result["total_transfers"].as_i64().unwrap_or(0);
    
    if transfers.is_empty() {
        println!("📭 No transfers found for this loan");
        return Ok(());
    }
    
    println!("📊 Found {} transfers for loan {}", total_transfers, args.loan_id);
    println!("");
    
    // Display transfers
    for (i, transfer) in transfers.iter().enumerate() {
        let transfer_id = transfer["transfer_id"].as_str().unwrap_or("unknown");
        let from_email = transfer["from_lender_email"].as_str().unwrap_or("unknown");
        let to_email = transfer["to_lender_email"].as_str().unwrap_or("unknown");
        let price = transfer["purchase_price"].as_str().unwrap_or("0");
        let status = transfer["transfer_status"].as_str().unwrap_or("unknown");
        let date = transfer["transfer_date"].as_str().unwrap_or("unknown");
        
        println!("{}. Transfer {} ({})", i + 1, transfer_id, status.to_uppercase());
        println!("   👥 {} → {}", from_email, to_email);
        println!("   💰 Purchase Price: ${}", price);
        println!("   📅 Date: {}", date);
        
        if args.verbose {
            if let Some(notes) = transfer["notes"].as_str() {
                if !notes.is_empty() {
                    println!("   📝 Notes: {}", notes);
                }
            }
            
            if let Some(completed) = transfer["completed_at"].as_str() {
                println!("   ✅ Completed: {}", completed);
            }
        }
        
        println!("");
    }
    
    Ok(())
}

/// View user transfer history
async fn user_transfer_history(args: UserTransferHistoryArgs) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base_url = get_api_base_url();
    
    // Validate user ID
    Uuid::parse_str(&args.user_id).map_err(|_| "Invalid user ID format")?;
    
    println!("👤 Getting transfer history for user {}...", args.user_id);
    
    // Call your user transfer history API endpoint
    let response = client
        .get(&format!("{}/api/v1/user/{}/transfers", base_url, args.user_id))
        .query(&[("limit", args.limit.to_string()), ("offset", args.offset.to_string())])
        .send()
        .await?;
    
    if !response.status().is_success() {
        handle_api_error(&response).await?;
        return Ok(());
    }
    
    let result: serde_json::Value = response.json().await?;
    
    let transfers = result["transfers"].as_array().unwrap_or(&vec![]);
    let has_more = result["has_more"].as_bool().unwrap_or(false);
    
    if transfers.is_empty() {
        println!("📭 No transfers found for this user");
        return Ok(());
    }
    
    println!("📊 Found {} transfers", transfers.len());
    println!("");
    
    // Display transfers
    for (i, transfer) in transfers.iter().enumerate() {
        let transfer_id = transfer["transfer_id"].as_str().unwrap_or("unknown");
        let loan_id = transfer["loan_id"].as_str().unwrap_or("unknown");
        let from_email = transfer["from_lender_email"].as_str().unwrap_or("unknown");
        let to_email = transfer["to_lender_email"].as_str().unwrap_or("unknown");
        let price = transfer["purchase_price"].as_str().unwrap_or("0");
        let status = transfer["transfer_status"].as_str().unwrap_or("unknown");
        let date = transfer["transfer_date"].as_str().unwrap_or("unknown");
        
        // Determine if user was buyer or seller
        let user_role = if transfer["from_lender_id"].as_str() == Some(&args.user_id) {
            "SELLER"
        } else {
            "BUYER"
        };
        
        println!("{}. {} - {} ({})", i + 1, user_role, loan_id, status.to_uppercase());
        println!("   👥 {} → {}", from_email, to_email);
        println!("   💰 Price: ${}", price);
        println!("   📅 Date: {}", date);
        
        if args.verbose {
            println!("   🆔 Transfer ID: {}", transfer_id);
            if let Some(notes) = transfer["notes"].as_str() {
                if !notes.is_empty() {
                    println!("   📝 Notes: {}", notes);
                }
            }
        }
        
        println!("");
    }
    
    if has_more {
        println!("💡 Use --offset {} to see more results", args.offset + args.limit);
    }
    
    Ok(())
}
```

---

## 🔧 **Infrastructure Updates (Coordinate with Worker A)**

### **Your Infrastructure Files**

**Add to `api/src/paths.rs`**:
```rust
// ============================================================================
// TRANSFER PATHS (Worker B)
// ============================================================================

/// Execute loan ownership transfer
pub const LOAN_TRANSFER: &str = api_path!("/loan/{loan_id}/transfer");

/// Get loan transfer history
pub const LOAN_TRANSFER_HISTORY: &str = api_path!("/loan/{loan_id}/transfers");

/// Get user transfer history
pub const USER_TRANSFER_HISTORY: &str = api_path!("/user/{user_id}/transfers");

/// Validate transfer eligibility
pub const LOAN_TRANSFER_VALIDATE: &str = api_path!("/loan/{loan_id}/transfer/validate");
```

**Add to `api/src/lib.rs`**:
```rust
// Add to existing mod declarations
pub mod transfer_handlers;

// Add to existing use declarations (if any)
pub use transfer_handlers::*;
```

### **Shared Infrastructure (Coordinate with Worker A)**

**`api/src/router.rs`** - Both workers add routes:
```rust
// Your transfer routes (Worker B)
use crate::transfer_handlers::{
    transfer_loan_ownership_endpoint, get_loan_transfer_history_endpoint,
    get_user_transfer_history_endpoint, validate_transfer_endpoint
};

// Add your routes to the router
.route(paths::LOAN_TRANSFER, post(transfer_loan_ownership_endpoint))
.route(paths::LOAN_TRANSFER_HISTORY, get(get_loan_transfer_history_endpoint))
.route(paths::USER_TRANSFER_HISTORY, get(get_user_transfer_history_endpoint))
.route(paths::LOAN_TRANSFER_VALIDATE, post(validate_transfer_endpoint))
```

**`cli/src/lib.rs`** - Both workers add modules:
```rust
// Add to existing mod declarations
pub mod transfers;  // Your module

// Add to existing use declarations
pub use transfers::*;
```

**`cli/src/cli.rs`** - Both workers add commands:
```rust
// Add to imports
use crate::transfers::{TransferCommands, handle_transfer_command};

// Add to main Commands enum
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// Transfer management commands (Worker B)
    #[command(subcommand)]
    Transfer(TransferCommands),
}

// Add to main command handler
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing code ...
    
    match args.command {
        // ... existing command handlers ...
        
        Commands::Transfer(transfer_cmd) => {
            handle_transfer_command(transfer_cmd).await
        }
    }
}
```

---

## 🧪 **Phase 2 Testing Strategy**

### **Database Testing**
```bash
# Test your transfer database functions
cd db && cargo test transfers

# Test transfer validation
cargo test validate_transfer_eligibility

# Test atomic transfer execution
cargo test execute_loan_transfer
```

### **API Testing**
```bash
# Test your transfer API endpoints
cd api && cargo test transfer_handlers

# Manual API testing (requires Phase 1 sample data)
curl -X POST "http://localhost:8080/api/v1/loan/{loan_id}/transfer/validate" \
  -H "Content-Type: application/json" \
  -d '{"new_lender_id": "buyer-uuid", "purchase_price": "50000.00"}'

curl -X POST "http://localhost:8080/api/v1/loan/{loan_id}/transfer" \
  -H "Content-Type: application/json" \
  -d '{"new_lender_id": "buyer-uuid", "purchase_price": "50000.00"}'
```

### **CLI Testing**
```bash
# Test your transfer CLI commands
make cli ARGS="transfer validate --loan-id {UUID} --buyer-id {UUID} --purchase-price 50000"
make cli ARGS="transfer execute --loan-id {UUID} --buyer-id {UUID} --purchase-price 50000"
make cli ARGS="transfer loan-history --loan-id {UUID}"
make cli ARGS="transfer user-history --user-id {UUID}"
```

### **Financial Integration Testing**
```bash
# Test balance integration (requires existing balance system)
# Check user balances before/after transfer
make cli ARGS="get-user-balance-by-address --address 0x..."

# Test financial transaction recording
psql $DATABASE_URL -c "SELECT * FROM account_financial_transactions WHERE transaction_type IN ('LOAN_PURCHASE', 'LOAN_SALE') ORDER BY processed_at DESC LIMIT 5;"
```

---

## 📋 **Phase 2 Coordination with Worker A**

### **What You Need from Worker A**:
1. **Marketplace API Format**: Consistent `MarketplaceDeal` response structure
2. **Route Patterns**: REST endpoint naming conventions
3. **Error Handling Format**: Standard HTTP error responses
4. **CLI Command Structure**: Pattern for command organization

### **What You Provide to Worker A**:
1. **Transfer Integration**: How transfers update marketplace status
2. **Financial Patterns**: Payment settlement error handling
3. **Database Usage**: How you use Phase 1 tables and functions
4. **Authentication Needs**: Requirements for user identification

### **Daily Sync Points**:
- **Day 1 End**: Share transfer validation and payment settlement patterns
- **Day 2 Midday**: Coordinate infrastructure file updates
- **Day 2 End**: Finalize shared infrastructure integration

---

## ✅ **Phase 2 Definition of Done**

### **Database Success Criteria**:
- [ ] All transfer database functions implemented and tested
- [ ] Atomic transfer operations work correctly
- [ ] Payment settlement integrates with existing balance system
- [ ] Transfer validation covers all business rules
- [ ] Financial transaction recording follows existing patterns

### **API Success Criteria**:
- [ ] All 4 transfer endpoints implemented and tested
- [ ] Proper error handling with HTTP status codes
- [ ] Metrics and logging integration
- [ ] OpenAPI documentation complete
- [ ] Payment settlement works end-to-end

### **CLI Success Criteria**:
- [ ] All 4 CLI commands working with your APIs
- [ ] Rich output formatting for transfer results
- [ ] Validation feedback before executing transfers
- [ ] Transfer history displays useful information

### **Integration Success Criteria**:
- [ ] Infrastructure files updated (paths.rs, lib.rs)
- [ ] Router integration ready for Worker A merge
- [ ] CLI integration ready for Worker A merge
- [ ] No conflicts with Worker A's parallel work

### **Ready for Phase 3**:
- [ ] All endpoints tested independently
- [ ] Payment settlement proven safe and atomic
- [ ] Integration points documented for Worker A
- [ ] End-to-end transfer workflow ready

**You own the transfer engine - make it bulletproof and secure! 🔄💰**

---

## 🚀 **Getting Started**

### **Day 1: Transfer Business Logic**
1. **Create `transfers.rs`** - Database operations and validation
2. **Test atomic operations** - Ensure transaction safety
3. **Test payment integration** - Verify balance system works

### **Day 2: API Implementation**
1. **Create `transfer_handlers.rs`** - 4 core API endpoints
2. **Test financial integration** - End-to-end payment flow
3. **Add infrastructure updates** - paths.rs, lib.rs

### **Day 2.5: CLI & Integration Prep**
1. **Create `transfers.rs` CLI** - 4 corresponding commands
2. **Test CLI → API → Database flow** - Full transfer workflow
3. **Coordinate with Worker A** - Finalize shared infrastructure

**Ready to build the transfer engine! Let's make loan ownership transfers seamless and secure! 🔄🚀**
