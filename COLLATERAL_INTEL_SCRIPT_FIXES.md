# Test Collateral Intel Script Fixes

## Issue Summary
The `test_collateral_intel.rs` script was failing after ZK updates and state DB schema changes (see Linear issue ARDGBL-990).

## Root Causes Identified

### 1. API Schema Mismatch
The script's `CreateDealRequest` struct didn't match the actual API endpoint schema after recent updates.

**Problem:**
- Missing optional fields like `originator_name`, `investor_name`, etc.
- Field naming mismatch: used `deal_status` instead of `loan_status`
- Missing marketplace fields: `risk_level`, `asset_class`, `minimum_investment`, etc.

### 2. Database Schema Updates
The `collateral_matches` table was recreated in migration 061, but the script had an outdated comment saying it was removed.

**Problem:**
- Function `get_collateral_matches_from_db` returned hardcoded `0` instead of querying the database
- Comment incorrectly stated table was "removed in migration 060"

## Fixes Applied

### Fix 1: Updated CreateDealRequest Struct
**File:** `scripts/test_collateral_intel.rs`

**Changes:**
```rust
#[derive(Debug, Serialize, Deserialize)]
struct CreateDealRequest {
    originator_id: String,
    originator_name: Option<String>,          // ADDED
    investor_id: Option<String>,
    investor_name: Option<String>,            // ADDED
    borrower_id: Option<String>,
    borrower_name: Option<String>,            // ADDED
    principal_amount: String,
    interest_rate_bps: i32,
    term_days: Option<u32>,                   // ADDED
    loan_status: Option<String>,              // RENAMED from deal_status
    status: Option<String>,                   // Now explicitly negotiation status
    metadata: Option<serde_json::Value>,      // ADDED
    custom_deal_id: Option<String>,           // ADDED
    industry: Option<String>,                 // ADDED
    document_ids: Option<Vec<String>>,        // ADDED
    title: Option<String>,
    description: Option<String>,
    risk_level: Option<String>,               // ADDED
    asset_class: Option<String>,              // ADDED
    region: Option<String>,                   // ADDED
    minimum_investment: Option<String>,       // ADDED
    collateral_id: Option<String>,
}
```

**Impact:**
- API calls in `create_deal_via_api()` now match the actual endpoint
- Prevents 400 Bad Request errors from API
- Supports new marketplace and company auto-creation features

### Fix 2: Updated create_deal_via_api Function
**File:** `scripts/test_collateral_intel.rs`

**Changes:**
```rust
let request = CreateDealRequest {
    originator_id: originator_id.to_string(),
    originator_name: None,
    investor_id: investor_id.map(|id| id.to_string()),
    investor_name: None,
    borrower_id: None,
    borrower_name: None,
    principal_amount: "1000000.00".to_string(),
    interest_rate_bps: 500,
    term_days: Some(365),
    loan_status: Some(deal_status.to_string()),  // Payment status
    status: Some("active".to_string()),          // Negotiation status
    metadata: None,
    custom_deal_id: None,
    industry: None,
    document_ids: None,
    title: Some(title.to_string()),
    description: Some(format!("Test deal: {}", title)),
    risk_level: None,
    asset_class: None,
    region: None,
    minimum_investment: None,
    collateral_id: collateral_id.map(|id| id.to_string()),
};
```

### Fix 3: Restored get_collateral_matches_from_db Function
**File:** `scripts/test_collateral_intel.rs`

**Changes:**
```rust
#[allow(dead_code)]
async fn get_collateral_matches_from_db(
    ctx: &TestContext,
    collateral_id: Uuid,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Note: collateral_matches table was recreated in migration 061
    // It stores detection results received from arda-collateral-intel microservice
    let result = sqlx::query!(
        "SELECT COUNT(*) as count FROM collateral_matches WHERE collateral_id = $1",
        collateral_id
    )
    .fetch_one(&ctx.pool)
    .await?;
    
    Ok(result.count.unwrap_or(0) as usize)
}
```

**Impact:**
- Function now correctly queries the recreated `collateral_matches` table
- Matches are properly counted instead of returning hardcoded `0`
- Comment updated to reflect current schema (migration 061)

## Database Schema Context

### collateral_matches Table (Migration 061)
```sql
CREATE TABLE collateral_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    matched_collateral_id UUID NOT NULL REFERENCES collateral(id) ON DELETE CASCADE,
    match_score DECIMAL(5, 2) NOT NULL CHECK (match_score >= 0 AND match_score <= 100),
    match_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(collateral_id, matched_collateral_id),
    CHECK (
        collateral_id != matched_collateral_id 
        OR match_type IN ('self_double_pledge', 'exact_name')
    )
);
```

**Key Points:**
- Table stores detection results from `arda-collateral-intel` microservice
- Allows self-referential matches for double-pledge detection
- Match types: `exact_name`, `custom_id`, `address`, `fuzzy_name`, `semantic`, `self_double_pledge`

### state_transactions Table (Migration 079)
The script does NOT directly reference the old transaction tables, so no changes were needed for the transaction table consolidation. All queries properly use the current schema.

## Verified Items

✅ No references to old `deal_transactions` table  
✅ No references to old `incoming_transactions` table  
✅ All database queries use current schema  
✅ API request structure matches current endpoint  
✅ Comments updated to reflect current state  

## Testing Status

**⚠️ BLOCKED:** Cannot fully test due to build environment requirements:
- Requires `cargo-prove` command (SP1 toolchain)
- Requires SP1 program binary at `build/program`
- Requires C++ build environment for `sp1-core-machine` crate

**To complete testing:**
```bash
# 1. Install SP1 toolchain
curl -L https://sp1.succinct.xyz | bash
source ~/.bashrc

# 2. Build SP1 program
make build-program

# 3. Run the test script
cargo run --bin test_collateral_intel -- --help
```

## Expected Behavior After Fixes

When the script runs successfully, it should:

1. ✅ Create deals via API without 400 errors
2. ✅ Properly query `collateral_matches` table
3. ✅ Detect same-user double pledges
4. ✅ Detect cross-user double pledges
5. ✅ Handle paid-off deals correctly
6. ✅ Process batch uploads
7. ✅ Query queue status from Intel service

## Related Documentation

- **Transaction Consolidation:** `/workspace/docs/architecture/transaction-table-consolidation.md`
- **API Endpoints:** `http://0.0.0.0:8080/swagger-ui/` (when server is running)
- **Database Migrations:**
  - `db/migrations/061_recreate_collateral_matches.sql`
  - `db/migrations/063_allow_self_referential_collateral_matches.sql`
  - `db/migrations/079_consolidate_transaction_tables.sql`

## Summary

All code-level fixes have been applied to `scripts/test_collateral_intel.rs`:
- ✅ API schema updated to match current endpoint
- ✅ Database queries updated for current schema
- ✅ Comments corrected
- ⏳ Testing blocked by SP1 build environment setup

The script is now aligned with the ZK updates and state DB schema changes.
