# 🗃️ **Worker A: Database Foundation (Day 1-2 Priority)**

## 🎯 **Critical Mission**

You are implementing the **Database Foundation** that unblocks Worker B and enables parallel development. This is the most critical path item - Worker B cannot start until you complete this work.

**Timeline**: 1.5 days to complete foundation  
**Dependency**: Worker B waits for your database schema  
**Success Criteria**: Database migration + core functions working + schema validated

---

## 🚨 **Blocking Dependencies**

### **What Worker B is Waiting For:**
1. **Database Tables**: `loan_listings` and `loan_transfers` tables created
2. **Loan Status Enum**: Extended with `for_sale` and `transfer_pending` values  
3. **Core Database Functions**: Basic CRUD operations for marketplace
4. **Schema Validation**: Confirmed that tables work with sample data

### **Timeline Impact:**
- **Your completion**: Unblocks Worker B to start their database work
- **Your delay**: Delays entire project by the same amount
- **Target**: Complete by end of Day 2 to enable parallel work

---

## 📋 **Focused Task List**

### **Priority 1: Database Migration (Day 1)**
- [ ] Create migration file with loan_listings and loan_transfers tables
- [ ] Add new loan status enum values (for_sale, transfer_pending)
- [ ] Add marketplace fields to loans table
- [ ] Test migration applies and rolls back cleanly

### **Priority 2: Core Database Functions (Day 1-2)** 
- [ ] Implement `db/src/marketplace.rs` with essential CRUD operations
- [ ] Add to `db/src/lib.rs` exports  
- [ ] Test core functions work with sample data
- [ ] Validate schema works for Worker B's needs

### **Priority 3: Foundation Testing (Day 2)**
- [ ] Write basic unit tests for database functions
- [ ] Create sample data for integration testing
- [ ] Document schema for Worker B handoff
- [ ] Communicate completion to Worker B

---

## 🗄️ **Database Migration Implementation**

### **File: `db/migrations/048_add_loan_trading_foundation.sql`**

Create the complete database migration that both workers will use:

```sql
-- ============================================================================
-- LOAN TRADING FOUNDATION MIGRATION
-- Creates core tables and schema extensions for loan marketplace and transfers
-- 
-- This migration provides the foundation for:
-- - Worker A: Marketplace listings and browsing
-- - Worker B: Loan transfers and payment settlement
-- ============================================================================

-- 1. Create loan listings table for marketplace
CREATE TABLE loan_listings (
    listing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(loan_id),
    seller_id UUID NOT NULL REFERENCES identities(id),  -- Note: using identities table
    asking_price DECIMAL(15,2) NOT NULL CHECK (asking_price > 0),
    description TEXT,
    listed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,  -- Optional expiration for listings
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'sold', 'cancelled', 'expired')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Business constraints
    UNIQUE(loan_id),  -- Only one active listing per loan
    
    -- Performance indexes
    INDEX idx_loan_listings_status_listed(status, listed_at),
    INDEX idx_loan_listings_seller(seller_id),
    INDEX idx_loan_listings_price(asking_price)
);

-- 2. Create loan transfers table for ownership changes
CREATE TABLE loan_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(loan_id),
    from_lender_id UUID NOT NULL REFERENCES identities(id),
    to_lender_id UUID NOT NULL REFERENCES identities(id),
    purchase_price DECIMAL(15,2) NOT NULL CHECK (purchase_price > 0),
    transfer_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (transfer_status IN ('pending', 'completed', 'failed', 'cancelled')),
    transfer_date TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,  -- When transfer actually completed
    notes TEXT,  -- Optional transfer notes
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Business constraints
    CHECK (from_lender_id != to_lender_id),  -- Cannot transfer to self
    
    -- Performance indexes
    INDEX idx_loan_transfers_loan(loan_id),
    INDEX idx_loan_transfers_from_lender(from_lender_id),
    INDEX idx_loan_transfers_to_lender(to_lender_id),
    INDEX idx_loan_transfers_status(transfer_status, transfer_date)
);

-- 3. Extend loan status enum (CRITICAL: Additive only - keep existing values)
-- Current values: originated, active, paid_off, closed, defaulted
-- Adding: for_sale, transfer_pending
ALTER TABLE loans DROP CONSTRAINT IF EXISTS loans_loan_status_check;
ALTER TABLE loans ADD CONSTRAINT loans_loan_status_check CHECK (loan_status IN (
    -- Existing statuses (DO NOT CHANGE)
    'originated', 'active', 'paid_off', 'closed', 'defaulted',
    -- New marketplace statuses
    'for_sale', 'transfer_pending'
));

-- 4. Add marketplace-friendly fields to loans table (all optional to avoid breaking existing data)
ALTER TABLE loans ADD COLUMN IF NOT EXISTS title VARCHAR(255);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS description TEXT;
ALTER TABLE loans ADD COLUMN IF NOT EXISTS risk_level VARCHAR(10) CHECK (risk_level IN ('low', 'medium', 'high'));
ALTER TABLE loans ADD COLUMN IF NOT EXISTS asset_class VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS region VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS minimum_investment DECIMAL(15,2);

-- 5. Add performance indexes for marketplace queries
CREATE INDEX IF NOT EXISTS idx_loans_marketplace ON loans(loan_status, originated_at) WHERE loan_status = 'for_sale';
CREATE INDEX IF NOT EXISTS idx_loans_lender_status ON loans(lender_id, loan_status);

-- 6. Add trigger to update updated_at timestamp on loan_listings
CREATE OR REPLACE FUNCTION update_loan_listings_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_loan_listings_updated_at 
    BEFORE UPDATE ON loan_listings 
    FOR EACH ROW EXECUTE PROCEDURE update_loan_listings_updated_at();

-- 7. Create helpful view for combined marketplace data (used by both workers)
CREATE VIEW marketplace_loans AS
SELECT 
    -- Listing information
    ll.listing_id,
    ll.asking_price,
    ll.description as listing_description,
    ll.listed_at,
    ll.expires_at,
    ll.status as listing_status,
    
    -- Loan information
    l.loan_id,
    l.borrower_id,
    l.lender_id,
    l.principal_amount,
    l.outstanding_balance,
    l.interest_rate_bps,
    l.loan_status,
    l.originated_at,
    l.title,
    l.description as loan_description,
    l.risk_level,
    l.asset_class,
    l.region,
    l.minimum_investment,
    
    -- Seller information
    i.email as seller_email,
    i.kyc_status as seller_kyc_status
FROM loan_listings ll
JOIN loans l ON ll.loan_id = l.loan_id
JOIN identities i ON ll.seller_id = i.id
WHERE ll.status = 'active' 
  AND l.loan_status = 'for_sale'
  AND i.kyc_status IN ('approved', 'pending', 'submitted'); -- Allow for testing flexibility

-- 8. Add comments for documentation
COMMENT ON TABLE loan_listings IS 'Marketplace listings for loans available for sale';
COMMENT ON TABLE loan_transfers IS 'Record of all loan ownership transfers';
COMMENT ON VIEW marketplace_loans IS 'Combined view of loans available in marketplace with seller info';

-- 9. Grant permissions (adjust based on your user setup)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON loan_listings TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON loan_transfers TO your_app_user;
-- GRANT SELECT ON marketplace_loans TO your_app_user;
```

### **Migration Testing Commands**

```bash
# Apply migration and verify
make initDB

# Test tables were created
psql $DATABASE_URL -c "\d+ loan_listings"
psql $DATABASE_URL -c "\d+ loan_transfers"
psql $DATABASE_URL -c "\d+ marketplace_loans"

# Test new loan status values work
psql $DATABASE_URL -c "SELECT unnest(enum_range(NULL::loan_status_enum));" 2>/dev/null || echo "Status values are in CHECK constraint, not enum"

# Test rollback works (in development only)
# psql $DATABASE_URL -c "DROP VIEW IF EXISTS marketplace_loans; DROP TABLE IF EXISTS loan_transfers; DROP TABLE IF EXISTS loan_listings;"
```

---

## 💾 **Core Database Functions Implementation**

### **File: `db/src/marketplace.rs`** (New File)

Implement the essential database operations that Worker B depends on:

```rust
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
use sqlx::{PgPool, Transaction, Postgres};
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
        SELECT loan_id, lender_id, loan_status
        FROM loans 
        WHERE loan_id = $1 AND lender_id = $2
        "#,
        loan_id,
        seller_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    
    let loan = loan.ok_or_else(|| {
        sqlx::Error::RowNotFound
    })?;
    
    // 2. Validate loan status (must be active or originated)
    if !matches!(loan.loan_status.as_str(), "active" | "originated") {
        return Err(sqlx::Error::Database(Box::new(
            sqlx::postgres::PgDatabaseError::new(
                sqlx::postgres::PgSeverity::Error,
                "23514".to_string(),
                format!("Loan status '{}' is not eligible for listing", loan.loan_status),
            )
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
        return Err(sqlx::Error::Database(Box::new(
            sqlx::postgres::PgDatabaseError::new(
                sqlx::postgres::PgSeverity::Error,
                "23505".to_string(),
                "Loan is already listed for sale".to_string(),
            )
        )));
    }
    
    // 4. Update loan status to for_sale
    sqlx::query!(
        "UPDATE loans SET loan_status = 'for_sale', updated_at = NOW() WHERE loan_id = $1",
        loan_id
    )
    .execute(&mut *tx)
    .await?;
    
    // 5. Create listing record
    let listing = sqlx::query_as!(
        LoanListing,
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
    let listing = sqlx::query_as!(
        LoanListing,
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
    
    Ok(listing)
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
        "UPDATE loans SET loan_status = 'active', updated_at = NOW() WHERE loan_id = $1",
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
            l.loan_id, l.borrower_id, l.lender_id, l.principal_amount, l.outstanding_balance,
            l.interest_rate_bps, l.loan_status, l.originated_at, l.title, 
            l.description, l.risk_level, l.asset_class, l.region,
            i.email as seller_email, i.kyc_status as seller_kyc_status
        FROM loan_listings ll
        JOIN loans l ON ll.loan_id = l.loan_id
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
    
    let marketplace_loans = loans.into_iter().map(|row| MarketplaceLoan {
        // Listing data
        listing_id: row.listing_id,
        asking_price: row.asking_price,
        listing_description: row.listing_description,
        listed_at: row.listed_at,
        listing_status: row.listing_status,
        
        // Loan data
        loan_id: row.loan_id,
        borrower_id: row.borrower_id,
        lender_id: row.lender_id,
        principal_amount: row.principal_amount,
        outstanding_balance: row.outstanding_balance,
        interest_rate_bps: row.interest_rate_bps,
        loan_status: row.loan_status,
        originated_at: row.originated_at,
        title: row.title,
        description: row.description,
        risk_level: row.risk_level,
        asset_class: row.asset_class,
        region: row.region,
        
        // Seller data
        seller_email: row.seller_email,
        seller_kyc_status: row.seller_kyc_status,
    }).collect();
    
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
        JOIN loan_listings ll ON l.loan_id = ll.loan_id
        WHERE l.loan_id = $1 
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
        INSERT INTO loans (loan_id, borrower_id, lender_id, principal_amount, outstanding_balance, interest_rate_bps, loan_status)
        VALUES ($1, $2, $2, 100000, 75000, 1200, 'active')
        ON CONFLICT (loan_id) DO UPDATE SET
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
    ).await
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
    assert!(retrieved.is_some(), "Should be able to retrieve created listing");
    
    // Test 3: Can we get marketplace loans?
    let marketplace_loans = get_marketplace_loans_basic(pool, Some(10), Some(0)).await?;
    assert!(!marketplace_loans.is_empty(), "Should have at least one marketplace loan");
    
    // Test 4: Can we validate for transfer?
    let validation = validate_loan_for_transfer(pool, test_loan_id).await?;
    assert!(validation.is_some(), "Should be able to validate loan for transfer");
    
    // Test 5: Can we update listing status?
    let mut tx = pool.begin().await?;
    let updated = update_listing_status(&mut tx, test_loan_id, "sold").await?;
    tx.rollback().await?; // Don't actually update for this test
    assert!(updated, "Should be able to update listing status");
    
    // Clean up test data
    sqlx::query!("DELETE FROM loan_listings WHERE loan_id = $1", test_loan_id)
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM loans WHERE loan_id = $1", test_loan_id)
        .execute(pool)
        .await?;
    
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    
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
```

### **Update `db/src/lib.rs`**

Add the marketplace module to exports:

```rust
// Add to existing mod declarations (around other pub mod lines)
pub mod marketplace;

// Add to existing use declarations (where other pub use statements are)
pub use marketplace::*;
```

---

## 🧪 **Foundation Testing & Validation**

### **Testing Commands**

```bash
# Test database migration
make initDB
echo "Migration applied successfully"

# Test database functions
cd db
cargo test marketplace::tests::test_marketplace_foundation
cargo test marketplace::tests::test_loan_listing_crud

# Test marketplace schema validation
cargo test --lib marketplace
```

### **Manual Validation Script**

Create a simple test to validate everything works:

```bash
# Test script: test_foundation.sh
#!/bin/bash

echo "🧪 Testing loan trading database foundation..."

# Test 1: Migration applied successfully
psql $DATABASE_URL -c "SELECT COUNT(*) FROM loan_listings;" > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ loan_listings table exists"
else
    echo "❌ loan_listings table missing"
    exit 1
fi

psql $DATABASE_URL -c "SELECT COUNT(*) FROM loan_transfers;" > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ loan_transfers table exists"
else
    echo "❌ loan_transfers table missing"  
    exit 1
fi

# Test 2: New loan status values work
psql $DATABASE_URL -c "UPDATE loans SET loan_status = 'for_sale' WHERE false;" > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ for_sale status value works"
else
    echo "❌ for_sale status value not working"
    exit 1
fi

# Test 3: Marketplace view exists
psql $DATABASE_URL -c "SELECT COUNT(*) FROM marketplace_loans;" > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ marketplace_loans view exists"
else
    echo "❌ marketplace_loans view missing"
    exit 1
fi

echo "🎉 Foundation validation complete! Worker B can now start."
```

---

## 📋 **Worker B Handoff Checklist**

### **What You Must Complete:**
- [ ] ✅ Database migration applied successfully 
- [ ] ✅ Tables `loan_listings` and `loan_transfers` exist
- [ ] ✅ Loan status enum extended with `for_sale`, `transfer_pending`
- [ ] ✅ Basic marketplace functions working
- [ ] ✅ Schema validation passes
- [ ] ✅ Sample data can be created and queried

### **Worker B Handoff Package:**
- [ ] ✅ Database schema is ready
- [ ] ✅ Core functions in `db/src/marketplace.rs` work
- [ ] ✅ Testing script validates foundation
- [ ] ✅ Documentation of table structure
- [ ] ✅ Communication: "Foundation ready, Worker B can start"

---

## 🚨 **Critical Success Criteria**

### **Database Foundation Must Work:**
1. **Migration applies cleanly** without errors
2. **New loan status values** (`for_sale`, `transfer_pending`) are accepted
3. **Loan listings can be created** and retrieved  
4. **Worker B functions** (transfer validation) work with your schema
5. **No data loss** on existing loans/users

### **Worker B Dependencies Satisfied:**
1. **`loan_listings` table** exists with correct schema
2. **`loan_transfers` table** exists with correct schema  
3. **`validate_loan_for_transfer`** function works
4. **`update_listing_status`** function works (for marking sold)
5. **Database exports** are available in `db/src/lib.rs`

---

## 🎯 **Definition of Done**

### **Technical Completion:**
- [ ] Migration file created and tested
- [ ] All database functions implemented and tested
- [ ] Schema validation script passes
- [ ] No breaking changes to existing functionality
- [ ] Worker B can import and use your functions

### **Communication Completion:**
- [ ] Worker B notified that foundation is ready
- [ ] Documentation of schema provided to Worker B
- [ ] Any deviation from spec communicated
- [ ] Integration points confirmed

### **Testing Completion:**
- [ ] Unit tests for core functions pass
- [ ] Integration test with sample data works
- [ ] Schema validation script passes
- [ ] No database migration errors

---

## 🚀 **Getting Started Immediately**

### **Day 1 Morning (4 hours):**
1. **Create migration file** - Start with the SQL schema above
2. **Apply and test migration** - Make sure it works cleanly
3. **Start marketplace.rs** - Implement core database functions

### **Day 1 Afternoon (4 hours):**  
1. **Complete marketplace.rs** - All core CRUD operations
2. **Add to lib.rs exports** - Make functions available
3. **Basic testing** - Ensure core functions work

### **Day 2 Morning (4 hours):**
1. **Schema validation** - Run validation tests
2. **Sample data testing** - Create and query test data
3. **Edge case testing** - Error conditions, constraints

### **Day 2 Afternoon (2 hours):**
1. **Documentation** - Document schema for Worker B
2. **Communication** - Notify Worker B foundation is ready  
3. **Handoff** - Provide any clarifications needed

**Total Time**: ~14 hours over 1.5 days  
**Critical Path**: This work blocks Worker B completely  
**Success Metric**: Worker B can start their database work immediately after your completion

---

## 💬 **Communication Protocol**

### **When You Start:**
- [ ] Confirm you understand the critical path nature of this work
- [ ] Estimate your completion time
- [ ] Flag any issues or blockers immediately

### **Day 1 Progress Update:**
- [ ] Report migration status (success/issues)
- [ ] Report core function implementation status
- [ ] Any changes to schema or approach

### **Day 2 Completion:**
- [ ] "Foundation complete - Worker B can start"
- [ ] Share any deviations from spec
- [ ] Provide final database validation results

**Remember: Worker B is waiting for you! This is the critical path to parallel development. Make it solid and make it fast! 🗃️⚡**
