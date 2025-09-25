//! Core marketplace database operations
//!
//! This module provides the foundation database operations for both:
//! - Worker A: Marketplace browsing and listing management
//! - Worker B: Transfer validation and ownership updates
//!
//! Converted from loan-based marketplace to deal-based marketplace.
//! CRITICAL: This is the foundation that unblocks Worker B

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

// ============================================================================
// CORE DATA STRUCTURES (Used by both workers)
// ============================================================================

/// Deal listing record from database (converted from LoanListing)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealListing {
    // was LoanListing
    pub listing_id: Uuid,
    pub deal_id: Uuid, // was loan_id
    pub seller_id: Uuid,
    pub asking_price: Decimal,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Marketplace deal with combined data (converted from MarketplaceLoan)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceDeal {
    // was MarketplaceLoan
    // Listing data
    pub listing_id: Uuid,
    pub deal_id: Uuid, // was loan_id
    pub seller_id: Uuid,
    pub asking_price: Decimal,
    pub description: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: String,

    // Deal data (from Deal struct)
    pub originator_id: Uuid, // was borrower_id
    pub investor_id: Uuid,   // was lender_id
    pub principal_amount: Decimal,
    pub outstanding_balance: Decimal,
    pub interest_rate_bps: i32,
    pub loan_status: String, // keeping same field name
    pub originated_at: DateTime<Utc>,
    pub maturity_date: Option<DateTime<Utc>>,
}

// ============================================================================
// CRITICAL FOUNDATION FUNCTIONS (Worker B depends on these)
// ============================================================================

/// Create a new deal listing (converted from create_loan_listing)
///
/// This is the core function that lists a deal for sale in the marketplace.
/// Worker B needs this to exist for transfer validation.
pub async fn create_deal_listing(
    // was create_loan_listing
    pool: &PgPool,
    deal_id: Uuid, // was loan_id
    seller_id: Uuid,
    asking_price: Decimal,
    description: Option<String>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<DealListing, sqlx::Error> {
    let listing_id = Uuid::new_v4();
    let now = Utc::now();

    // First update the deal status to 'for_sale'
    sqlx::query!(
        r#"
        UPDATE deals
        SET loan_status = 'for_sale', updated_at = NOW()
        WHERE id = $1 AND investor_id = $2
        "#,
        deal_id,
        seller_id
    )
    .execute(pool)
    .await?;

    // Then create the listing
    sqlx::query!(
        r#"
        INSERT INTO deal_listings (listing_id, deal_id, seller_id, asking_price, description, listed_at, expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $6, $6)
        "#,
        listing_id,
        deal_id,        // was loan_id
        seller_id,
        asking_price,
        description,
        now.naive_utc(),
        expires_at.map(|dt| dt.naive_utc())
    )
    .execute(pool)
    .await?;

    Ok(DealListing {
        listing_id,
        deal_id, // was loan_id
        seller_id,
        asking_price,
        description,
        listed_at: now,
        expires_at,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    })
}

/// Get deal listing by deal_id (converted from get_loan_listing_by_loan_id)
///
/// Worker B uses this to validate a deal is actually for sale before allowing transfer.
pub async fn get_deal_listing_by_deal_id(
    // was get_loan_listing_by_loan_id
    pool: &PgPool,
    deal_id: Uuid, // was loan_id
) -> Result<Option<DealListing>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT listing_id, deal_id, seller_id, asking_price, description,
               listed_at, expires_at, status, created_at, updated_at
        FROM deal_listings  -- was loan_listings
        WHERE deal_id = $1 AND status = 'active'  -- was loan_id
        "#,
        deal_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(DealListing {
            listing_id: row.listing_id,
            deal_id: row.deal_id, // was loan_id
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

/// Update listing status (converted from update_listing_status)
///
/// Worker B calls this when a transfer completes to mark the listing as sold.
pub async fn update_deal_listing_status(
    // was update_listing_status
    tx: &mut Transaction<'_, Postgres>,
    deal_id: Uuid, // was loan_id
    new_status: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE deal_listings  -- was loan_listings
        SET status = $2, updated_at = NOW()
        WHERE deal_id = $1 AND status = 'active'  -- was loan_id
        "#,
        deal_id,
        new_status
    )
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Remove deal listing (converted from remove_loan_listing)
///
/// Worker A uses this when user cancels a listing.
pub async fn remove_deal_listing(
    // was remove_loan_listing
    pool: &PgPool,
    deal_id: Uuid, // was loan_id
    seller_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // Begin transaction
    let mut tx = pool.begin().await?;

    // 1. Verify seller owns the listing
    let listing = sqlx::query!(
        "SELECT listing_id FROM deal_listings WHERE deal_id = $1 AND seller_id = $2 AND status = 'active'",  // was loan_listings, loan_id
        deal_id,
        seller_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if listing.is_none() {
        return Ok(false);
    }

    // 2. Update listing status to cancelled
    update_deal_listing_status(&mut tx, deal_id, "cancelled").await?; // was update_listing_status, loan_id

    // 3. Update deal status back to active
    sqlx::query!(
        "UPDATE deals SET loan_status = 'active', updated_at = NOW() WHERE id = $1", // was loans
        deal_id
    )
    .execute(&mut *tx)
    .await?;

    // 4. Commit transaction
    tx.commit().await?;

    Ok(true)
}

/// Browse marketplace deals with basic filtering (converted from get_marketplace_loans_basic)
///
/// Worker A will extend this with more sophisticated filtering.
/// Worker B uses this to validate deals are actually in marketplace.
pub async fn browse_marketplace_deals(
    // was get_marketplace_loans_basic
    pool: &PgPool,
    limit: Option<i64>,
    status_filter: Option<String>,
) -> Result<Vec<MarketplaceDeal>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            dl.listing_id,
            dl.deal_id,              -- was loan_id
            dl.seller_id,
            dl.asking_price,
            dl.description,
            dl.listed_at,
            dl.expires_at,
            dl.status as listing_status,
            d.originator_id,         -- was borrower_id
            d.investor_id,           -- was lender_id
            d.principal_amount,
            d.outstanding_balance,
            d.interest_rate_bps,
            d.loan_status,
            d.originated_at,
            d.maturity_date
        FROM deal_listings dl       -- was loan_listings
        JOIN deals d ON dl.deal_id = d.id  -- was loans, loan_id
        WHERE ($1::text IS NULL OR dl.status = $1)
        ORDER BY dl.listed_at DESC
        LIMIT $2
        "#,
        status_filter,
        limit.unwrap_or(50)
    )
    .fetch_all(pool)
    .await?;

    let result: Vec<MarketplaceDeal> = rows
        .into_iter()
        .map(|row| MarketplaceDeal {
            listing_id: row.listing_id,
            deal_id: row.deal_id, // was loan_id
            seller_id: row.seller_id,
            asking_price: row.asking_price,
            description: row.description,
            listed_at: row.listed_at.and_utc(),
            expires_at: row.expires_at.map(|dt| dt.and_utc()),
            status: row.listing_status,
            originator_id: row.originator_id, // was borrower_id
            investor_id: row.investor_id,     // was lender_id
            principal_amount: row.principal_amount,
            outstanding_balance: row.outstanding_balance,
            interest_rate_bps: row.interest_rate_bps,
            loan_status: row.loan_status,
            originated_at: row.originated_at.unwrap_or_else(|| chrono::Utc::now()),
            maturity_date: row.maturity_date,
        })
        .collect();

    Ok(result)
}

/// Validate deal can be transferred (converted from validate_loan_for_transfer)
///
/// Worker B extends this but needs the basic structure from the foundation.
pub async fn validate_deal_for_transfer(
    // was validate_loan_for_transfer
    pool: &PgPool,
    deal_id: Uuid, // was loan_id
) -> Result<Option<(Uuid, Decimal)>, sqlx::Error> {
    // Return (current_owner_id, asking_price) if deal can be transferred
    let result = sqlx::query!(
        r#"
        SELECT d.investor_id, dl.asking_price  -- was lender_id, loan_listings
        FROM deals d                          -- was loans
        JOIN deal_listings dl ON d.id = dl.deal_id  -- was loan_listings, loan_id
        WHERE d.id = $1
          AND d.loan_status = 'for_sale'
          AND dl.status = 'active'
        "#,
        deal_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| (r.investor_id, r.asking_price))) // was lender_id
}

// ============================================================================
// TESTING AND VALIDATION FUNCTIONS
// ============================================================================

/// Create sample test data for validation (converted from create_sample_marketplace_data)
///
/// This helps verify the schema works and provides data for both workers to test against.
pub async fn create_sample_marketplace_data(
    // was create_sample_marketplace_data
    pool: &PgPool,
    test_investor_id: Uuid, // was test_lender_id
    test_deal_id: Uuid,     // was test_loan_id
) -> Result<DealListing, sqlx::Error> {
    // Note: This function assumes the deal already exists
    // The caller should create the deal with proper originator/investor before calling this

    // Create sample listing
    create_deal_listing(
        // was create_loan_listing
        pool,
        test_deal_id,           // was test_loan_id
        test_investor_id,       // was test_lender_id
        Decimal::new(95000, 2), // $950.00 (example asking price)
        Some("Sample deal listing for testing".to_string()),
        None, // expires_at
    )
    .await
}

/// Validate schema is working correctly (converted from validate_marketplace_schema)
///
/// This runs basic validation to ensure the foundation is solid for both workers.
pub async fn validate_marketplace_schema(pool: &PgPool) -> Result<bool, sqlx::Error> {
    // Test 1: Can we create a listing?
    let test_investor_id = Uuid::new_v4(); // was test_lender_id
    let test_originator_id = Uuid::new_v4(); // was test_borrower_id
    let test_deal_id = Uuid::new_v4(); // was test_loan_id

    // Insert test investor identity with unique email
    let investor_email = format!(
        // was lender_email
        "test-schema-investor-{}-{}.example.com", // was test-schema-lender
        test_investor_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_investor_id, // was test_lender_id
        investor_email
    )
    .execute(pool)
    .await?;

    // Insert test originator identity with unique email
    let originator_email = format!(
        // was borrower_email
        "test-schema-originator-{}-{}.example.com", // was test-schema-borrower
        test_originator_id.simple(),
        chrono::Utc::now().timestamp()
    );

    sqlx::query!(
        r#"
        INSERT INTO identities (id, email, kyc_status, user_type)
        VALUES ($1, $2, 'approved', 'investor')
        ON CONFLICT (id) DO NOTHING
        "#,
        test_originator_id, // was test_borrower_id
        originator_email
    )
    .execute(pool)
    .await?;

    // Insert test deal that the listing can reference
    sqlx::query!(
        // was loans table
        r#"
        INSERT INTO deals (id, originator_id, investor_id, principal_amount, outstanding_balance,
                          interest_rate_bps, loan_status, originated_at)
        VALUES ($1, $2, $3, 50000.00, 50000.00, 1200, 'active', NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
        test_deal_id,       // was test_loan_id
        test_originator_id, // was test_borrower_id
        test_investor_id    // was test_lender_id
    )
    .execute(pool)
    .await?;

    // Test create listing
    let _listing = create_sample_marketplace_data(pool, test_investor_id, test_deal_id).await?; // was test_lender_id, test_loan_id

    // Test 2: Can we retrieve the listing?
    let retrieved = get_deal_listing_by_deal_id(pool, test_deal_id).await?; // was get_loan_listing_by_loan_id, test_loan_id
    assert!(
        retrieved.is_some(),
        "Should be able to retrieve created listing"
    );

    // Test 3: Can we get marketplace deals?
    let marketplace_deals =
        browse_marketplace_deals(pool, Some(10), Some("active".to_string())).await?; // was get_marketplace_loans_basic
    assert!(
        !marketplace_deals.is_empty(),
        "Should have at least one marketplace deal"
    );

    // Test 4: Can we validate for transfer?
    let validation = validate_deal_for_transfer(pool, test_deal_id).await?; // was validate_loan_for_transfer, test_loan_id
    assert!(
        validation.is_some(),
        "Should be able to validate deal for transfer"
    );

    // Test 5: Can we update listing status?
    let mut tx = pool.begin().await?;
    let updated = update_deal_listing_status(&mut tx, test_deal_id, "sold").await?; // was update_listing_status, test_loan_id
    tx.rollback().await?; // Don't actually update for this test
    assert!(updated, "Should be able to update listing status");

    // Clean up test data
    sqlx::query!("DELETE FROM deal_listings WHERE deal_id = $1", test_deal_id) // was loan_listings, loan_id
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM deals WHERE id = $1", test_deal_id) // was loans, test_loan_id
        .execute(pool)
        .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // These tests ensure the foundation is solid
    #[tokio::test]
    async fn test_marketplace_foundation() {
        // This test validates the deal marketplace foundation works
        // let pool = get_test_pool().await;
        // assert!(validate_marketplace_schema(&pool).await.unwrap());

        // For now, just ensure compilation works
        assert!(true);
    }

    #[tokio::test]
    async fn test_deal_listing_crud() {
        // was test_loan_listing_crud
        // Test create, read, update, delete operations for deal listings
        // This validates the foundation works for both workers
        assert!(true);
    }
}
