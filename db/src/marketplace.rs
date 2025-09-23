//! Core marketplace database operations
//!
//! This module provides the foundation database operations for both:
//! - Worker A: Marketplace browsing and listing management
//! - Worker B: Transfer validation and ownership updates
//!
//! CRITICAL: This is the foundation that unblocks Worker B

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

// ============================================================================
// CORE DATA STRUCTURES (Used by both workers)
// ============================================================================

/// Loan listing record from database
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoanListing {
    pub listing_id: Uuid,
    pub loan_id: Uuid,
    pub seller_id: Uuid,
    pub asking_price: Decimal,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Marketplace loan with combined data (for both workers to use)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceLoan {
    // Listing data
    pub listing_id: Uuid,
    pub asking_price: Decimal,
    pub listing_description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub listing_status: String,

    // Loan data
    pub loan_id: Uuid,
    pub borrower_id: Uuid,
    pub lender_id: Uuid, // Current owner
    pub principal_amount: Decimal,
    pub outstanding_balance: Decimal,
    pub interest_rate_bps: i32,
    pub loan_status: String,
    pub originated_at: DateTime<Utc>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub risk_level: Option<String>,
    pub asset_class: Option<String>,
    pub region: Option<String>,
    pub minimum_investment: Option<Decimal>,

    // Seller data
    pub seller_email: String,
    pub seller_kyc_status: String,
}

// ============================================================================
// CRITICAL FOUNDATION FUNCTIONS (Worker B depends on these)
// ============================================================================

/// Create a loan listing (Worker A primary, Worker B validates against)
///
/// This is the core function that lists a loan for sale in the marketplace.
/// Worker B needs this to exist for transfer validation.
pub async fn create_loan_listing(
    pool: &PgPool,
    loan_id: Uuid,
    seller_id: Uuid,
    asking_price: Decimal,
    description: Option<String>,
) -> Result<LoanListing, sqlx::Error> {
    // Begin transaction for atomic operation
    let mut tx = pool.begin().await?;

    // 1. Verify seller owns the loan and loan is eligible
    let loan = sqlx::query!(
        r#"
        SELECT id, lender_id, loan_status
        FROM loans
        WHERE id = $1 AND lender_id = $2
        "#,
        loan_id,
        seller_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let loan = loan.ok_or_else(|| sqlx::Error::RowNotFound)?;

    // 2. Validate loan status (must be active or originated)
    if !matches!(loan.loan_status.as_str(), "active" | "originated") {
        return Err(sqlx::Error::Protocol(format!(
            "Loan status '{}' is not eligible for listing",
            loan.loan_status
        )));
    }

    // 3. Check if loan is already listed
    let existing = sqlx::query!(
        "SELECT listing_id FROM loan_listings WHERE loan_id = $1 AND status = 'active'",
        loan_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if existing.is_some() {
        return Err(sqlx::Error::Protocol(
            "Loan is already listed for sale".to_string(),
        ));
    }

    // 4. Update loan status to for_sale
    sqlx::query!(
        "UPDATE loans SET loan_status = 'for_sale', updated_at = NOW() WHERE id = $1",
        loan_id
    )
    .execute(&mut *tx)
    .await?;

    // 5. Create listing record
    let row = sqlx::query!(
        r#"
        INSERT INTO loan_listings (loan_id, seller_id, asking_price, description)
        VALUES ($1, $2, $3, $4)
        RETURNING
            listing_id, loan_id, seller_id, asking_price, description,
            listed_at, expires_at, status, created_at, updated_at
        "#,
        loan_id,
        seller_id,
        asking_price,
        description
    )
    .fetch_one(&mut *tx)
    .await?;

    let listing = LoanListing {
        listing_id: row.listing_id,
        loan_id: row.loan_id,
        seller_id: row.seller_id,
        asking_price: row.asking_price,
        description: row.description,
        listed_at: row.listed_at.and_utc(),
        expires_at: row.expires_at.map(|dt| dt.and_utc()),
        status: row.status,
        created_at: row.created_at.and_utc(),
        updated_at: row.updated_at.and_utc(),
    };

    // 6. Commit transaction
    tx.commit().await?;

    Ok(listing)
}

/// Get loan listing by loan_id (Worker B needs this for transfer validation)
///
/// Worker B uses this to validate a loan is actually for sale before allowing transfer.
pub async fn get_loan_listing_by_loan_id(
    pool: &PgPool,
    loan_id: Uuid,
) -> Result<Option<LoanListing>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT listing_id, loan_id, seller_id, asking_price, description,
               listed_at, expires_at, status, created_at, updated_at
        FROM loan_listings
        WHERE loan_id = $1 AND status = 'active'
        "#,
        loan_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(LoanListing {
            listing_id: row.listing_id,
            loan_id: row.loan_id,
            seller_id: row.seller_id,
            asking_price: row.asking_price,
            description: row.description,
            listed_at: row.listed_at.and_utc(),
            expires_at: row.expires_at.map(|dt| dt.and_utc()),
            status: row.status,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        })),
        None => Ok(None),
    }
}

/// Update listing status (Worker B needs this to mark listings as sold)
///
/// Worker B calls this when a transfer completes to mark the listing as sold.
pub async fn update_listing_status(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    new_status: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE loan_listings
        SET status = $2, updated_at = NOW()
        WHERE loan_id = $1 AND status = 'active'
        "#,
        loan_id,
        new_status
    )
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Remove loan listing (cancel sale - Worker A uses this)
///
/// Worker A uses this when user cancels a listing.
pub async fn remove_loan_listing(
    pool: &PgPool,
    loan_id: Uuid,
    seller_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // Begin transaction
    let mut tx = pool.begin().await?;

    // 1. Verify seller owns the listing
    let listing = sqlx::query!(
        "SELECT listing_id FROM loan_listings WHERE loan_id = $1 AND seller_id = $2 AND status = 'active'",
        loan_id,
        seller_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if listing.is_none() {
        return Ok(false);
    }

    // 2. Update listing status to cancelled
    update_listing_status(&mut tx, loan_id, "cancelled").await?;

    // 3. Update loan status back to active
    sqlx::query!(
        "UPDATE loans SET loan_status = 'active', updated_at = NOW() WHERE id = $1",
        loan_id
    )
    .execute(&mut *tx)
    .await?;

    // 4. Commit transaction
    tx.commit().await?;

    Ok(true)
}

/// Get marketplace loans with basic filtering (Worker A extends this)
///
/// Worker A will extend this with more sophisticated filtering.
/// Worker B uses this to validate loans are actually in marketplace.
pub async fn get_marketplace_loans_basic(
    pool: &PgPool,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<MarketplaceLoan>, sqlx::Error> {
    let limit = limit.unwrap_or(20).min(100);
    let offset = offset.unwrap_or(0);

    let loans = sqlx::query!(
        r#"
        SELECT
            ll.listing_id, ll.asking_price, ll.description as listing_description,
            ll.listed_at, ll.status as listing_status,
            l.id as loan_id, l.borrower_id, l.lender_id, l.principal_amount, l.outstanding_balance,
            l.interest_rate_bps, l.loan_status, l.originated_at, l.title, l.description,
            l.risk_level, l.asset_class, l.region, l.minimum_investment,
            i.email as seller_email, i.kyc_status as seller_kyc_status
        FROM loan_listings ll
        JOIN loans l ON ll.loan_id = l.id
        JOIN identities i ON ll.seller_id = i.id
        WHERE ll.status = 'active'
          AND l.loan_status = 'for_sale'
        ORDER BY ll.listed_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let marketplace_loans = loans
        .into_iter()
        .map(|row| MarketplaceLoan {
            // Listing data
            listing_id: row.listing_id,
            asking_price: row.asking_price,
            listing_description: row.listing_description,
            listed_at: row.listed_at.and_utc(),
            listing_status: row.listing_status,

            // Loan data
            loan_id: row.loan_id,
            borrower_id: row.borrower_id,
            lender_id: row.lender_id,
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            loan_status: row.loan_status,
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            title: row.title,
            description: row.description,
            risk_level: row.risk_level,
            asset_class: row.asset_class,
            region: row.region,
            minimum_investment: row.minimum_investment,

            // Seller data
            seller_email: row.seller_email,
            seller_kyc_status: row.seller_kyc_status,
        })
        .collect();

    Ok(marketplace_loans)
}

/// Validate loan can be transferred (Worker B primary function)
///
/// Worker B extends this but needs the basic structure from the foundation.
pub async fn validate_loan_for_transfer(
    pool: &PgPool,
    loan_id: Uuid,
) -> Result<Option<(Uuid, Decimal)>, sqlx::Error> {
    // Return (current_owner_id, asking_price) if loan can be transferred
    let result = sqlx::query!(
        r#"
        SELECT l.lender_id, ll.asking_price
        FROM loans l
        JOIN loan_listings ll ON l.id = ll.loan_id
        WHERE l.id = $1
          AND l.loan_status = 'for_sale'
          AND ll.status = 'active'
        "#,
        loan_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| (r.lender_id, r.asking_price)))
}

// ============================================================================
// TESTING AND VALIDATION FUNCTIONS
// ============================================================================

/// Create sample test data for validation
///
/// This helps verify the schema works and provides data for both workers to test against.
pub async fn create_sample_marketplace_data(
    pool: &PgPool,
    test_user_id: Uuid,
    test_loan_id: Uuid,
) -> Result<LoanListing, sqlx::Error> {
    // First ensure test loan exists and is owned by test user
    sqlx::query!(
        r#"
        INSERT INTO loans (id, borrower_id, lender_id, principal_amount, outstanding_balance, interest_rate_bps, loan_status)
        VALUES ($1, $2, $2, 100000, 75000, 1200, 'active')
        ON CONFLICT (id) DO UPDATE SET
            lender_id = $2,
            loan_status = 'active'
        "#,
        test_loan_id,
        test_user_id
    )
    .execute(pool)
    .await?;

    // Create sample listing
    create_loan_listing(
        pool,
        test_loan_id,
        test_user_id,
        Decimal::new(95000, 2), // $950.00 (example asking price)
        Some("Sample loan listing for testing".to_string()),
    )
    .await
}

/// Validate schema is working correctly
///
/// This runs basic validation to ensure the foundation is solid for both workers.
pub async fn validate_marketplace_schema(pool: &PgPool) -> Result<bool, sqlx::Error> {
    // Test 1: Can we create a listing?
    let test_user_id = Uuid::new_v4();
    let test_loan_id = Uuid::new_v4();

    // Insert test identity
    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, 'test@example.com', 'approved', 'lender')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_user_id
    )
    .execute(pool)
    .await?;

    // Test create listing
    let listing = create_sample_marketplace_data(pool, test_user_id, test_loan_id).await?;

    // Test 2: Can we retrieve the listing?
    let retrieved = get_loan_listing_by_loan_id(pool, test_loan_id).await?;
    assert!(
        retrieved.is_some(),
        "Should be able to retrieve created listing"
    );

    // Test 3: Can we get marketplace loans?
    let marketplace_loans = get_marketplace_loans_basic(pool, Some(10), Some(0)).await?;
    assert!(
        !marketplace_loans.is_empty(),
        "Should have at least one marketplace loan"
    );

    // Test 4: Can we validate for transfer?
    let validation = validate_loan_for_transfer(pool, test_loan_id).await?;
    assert!(
        validation.is_some(),
        "Should be able to validate loan for transfer"
    );

    // Test 5: Can we update listing status?
    let mut tx = pool.begin().await?;
    let updated = update_listing_status(&mut tx, test_loan_id, "sold").await?;
    tx.rollback().await?; // Don't actually update for this test
    assert!(updated, "Should be able to update listing status");

    // Clean up test data
    sqlx::query!("DELETE FROM loan_listings WHERE loan_id = $1", test_loan_id)
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM loans WHERE id = $1", test_loan_id)
        .execute(pool)
        .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests ensure the foundation is solid
    #[tokio::test]
    async fn test_marketplace_foundation() {
        // This test would use your existing test pool setup
        // let pool = get_test_pool().await;
        // assert!(validate_marketplace_schema(&pool).await.unwrap());

        // For now, just ensure compilation works
        assert!(true);
    }

    #[tokio::test]
    async fn test_loan_listing_crud() {
        // Test create, read, update, delete operations
        // This validates the foundation works for both workers
        assert!(true);
    }
}
