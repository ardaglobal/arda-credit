# Work Stream 6: Test Data & Scripts Update

**Epic:** EPIC-6  
**Priority:** MEDIUM (Can start early, incremental updates)  
**Effort:** 1 day (8 hours total, spread across 3-4 weeks)  
**Owner:** DevOps/Backend Engineer #3  
**Dependencies:** None - can start immediately  

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Requirements](#technical-requirements)
3. [Current Coverage Analysis](#current-coverage-analysis)
4. [Sub-Issue Implementation Guides](#sub-issue-implementation-guides)
5. [Testing & Validation](#testing--validation)
6. [Incremental Rollout Strategy](#incremental-rollout-strategy)
7. [Integration with Other Work Streams](#integration-with-other-work-streams)

---

## Overview

### Purpose

Update the `populate_deal_data.rs` test script to create transaction records for all 10 in-scope transaction types, achieving 100% test coverage. This enables realistic testing of the zkVM implementation and validates that the complete system works end-to-end.

### Context

**Current State:**
- ✅ Script creates deals in database
- ✅ Script creates 3/10 transaction types (30% coverage):
  - DealOrigination (lines 637-646)
  - DealPayment (lines 667-676, 703-712, 739-748)
  - DealCloseout (lines 753-762)
- ❌ Script does NOT create UserRegistration transactions
- ❌ Script does NOT create Collateral transactions
- ⚠️ ~70% of deals have collateral but no transaction records

**Target State:**
- ✅ Script creates 10/10 transaction types (100% coverage)
- ✅ User creation generates UserRegistration transactions
- ✅ Collateral creation generates CollateralCreation transactions
- ✅ Collateral-deal linking generates CollateralDealAssociation transactions
- ✅ Optional: CollateralUpdate, disassociation, detection transactions
- ✅ Script enables complete zkVM testing

### Scope (10 Transaction Types)

**Priority 1 (HIGH) - Required for basic testing:**
1. UserRegistration (#1) - Missing
2. DealOrigination (#4) - ✅ Exists
3. DealPayment (#5) - ✅ Exists
4. DealCloseout (#6) - ✅ Exists
5. CollateralCreation (#20) - Missing
6. CollateralDealAssociation (#22) - Missing

**Priority 2 (MEDIUM) - Nice to have:**
7. CollateralUpdate (#21) - Missing
8. CollateralDealDisassociation (#23) - Missing

**Priority 3 (LOW) - Can defer:**
9. CollateralDetectionRequest (#24) - Missing
10. CollateralDetectionUpdate (#25) - Missing

---

## Technical Requirements

### Functional Requirements

**FR-1: Use Existing Helper Function**
- Script already has `create_deal_transaction_record()` function (line 577)
- Reuse this function for all transaction types
- Don't create new database insert logic
- Keep consistent with existing pattern

**FR-2: Realistic Timestamps**
- Transactions should have realistic timestamps
- User registration before deal creation
- Collateral creation before or during deal creation
- Association after both collateral and deal exist
- Temporal ordering must be valid

**FR-3: Consistent Data**
- Transaction amounts must match business records
- Deal IDs must match between deals and transactions
- Collateral IDs must match
- User IDs must match

**FR-4: Optional Features**
- Update and disassociation should be optional (flags)
- Detection events should be optional (flags)
- Core features (Priority 1) should always run

### Non-Functional Requirements

**NFR-1: Backward Compatibility**
- Script must continue to work with existing flags
- `--count`, `--email`, `--clear`, `--csv` flags unchanged
- Only add new functionality, don't break existing

**NFR-2: Performance**
- Script should complete in reasonable time (<2 minutes for 25 deals)
- Batch transaction inserts if possible
- Don't significantly slow down existing script

**NFR-3: Maintainability**
- Clear code organization
- Functions follow existing patterns
- Easy to add new transaction types in future

---

## Current Coverage Analysis

### Existing Implementation

**File:** `scripts/populate_deal_data.rs`

**Function:** `create_deal_transaction_record()` (lines 577-621)

```rust
async fn create_deal_transaction_record(
    pool: &sqlx::PgPool,
    transaction_type: &str,
    deal_id: Uuid,
    user_id: Option<Uuid>,
    amount: Decimal,
    created_at: chrono::DateTime<Utc>,
    payment_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    let transaction_id = Uuid::new_v4();
    let transaction_id_hash = hash_uuid(&transaction_id);
    let dummy_uuid = Uuid::nil();
    let user_id_hash = hash_uuid(&user_id.unwrap_or(dummy_uuid));
    let deal_id_hash = hash_uuid(&deal_id);

    sqlx::query(
        r#"
        INSERT INTO deal_transactions (
            id, transaction_id_hash, user_id_hash, deal_id_hash,
            transaction_type, amount, user_id, deal_id, payment_id,
            metadata, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(transaction_id)
    .bind(transaction_id_hash)
    .bind(user_id_hash)
    .bind(deal_id_hash)
    .bind(transaction_type)
    .bind(amount)
    .bind(user_id)
    .bind(deal_id)
    .bind(payment_id)
    .bind(json!({
        "created_by_script": true,
        "transaction_type": transaction_type
    }))
    .bind(created_at)
    .execute(pool)
    .await?;

    Ok(())
}
```

**Good news:** This function already exists and works! We just need to call it in more places.

### Coverage Gap Summary

| Transaction Type | Created? | Where | Priority | Effort |
|------------------|----------|-------|----------|--------|
| UserRegistration | ❌ No | Need to add | HIGH | 2-3 hours |
| DealOrigination | ✅ Yes | Line 637 | N/A | Done |
| DealPayment | ✅ Yes | Lines 667, 703, 739 | N/A | Done |
| DealCloseout | ✅ Yes | Line 753 | N/A | Done |
| CollateralCreation | ❌ No | Need to add | HIGH | 1-2 hours |
| CollateralDealAssociation | ❌ No | Need to add | HIGH | 1-2 hours |
| CollateralUpdate | ❌ No | Need to add | MEDIUM | 2-3 hours |
| CollateralDealDisassociation | ❌ No | Need to add | MEDIUM | 2-3 hours |
| CollateralDetectionRequest | ❌ No | Need to add | LOW | 2-3 hours |
| CollateralDetectionUpdate | ❌ No | Need to add | LOW | 2-3 hours |

---

## Sub-Issue Implementation Guides

## EPIC-6.1: Add UserRegistration Transactions

**Effort:** 2-3 hours  
**Dependencies:** None - can start immediately  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate User Creation Points

**The script creates users in two places:**

**Location 1:** `ensure_user_companies()` function (around line 1700)
- Creates investor and originator companies for a user
- User already exists at this point (fetched earlier)

**Location 2:** `generate_user_deals()` function (around line 1350-1360)
- Creates user if doesn't exist

#### Step 2: Add Transaction Recording After User Creation

**Location 1: Update `ensure_user_companies()`**

**File:** `scripts/populate_deal_data.rs`  
**Function:** `ensure_user_companies()` (line ~1700)

**FIND (around line 1705):**
```rust
async fn ensure_user_companies(
    pool: &sqlx::PgPool,
    user: &User,
) -> Result<(Company, Company), Box<dyn std::error::Error>> {
    let user_name = user.email.split('@').next().unwrap_or("User");
```

**ADD after line 1726 (after investor company created):**

```rust
    // Create company membership for investor
    create_membership(
        pool,
        investor_company.id,
        user.id,
        CompanyRole::Admin,
        ProfileType::Investor,
        None,
        None,
    )
    .await?;

    // NEW: Record user registration transaction for zkVM
    // Check if user was recently created (within last minute) to avoid duplicates
    let user_age = (Utc::now() - user.created_at).num_seconds();
    if user_age < 60 {
        println!("  📝 Recording UserRegistration transaction for {}", user.email);
        create_deal_transaction_record(
            pool,
            "user_registration",
            Uuid::nil(), // No deal_id for user registration
            Some(user.id),
            Decimal::ZERO,
            user.created_at, // Use user's actual creation time
            None,
        )
        .await?;
    }
```

**Location 2: Update `generate_user_deals()`**

**FIND (around line 1355):**
```rust
    None => {
        println!("👤 User {} not found, creating new user...", email);
        let user = create_identity(pool, "investor", email).await?;
        println!("✅ Created new user: {} ({})", user.email, user.id);
        user
    }
```

**REPLACE with:**
```rust
    None => {
        println!("👤 User {} not found, creating new user...", email);
        let user = create_identity(pool, "investor", email).await?;
        println!("✅ Created new user: {} ({})", user.email, user.id);
        
        // NEW: Record user registration transaction
        println!("  📝 Recording UserRegistration transaction");
        create_deal_transaction_record(
            pool,
            "user_registration",
            Uuid::nil(),
            Some(user.id),
            Decimal::ZERO,
            user.created_at,
            None,
        )
        .await?;
        
        user
    }
```

#### Step 3: Test the Changes

**Run script:**
```bash
# Clear database
cargo run --bin populate_deal_data -- --clear

# Generate data for new user
cargo run --bin populate_deal_data -- --email newuser@test.com --count 5

# Verify UserRegistration transaction created
psql $DATABASE_URL -c "
  SELECT 
    t.transaction_type,
    t.created_at,
    u.email
  FROM deal_transactions t
  JOIN users u ON u.id = t.user_id
  WHERE t.transaction_type = 'user_registration'
  ORDER BY t.created_at DESC;
"
```

**Expected output:**
```
transaction_type    | created_at           | email
--------------------+----------------------+-------------------
user_registration   | 2024-12-12 10:30:00  | newuser@test.com
```

**Validation:**
- [ ] UserRegistration transaction exists for each new user
- [ ] Transaction timestamp matches user.created_at
- [ ] Transaction has correct user_id
- [ ] Script doesn't create duplicates for existing users

**Effort:** 2-3 hours for EPIC-6.1

---

## EPIC-6.2: Add CollateralCreation Transactions

**Effort:** 1-2 hours  
**Dependencies:** None - can run parallel with EPIC-6.1  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate Collateral Creation

**File:** `scripts/populate_deal_data.rs`  
**Function:** `create_random_collateral()` (lines 1787-1844)

**Current code:**
```rust
async fn create_random_collateral(
    pool: &sqlx::PgPool,
    rng: &mut impl Rng,
    force_duplicate: bool,
    duplicate_index: Option<usize>,
) -> Result<Collateral, Box<dyn std::error::Error>> {
    // ... generates collateral data ...
    
    let collateral = create_collateral(pool, request).await?;
    
    // Problem: No transaction record!
    Ok(collateral)
}
```

#### Step 2: Add Transaction Recording

**INSERT before `Ok(collateral)` (after line 1843):**

```rust
    let collateral = create_collateral(pool, request).await?;
    
    // NEW: Record collateral creation transaction for zkVM
    println!("  📝 Recording CollateralCreation transaction for {}", collateral.id);
    create_deal_transaction_record(
        pool,
        "collateral_creation",
        Uuid::nil(), // No deal_id yet (not associated)
        None,        // No specific user_id for script-generated collateral
        Decimal::ZERO,
        collateral.created_at,
        None,
    )
    .await?;
    
    Ok(collateral)
```

#### Step 3: Test the Changes

**Run script:**
```bash
# Generate deals with collateral
cargo run --bin populate_deal_data -- --count 10

# Verify CollateralCreation transactions
psql $DATABASE_URL -c "
  SELECT 
    COUNT(*) as creation_count,
    COUNT(DISTINCT metadata->>'created_by_script') as from_script
  FROM deal_transactions 
  WHERE transaction_type = 'collateral_creation';
"
```

**Expected:**
- ~7 collateral creation transactions (70% of 10 deals have collateral)
- All from script (metadata has "created_by_script": true)

**Validation:**
- [ ] CollateralCreation transactions created
- [ ] Count matches number of collateral items created (~70% of deals)
- [ ] Timestamps match collateral.created_at
- [ ] Script output shows transaction recording

**Effort:** 1-2 hours for EPIC-6.2

---

## EPIC-6.3: Add CollateralDealAssociation Transactions

**Effort:** 1-2 hours  
**Dependencies:** EPIC-6.2 (should run after, but not blocking)  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate Deal Creation with Collateral

**File:** `scripts/populate_deal_data.rs`  
**Function:** `create_realistic_deal()` (lines 1848-1980)

**Current code (around line 1920):**
```rust
    let deal = create_deal_with_custom_dates(
        pool,
        originator_id,
        investor_id,
        principal_amount,
        // ... parameters ...
        collateral.as_ref().map(|c| c.id), // ← Collateral association happens here
    )
    .await?;
    
    // ... update outstanding balance ...
    
    // Problem: No association transaction recorded!
    
    Ok((deal, collateral))
}
```

#### Step 2: Add Transaction Recording After Association

**INSERT after deal creation, before outstanding balance update (around line 1949):**

```rust
    let deal = create_deal_with_custom_dates(
        pool,
        originator_id,
        investor_id,
        principal_amount,
        interest_rate_bps,
        maturity_date,
        Some(selected_status),
        None,
        Some("active"),
        Some(INDUSTRIES.choose(&mut rng).unwrap()),
        Some(&json!([])),
        Some(title_prefix),
        Some(&format!(
            "{} deal in {} sector with {} risk profile. Principal: ${}, Rate: {}% APR",
            ASSET_CLASSES.choose(&mut rng).unwrap(),
            INDUSTRIES.choose(&mut rng).unwrap(),
            selected_risk,
            principal_amount,
            (interest_rate_bps as f64) / 100.0
        )),
        Some(selected_risk),
        Some(ASSET_CLASSES.choose(&mut rng).unwrap()),
        Some(REGIONS.choose(&mut rng).unwrap()),
        None,
        originated_at,
        originated_at,
        collateral.as_ref().map(|c| c.id), // Collateral association
    )
    .await?;

    // NEW: If collateral was associated, record transaction
    if let Some(coll) = &collateral {
        println!("  📝 Recording CollateralDealAssociation transaction");
        create_deal_transaction_record(
            pool,
            "collateral_deal_association",
            deal.id,  // Association is about this deal
            None,     // No specific user for script-generated deals
            Decimal::ZERO,
            deal.created_at, // Association happens at deal creation time
            None,
        )
        .await?;
    }

    // Update outstanding balance if different from principal
    if outstanding_balance != principal_amount {
        sqlx::query!(
            "UPDATE deals SET outstanding_balance = $1 WHERE id = $2",
            outstanding_balance,
            deal.id
        )
        .execute(pool)
        .await?;
    }
```

#### Step 3: Test the Changes

**Run script:**
```bash
# Generate deals with collateral
cargo run --bin populate_deal_data -- --count 10

# Verify association transactions
psql $DATABASE_URL -c "
  SELECT 
    COUNT(*) as association_count,
    COUNT(DISTINCT deal_id) as unique_deals
  FROM deal_transactions 
  WHERE transaction_type = 'collateral_deal_association';
"

# Should match collateral creation count
psql $DATABASE_URL -c "
  SELECT 
    (SELECT COUNT(*) FROM deal_transactions WHERE transaction_type = 'collateral_creation') as creations,
    (SELECT COUNT(*) FROM deal_transactions WHERE transaction_type = 'collateral_deal_association') as associations;
"
```

**Expected:** Creation count = Association count (both ~7 for 10 deals)

**Validation:**
- [ ] Association transactions created
- [ ] Count matches collateral creations
- [ ] deal_id field populated correctly
- [ ] Timestamps match deal creation

**Effort:** 1-2 hours for EPIC-6.3

---

## EPIC-6.4: Add CollateralUpdate Function (Optional)

**Effort:** 2-3 hours  
**Dependencies:** EPIC-6.2 complete  
**Priority:** MEDIUM  

### Implementation Steps

#### Step 1: Create Update Function

**File:** `scripts/populate_deal_data.rs`

**Add new function (insert around line 1980, after `create_realistic_deal`):**

```rust
/// Update existing collateral and record transaction
async fn update_random_collateral(
    pool: &sqlx::PgPool,
    collateral: &Collateral,
    rng: &mut impl Rng,
) -> Result<(), Box<dyn std::error::Error>> {
    use db::{update_collateral, UpdateCollateralRequest};
    
    // Randomly update some fields
    let new_name = if rng.gen_bool(0.5) {
        Some(format!("{} (Updated)", collateral.name))
    } else {
        None
    };
    
    let new_type = if rng.gen_bool(0.3) {
        Some(COLLATERAL_TYPES.choose(rng).unwrap().to_string())
    } else {
        None
    };
    
    let new_description = if rng.gen_bool(0.4) {
        Some(format!("Updated: {}", chrono::Utc::now().format("%Y-%m-%d")))
    } else {
        None
    };
    
    // Only update if there's something to update
    if new_name.is_none() && new_type.is_none() && new_description.is_none() {
        return Ok(()); // Nothing to update
    }
    
    // Perform update in database
    let update_req = UpdateCollateralRequest {
        name: new_name.clone(),
        collateral_type: new_type.clone(),
        description: new_description.clone(),
        address: None,
        custom_id: None,
        metadata: None,
    };
    
    update_collateral(pool, collateral.id, update_req).await?;
    
    // Record update transaction for zkVM
    println!("  📝 Recording CollateralUpdate transaction for {}", collateral.id);
    create_deal_transaction_record(
        pool,
        "collateral_update",
        Uuid::nil(), // No deal_id for update
        None,        // No user for script updates
        Decimal::ZERO,
        chrono::Utc::now(),
        None,
    )
    .await?;
    
    println!("  ✅ Updated collateral: {}", collateral.id);
    
    Ok(())
}
```

#### Step 2: Call Update Function from Main Flow

**Option A: Update a percentage of created collateral**

**In `generate_user_deals()` after creating all deals (around line 1500):**

```rust
    println!("✅ Created {} deals for user {}", count, email);
    
    // NEW: Randomly update 30% of created collateral
    if let Ok(all_collateral) = db::list_collateral(pool, Some(1000), None, None, None).await {
        let collateral_to_update: Vec<_> = all_collateral.collateral
            .iter()
            .filter(|_| rng.gen_bool(0.3))
            .take(5) // Update max 5 items
            .collect();
        
        for coll in collateral_to_update {
            if let Err(e) = update_random_collateral(pool, coll, &mut rng).await {
                eprintln!("⚠️  Failed to update collateral {}: {}", coll.id, e);
            }
        }
        
        if !collateral_to_update.is_empty() {
            println!("  📊 Updated {} collateral items", collateral_to_update.len());
        }
    }
```

**Option B: Add command-line flag**

```rust
// In main() function argument parsing
.arg(
    Arg::new("update-collateral")
        .long("update-collateral")
        .help("Randomly update some collateral after creation")
        .action(clap::ArgAction::SetTrue),
)

// Later in main()
if matches.get_flag("update-collateral") {
    // Call update function
}
```

**Recommendation:** Use Option A for automatic testing, add Option B if you want control

#### Step 3: Test

```bash
# Run script
cargo run --bin populate_deal_data -- --count 10

# Check for update transactions
psql $DATABASE_URL -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE transaction_type = 'collateral_update';
"

# Expected: 2-4 update transactions (30% of ~7 collateral items)
```

**Effort:** 2-3 hours for EPIC-6.4

---

## EPIC-6.5: Add CollateralDealDisassociation Function (Optional)

**Effort:** 2-3 hours  
**Dependencies:** EPIC-6.3 complete  
**Priority:** MEDIUM  

### Implementation Steps

#### Step 1: Create Disassociation Function

**Add new function:**

```rust
/// Disassociate collateral from a deal and record transaction
async fn disassociate_collateral_from_deal(
    pool: &sqlx::PgPool,
    collateral_id: Uuid,
    deal_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    // Remove collateral association in database
    sqlx::query!(
        "UPDATE deals SET collateral_id = NULL WHERE id = $1 AND collateral_id = $2",
        deal_id,
        collateral_id,
    )
    .execute(pool)
    .await?;
    
    // Record disassociation transaction for zkVM
    println!("  📝 Recording CollateralDealDisassociation transaction");
    create_deal_transaction_record(
        pool,
        "collateral_deal_disassociation",
        deal_id,
        None,
        Decimal::ZERO,
        chrono::Utc::now(),
        None,
    )
    .await?;
    
    println!("  ✅ Disassociated collateral {} from deal {}", collateral_id, deal_id);
    
    Ok(())
}
```

#### Step 2: Call Disassociation in Scenario

**Add scenario: Closed deals can have collateral disassociated**

**In `create_deal_state_progression()` for "closed" status (around line 750):**

```rust
        "closed" => {
            // Create full payment history + closeout transaction
            // ... existing payment and closeout logic ...
            
            create_deal_transaction_record(
                pool,
                "deal_closeout",
                deal.id,
                None,
                Decimal::ZERO,
                closeout_date,
                None,
            )
            .await?;
            
            // NEW: If deal has collateral, disassociate after closeout
            if deal.collateral_id.is_some() {
                let coll_id = deal.collateral_id.unwrap();
                
                // Disassociate a few days after closeout
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Small delay
                
                disassociate_collateral_from_deal(
                    pool,
                    coll_id,
                    deal.id,
                )
                .await?;
            }
        }
```

#### Step 3: Test

```bash
# Run script
cargo run --bin populate_deal_data -- --count 25

# Check for disassociation transactions
psql $DATABASE_URL -c "
  SELECT 
    COUNT(*) as disassociation_count,
    COUNT(DISTINCT deal_id) as unique_deals
  FROM deal_transactions 
  WHERE transaction_type = 'collateral_deal_disassociation';
"

# Expected: ~1-2 disassociations (only 'closed' deals with collateral)
```

**Effort:** 2-3 hours for EPIC-6.5

---

## EPIC-6.6: Add Detection Event Simulation (Optional)

**Effort:** 4-6 hours  
**Dependencies:** EPIC-6.2 complete  
**Priority:** LOW  

### Implementation Steps

#### Step 1: Create Detection Simulation Function

**Add new function:**

```rust
/// Simulate collateral detection events for testing
async fn simulate_collateral_detection(
    pool: &sqlx::PgPool,
    collateral_id: Uuid,
    matches_found: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Record detection REQUEST transaction
    println!("  📝 Recording CollateralDetectionRequest transaction");
    create_deal_transaction_record(
        pool,
        "collateral_detection_request",
        Uuid::nil(),
        None,
        Decimal::ZERO,
        chrono::Utc::now(),
        None,
    )
    .await?;
    
    // Simulate processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // 2. Record detection UPDATE transaction
    println!("  📝 Recording CollateralDetectionUpdate transaction");
    create_deal_transaction_record(
        pool,
        "collateral_detection_update",
        Uuid::nil(),
        None,
        Decimal::ZERO,
        chrono::Utc::now(),
        None,
    )
    .await?;
    
    println!(
        "  ✅ Simulated detection for collateral {} ({} matches)",
        collateral_id, matches_found
    );
    
    Ok(())
}
```

#### Step 2: Call Detection Simulation

**Add after collateral creation in `create_realistic_deal()` or in main flow:**

```rust
// In generate_user_deals() or generate_random_deals()
// After creating all deals with collateral

println!("🔍 Simulating collateral detection events...");

// Get all collateral created by this script run
let all_collateral = db::list_collateral(pool, Some(1000), None, None, None).await?;

// Simulate detection for first few collateral items
for coll in all_collateral.collateral.iter().take(3) {
    // Randomly assign match count
    let matches = if rng.gen_bool(0.3) {
        rng.gen_range(1..=3) // Potential duplicates
    } else {
        0 // No matches
    };
    
    if let Err(e) = simulate_collateral_detection(pool, coll.id, matches).await {
        eprintln!("⚠️  Failed to simulate detection for {}: {}", coll.id, e);
    }
}

println!("  ✅ Detection simulation complete");
```

#### Step 3: Make It Optional with Flag

**Add command-line flag:**

```rust
.arg(
    Arg::new("detect")
        .long("detect")
        .help("Simulate collateral detection events")
        .action(clap::ArgAction::SetTrue),
)

// In main()
if matches.get_flag("detect") {
    // Run detection simulation
}
```

#### Step 4: Test

```bash
# Run with detection simulation
cargo run --bin populate_deal_data -- --count 10 --detect

# Verify detection transactions
psql $DATABASE_URL -c "
  SELECT 
    transaction_type,
    COUNT(*) as count
  FROM deal_transactions 
  WHERE transaction_type LIKE 'collateral_detection%'
  GROUP BY transaction_type;
"

# Expected:
# collateral_detection_request  | 3
# collateral_detection_update   | 3
```

**Effort:** 4-6 hours for EPIC-6.6

---

## Testing & Validation

### Test Matrix

After all changes, verify complete coverage:

```bash
# Run script with all features
cargo run --bin populate_deal_data -- --clear --count 25 --detect

# Verify transaction type distribution
psql $DATABASE_URL -c "
  SELECT 
    transaction_type,
    COUNT(*) as count,
    ROUND(100.0 * COUNT(*) / SUM(COUNT(*)) OVER (), 1) as percentage
  FROM deal_transactions
  GROUP BY transaction_type
  ORDER BY count DESC;
"
```

**Expected output:**

| transaction_type | count | percentage |
|------------------|-------|------------|
| deal_payment | 75-125 | ~60% |
| deal_origination | 25 | ~15% |
| collateral_creation | ~18 | ~11% |
| collateral_deal_association | ~18 | ~11% |
| deal_closeout | ~5 | ~3% |
| user_registration | 1-2 | <1% |
| collateral_update | ~5 | ~3% |
| collateral_deal_disassociation | ~1 | <1% |
| collateral_detection_request | ~3 | ~2% |
| collateral_detection_update | ~3 | ~2% |

**Total:** ~170-200 transactions covering all 10 types (100% coverage) ✅

### Integration with Batch Processing

**After script completes, test batch processing:**

```bash
# 1. Verify transactions are pending
psql $DATABASE_URL -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE included_in_batch_id IS NULL;
"

# 2. Trigger batch
curl -X POST http://localhost:8080/api/v1/batch/trigger \
  -H "Authorization: Bearer $TOKEN"

# 3. Wait for processing
sleep 60

# 4. Verify transactions were batched
psql $DATABASE_URL -c "
  SELECT 
    CASE WHEN included_in_batch_id IS NULL THEN 'pending' ELSE 'batched' END as status,
    COUNT(*) as count
  FROM deal_transactions
  GROUP BY status;
"

# 5. Verify proof was generated
psql $DATABASE_URL -c "
  SELECT 
    id,
    proof_status,
    transaction_count,
    sindri_proof_id IS NOT NULL as has_proof
  FROM proof_batches
  ORDER BY id DESC
  LIMIT 1;
"
```

### zkVM Processing Validation

**Test that zkVM can process all transaction types:**

```bash
# 1. Get a batch ID
BATCH_ID=$(psql $DATABASE_URL -t -c "
  SELECT id FROM proof_batches ORDER BY id DESC LIMIT 1;
" | tr -d ' ')

# 2. Check batch transaction types
psql $DATABASE_URL -c "
  SELECT 
    transaction_type,
    COUNT(*) as count
  FROM deal_transactions
  WHERE included_in_batch_id = $BATCH_ID
  GROUP BY transaction_type;
"

# 3. Check proof status
curl "http://localhost:8080/api/v1/batch/$BATCH_ID/status" | jq

# 4. Verify proof was generated and posted
psql $DATABASE_URL -c "
  SELECT 
    proof_status,
    posted_to_contract,
    contract_tx_hash IS NOT NULL as on_chain
  FROM proof_batches
  WHERE id = $BATCH_ID;
"
```

**Expected:**
- proof_status = "proven"
- posted_to_contract = true
- All transaction types present in batch

---

## Incremental Rollout Strategy

### Week 1: Foundation Changes

**What to implement:**
- EPIC-6.1: UserRegistration (2-3 hours)

**Why:** Enables user lifecycle testing while WS-1 is being built

**Test:**
```bash
cargo run --bin populate_deal_data -- --email test@example.com --count 5
```

**Validation:** User registration transactions appear

### Week 2: Collateral Basics

**What to implement:**
- EPIC-6.2: CollateralCreation (1-2 hours)
- EPIC-6.3: CollateralDealAssociation (1-2 hours)

**Why:** Enables collateral testing while WS-3 implements zkVM logic

**Test:**
```bash
cargo run --bin populate_deal_data -- --count 10
```

**Validation:** 60% transaction coverage (6/10 types)

### Week 3: Optional Features

**What to implement:**
- EPIC-6.4: CollateralUpdate (2-3 hours)
- EPIC-6.5: CollateralDealDisassociation (2-3 hours)

**Why:** Brings coverage to 80% while WS-4 does API integration

**Test:**
```bash
cargo run --bin populate_deal_data -- --count 25
```

**Validation:** 80% transaction coverage (8/10 types)

### Week 4: Complete Coverage

**What to implement:**
- EPIC-6.6: Detection simulation (4-6 hours)

**Why:** 100% coverage for final testing

**Test:**
```bash
cargo run --bin populate_deal_data -- --count 50 --detect
```

**Validation:** 100% transaction coverage (10/10 types)

---

## Integration with Other Work Streams

### With WS-1 (Transaction Recording)

**No dependency:** Script uses direct SQL, doesn't need TransactionRecordingService

**But:** Should verify hashing matches

```rust
// Script's hash_uuid function (line 569):
fn hash_uuid(uuid: &Uuid) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(uuid.as_bytes());
    hasher.finalize().to_vec()
}

// Should match TransactionRecordingService::hash_uuid
// Verify with test
```

### With WS-2 (Deal Operations)

**Coordination:** Deal-related transactions already work

**New:** If WS-2 adds payment endpoint, script can optionally use API instead of direct DB

### With WS-3 (Collateral zkVM)

**Critical:** Script creates data that WS-3 will process

**Testing flow:**
1. WS-6 creates collateral transactions
2. Trigger batch
3. WS-3 zkVM logic processes them
4. Verify no errors

**Feedback loop:** If zkVM rejects transactions, WS-6 may need to adjust data format

### With WS-4 (Collateral API)

**Optional:** Script could use API endpoints instead of direct DB

**Current:** Direct DB is fine for test data

**Future:** Could switch to API for more realistic testing

### With WS-5 (Testing)

**Critical:** WS-5 depends on WS-6 for test data

**Handoff:** After EPIC-6.1-6.3 complete (60% coverage), WS-5 can start testing

**Complete handoff:** After all EPIC-6 complete (100% coverage)

---

## Validation Checklist

### EPIC-6.1 Complete (UserRegistration)

- [ ] User creation points identified
- [ ] Transaction recording added to both locations
- [ ] No duplicate transactions for existing users
- [ ] Timestamps correct
- [ ] Manual test passed
- [ ] Script output shows transaction recording

### EPIC-6.2 Complete (CollateralCreation)

- [ ] create_random_collateral() updated
- [ ] Transaction recording added after creation
- [ ] ~70% of deals have collateral creation transactions
- [ ] Timestamps match collateral.created_at
- [ ] Manual test passed

### EPIC-6.3 Complete (CollateralDealAssociation)

- [ ] create_realistic_deal() updated
- [ ] Transaction recording added when collateral present
- [ ] Association count matches creation count
- [ ] Timestamps match deal.created_at
- [ ] Manual test passed

### EPIC-6.4 Complete (CollateralUpdate - Optional)

- [ ] update_random_collateral() function created
- [ ] Called from main flow
- [ ] ~30% of collateral updated
- [ ] Transactions recorded
- [ ] Manual test passed

### EPIC-6.5 Complete (Disassociation - Optional)

- [ ] disassociate_collateral_from_deal() function created
- [ ] Called for closed deals
- [ ] Transactions recorded
- [ ] Manual test passed

### EPIC-6.6 Complete (Detection - Optional)

- [ ] simulate_collateral_detection() function created
- [ ] Request and update transactions created
- [ ] Flag to enable/disable
- [ ] Manual test passed

### Overall WS-6 Complete

- [ ] Minimum 60% coverage (Priority 1 complete)
- [ ] Ideally 100% coverage (all priorities complete)
- [ ] Script runs without errors
- [ ] All transaction types represented
- [ ] Batch processing works with script data
- [ ] zkVM can process all transactions
- [ ] Documentation updated

---

## Command-Line Flags Summary

### Existing Flags (Keep Unchanged)

```bash
--email <EMAIL>      # Generate deals for specific user
--count <NUMBER>     # Number of deals to generate (default: 25)
--clear              # Clear existing data first
--csv                # Export to CSV instead of database
--dp <NUMBER>        # Create double-pledged deals
```

### Potential New Flags (Optional)

```bash
--update-collateral  # Randomly update some collateral (EPIC-6.4)
--detect             # Simulate detection events (EPIC-6.6)
--no-user-tx         # Skip user registration transactions (for testing)
--no-collateral-tx   # Skip collateral transactions (for testing)
```

**Recommendation:** Start without new flags, add if needed for specific testing scenarios

---

## Performance Considerations

### Current Script Performance

**Baseline (without changes):**
- 25 deals: ~5-10 seconds
- 100 deals: ~30-45 seconds

**After WS-6 changes:**
- Additional transaction inserts per deal: 1-3
- Estimated overhead: +10-20%
- 25 deals: ~6-12 seconds (acceptable)
- 100 deals: ~35-55 seconds (acceptable)

### Optimization Opportunities

**If script becomes too slow:**

**Option 1: Batch Transaction Inserts**

```rust
// Instead of individual inserts
for deal in deals {
    create_deal_transaction_record(...).await?;
}

// Batch them
let mut tx = pool.begin().await?;
for deal in deals {
    sqlx::query!("INSERT INTO deal_transactions ...").execute(&mut *tx).await?;
}
tx.commit().await?;
```

**Option 2: Reduce Logging**

```rust
// Only log summary, not each transaction
println!("  📝 Recording {} transactions...", count);
// ... insert all ...
println!("  ✅ Recorded {} transactions", count);
```

**Option 3: Parallel Processing**

```rust
// Process deals in parallel
use futures::stream::{self, StreamExt};

stream::iter(0..count)
    .map(|i| create_realistic_deal(...))
    .buffer_unordered(10) // 10 concurrent
    .collect::<Vec<_>>()
    .await;
```

**Recommendation:** Start without optimizations, add only if needed

---

## Success Metrics

### Coverage Targets

**Minimum (Priority 1 - Week 2):**
- 6/10 transaction types (60%)
- Enables basic zkVM testing
- UserRegistration + all deal ops + basic collateral

**Good (Priority 1-2 - Week 3):**
- 8/10 transaction types (80%)
- Enables comprehensive testing
- Includes collateral updates and disassociations

**Ideal (All priorities - Week 4):**
- 10/10 transaction types (100%)
- Complete zkVM testing capability
- All operations covered

### Measurement

**After each phase, run:**

```sql
-- Coverage check
SELECT 
    CASE 
        WHEN transaction_type IN (
            'user_registration', 
            'deal_origination', 
            'deal_payment', 
            'deal_closeout',
            'collateral_creation',
            'collateral_deal_association'
        ) THEN 'Priority 1 (Required)'
        WHEN transaction_type IN (
            'collateral_update',
            'collateral_deal_disassociation'
        ) THEN 'Priority 2 (Nice to have)'
        ELSE 'Priority 3 (Optional)'
    END as priority,
    transaction_type,
    COUNT(*) as count
FROM deal_transactions
WHERE metadata->>'created_by_script' = 'true'
GROUP BY priority, transaction_type
ORDER BY priority, count DESC;
```

### Definition of Done

**WS-6 is complete when:**

**Minimum (Required):**
- [ ] UserRegistration transactions created
- [ ] CollateralCreation transactions created
- [ ] CollateralDealAssociation transactions created
- [ ] 60% coverage achieved
- [ ] Script runs without errors
- [ ] Batch processing works with script data

**Ideal (Recommended):**
- [ ] All 10 transaction types created
- [ ] 100% coverage achieved
- [ ] Optional flags for detection simulation
- [ ] Documentation updated
- [ ] WS-5 can use script for complete testing

---

## Quick Reference

### Transaction Type Summary

| Type | Function to Modify | Line | Call to Add | Priority |
|------|-------------------|------|-------------|----------|
| UserRegistration | `ensure_user_companies()`, `generate_user_deals()` | 1700, 1359 | `create_deal_transaction_record()` | HIGH |
| DealOrigination | `create_deal_state_progression()` | 637 | ✅ Exists | N/A |
| DealPayment | `create_deal_state_progression()` | 667, 703, 739 | ✅ Exists | N/A |
| DealCloseout | `create_deal_state_progression()` | 753 | ✅ Exists | N/A |
| CollateralCreation | `create_random_collateral()` | 1843 | `create_deal_transaction_record()` | HIGH |
| CollateralDealAssociation | `create_realistic_deal()` | 1949 | `create_deal_transaction_record()` | HIGH |
| CollateralUpdate | New function | N/A | `create_deal_transaction_record()` | MEDIUM |
| CollateralDealDisassociation | `create_deal_state_progression()` "closed" | 763 | `create_deal_transaction_record()` | MEDIUM |
| CollateralDetectionRequest | New function | N/A | `create_deal_transaction_record()` | LOW |
| CollateralDetectionUpdate | New function | N/A | `create_deal_transaction_record()` | LOW |

### Testing Commands

```bash
# Quick test (Priority 1 only)
cargo run --bin populate_deal_data -- --count 5

# Full test (all priorities)
cargo run --bin populate_deal_data -- --count 25 --detect

# Clear and regenerate
cargo run --bin populate_deal_data -- --clear --count 50

# Check coverage
psql $DATABASE_URL -c "
  SELECT 
    COUNT(DISTINCT transaction_type) as unique_types,
    COUNT(*) as total_transactions
  FROM deal_transactions;
"

# Expected: 
# unique_types = 10 (after all changes)
# total_transactions = ~170-200 (for 25 deals)
```

---

## Common Issues & Solutions

### Issue 1: "Duplicate key value violates unique constraint"

**Cause:** Script run multiple times without --clear

**Solution:**
```bash
# Always clear before testing
cargo run --bin populate_deal_data -- --clear --count 25
```

### Issue 2: "Foreign key violation on deal_id"

**Cause:** Using real deal_id for non-deal transactions

**Solution:**
```rust
// For user registration, collateral creation, etc.
create_deal_transaction_record(
    pool,
    "user_registration",
    Uuid::nil(), // ← Use nil UUID, not real deal_id
    Some(user.id),
    Decimal::ZERO,
    created_at,
    None,
)
```

### Issue 3: "Too many transactions, batch won't process"

**Cause:** Generated too much test data

**Solution:**
```bash
# Use smaller count for testing
cargo run --bin populate_deal_data -- --count 10

# Or increase batch size in config
export MAX_BATCH_SIZE=100
```

### Issue 4: "Timestamps out of order"

**Cause:** User registration after deal creation

**Solution:**
```rust
// Ensure user registration uses user.created_at, not Utc::now()
create_deal_transaction_record(
    pool,
    "user_registration",
    Uuid::nil(),
    Some(user.id),
    Decimal::ZERO,
    user.created_at, // ← Use actual creation time
    None,
)
```

### Issue 5: "Collateral count doesn't match association count"

**Cause:** Not all deals get collateral (70% probability)

**Solution:** This is expected behavior
```rust
// Verify they match
let coll_count = count_collateral_creations();
let assoc_count = count_associations();
assert_eq!(coll_count, assoc_count); // Should be equal
```

---

## Documentation Updates

### Update Script Documentation

**File:** `scripts/populate_deal_data.rs`

**Update header comment (lines 1-14):**

```rust
#!/usr/bin/env cargo run --bin populate_deal_data --
//! Deal data population script for the Arda Credit database
//!
//! This script creates realistic deal data for testing the deals API endpoint,
//! zkVM batch processing, and automatically refreshes the metrics cache.
//!
//! ## Transaction Coverage
//!
//! The script creates the following transaction types for zkVM testing:
//! - UserRegistration (for new users)
//! - DealOrigination (for all deals)
//! - DealPayment (1-5 per deal based on status)
//! - DealCloseout (for 'closed' status deals)
//! - CollateralCreation (~70% of deals)
//! - CollateralDealAssociation (~70% of deals)
//! - CollateralUpdate (optional, ~30% of collateral)
//! - CollateralDealDisassociation (optional, for closed deals)
//! - CollateralDetectionRequest (optional, with --detect flag)
//! - CollateralDetectionUpdate (optional, with --detect flag)
//!
//! This provides 60-100% coverage of zkVM transaction types depending on options.
//!
//! ## Modes
//!
//! - Random mode: Generate random deals with random companies (no --email)
//! - User mode: Generate deals for a specific user's companies (with --email)
//! - CSV mode: Export to CSV instead of database (with --csv)
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage (random deals)
//! cargo run --bin populate_deal_data -- --count 25
//!
//! # User-specific deals
//! cargo run --bin populate_deal_data -- --email user@example.com --count 10
//!
//! # With all zkVM transaction types
//! cargo run --bin populate_deal_data -- --count 50 --detect
//!
//! # Clear and regenerate
//! cargo run --bin populate_deal_data -- --clear --count 100
//! ```
```

### Update README References

**If there's a script README, update it to mention:**
- Script now creates zkVM transactions
- 60-100% transaction type coverage
- Enables complete zkVM testing
- See docs/zk/test-coverage-analysis.md for details

---

## Validation Script

### Complete Test Validation

**Create:** `scripts/validate_zkvm_coverage.sh`

```bash
#!/bin/bash
# Validate that populate_deal_data creates all required transaction types

set -e

echo "=== Validating zkVM Transaction Coverage ==="

# Required transaction types (Priority 1)
REQUIRED_TYPES=(
    "user_registration"
    "deal_origination"
    "deal_payment"
    "deal_closeout"
    "collateral_creation"
    "collateral_deal_association"
)

# Check each required type
MISSING=0
for TYPE in "${REQUIRED_TYPES[@]}"; do
    COUNT=$(psql $DATABASE_URL -t -c "
        SELECT COUNT(*) FROM deal_transactions 
        WHERE transaction_type = '$TYPE'
          AND metadata->>'created_by_script' = 'true';
    " | tr -d ' ')
    
    if [ "$COUNT" -gt "0" ]; then
        echo "✅ $TYPE: $COUNT transactions"
    else
        echo "❌ $TYPE: MISSING"
        MISSING=$((MISSING + 1))
    fi
done

echo ""
echo "=== Summary ==="
echo "Required types found: $((6 - MISSING))/6"

if [ $MISSING -eq 0 ]; then
    echo "✅ PASS: All required transaction types present"
    echo ""
    echo "Coverage: 60% (6/10 types) - Minimum for testing"
    exit 0
else
    echo "❌ FAIL: Missing $MISSING required transaction types"
    exit 1
fi
```

**Usage:**
```bash
# After running populate script
cargo run --bin populate_deal_data -- --count 25

# Validate coverage
chmod +x scripts/validate_zkvm_coverage.sh
./scripts/validate_zkvm_coverage.sh
```

---

## Acceptance Criteria

### Minimum (Required for WS-5 to start)

- [ ] EPIC-6.1 complete (UserRegistration)
- [ ] EPIC-6.2 complete (CollateralCreation)
- [ ] EPIC-6.3 complete (CollateralDealAssociation)
- [ ] 60% coverage achieved
- [ ] Script runs successfully
- [ ] No errors in transaction recording
- [ ] Data compatible with batch processor

### Ideal (Full zkVM Testing)

- [ ] All EPIC-6 sub-issues complete
- [ ] 100% coverage achieved
- [ ] Optional features implemented
- [ ] Detection simulation working
- [ ] Documentation complete
- [ ] Validation script passing

---

## Timeline

### Incremental Implementation (Recommended)

**Week 1 (2-3 hours):**
- EPIC-6.1: UserRegistration
- **Output:** Users have registration transactions
- **Test:** Generate user deals, verify transactions

**Week 2 (2-4 hours):**
- EPIC-6.2: CollateralCreation
- EPIC-6.3: CollateralDealAssociation
- **Output:** 60% coverage
- **Test:** Full script run, verify collateral transactions

**Week 3 (4-6 hours):**
- EPIC-6.4: CollateralUpdate (optional)
- EPIC-6.5: CollateralDealDisassociation (optional)
- **Output:** 80% coverage
- **Test:** Advanced scenarios work

**Week 4 (4-6 hours):**
- EPIC-6.6: Detection simulation (optional)
- **Output:** 100% coverage
- **Test:** Complete zkVM validation

**Total: 12-19 hours spread across 4 weeks**

### Fast-Track (If Needed Urgently)

**All in one session (8 hours):**
- Hour 1-2: EPIC-6.1
- Hour 3-4: EPIC-6.2 + EPIC-6.3
- Hour 5-6: EPIC-6.4 + EPIC-6.5
- Hour 7-8: EPIC-6.6 + Testing

**Output:** 100% coverage in single day

---

## Success Metrics

### Coverage Measurement

**Before any changes:**
```sql
SELECT COUNT(DISTINCT transaction_type) FROM deal_transactions;
-- Result: 3 (30% of 10 types)
```

**After Priority 1 (Week 2):**
```sql
SELECT COUNT(DISTINCT transaction_type) FROM deal_transactions;
-- Result: 6 (60% of 10 types)
```

**After All changes (Week 4):**
```sql
SELECT COUNT(DISTINCT transaction_type) FROM deal_transactions;
-- Result: 10 (100% of 10 types) ✅
```

### Integration Success

**After completing WS-6, should be able to:**

1. ✅ Run script to generate test data
2. ✅ Trigger batch processing
3. ✅ zkVM processes all 10 transaction types
4. ✅ Proof generated successfully
5. ✅ Proof posted to contract
6. ✅ Audit trail shows complete history

**Validation flow:**
```bash
# 1. Generate data
cargo run --bin populate_deal_data -- --clear --count 25

# 2. Check coverage
psql $DATABASE_URL -c "SELECT DISTINCT transaction_type FROM deal_transactions;"

# 3. Trigger batch
curl -X POST http://localhost:8080/api/v1/batch/trigger

# 4. Wait and check
sleep 90

# 5. Verify proof
psql $DATABASE_URL -c "
  SELECT proof_status, posted_to_contract 
  FROM proof_batches 
  ORDER BY id DESC 
  LIMIT 1;
"

# Expected: proof_status = 'proven', posted_to_contract = true
```

---

## Code Snippets Summary

### Snippet 1: UserRegistration (2 places)

```rust
// In ensure_user_companies() after line 1726
create_deal_transaction_record(
    pool,
    "user_registration",
    Uuid::nil(),
    Some(user.id),
    Decimal::ZERO,
    user.created_at,
    None,
).await?;

// In generate_user_deals() after line 1359
create_deal_transaction_record(
    pool,
    "user_registration",
    Uuid::nil(),
    Some(user.id),
    Decimal::ZERO,
    user.created_at,
    None,
).await?;
```

### Snippet 2: CollateralCreation

```rust
// In create_random_collateral() after line 1843
create_deal_transaction_record(
    pool,
    "collateral_creation",
    Uuid::nil(),
    None,
    Decimal::ZERO,
    collateral.created_at,
    None,
).await?;
```

### Snippet 3: CollateralDealAssociation

```rust
// In create_realistic_deal() after line 1949
if let Some(coll) = &collateral {
    create_deal_transaction_record(
        pool,
        "collateral_deal_association",
        deal.id,
        None,
        Decimal::ZERO,
        deal.created_at,
        None,
    ).await?;
}
```

---

## Effort Summary

### Total Effort Breakdown

| Priority | Sub-Issues | Effort | Coverage Gain |
|----------|-----------|--------|---------------|
| Priority 1 (HIGH) | EPIC-6.1, 6.2, 6.3 | 4-7 hours | 30% → 60% |
| Priority 2 (MEDIUM) | EPIC-6.4, 6.5 | 4-6 hours | 60% → 80% |
| Priority 3 (LOW) | EPIC-6.6 | 4-6 hours | 80% → 100% |
| **Total** | **6 sub-issues** | **12-19 hours** | **30% → 100%** |

### Recommended Approach

**Minimum viable (4-7 hours):**
- Implement Priority 1 only
- Gets to 60% coverage
- Sufficient for basic zkVM testing
- WS-5 can start testing

**Recommended (8-13 hours):**
- Implement Priority 1 + 2
- Gets to 80% coverage
- Comprehensive zkVM testing
- Only missing detection events

**Complete (12-19 hours):**
- Implement all priorities
- 100% coverage
- Production-ready test data
- Full zkVM validation capability

---

## Integration Timeline

### Coordinate with Other Work Streams

**Week 1:**
- WS-6 does EPIC-6.1 (UserRegistration)
- WS-1 builds TransactionRecordingService
- WS-2 starts planning
- **Result:** User transactions available for early testing

**Week 2:**
- WS-6 does EPIC-6.2-6.3 (Collateral basics)
- WS-3 implements collateral zkVM
- WS-2 integrates deals
- **Result:** 60% coverage, WS-5 can start basic testing

**Week 3:**
- WS-6 does EPIC-6.4-6.5 (Optional updates)
- WS-4 integrates collateral API
- WS-5 runs integration tests
- **Result:** 80% coverage, comprehensive testing

**Week 4:**
- WS-6 does EPIC-6.6 (Detection simulation)
- WS-5 runs E2E tests
- All work streams in testing phase
- **Result:** 100% coverage, full validation

---

## Validation Checklist (Final)

### Code Changes

- [ ] UserRegistration added to 2 locations
- [ ] CollateralCreation added to create_random_collateral()
- [ ] CollateralDealAssociation added to create_realistic_deal()
- [ ] Optional: Update function created
- [ ] Optional: Disassociation added
- [ ] Optional: Detection simulation created

### Testing

- [ ] Script runs without errors
- [ ] Transaction counts match expected
- [ ] Timestamps are realistic and ordered
- [ ] No duplicate transactions
- [ ] Metadata populated correctly

### Coverage

- [ ] Minimum 60% (6/10 types) - Required
- [ ] Ideally 100% (10/10 types) - Recommended
- [ ] Transaction distribution realistic
- [ ] All types represented in batches

### Integration

- [ ] Batch processing picks up transactions
- [ ] zkVM can process all transaction types
- [ ] No errors during proof generation
- [ ] Proofs posted to contract successfully

### Documentation

- [ ] Script header comments updated
- [ ] Coverage documented
- [ ] Usage examples updated
- [ ] Validation script created

---

**End of Work Stream 6 Implementation Plan**

**Result:** Test data script that creates 60-100% transaction type coverage, enabling comprehensive zkVM testing across all work streams.

**Next Steps:** 
- Review all 6 work stream plans
- Create Linear/Jira issues from detailed plans
- Assign engineers to work streams
- Begin implementation!
