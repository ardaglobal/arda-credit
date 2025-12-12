# Work Stream 4: Collateral API Integration

**Epic:** EPIC-4  
**Priority:** HIGH  
**Effort:** 4.5 days (36 hours)  
**Owner:** Backend Engineer #2  
**Dependencies:** EPIC-1 complete, EPIC-3 complete  

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Requirements](#technical-requirements)
3. [Current State Analysis](#current-state-analysis)
4. [Sub-Issue Implementation Guides](#sub-issue-implementation-guides)
5. [Webhook Integration](#webhook-integration)
6. [Testing Strategy](#testing-strategy)
7. [Validation Checklist](#validation-checklist)

---

## Overview

### Purpose

Integrate existing collateral API endpoints with the TransactionRecordingService and zkVM proof system. This work stream connects the collateral business logic (database operations) with the cryptographic proof layer (zkVM), enabling verifiable collateral state transitions and double-pledge prevention.

### Context

**Current State (Discovered):**
- ✅ Collateral CRUD endpoints EXIST in `api/src/handlers/taxonomy/collateral.rs`
- ✅ Basic transaction recording EXISTS (uses old `record_collateral_creation_transaction`)
- ✅ Event publisher system EXISTS for collateral-intel service
- ✅ Webhook handlers EXIST in `api/src/handlers/taxonomy/detection.rs`
- ❌ NOT using unified TransactionRecordingService
- ❌ Only CollateralCreation has transaction recording
- ❌ Association/disassociation may need new endpoints or updates
- ❌ Webhooks don't create deal_transactions records

**Target State:**
- ✅ All collateral endpoints use TransactionRecordingService
- ✅ All 6 collateral transaction types recorded
- ✅ Webhooks create deal_transactions for detection events
- ✅ Metadata properly formatted for zkVM
- ✅ All transactions flow to batch processor

### Scope (6 Transaction Types)

1. **CollateralCreation (#20)** - Update existing endpoint
2. **CollateralUpdate (#21)** - Update existing endpoint
3. **CollateralDealAssociation (#22)** - Create/update endpoint
4. **CollateralDealDisassociation (#23)** - Create/update endpoint
5. **CollateralDetectionRequest (#24)** - Integrate with webhook
6. **CollateralDetectionUpdate (#25)** - Integrate with webhook

---

## Technical Requirements

### Functional Requirements

**FR-1: Unified Transaction Recording**
- Replace `record_collateral_creation_transaction` with TransactionRecordingService
- All collateral operations use same service
- Consistent metadata formatting
- Consistent error handling

**FR-2: Metadata Format for zkVM**

Different transaction types need different metadata encoding:

**CollateralCreation/Update:**
```
Bytes 0-31: collateral_type as UTF-8 string (null-padded)
Bytes 32-63: Reserved for future use
```

**CollateralAssociation/Disassociation:**
```
Bytes 0-31: collateral_id (32 bytes, hashed UUID)
Bytes 32-63: deal_id (32 bytes, hashed UUID)
```

**Detection Request/Update:**
```
Bytes 0-31: collateral_id (32 bytes)
Bytes 32-63: Reserved or detection metadata
```

**FR-3: Association Endpoints**
- Create endpoint for associating collateral with deal (if doesn't exist)
- Create endpoint for disassociating collateral from deal (if doesn't exist)
- Or update existing deal creation to handle collateral association

**FR-4: Webhook Integration**
- Detection request webhook creates deal_transactions record
- Detection update webhook creates deal_transactions record
- Metadata includes detection results (private)

### Non-Functional Requirements

**NFR-1: Backward Compatibility**
- Existing collateral endpoints continue to work
- Response formats unchanged
- Only add transaction recording

**NFR-2: Event System Integration**
- Keep existing event publisher for collateral-intel
- Add transaction recording in parallel
- Both systems should work together

**NFR-3: Error Handling**
- Transaction recording failure should be logged but not fail request
- Or fail request for strict consistency (decision needed)
- Clear error messages

---

## Current State Analysis

### EPIC-4.1: Audit Existing Collateral Endpoints

**Effort:** 4 hours  
**Dependencies:** None - can start while EPIC-3 is in progress  
**Priority:** HIGH (informational, blocks planning)  

### Endpoints Inventory

Based on code analysis, here's what EXISTS:

| Endpoint | Method | Handler Function | Line | Transaction Recording? | Status |
|----------|--------|------------------|------|----------------------|--------|
| `/api/v1/collateral` | POST | `create_collateral_handler` | 202 | ⚠️ Old system | Needs update |
| `/api/v1/collateral/{id}` | GET | `get_collateral_handler` | 294 | N/A (read-only) | OK |
| `/api/v1/collateral/{id}` | PUT | `update_collateral_handler` | 462 | ❌ None | Needs addition |
| `/api/v1/collateral/{id}` | DELETE | `delete_collateral_handler` | 621 | ❌ None | Maybe add |
| `/api/v1/collateral` | GET | `list_collateral_handler` | 417 | N/A (read-only) | OK |
| `/api/v1/collateral/batch` | POST | `create_collateral_batch_handler` | 670 | ❌ None | Needs addition |

### Missing Endpoints (Need to Create or Find)

| Endpoint | Purpose | Exists? | Action Required |
|----------|---------|---------|-----------------|
| `POST /api/v1/collateral/{id}/associate` | Associate collateral with deal | ❓ Unknown | Check or create |
| `DELETE /api/v1/collateral/{id}/associate` | Disassociate from deal | ❓ Unknown | Check or create |

**Alternative:** Association might happen during deal creation (deal has `collateral_id` field)

### Webhook Handlers

**File:** `api/src/handlers/taxonomy/detection.rs`

**Webhooks that exist:**
- Webhook for detection results from collateral-intel service
- Called when detection completes
- Need to verify if it creates transactions

### Action Items from Audit

1. **Update create_collateral_handler** - Replace old transaction recording with new service
2. **Add to update_collateral_handler** - Add transaction recording
3. **Check association mechanism** - How are collateral and deals linked?
4. **Create/update association endpoint** - If needed
5. **Update webhook handlers** - Add transaction recording for detection events
6. **Test all integrations** - Verify zkVM can process

**Effort:** 4 hours for complete audit and planning

---

## Sub-Issue Implementation Guides

## EPIC-4.2: Integrate CollateralCreation Endpoint

**Effort:** 4 hours  
**Dependencies:** EPIC-1, EPIC-3.3 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Review Current Implementation

**File:** `api/src/handlers/taxonomy/collateral.rs` (line 202)

**Current code (lines 230-248):**
```rust
// Record creation transaction for audit trail
let initial_properties = serde_json::json!({
    "name": collateral.name.clone(),
    "collateral_type": collateral.collateral_type.clone(),
    "description": collateral.description.clone(),
    "address": collateral.address.clone(),
    "custom_id": collateral.custom_id.clone(),
});

if let Err(e) = record_collateral_creation_transaction(
    &state.pool,
    collateral.id,
    Some(user.id),
    initial_properties,
)
.await
{
    tracing::warn!("Failed to record collateral creation transaction: {}", e);
}
```

**Issue:** Uses old `record_collateral_creation_transaction` from db crate, not TransactionRecordingService

#### Step 2: Replace with TransactionRecordingService

**REPLACE lines 230-248 with:**

```rust
// Record creation transaction for zkVM
// Metadata must encode collateral_type in first 32 bytes for zkVM
let metadata = Some(serde_json::json!({
    "collateral_type": collateral.collateral_type.clone().unwrap_or_default(),
    "name": collateral.name.clone(),
    "address": collateral.address.clone(),
    "custom_id": collateral.custom_id.clone(),
    "created_by": user.id.to_string(),
}));

let tx_result = state.transaction_recording
    .record_transaction(
        arda_credit_lib::TransactionType::CollateralCreation,
        user.id,
        None, // No deal_id yet (not associated)
        rust_decimal::Decimal::ZERO,
        metadata,
    )
    .await;

match tx_result {
    Ok(tx_id) => {
        info!(
            "Recorded CollateralCreation transaction {} for collateral {}",
            tx_id, collateral.id
        );
    }
    Err(e) => {
        // Decision: Fail request or log and continue?
        tracing::error!("Failed to record collateral creation transaction: {}", e);
        // For MVP: Log but don't fail (collateral was created successfully)
        // For production: Consider failing for strict consistency
    }
}
```

#### Step 3: Add Import

**Add to imports at top of file:**

```rust
use arda_credit_lib::TransactionType;
use rust_decimal::Decimal;
```

#### Step 4: Remove Old Function Call

**Find and optionally deprecate:** `record_collateral_creation_transaction` in `db/src/` crate

**Or:** Keep both for now during transition, remove old one later

#### Step 5: Test

**Manual test:**
```bash
# Create collateral
curl -X POST http://localhost:8080/api/v1/collateral \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "Test Property",
    "collateral_type": "real_estate",
    "address": "123 Main St",
    "custom_id": "TEST-001"
  }'

# Verify in database
psql $DATABASE_URL -c "
  SELECT transaction_type, metadata->>'name' as name, metadata->>'collateral_type' as type
  FROM deal_transactions 
  WHERE transaction_type = 'collateral_creation' 
  ORDER BY created_at DESC 
  LIMIT 1;
"
```

**Validation:**
- [ ] Collateral created in database
- [ ] Transaction recorded in deal_transactions
- [ ] Metadata contains collateral_type
- [ ] Transaction can be queried from audit trail

**Effort:** 4 hours total for EPIC-4.2

---

## EPIC-4.3: Integrate CollateralUpdate Endpoint

**Effort:** 4 hours  
**Dependencies:** EPIC-1, EPIC-3.4 complete (can be parallel with EPIC-4.2)  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate Update Handler

**File:** `api/src/handlers/taxonomy/collateral.rs`  
**Function:** `update_collateral_handler` (line 462)

#### Step 2: Review Current Implementation

**Current code pattern (example):**
```rust
pub async fn update_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<UpdateCollateralApiRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse ID
    // Get existing collateral
    // Update in database
    // Return response
    // ❌ No transaction recording
}
```

#### Step 3: Add Transaction Recording

**Insert before return statement:**

```rust
pub async fn update_collateral_handler(
    State(state): State<ApiState>,
    Path(id_str): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<UpdateCollateralApiRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    // ... existing parsing and validation ...
    
    let collateral_id = parse_uuid(&id_str)?;
    
    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;
    
    // ... existing update logic ...
    
    let updated_collateral = update_collateral(&state.pool, collateral_id, update_req)
        .await
        .map_err(|e| internal_error("update_collateral", e))?;
    
    // NEW: Record update transaction for zkVM
    let metadata = Some(serde_json::json!({
        "collateral_type": updated_collateral.collateral_type.clone(),
        "name": updated_collateral.name.clone(),
        "address": updated_collateral.address.clone(),
        "updated_by": user.id.to_string(),
        "updated_fields": serde_json::json!({
            "name": req.name.is_some(),
            "collateral_type": req.collateral_type.is_some(),
            "description": req.description.is_some(),
            "address": req.address.is_some(),
        }),
    }));
    
    if let Err(e) = state.transaction_recording
        .record_transaction(
            TransactionType::CollateralUpdate,
            user.id,
            None, // No deal_id for update
            Decimal::ZERO,
            metadata,
        )
        .await
    {
        tracing::error!("Failed to record collateral update transaction: {}", e);
        // Decision: Log and continue, or return error?
        // For MVP: Log and continue
    }
    
    // ... existing response ...
    
    Ok(Json(collateral_to_response(updated_collateral)))
}
```

#### Step 4: Test

**Manual test:**
```bash
# Get a collateral ID
COLL_ID=$(psql $DATABASE_URL -t -c "SELECT id FROM collateral LIMIT 1;" | tr -d ' ')

# Update it
curl -X PUT "http://localhost:8080/api/v1/collateral/$COLL_ID" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "Updated Property Name",
    "description": "Updated description"
  }'

# Verify transaction
psql $DATABASE_URL -c "
  SELECT transaction_type, metadata->>'name', metadata->>'updated_fields'
  FROM deal_transactions 
  WHERE transaction_type = 'collateral_update' 
  ORDER BY created_at DESC 
  LIMIT 1;
"
```

**Effort:** 4 hours total for EPIC-4.3

---

## EPIC-4.4: Integrate Association/Disassociation Endpoints

**Effort:** 8 hours  
**Dependencies:** EPIC-1, EPIC-3.5 complete  
**Priority:** CRITICAL (double-pledge prevention)  

### Investigation Phase

#### Step 1: Determine Current Association Mechanism

**Check how collateral is currently associated with deals:**

**Option A: Via deal creation (deal has collateral_id field)**
```rust
// In create_deal_endpoint
let deal = create_deal(
    &pool,
    originator_id,
    investor_id,
    principal_amount,
    // ...
    Some(collateral_id),  // ← Association happens here
).await?;
```

**Option B: Separate association endpoint**
```
POST /api/v1/collateral/{collateral_id}/associate
{
  "deal_id": "uuid-123"
}
```

**Option C: Via deal field update**
```
PUT /api/v1/deal/{deal_id}
{
  "collateral_id": "uuid-456"
}
```

**Action:** Search codebase to determine which pattern is used

```bash
# Search for collateral association
grep -r "collateral_id" api/src/handlers/deals/
grep -r "associate.*collateral" api/src/handlers/
```

### Implementation (Assuming Option A: Association via Deal Creation)

If association happens during deal creation, we need to:

#### Step 2: Update create_deal_endpoint

**File:** `api/src/handlers/deals/deals.rs`

**Add transaction recording for collateral association:**

```rust
pub async fn create_deal_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateDealRequest>,
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // ... existing deal creation ...
    
    let deal = create_deal(&state.pool, /* params */).await?;
    
    // Record DealOrigination transaction (from WS-2)
    let deal_tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::DealOrigination,
            originator_id,
            Some(deal.id),
            deal.principal_amount,
            /* metadata */,
        )
        .await?;
    
    // NEW: If collateral was associated, record that too
    if let Some(collateral_id) = request.collateral_id {
        let collateral_uuid = Uuid::parse_str(&collateral_id)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid collateral ID".to_string()))?;
        
        // Prepare metadata with collateral_id and deal_id for zkVM
        // zkVM expects: bytes 0-31 = collateral_id, bytes 32-63 = deal_id
        let assoc_metadata = Some(serde_json::json!({
            "collateral_id": collateral_id,
            "deal_id": deal.id.to_string(),
            "associated_at": chrono::Utc::now().to_rfc3339(),
        }));
        
        if let Err(e) = state.transaction_recording
            .record_transaction(
                TransactionType::CollateralDealAssociation,
                user.id, // User who created the deal
                Some(deal.id),
                Decimal::ZERO,
                assoc_metadata,
            )
            .await
        {
            tracing::error!("Failed to record collateral association transaction: {}", e);
            // Log but don't fail - deal was created successfully
        }
    }
    
    // ... return response ...
}
```

### Implementation (If Separate Association Endpoint Needed)

#### Step 3: Create Association Endpoint (If Doesn't Exist)

**File:** `api/src/handlers/taxonomy/collateral.rs`

**Add new endpoint:**

```rust
/// Associate collateral with a deal
///
/// Links an existing piece of collateral to an existing deal.
/// This will be validated in the zkVM to prevent double-pledging.
///
#[utoipa::path(
    post,
    path = "/api/v1/collateral/{collateral_id}/associate",
    request_body = AssociateCollateralRequest,
    responses(
        (status = 200, description = "Collateral associated successfully"),
        (status = 400, description = "Invalid request or double-pledge detected"),
        (status = 404, description = "Collateral or deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Collateral"
)]
#[instrument(skip(state, headers))]
pub async fn associate_collateral_handler(
    State(state): State<ApiState>,
    Path(collateral_id_str): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AssociateCollateralRequest>,
) -> Result<Json<AssociationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral_id = parse_uuid(&collateral_id_str)?;
    let deal_id = parse_uuid(&req.deal_id)?;
    
    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;
    
    // Validate collateral exists
    let _collateral = get_collateral_by_id(&state.pool, collateral_id)
        .await
        .map_err(|_| not_found_error("Collateral not found"))?;
    
    // Validate deal exists
    let _deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| not_found_error("Deal not found"))?;
    
    // Check for existing active associations (double-pledge check)
    let existing_deal = sqlx::query!(
        r#"
        SELECT deal_id FROM deals 
        WHERE collateral_id = $1 
          AND deal_status IN ('originated', 'active', 'paid_off')
        LIMIT 1
        "#,
        collateral_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| internal_error("check_existing_association", e))?;
    
    if let Some(existing) = existing_deal {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "CollateralAlreadyPledged".to_string(),
                message: format!(
                    "Collateral is already pledged to deal {}",
                    existing.deal_id
                ),
                details: None,
            }),
        ));
    }
    
    // Update deal to include collateral_id
    sqlx::query!(
        "UPDATE deals SET collateral_id = $1, updated_at = NOW() WHERE id = $2",
        collateral_id,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| internal_error("update_deal_collateral", e))?;
    
    // Record association transaction for zkVM
    let assoc_metadata = Some(serde_json::json!({
        "collateral_id": collateral_id.to_string(),
        "deal_id": deal_id.to_string(),
        "associated_by": user.id.to_string(),
    }));
    
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::CollateralDealAssociation,
            user.id,
            Some(deal_id),
            Decimal::ZERO,
            assoc_metadata,
        )
        .await
        .map_err(|e| internal_error("record_association_transaction", e))?;
    
    info!(
        "Associated collateral {} with deal {}, transaction {}",
        collateral_id, deal_id, tx_id
    );
    
    Ok(Json(AssociationResponse {
        collateral_id: collateral_id.to_string(),
        deal_id: deal_id.to_string(),
        transaction_id: tx_id.to_string(),
        associated_at: chrono::Utc::now(),
    }))
}

/// Request to associate collateral with deal
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssociateCollateralRequest {
    pub deal_id: String,
}

/// Response from association
#[derive(Debug, Serialize, ToSchema)]
pub struct AssociationResponse {
    pub collateral_id: String,
    pub deal_id: String,
    pub transaction_id: String,
    pub associated_at: chrono::DateTime<chrono::Utc>,
}
```

#### Step 4: Create Disassociation Endpoint

```rust
/// Disassociate collateral from a deal
///
/// Removes the link between collateral and a deal.
/// Typically used when a deal is closed or when replacing collateral.
///
#[utoipa::path(
    delete,
    path = "/api/v1/collateral/{collateral_id}/associate",
    params(
        ("collateral_id" = String, Path, description = "Collateral ID")
    ),
    request_body = DisassociateCollateralRequest,
    responses(
        (status = 200, description = "Collateral disassociated successfully"),
        (status = 404, description = "Association not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Collateral"
)]
#[instrument(skip(state, headers))]
pub async fn disassociate_collateral_handler(
    State(state): State<ApiState>,
    Path(collateral_id_str): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<DisassociateCollateralRequest>,
) -> Result<Json<DisassociationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral_id = parse_uuid(&collateral_id_str)?;
    let deal_id = parse_uuid(&req.deal_id)?;
    
    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;
    
    // Remove collateral_id from deal
    sqlx::query!(
        "UPDATE deals SET collateral_id = NULL, updated_at = NOW() WHERE id = $1 AND collateral_id = $2",
        deal_id,
        collateral_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| internal_error("update_deal_collateral", e))?;
    
    // Record disassociation transaction for zkVM
    let disassoc_metadata = Some(serde_json::json!({
        "collateral_id": collateral_id.to_string(),
        "deal_id": deal_id.to_string(),
        "disassociated_by": user.id.to_string(),
    }));
    
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::CollateralDealDisassociation,
            user.id,
            Some(deal_id),
            Decimal::ZERO,
            disassoc_metadata,
        )
        .await
        .map_err(|e| internal_error("record_disassociation_transaction", e))?;
    
    info!(
        "Disassociated collateral {} from deal {}, transaction {}",
        collateral_id, deal_id, tx_id
    );
    
    Ok(Json(DisassociationResponse {
        collateral_id: collateral_id.to_string(),
        deal_id: deal_id.to_string(),
        transaction_id: tx_id.to_string(),
        disassociated_at: chrono::Utc::now(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DisassociateCollateralRequest {
    pub deal_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DisassociationResponse {
    pub collateral_id: String,
    pub deal_id: String,
    pub transaction_id: String,
    pub disassociated_at: chrono::DateTime<chrono::Utc>,
}
```

#### Step 5: Add Routes

**File:** `api/src/api/router.rs`

**Find collateral routes section and add:**

```rust
// Collateral endpoints
.route("/api/v1/collateral", post(collateral::create_collateral_handler))
.route("/api/v1/collateral", get(collateral::list_collateral_handler))
.route("/api/v1/collateral/:id", get(collateral::get_collateral_handler))
.route("/api/v1/collateral/:id", put(collateral::update_collateral_handler))
.route("/api/v1/collateral/:id", delete(collateral::delete_collateral_handler))
// NEW: Association endpoints
.route("/api/v1/collateral/:collateral_id/associate", post(collateral::associate_collateral_handler))
.route("/api/v1/collateral/:collateral_id/associate", delete(collateral::disassociate_collateral_handler))
```

**Effort:** 8 hours total for EPIC-4.4

---

## EPIC-4.5: Create Detection Event Webhooks

**Effort:** 12 hours  
**Dependencies:** EPIC-1, EPIC-3.6 complete  
**Priority:** MEDIUM  

### Implementation Steps

#### Step 1: Review Existing Webhook Handler

**File:** `api/src/handlers/taxonomy/detection.rs`

**Current implementation analysis needed:**
- Does webhook handler already exist?
- What's the payload format from collateral-intel service?
- How are detection results currently stored?

#### Step 2: Add Transaction Recording to Webhook

**Expected pattern:**

```rust
/// Webhook handler for collateral detection results
///
/// Called by arda-collateral-intel service when duplicate detection completes.
/// Records the detection request and results as transactions for zkVM.
///
#[instrument(skip(state))]
pub async fn collateral_detection_webhook_handler(
    State(state): State<ApiState>,
    Json(payload): Json<DetectionWebhookPayload>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    info!("Received detection webhook for {} collateral items", payload.results.len());
    
    for result in payload.results {
        let collateral_id = parse_uuid(&result.collateral_id)
            .map_err(|e| internal_error("parse_collateral_id", e))?;
        
        // 1. Record detection REQUEST transaction (if not already recorded)
        // This represents when the detection was initiated
        if result.is_new_request {
            let request_metadata = Some(serde_json::json!({
                "collateral_id": collateral_id.to_string(),
                "requested_at": result.requested_at,
                "check_type": result.check_type,
            }));
            
            if let Err(e) = state.transaction_recording
                .record_transaction(
                    TransactionType::CollateralDetectionRequest,
                    Uuid::nil(), // No user for system-initiated detection
                    None, // No deal yet
                    Decimal::ZERO,
                    request_metadata,
                )
                .await
            {
                tracing::error!("Failed to record detection request transaction: {}", e);
            }
        }
        
        // 2. Record detection UPDATE transaction
        // This represents the detection results
        let update_metadata = Some(serde_json::json!({
            "collateral_id": collateral_id.to_string(),
            "matches_found": result.matches_found,
            "confidence_score": result.confidence_score,
            "duplicate_ids": result.duplicate_ids, // Private - stays in metadata
            "detection_status": result.status,
        }));
        
        if let Err(e) = state.transaction_recording
            .record_transaction(
                TransactionType::CollateralDetectionUpdate,
                Uuid::nil(), // System transaction
                None,
                Decimal::ZERO,
                update_metadata,
            )
            .await
        {
            tracing::error!("Failed to record detection update transaction: {}", e);
        }
        
        // 3. Update collateral record in database (existing logic)
        update_collateral_detection_results(&state.pool, collateral_id, &result).await?;
    }
    
    Ok(StatusCode::OK)
}

/// Payload from collateral-intel service
#[derive(Debug, Deserialize, ToSchema)]
pub struct DetectionWebhookPayload {
    pub results: Vec<DetectionResult>,
}

#[derive(Debug, Deserialize)]
pub struct DetectionResult {
    pub collateral_id: String,
    pub is_new_request: bool,
    pub requested_at: String,
    pub check_type: String,
    pub matches_found: i32,
    pub confidence_score: f64,
    pub duplicate_ids: Vec<String>,
    pub status: String,
}
```

#### Step 3: Add Webhook Route

**File:** `api/src/api/router.rs`

```rust
// Internal webhooks (from collateral-intel service)
.route("/api/v1/webhooks/collateral/detection", post(detection::collateral_detection_webhook_handler))
```

#### Step 4: Add Webhook Security

**Important:** Webhooks should be authenticated to prevent spoofing

```rust
// Add webhook signature validation
pub async fn collateral_detection_webhook_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<DetectionWebhookPayload>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Validate webhook signature
    validate_webhook_signature(&headers, &state.config.webhook_secret)?;
    
    // ... process webhook ...
}

fn validate_webhook_signature(
    headers: &axum::http::HeaderMap,
    secret: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let signature = headers
        .get("x-collateral-intel-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "MissingSignature".to_string(),
                message: "Webhook signature required".to_string(),
                details: None,
            }),
        ))?;
    
    // Validate signature matches expected value
    // Implementation depends on collateral-intel service's signing method
    
    Ok(())
}
```

**Effort:** 12 hours total for EPIC-4.5

---

## EPIC-4.6: Collateral API Integration Tests

**Effort:** 8 hours  
**Dependencies:** EPIC-4.2-4.5 complete  
**Priority:** HIGH  

### Test Strategy

**File:** `api/tests/collateral_zkvm_integration_tests.rs` (new file)

### Test Suite

#### Test 1: CollateralCreation Integration

```rust
#[sqlx::test]
async fn test_collateral_creation_creates_transaction(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let user = create_test_user(&state).await;
    
    // Create collateral via API
    let request = CreateCollateralApiRequest {
        name: "Test Property".to_string(),
        collateral_type: Some("real_estate".to_string()),
        description: Some("Test property for zkVM".to_string()),
        address: Some("123 Test St".to_string()),
        custom_id: Some("TEST-001".to_string()),
        metadata: None,
    };
    
    let response = create_collateral_handler(
        State(state.clone()),
        create_auth_headers(&user),
        Json(request),
    )
    .await
    .expect("Collateral creation should succeed");
    
    let collateral_id = Uuid::parse_str(&response.1 .0.id).unwrap();
    
    // Verify transaction was recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE transaction_type = 'collateral_creation'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].user_id, Some(user.id));
    
    // Verify metadata contains collateral details
    let metadata = transactions[0].metadata.as_ref().unwrap();
    assert_eq!(metadata.get("collateral_type").unwrap(), "real_estate");
    assert_eq!(metadata.get("name").unwrap(), "Test Property");
}
```

#### Test 2: CollateralUpdate Integration

```rust
#[sqlx::test]
async fn test_collateral_update_creates_transaction(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let user = create_test_user(&state).await;
    
    // Create collateral first
    let collateral = create_test_collateral(&state, &user).await;
    
    // Update collateral
    let update_request = UpdateCollateralApiRequest {
        name: Some("Updated Property Name".to_string()),
        collateral_type: Some("commercial_property".to_string()),
        description: None,
        address: None,
        custom_id: None,
        metadata: None,
    };
    
    let response = update_collateral_handler(
        State(state.clone()),
        Path(collateral.id.to_string()),
        create_auth_headers(&user),
        Json(update_request),
    )
    .await
    .expect("Update should succeed");
    
    // Verify transaction recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE transaction_type = 'collateral_update'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].user_id, Some(user.id));
}
```

#### Test 3: Association Prevents Double-Pledge

```rust
#[sqlx::test]
async fn test_association_prevents_double_pledge(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let user = create_test_user(&state).await;
    
    // Create collateral and two deals
    let collateral = create_test_collateral(&state, &user).await;
    let deal1 = create_test_deal(&state, &user).await;
    let deal2 = create_test_deal(&state, &user).await;
    
    // Associate with deal1
    let assoc1_request = AssociateCollateralRequest {
        deal_id: deal1.id.to_string(),
    };
    
    let response1 = associate_collateral_handler(
        State(state.clone()),
        Path(collateral.id.to_string()),
        create_auth_headers(&user),
        Json(assoc1_request),
    )
    .await;
    
    assert!(response1.is_ok());
    
    // Try to associate with deal2 - should fail
    let assoc2_request = AssociateCollateralRequest {
        deal_id: deal2.id.to_string(),
    };
    
    let response2 = associate_collateral_handler(
        State(state.clone()),
        Path(collateral.id.to_string()),
        create_auth_headers(&user),
        Json(assoc2_request),
    )
    .await;
    
    assert!(response2.is_err());
    let (status, error) = response2.unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error.0.error.contains("AlreadyPledged"));
    
    // Verify only one association transaction
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions 
         WHERE transaction_type = 'collateral_deal_association'
         AND metadata->>'collateral_id' = $1",
        collateral.id.to_string()
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 1, "Should only have one association");
}
```

#### Test 4: Disassociation Enables Reassociation

```rust
#[sqlx::test]
async fn test_disassociation_enables_reassociation(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let user = create_test_user(&state).await;
    
    let collateral = create_test_collateral(&state, &user).await;
    let deal1 = create_test_deal(&state, &user).await;
    let deal2 = create_test_deal(&state, &user).await;
    
    // Associate with deal1
    associate_test_collateral(&state, &user, collateral.id, deal1.id).await;
    
    // Disassociate from deal1
    let disassoc_request = DisassociateCollateralRequest {
        deal_id: deal1.id.to_string(),
    };
    
    disassociate_collateral_handler(
        State(state.clone()),
        Path(collateral.id.to_string()),
        create_auth_headers(&user),
        Json(disassoc_request),
    )
    .await
    .expect("Disassociation should succeed");
    
    // Now should be able to associate with deal2
    let assoc2_request = AssociateCollateralRequest {
        deal_id: deal2.id.to_string(),
    };
    
    let response = associate_collateral_handler(
        State(state.clone()),
        Path(collateral.id.to_string()),
        create_auth_headers(&user),
        Json(assoc2_request),
    )
    .await;
    
    assert!(response.is_ok(), "Should allow reassociation after disassociation");
    
    // Verify transaction history
    let transactions = sqlx::query!(
        "SELECT transaction_type FROM deal_transactions 
         WHERE metadata->>'collateral_id' = $1 
         ORDER BY created_at ASC",
        collateral.id.to_string()
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 3);
    assert_eq!(transactions[0].transaction_type, "collateral_deal_association");
    assert_eq!(transactions[1].transaction_type, "collateral_deal_disassociation");
    assert_eq!(transactions[2].transaction_type, "collateral_deal_association");
}
```

#### Test 5: Webhook Creates Transactions

```rust
#[sqlx::test]
async fn test_detection_webhook_creates_transactions(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    
    let collateral_id = create_test_collateral_id();
    
    // Simulate webhook from collateral-intel service
    let webhook_payload = DetectionWebhookPayload {
        results: vec![
            DetectionResult {
                collateral_id: collateral_id.to_string(),
                is_new_request: true,
                requested_at: chrono::Utc::now().to_rfc3339(),
                check_type: "duplicate_detection".to_string(),
                matches_found: 2,
                confidence_score: 0.85,
                duplicate_ids: vec!["uuid1".to_string(), "uuid2".to_string()],
                status: "potential_duplicates".to_string(),
            }
        ],
    };
    
    let response = collateral_detection_webhook_handler(
        State(state.clone()),
        create_webhook_headers(),
        Json(webhook_payload),
    )
    .await;
    
    assert!(response.is_ok());
    
    // Verify both request and update transactions created
    let request_tx = sqlx::query!(
        "SELECT * FROM deal_transactions 
         WHERE transaction_type = 'collateral_detection_request'
         ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    
    assert!(request_tx.is_some());
    
    let update_tx = sqlx::query!(
        "SELECT * FROM deal_transactions 
         WHERE transaction_type = 'collateral_detection_update'
         ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    
    assert!(update_tx.is_some());
    
    // Verify metadata includes detection results
    let metadata = update_tx.unwrap().metadata.unwrap();
    assert_eq!(metadata.get("matches_found").unwrap(), 2);
    assert_eq!(metadata.get("confidence_score").unwrap(), 0.85);
}
```

**Effort:** 12 hours total for EPIC-4.5

---

## Webhook Integration

### Collateral-Intel Service Integration

**Service:** `arda-collateral-intel` (separate microservice)

**Flow:**
```
1. Deal created with collateral → Event published
2. Collateral-intel service receives event
3. Service performs duplicate detection
4. Service calls webhook with results
5. Our API receives webhook
6. Create detection request transaction (if new)
7. Create detection update transaction
8. Both flow to zkVM batch processor
```

### Webhook Endpoint Specification

**Endpoint:** `POST /api/v1/webhooks/collateral/detection`

**Authentication:** HMAC signature in header

**Request Body:**
```json
{
  "results": [
    {
      "collateral_id": "550e8400-e29b-41d4-a716-446655440000",
      "is_new_request": true,
      "requested_at": "2024-12-12T10:30:00Z",
      "check_type": "duplicate_detection",
      "matches_found": 2,
      "confidence_score": 0.85,
      "duplicate_ids": ["uuid1", "uuid2"],
      "status": "potential_duplicates"
    }
  ]
}
```

**Response:**
```json
{
  "received": 1,
  "processed": 1,
  "errors": []
}
```

### Security Considerations

**Webhook Authentication:**

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn validate_webhook_signature(
    headers: &axum::http::HeaderMap,
    body: &[u8],
    secret: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let signature = headers
        .get("x-collateral-intel-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "MissingSignature".to_string(),
                message: "Webhook signature required".to_string(),
                details: None,
            }),
        ))?;
    
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| internal_error("hmac_creation", e))?;
    
    mac.update(body);
    
    let expected = hex::encode(mac.finalize().into_bytes());
    
    if signature != expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "InvalidSignature".to_string(),
                message: "Webhook signature validation failed".to_string(),
                details: None,
            }),
        ));
    }
    
    Ok(())
}
```

---

## Testing Strategy

### Integration Test Matrix

| Test Category | Test Count | Effort | Files |
|---------------|-----------|--------|-------|
| Creation integration | 3 tests | 2 hours | collateral_zkvm_integration_tests.rs |
| Update integration | 2 tests | 1 hour | collateral_zkvm_integration_tests.rs |
| Association integration | 4 tests | 2 hours | collateral_zkvm_integration_tests.rs |
| Webhook integration | 3 tests | 2 hours | collateral_zkvm_integration_tests.rs |
| Error cases | 4 tests | 1.5 hours | collateral_zkvm_integration_tests.rs |
| **Total** | **16 tests** | **8.5 hours** | |

### Manual Testing Procedures

**Test Script:** `test_collateral_integration.sh`

```bash
#!/bin/bash
set -e

echo "=== Testing Collateral zkVM Integration ==="

# 1. Create collateral
echo "1. Creating collateral..."
COLL_RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/collateral \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "Test Property",
    "collateral_type": "real_estate",
    "address": "123 Test St"
  }')

COLL_ID=$(echo $COLL_RESPONSE | jq -r '.id')
echo "   Collateral created: $COLL_ID"

# Verify transaction
psql $DATABASE_URL -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE transaction_type = 'collateral_creation'
" | grep -q "1" && echo "   ✅ CollateralCreation transaction recorded"

# 2. Update collateral
echo "2. Updating collateral..."
curl -s -X PUT "http://localhost:8080/api/v1/collateral/$COLL_ID" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "Updated Property Name"
  }' > /dev/null

psql $DATABASE_URL -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE transaction_type = 'collateral_update'
" | grep -q "1" && echo "   ✅ CollateralUpdate transaction recorded"

# 3. Create deal
echo "3. Creating deal..."
DEAL_RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{
    \"originator_id\":\"$ORIGINATOR_ID\",
    \"investor_id\":\"$INVESTOR_ID\",
    \"principal_amount\":\"100000.00\",
    \"interest_rate_bps\":500
  }")

DEAL_ID=$(echo $DEAL_RESPONSE | jq -r '.deal_id')
echo "   Deal created: $DEAL_ID"

# 4. Associate collateral with deal
echo "4. Associating collateral with deal..."
curl -s -X POST "http://localhost:8080/api/v1/collateral/$COLL_ID/associate" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{\"deal_id\":\"$DEAL_ID\"}" > /dev/null

psql $DATABASE_URL -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE transaction_type = 'collateral_deal_association'
" | grep -q "1" && echo "   ✅ CollateralDealAssociation transaction recorded"

# 5. Try double-pledge (should fail)
echo "5. Testing double-pledge prevention..."
DEAL2_ID=$(create_second_deal)

DOUBLE_PLEDGE_RESPONSE=$(curl -s -w "%{http_code}" -X POST \
  "http://localhost:8080/api/v1/collateral/$COLL_ID/associate" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{\"deal_id\":\"$DEAL2_ID\"}")

if echo "$DOUBLE_PLEDGE_RESPONSE" | grep -q "400"; then
    echo "   ✅ Double-pledge prevented at API level"
else
    echo "   ❌ Double-pledge was not prevented!"
    exit 1
fi

# 6. Trigger batch processing
echo "6. Triggering batch..."
curl -s -X POST http://localhost:8080/api/v1/batch/trigger \
  -H "Authorization: Bearer $TOKEN" > /dev/null

sleep 5

# 7. Verify transactions batched and prove
BATCHED=$(psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE transaction_type LIKE 'collateral%'
    AND included_in_batch_id IS NOT NULL
" | tr -d ' ')

echo "   Batched transactions: $BATCHED"
[[ "$BATCHED" -ge "3" ]] && echo "   ✅ Collateral transactions batched"

echo ""
echo "=== Collateral Integration Test Complete ==="
```

---

## Validation Checklist

### EPIC-4 Complete When:

**Endpoints Updated:**
- [ ] CollateralCreation uses TransactionRecordingService
- [ ] CollateralUpdate uses TransactionRecordingService
- [ ] Association endpoint exists and records transactions
- [ ] Disassociation endpoint exists and records transactions
- [ ] Webhook handler records detection transactions

**Transaction Recording:**
- [ ] All 6 collateral transaction types can be created
- [ ] Metadata formatted correctly for zkVM
- [ ] Transaction IDs returned to callers
- [ ] Errors handled gracefully

**Integration:**
- [ ] Collateral API → deal_transactions → batch processor
- [ ] Webhook → deal_transactions → batch processor
- [ ] Event publisher still works (backward compatible)
- [ ] No breaking changes to existing API

**Testing:**
- [ ] 16+ integration tests passing
- [ ] Manual test script passes
- [ ] Double-pledge prevention tested
- [ ] Webhook integration tested
- [ ] Code coverage >80%

**Documentation:**
- [ ] All endpoints documented in Swagger
- [ ] Webhook specification documented
- [ ] Integration guide for collateral-intel team
- [ ] Handoff guide for WS-5 (testing)

---

## Timeline

### Day-by-Day Breakdown

**Day 1 (8 hours):**
- EPIC-4.1: Audit existing endpoints (4 hours)
- EPIC-4.2: Update CollateralCreation (4 hours)
- **Deliverable:** Creation integrated, audit complete

**Day 2 (8 hours):**
- EPIC-4.3: Update CollateralUpdate (4 hours)
- EPIC-4.4: Association endpoints (start, 4 hours)
- **Deliverable:** Update integrated, association planned

**Day 3 (8 hours):**
- EPIC-4.4: Association endpoints (finish, 4 hours)
- EPIC-4.5: Webhook integration (start, 4 hours)
- **Deliverable:** Association complete, webhook started

**Day 4 (8 hours):**
- EPIC-4.5: Webhook integration (finish, 8 hours)
- **Deliverable:** All endpoints integrated

**Day 5 (4 hours):**
- EPIC-4.6: Integration tests (4 hours of 8)
- **Deliverable:** Core tests complete

**Total: 4.5 days (36 hours)**

---

## Success Metrics

### Definition of Done

**EPIC-4 is complete when:**

1. ✅ All collateral CRUD operations create transactions
2. ✅ Association/disassociation endpoints working
3. ✅ Webhooks create detection transactions
4. ✅ All 6 collateral transaction types can be triggered
5. ✅ Integration tests passing (16+ tests)
6. ✅ Manual test script succeeds
7. ✅ No breaking changes to existing API
8. ✅ Documentation complete
9. ✅ Ready for WS-5 (E2E testing with zkVM)

### Measurement

```bash
# Verify all 6 transaction types can be created
psql $DATABASE_URL -c "
  SELECT transaction_type, COUNT(*) 
  FROM deal_transactions 
  WHERE transaction_type LIKE 'collateral%'
  GROUP BY transaction_type;
"

# Expected output:
# collateral_creation              | 5
# collateral_update                | 2
# collateral_deal_association      | 3
# collateral_deal_disassociation   | 1
# collateral_detection_request     | 4
# collateral_detection_update      | 4
```

---

**End of Work Stream 4 Implementation Plan**

**Next:** After EPIC-4 is complete, WS-5 (Testing & Validation) can perform end-to-end tests with complete collateral zkVM integration.
