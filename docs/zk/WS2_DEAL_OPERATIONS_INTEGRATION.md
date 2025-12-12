# Work Stream 2: Core Deal Operations Integration

**Epic:** EPIC-2  
**Priority:** HIGH  
**Effort:** 3.5 days (28 hours)  
**Owner:** Backend Engineer #1  
**Dependencies:** EPIC-1 (TransactionRecordingService) must be complete  

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Requirements](#technical-requirements)
3. [Sub-Issue Implementation Guides](#sub-issue-implementation-guides)
4. [Testing Strategy](#testing-strategy)
5. [API Documentation](#api-documentation)
6. [Integration Validation](#integration-validation)
7. [Deployment Considerations](#deployment-considerations)

---

## Overview

### Purpose

Integrate the four core deal and user operations with the zkVM proof system by adding transaction recording to existing API endpoints and creating new endpoints where needed. This work stream enables user registration, deal creation, payments, and closeouts to flow through the ZK proof system.

### Context

**Current State:**
- ✅ `POST /api/v1/user` - Creates user in database
- ✅ `POST /api/v1/deal` - Creates deal in database
- ❌ No payment endpoint exists
- ❌ No closeout endpoint exists
- ❌ None of these create deal_transactions records

**Target State:**
- ✅ User creation records UserRegistration transaction
- ✅ Deal creation records DealOrigination transaction
- ✅ NEW: Payment endpoint records DealPayment transaction
- ✅ NEW: Closeout endpoint records DealCloseout transaction
- ✅ All transactions flow through zkVM batch processor

### Scope (4 Transaction Types)

1. **UserRegistration (#1)** - Modify existing endpoint
2. **DealOrigination (#4)** - Modify existing endpoint
3. **DealPayment (#5)** - Create new endpoint + integrate
4. **DealCloseout (#6)** - Create new endpoint + integrate

**Out of Scope:** KYC, deposits, withdrawals, admin operations (per requirements)

---

## Technical Requirements

### Functional Requirements

**FR-1: UserRegistration Integration**
- Existing user creation endpoint must call TransactionRecordingService
- Transaction recorded synchronously (in same HTTP request)
- Metadata includes email and user_type
- If transaction recording fails, API returns error

**FR-2: DealOrigination Integration**
- Existing deal creation endpoint must call TransactionRecordingService
- Transaction recorded after deal is created in database
- Metadata encodes originator_id and investor_id (first 32 bytes each) for zkVM
- Amount equals principal_amount
- If transaction recording fails, consider rolling back deal creation

**FR-3: DealPayment Endpoint Creation**
- New endpoint: `POST /api/v1/deal/{deal_id}/payment`
- Validates deal exists and is active/originated
- Creates payment record in database
- Updates deal outstanding_balance
- Records DealPayment transaction
- Returns payment details

**FR-4: DealCloseout Endpoint Creation**
- New endpoint: `POST /api/v1/deal/{deal_id}/closeout`
- Validates deal is paid_off (outstanding_balance = 0)
- Validates user is investor (permission check)
- Updates deal status to closed
- Records DealCloseout transaction
- Returns deal details

### Non-Functional Requirements

**NFR-1: Backward Compatibility**
- Existing endpoints continue to work
- Responses maintain same structure
- Only add transaction recording, don't change behavior

**NFR-2: Transaction Atomicity**
- If deal_transactions recording fails, decide whether to fail entire request
- Recommendation: Fail entire request for consistency
- Alternative: Log error and continue (eventual consistency)

**NFR-3: Performance**
- Transaction recording should add <50ms per request
- Batch operations should complete in <200ms
- No regression in existing endpoint latency

**NFR-4: Error Messages**
- Clear error messages for validation failures
- Distinguish between business errors and zkVM recording errors
- Log enough context for debugging

---

## Sub-Issue Implementation Guides

## EPIC-2.1: Integrate UserRegistration

**Effort:** 4 hours  
**Dependencies:** EPIC-1 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate User Creation Endpoint

**Find the endpoint:**
```bash
# Search for user creation handler
grep -r "create_user\|POST.*user" api/src/handlers/users/
```

**Expected file:** `api/src/handlers/users/users.rs`

**Expected function:** Something like `create_user_endpoint`, `register_user`, or similar

#### Step 2: Review Current Implementation

**Current pattern (example):**
```rust
pub async fn create_user_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    // 1. Validate email
    // 2. Check if user exists
    // 3. Create user in database
    // 4. Return response
}
```

#### Step 3: Add TransactionRecordingService Import

**Add to imports at top of file:**
```rust
use crate::services::TransactionRecordingService;
use arda_credit_lib::TransactionType;
use serde_json::json;
```

#### Step 4: Add Transaction Recording Call

**Insert after user creation, before return:**

```rust
pub async fn create_user_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    // ... existing validation ...
    
    // Create user in database
    let user = create_identity(&state.pool, &request.user_type, &request.email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // NEW: Record transaction for zkVM
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::UserRegistration,
            user.id,
            None, // No deal_id for user registration
            Decimal::ZERO, // No amount for user registration
            Some(json!({
                "email": request.email.clone(),
                "user_type": request.user_type.clone(),
            })),
        )
        .await
        .map_err(|e| {
            error!("Failed to record user registration transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("User created but transaction recording failed: {}", e)
            )
        })?;
    
    info!("User created with transaction {}", tx_id);
    
    // Return response
    Ok(Json(UserResponse {
        user_id: user.id.to_string(),
        email: user.email,
        user_type: user.user_type,
        kyc_status: user.kyc_status,
        created_at: user.created_at,
    }))
}
```

#### Step 5: Add Error Handling Discussion

**Decision Point:** What happens if transaction recording fails?

**Option A: Fail the entire request (Recommended)**
```rust
// If recording fails, return error to user
.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
```
- ✅ Ensures consistency (user + transaction both exist or neither)
- ❌ User creation might fail even though user was created in DB
- 💡 Consider adding rollback if needed

**Option B: Log and continue**
```rust
// If recording fails, log but don't fail request
if let Err(e) = state.transaction_recording.record_transaction(...).await {
    error!("Failed to record transaction but continuing: {}", e);
    metrics::counter!("zkvm_recording_failures_total").increment(1);
}
```
- ✅ User gets their account even if recording fails
- ❌ Transaction won't be proven in zkVM
- ❌ Creates inconsistent state

**Recommendation:** Use Option A for MVP, add rollback mechanism later if needed

#### Step 6: Test the Integration

**Manual test:**
```bash
# Start server
make server

# Create user
curl -X POST http://localhost:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "user_type": "investor"
  }'

# Check deal_transactions table
psql $DATABASE_URL -c "
  SELECT id, transaction_type, user_id, amount 
  FROM deal_transactions 
  WHERE transaction_type = 'user_registration' 
  ORDER BY created_at DESC 
  LIMIT 1;
"

# Should see a record!
```

**Validation:**
- [ ] User created in `users` table
- [ ] Transaction created in `deal_transactions` table
- [ ] Transaction type is "user_registration"
- [ ] user_id matches between tables
- [ ] API returns 201 Created

**Effort:** 4 hours total

---

## EPIC-2.2: Integrate DealOrigination

**Effort:** 4 hours  
**Dependencies:** EPIC-1 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Locate Deal Creation Endpoint

**File:** `api/src/handlers/deals/deals.rs`  
**Function:** `create_deal_endpoint` (line ~616 based on audit)

#### Step 2: Review Current Implementation

**Current structure:**
```rust
pub async fn create_deal_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateDealRequest>,
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // 1. Parse UUIDs (originator_id, investor_id, borrower_id)
    // 2. Validate amounts and terms
    // 3. Create deal in database
    // 4. Return response
}
```

#### Step 3: Add Transaction Recording

**Insert after deal creation (around line ~750 based on audit):**

```rust
pub async fn create_deal_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateDealRequest>,
) -> Result<Json<DealDetailsResponse>, (StatusCode, String)> {
    // ... existing validation and parsing ...
    
    // Create deal in database (existing code)
    let deal = create_deal(
        &state.pool,
        originator_id,
        investor_id,
        borrower_id,
        principal_amount,
        interest_rate_bps,
        maturity_date,
        // ... other parameters
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // NEW: Record transaction for zkVM
    // CRITICAL: Metadata must encode originator_id and investor_id for zkVM
    let investor_id_for_zkvm = investor_id.unwrap_or(Uuid::nil());
    
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::DealOrigination,
            originator_id,
            Some(deal.id),
            deal.principal_amount,
            Some(json!({
                // For zkVM: encode IDs in metadata
                // zkVM expects first 32 bytes = originator, next 32 = investor
                "originator_id": originator_id.to_string(),
                "investor_id": investor_id_for_zkvm.to_string(),
                "interest_rate_bps": deal.interest_rate_bps,
                "term_days": request.term_days,
            })),
        )
        .await
        .map_err(|e| {
            error!("Failed to record deal origination transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Deal created but transaction recording failed: {}", e)
            )
        })?;
    
    info!(
        "Deal {} created with transaction {}",
        deal.id,
        tx_id
    );
    
    // ... existing response formatting ...
}
```

#### Step 4: Handle Metadata Encoding for zkVM

**CRITICAL:** The zkVM expects metadata formatted specifically:

From `lib/src/deal_processor.rs` lines 156-160:
```rust
let metadata = transaction.metadata.as_bytes();
let mut borrower_id = [0u8; 32];
let mut lender_id = [0u8; 32];
borrower_id.copy_from_slice(&metadata[0..32]);
lender_id.copy_from_slice(&metadata[32..64]);
```

**This means metadata needs originator and investor UUIDs in first 64 bytes!**

**Two approaches:**

**Approach A: Encode in metadata JSON (simpler)**
```rust
// Store UUIDs as strings in JSON
// Batch processor will need to parse and encode
let metadata = Some(json!({
    "originator_id": originator_id.to_string(),
    "investor_id": investor_id.to_string(),
    // ... other fields
}));
```

**Approach B: Encode as binary in metadata (matches zkVM exactly)**
```rust
// Create byte array with originator and investor UUIDs
let mut metadata_bytes = vec![0u8; 64];
metadata_bytes[0..16].copy_from_slice(originator_id.as_bytes());
metadata_bytes[32..48].copy_from_slice(investor_id.as_bytes());

// Store as base64 in JSON for compatibility
let metadata = Some(json!({
    "zkvm_bytes": base64::encode(&metadata_bytes),
    "originator_id": originator_id.to_string(), // Human-readable
    "investor_id": investor_id.to_string(),
}));
```

**Recommendation:** Use Approach A for now, let batch processor handle encoding. Simpler and more debuggable.

**Note for batch processor team:** Will need to handle this encoding when converting to zkVM format.

#### Step 5: Test the Integration

**Manual test:**
```bash
# Create two companies first (originator and investor)
ORIGINATOR_ID=$(curl -X POST http://localhost:8080/api/v1/company \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Originator Corp"}' | jq -r '.company_id')

INVESTOR_ID=$(curl -X POST http://localhost:8080/api/v1/company \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Investor Corp"}' | jq -r '.company_id')

# Create deal
curl -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d "{
    \"originator_id\": \"$ORIGINATOR_ID\",
    \"investor_id\": \"$INVESTOR_ID\",
    \"principal_amount\": \"100000.00\",
    \"interest_rate_bps\": 500,
    \"term_days\": 365
  }"

# Verify transaction recorded
psql $DATABASE_URL -c "
  SELECT id, transaction_type, deal_id, amount 
  FROM deal_transactions 
  WHERE transaction_type = 'deal_origination' 
  ORDER BY created_at DESC 
  LIMIT 1;
"
```

**Validation:**
- [ ] Deal created in `deals` table
- [ ] Transaction created in `deal_transactions` table
- [ ] Transaction type is "deal_origination"
- [ ] deal_id matches
- [ ] amount equals principal_amount
- [ ] Metadata contains originator and investor IDs

**Effort:** 4 hours total

---

## EPIC-2.3: Create Payment Endpoint

**Effort:** 8 hours  
**Dependencies:** EPIC-1 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Define Request/Response Types

**File:** `api/src/handlers/deals/deals.rs`

**Add to file:**

```rust
/// Request to make a payment on a deal
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct MakePaymentRequest {
    /// Payment amount in USDC (e.g., "5000.00")
    pub amount: String,
    
    /// Payment type (optional, defaults to "regular")
    #[serde(default = "default_payment_type")]
    pub payment_type: Option<String>,
    
    /// Optional notes about the payment
    pub notes: Option<String>,
}

fn default_payment_type() -> Option<String> {
    Some("regular".to_string())
}

/// Response from payment creation
#[derive(Debug, Serialize, ToSchema)]
pub struct PaymentResponse {
    /// Payment ID
    pub payment_id: String,
    
    /// Deal ID
    pub deal_id: String,
    
    /// Payment amount
    pub amount: String,
    
    /// Payment type
    pub payment_type: String,
    
    /// Updated deal outstanding balance
    pub new_outstanding_balance: String,
    
    /// Deal status after payment
    pub deal_status: String,
    
    /// Transaction ID (for zkVM tracking)
    pub transaction_id: String,
    
    /// When payment was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

#### Step 2: Implement Payment Endpoint

**Add to `api/src/handlers/deals/deals.rs`:**

```rust
/// Make a payment on a deal
///
/// Creates a payment record, updates the deal's outstanding balance,
/// and records a transaction for zkVM processing.
///
/// The deal status may transition:
/// - originated → active (first payment)
/// - active → paid_off (if balance reaches 0)
///
/// # Validations
/// - Deal must exist
/// - Deal must be in originated or active status
/// - Payment amount must be > 0
/// - Payment amount must be <= outstanding balance
/// - User must have permission (originator or authorized party)
///
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/payment",
    request_body = MakePaymentRequest,
    responses(
        (status = 200, description = "Payment created successfully", body = PaymentResponse),
        (status = 400, description = "Invalid request - check amount or deal status"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - user doesn't have permission"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn make_payment_endpoint(
    State(state): State<ApiState>,
    Path(deal_id_str): Path<String>,
    Json(request): Json<MakePaymentRequest>,
) -> Result<Json<PaymentResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    // Record metrics
    metrics::counter!("api_payment_requests_total").increment(1);
    
    // 1. Parse deal_id
    let deal_id = Uuid::parse_str(&deal_id_str).map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid deal ID format".to_string())
    })?;
    
    // 2. Parse amount
    let amount = Decimal::from_str(&request.amount).map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid amount format".to_string())
    })?;
    
    if amount <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Payment amount must be greater than zero".to_string()
        ));
    }
    
    // 3. Get deal and validate status
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?;
    
    if deal.deal_status != "originated" && deal.deal_status != "active" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot make payment on deal with status '{}'. Must be 'originated' or 'active'.",
                deal.deal_status
            )
        ));
    }
    
    // 4. Validate payment amount doesn't exceed outstanding balance
    if amount > deal.outstanding_balance {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Payment amount ${} exceeds outstanding balance ${}",
                amount,
                deal.outstanding_balance
            )
        ));
    }
    
    // 5. Get authenticated user (from session middleware)
    // TODO: Extract user_id from session/auth context
    // For now, assume originator is making payment
    let payer_id = deal.originator_id;
    
    // 6. Create payment record in database
    let payment_id = Uuid::new_v4();
    let payment_type = request.payment_type.unwrap_or_else(|| "regular".to_string());
    
    sqlx::query!(
        r#"
        INSERT INTO deal_payments (
            id, deal_id, payer_id, amount, payment_type, notes, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        "#,
        payment_id,
        deal_id,
        payer_id,
        amount,
        payment_type,
        request.notes,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to create payment record: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // 7. Update deal outstanding balance
    let new_balance = deal.outstanding_balance - amount;
    
    // Determine new deal status
    let new_status = if new_balance == Decimal::ZERO {
        "paid_off"
    } else if deal.deal_status == "originated" {
        "active" // First payment activates the deal
    } else {
        &deal.deal_status // Stay in current status
    };
    
    sqlx::query!(
        r#"
        UPDATE deals 
        SET outstanding_balance = $1,
            deal_status = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        new_balance,
        new_status,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to update deal balance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // 8. Record transaction for zkVM
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::DealPayment,
            payer_id,
            Some(deal_id),
            amount,
            Some(json!({
                "payment_id": payment_id.to_string(),
                "payment_type": payment_type.clone(),
                "notes": request.notes,
                "previous_balance": deal.outstanding_balance.to_string(),
                "new_balance": new_balance.to_string(),
                "status_transition": format!("{} → {}", deal.deal_status, new_status),
            })),
        )
        .await
        .map_err(|e| {
            error!("Failed to record payment transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Payment created but transaction recording failed: {}", e)
            )
        })?;
    
    let duration = start_time.elapsed();
    info!(
        "Payment {} created for deal {} in {:?}, transaction {}",
        payment_id,
        deal_id,
        duration,
        tx_id
    );
    
    metrics::histogram!("api_payment_duration_seconds").record(duration.as_secs_f64());
    
    // 9. Return response
    Ok(Json(PaymentResponse {
        payment_id: payment_id.to_string(),
        deal_id: deal_id.to_string(),
        amount: amount.to_string(),
        payment_type,
        new_outstanding_balance: new_balance.to_string(),
        deal_status: new_status.to_string(),
        transaction_id: tx_id.to_string(),
        created_at: chrono::Utc::now(),
    }))
}
```

#### Step 3: Add Route to Router

**File:** `api/src/api/router.rs`

**Find the deals routes section** (search for `/api/v1/deal`)

**Add:**
```rust
// Deal operations
.route(paths::DEAL_CREATE, post(deals::create_deal_endpoint))
.route(paths::DEAL_GET, get(deals::get_deal_endpoint))
.route(paths::DEAL_UPDATE, put(deals::update_deal_endpoint))
// NEW: Payment endpoint
.route("/api/v1/deal/:deal_id/payment", post(deals::make_payment_endpoint))
```

**Or define in paths.rs:**
```rust
// In api/src/config/paths.rs
pub const DEAL_PAYMENT: &str = "/api/v1/deal/:deal_id/payment";

// Then use:
.route(paths::DEAL_PAYMENT, post(deals::make_payment_endpoint))
```

#### Step 4: Update OpenAPI Documentation

**File:** `api/src/api/openapi.rs`

The `#[utoipa::path]` macro should automatically add the endpoint to OpenAPI spec if your ApiDoc includes the deals module.

**Verify in ApiDoc struct:**
```rust
#[derive(OpenApi)]
#[openapi(
    paths(
        // ... existing paths ...
        deals::make_payment_endpoint,  // NEW
    ),
    components(schemas(
        MakePaymentRequest,  // NEW
        PaymentResponse,     // NEW
        // ... existing schemas ...
    ))
)]
struct ApiDoc;
```

#### Step 5: Test Payment Endpoint

**Manual test:**
```bash
# Create a deal first
DEAL_ID=$(curl -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "originator_id": "...",
    "investor_id": "...",
    "principal_amount": "100000.00",
    "interest_rate_bps": 500
  }' | jq -r '.deal_id')

# Make a payment
curl -X POST "http://localhost:8080/api/v1/deal/$DEAL_ID/payment" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "amount": "25000.00",
    "notes": "First payment"
  }'

# Verify in database
psql $DATABASE_URL -c "
  SELECT 
    dt.transaction_type, 
    dt.amount, 
    dt.deal_id,
    d.outstanding_balance,
    d.deal_status
  FROM deal_transactions dt
  JOIN deals d ON d.id = dt.deal_id
  WHERE dt.transaction_type = 'deal_payment'
    AND dt.deal_id = '$DEAL_ID'
  ORDER BY dt.created_at DESC
  LIMIT 1;
"
```

**Expected:**
- Deal outstanding_balance reduced by payment amount
- Deal status changed to "active" (if was "originated")
- Payment record created in deal_payments
- Transaction record created in deal_transactions

**Validation:**
- [ ] Payment creates payment record
- [ ] Payment updates deal balance
- [ ] Payment updates deal status appropriately
- [ ] Payment creates deal_transactions record
- [ ] API returns correct response
- [ ] Swagger UI shows new endpoint

**Effort:** 8 hours total

---

## EPIC-2.4: Create Closeout Endpoint

**Effort:** 4 hours  
**Dependencies:** EPIC-1 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Define Request/Response Types

```rust
/// Request to close out a paid-off deal
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CloseoutDealRequest {
    /// Optional notes about the closeout
    pub notes: Option<String>,
    
    /// Confirmation that deal is fully paid (required)
    pub confirm_paid_off: bool,
}

/// Response from deal closeout
#[derive(Debug, Serialize, ToSchema)]
pub struct CloseoutResponse {
    /// Deal ID
    pub deal_id: String,
    
    /// Deal status (should be "closed")
    pub deal_status: String,
    
    /// Transaction ID (for zkVM tracking)
    pub transaction_id: String,
    
    /// When closeout was completed
    pub closed_at: chrono::DateTime<chrono::Utc>,
}
```

#### Step 2: Implement Closeout Endpoint

```rust
/// Close out a paid-off deal
///
/// Marks a deal as closed after it has been fully paid off.
/// This is the final state in the deal lifecycle.
///
/// # Validations
/// - Deal must exist
/// - Deal must have status "paid_off"
/// - Outstanding balance must be exactly 0
/// - User must be the investor (permission check)
/// - User must confirm the closeout
///
#[utoipa::path(
    post,
    path = "/api/v1/deal/{deal_id}/closeout",
    request_body = CloseoutDealRequest,
    responses(
        (status = 200, description = "Deal closed successfully", body = CloseoutResponse),
        (status = 400, description = "Invalid request - check deal status or confirmation"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - only investor can close deal"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
pub async fn closeout_deal_endpoint(
    State(state): State<ApiState>,
    Path(deal_id_str): Path<String>,
    Json(request): Json<CloseoutDealRequest>,
) -> Result<Json<CloseoutResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    metrics::counter!("api_closeout_requests_total").increment(1);
    
    // 1. Parse deal_id
    let deal_id = Uuid::parse_str(&deal_id_str).map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid deal ID format".to_string())
    })?;
    
    // 2. Validate confirmation
    if !request.confirm_paid_off {
        return Err((
            StatusCode::BAD_REQUEST,
            "Must confirm deal is paid off (confirm_paid_off: true)".to_string()
        ));
    }
    
    // 3. Get deal and validate status
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Deal not found".to_string()))?;
    
    if deal.deal_status != "paid_off" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot close deal with status '{}'. Must be 'paid_off' (fully paid).",
                deal.deal_status
            )
        ));
    }
    
    if deal.outstanding_balance != Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot close deal with outstanding balance ${}. Must be fully paid.",
                deal.outstanding_balance
            )
        ));
    }
    
    // 4. Get authenticated user and check permission
    // TODO: Extract user_id from session/auth middleware
    // For now, assume investor is closing
    let investor_id = deal.investor_id.ok_or((
        StatusCode::BAD_REQUEST,
        "Deal has no investor assigned".to_string()
    ))?;
    
    // TODO: Add permission check
    // if user_id != investor_id {
    //     return Err((StatusCode::FORBIDDEN, "Only investor can close deal"));
    // }
    
    // 5. Update deal status to closed
    sqlx::query!(
        r#"
        UPDATE deals 
        SET deal_status = 'closed',
            updated_at = NOW()
        WHERE id = $1
        "#,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to close deal: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // 6. Record transaction for zkVM
    let tx_id = state.transaction_recording
        .record_transaction(
            TransactionType::DealCloseout,
            investor_id,
            Some(deal_id),
            Decimal::ZERO, // No amount for closeout
            Some(json!({
                "notes": request.notes,
                "closed_by": investor_id.to_string(),
                "original_principal": deal.principal_amount.to_string(),
                "final_status": "closed",
            })),
        )
        .await
        .map_err(|e| {
            error!("Failed to record closeout transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Deal closed but transaction recording failed: {}", e)
            )
        })?;
    
    let duration = start_time.elapsed();
    info!(
        "Deal {} closed in {:?}, transaction {}",
        deal_id,
        duration,
        tx_id
    );
    
    metrics::histogram!("api_closeout_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("api_closeout_success_total").increment(1);
    
    // 7. Return response
    Ok(Json(CloseoutResponse {
        deal_id: deal_id.to_string(),
        deal_status: "closed".to_string(),
        transaction_id: tx_id.to_string(),
        closed_at: chrono::Utc::now(),
    }))
}
```

#### Step 3: Add Route to Router

**File:** `api/src/api/router.rs`

```rust
.route("/api/v1/deal/:deal_id/closeout", post(deals::closeout_deal_endpoint))
```

#### Step 4: Test Closeout Endpoint

**Manual test - complete flow:**

```bash
# 1. Create deal
DEAL_ID=$(curl -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -d '{
    "originator_id": "...",
    "investor_id": "...",
    "principal_amount": "10000.00",
    "interest_rate_bps": 500
  }' | jq -r '.deal_id')

# 2. Make payment to pay off deal
curl -X POST "http://localhost:8080/api/v1/deal/$DEAL_ID/payment" \
  -H "Content-Type: application/json" \
  -d '{
    "amount": "10000.00"
  }'

# 3. Close out deal
curl -X POST "http://localhost:8080/api/v1/deal/$DEAL_ID/closeout" \
  -H "Content-Type: application/json" \
  -d '{
    "confirm_paid_off": true,
    "notes": "Deal completed successfully"
  }'

# 4. Verify transaction history
psql $DATABASE_URL -c "
  SELECT transaction_type, amount, created_at 
  FROM deal_transactions 
  WHERE deal_id = '$DEAL_ID' 
  ORDER BY created_at ASC;
"

# Should see:
# deal_origination | 10000.00 | timestamp1
# deal_payment     | 10000.00 | timestamp2
# deal_closeout    | 0.00     | timestamp3
```

**Validation:**
- [ ] Deal status changed to "closed"
- [ ] Closeout transaction recorded
- [ ] Can't make payments after closeout
- [ ] Audit trail shows complete lifecycle

**Effort:** 4 hours total

---

## EPIC-2.5: Integration Tests

**Effort:** 8 hours  
**Dependencies:** EPIC-2.1-2.4 complete  
**Priority:** HIGH  

### Test File Structure

**File:** `api/tests/deal_zkvm_integration_tests.rs`

### Test Suite

#### Test 1: UserRegistration Flow

```rust
use api::services::TransactionRecordingService;
use arda_credit_db::*;
use sqlx::PgPool;

#[sqlx::test]
async fn test_user_registration_creates_transaction(pool: PgPool) {
    // Setup
    let state = create_test_api_state(pool.clone()).await;
    
    // Create user via API
    let request = CreateUserRequest {
        email: "test@example.com".to_string(),
        user_type: "investor".to_string(),
    };
    
    let response = create_user_endpoint(
        State(state.clone()),
        Json(request),
    )
    .await
    .expect("User creation should succeed");
    
    let user_id = Uuid::parse_str(&response.0.user_id).unwrap();
    
    // Verify user exists
    let user = get_identity_by_id(&pool, user_id)
        .await
        .expect("User should exist");
    
    assert_eq!(user.email, "test@example.com");
    
    // Verify transaction was recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE user_id = $1",
        user_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].transaction_type, "user_registration");
    assert_eq!(transactions[0].amount, Decimal::ZERO);
    assert!(transactions[0].included_in_batch_id.is_none()); // Not batched yet
}
```

#### Test 2: DealOrigination Flow

```rust
#[sqlx::test]
async fn test_deal_creation_creates_transaction(pool: PgPool) {
    // Setup
    let state = create_test_api_state(pool.clone()).await;
    
    // Create companies
    let originator = create_test_company(&pool, "Originator Corp").await;
    let investor = create_test_company(&pool, "Investor Corp").await;
    
    // Create deal via API
    let request = CreateDealRequest {
        originator_id: originator.id.to_string(),
        investor_id: Some(investor.id.to_string()),
        principal_amount: "100000.00".to_string(),
        interest_rate_bps: 500,
        term_days: Some(365),
        // ... other fields
    };
    
    let response = create_deal_endpoint(
        State(state.clone()),
        Json(request),
    )
    .await
    .expect("Deal creation should succeed");
    
    let deal_id = Uuid::parse_str(&response.0.deal_id).unwrap();
    
    // Verify deal exists
    let deal = get_deal_by_id(&pool, deal_id)
        .await
        .expect("Deal should exist");
    
    assert_eq!(deal.principal_amount, Decimal::from_str("100000.00").unwrap());
    
    // Verify transaction was recorded
    let transactions = sqlx::query!(
        "SELECT * FROM deal_transactions WHERE deal_id = $1",
        deal_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].transaction_type, "deal_origination");
    assert_eq!(transactions[0].amount, Decimal::from_str("100000.00").unwrap());
    
    // Verify metadata contains originator and investor IDs
    let metadata = transactions[0].metadata.as_ref().unwrap();
    assert!(metadata.get("originator_id").is_some());
    assert!(metadata.get("investor_id").is_some());
}
```

#### Test 3: Complete Deal Lifecycle

```rust
#[sqlx::test]
async fn test_complete_deal_lifecycle_with_transactions(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    
    // 1. Create deal
    let deal_id = create_test_deal(&state).await;
    
    // 2. Make partial payment
    let payment1_response = make_payment_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(MakePaymentRequest {
            amount: "50000.00".to_string(),
            payment_type: None,
            notes: Some("First payment".to_string()),
        }),
    )
    .await
    .expect("First payment should succeed");
    
    assert_eq!(payment1_response.0.new_outstanding_balance, "50000.00");
    assert_eq!(payment1_response.0.deal_status, "active");
    
    // 3. Make final payment
    let payment2_response = make_payment_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(MakePaymentRequest {
            amount: "50000.00".to_string(),
            payment_type: None,
            notes: Some("Final payment".to_string()),
        }),
    )
    .await
    .expect("Final payment should succeed");
    
    assert_eq!(payment2_response.0.new_outstanding_balance, "0.00");
    assert_eq!(payment2_response.0.deal_status, "paid_off");
    
    // 4. Close out deal
    let closeout_response = closeout_deal_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(CloseoutDealRequest {
            notes: Some("Deal completed".to_string()),
            confirm_paid_off: true,
        }),
    )
    .await
    .expect("Closeout should succeed");
    
    assert_eq!(closeout_response.0.deal_status, "closed");
    
    // 5. Verify complete transaction history
    let transactions = sqlx::query!(
        "SELECT transaction_type, amount 
         FROM deal_transactions 
         WHERE deal_id = $1 
         ORDER BY created_at ASC",
        deal_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    
    assert_eq!(transactions.len(), 4); // origination + 2 payments + closeout
    assert_eq!(transactions[0].transaction_type, "deal_origination");
    assert_eq!(transactions[1].transaction_type, "deal_payment");
    assert_eq!(transactions[2].transaction_type, "deal_payment");
    assert_eq!(transactions[3].transaction_type, "deal_closeout");
    
    // Verify amounts
    assert_eq!(transactions[0].amount, Decimal::from_str("100000.00").unwrap());
    assert_eq!(transactions[1].amount, Decimal::from_str("50000.00").unwrap());
    assert_eq!(transactions[2].amount, Decimal::from_str("50000.00").unwrap());
    assert_eq!(transactions[3].amount, Decimal::ZERO);
}
```

#### Test 4: Error Cases

```rust
#[sqlx::test]
async fn test_payment_exceeds_balance_fails(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let deal_id = create_test_deal(&state).await;
    
    // Try to pay more than outstanding balance
    let result = make_payment_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(MakePaymentRequest {
            amount: "200000.00".to_string(), // Deal is only $100k
            payment_type: None,
            notes: None,
        }),
    )
    .await;
    
    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("exceeds outstanding balance"));
    
    // Verify no transaction was recorded
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM deal_transactions 
         WHERE deal_id = $1 AND transaction_type = 'deal_payment'",
        deal_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert_eq!(count.count, Some(0));
}

#[sqlx::test]
async fn test_closeout_without_payoff_fails(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    let deal_id = create_test_deal(&state).await;
    
    // Try to close deal that isn't paid off
    let result = closeout_deal_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(CloseoutDealRequest {
            notes: None,
            confirm_paid_off: true,
        }),
    )
    .await;
    
    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("Must be 'paid_off'"));
}

#[sqlx::test]
async fn test_payment_on_closed_deal_fails(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    
    // Create and close deal
    let deal_id = create_and_close_test_deal(&state).await;
    
    // Try to make payment on closed deal
    let result = make_payment_endpoint(
        State(state.clone()),
        Path(deal_id.to_string()),
        Json(MakePaymentRequest {
            amount: "1000.00".to_string(),
            payment_type: None,
            notes: None,
        }),
    )
    .await;
    
    assert!(result.is_err());
    let (status, message) = result.unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("Cannot make payment on deal with status"));
}
```

#### Test 5: Batch Processing Integration

```rust
#[sqlx::test]
async fn test_transactions_can_be_batched(pool: PgPool) {
    let state = create_test_api_state(pool.clone()).await;
    
    // Create multiple deals
    for i in 0..5 {
        create_test_deal(&state).await;
    }
    
    // Verify 5 origination transactions created
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM deal_transactions 
         WHERE transaction_type = 'deal_origination'
           AND included_in_batch_id IS NULL"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert_eq!(count.count, Some(5));
    
    // Manually trigger batch processing
    if let Some(batch_processor) = &state.batch_processor {
        batch_processor.trigger_batch()
            .expect("Batch trigger should succeed");
    }
    
    // Wait for batch processing
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Verify transactions were batched
    let batched_count = sqlx::query!(
        "SELECT COUNT(*) as count FROM deal_transactions 
         WHERE transaction_type = 'deal_origination'
           AND included_in_batch_id IS NOT NULL"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert!(batched_count.count.unwrap() > 0, "Transactions should be batched");
}
```

**Effort:** 8 hours total

---

## Testing Strategy

### Unit Tests (Per Endpoint)

**Test Matrix:**

| Endpoint | Happy Path | Error Cases | Edge Cases | Total Tests |
|----------|-----------|-------------|-----------|-------------|
| UserRegistration | User created + transaction recorded | Duplicate email, invalid email, recording fails | Empty email, special characters | 5 tests |
| DealOrigination | Deal created + transaction recorded | Invalid UUIDs, missing investor, recording fails | Zero amount, negative rate | 6 tests |
| Payment | Payment processed + transaction recorded | Amount exceeds balance, deal closed, recording fails | Zero amount, negative amount, exact balance | 7 tests |
| Closeout | Deal closed + transaction recorded | Not paid off, not investor, recording fails | Already closed, no investor | 5 tests |

**Total:** ~23 integration tests

### Integration Test Execution

```bash
# Run all integration tests
cargo test --test deal_zkvm_integration_tests

# Run specific test
cargo test --test deal_zkvm_integration_tests test_complete_deal_lifecycle

# Run with output
cargo test --test deal_zkvm_integration_tests -- --nocapture

# Check coverage
cargo tarpaulin --test deal_zkvm_integration_tests
```

### Manual Testing Checklist

Before marking complete:

**UserRegistration:**
- [ ] Create user via API
- [ ] Check users table has record
- [ ] Check deal_transactions has record
- [ ] Verify transaction_type = "user_registration"
- [ ] Verify user_id matches
- [ ] Try creating duplicate user (should fail)

**DealOrigination:**
- [ ] Create deal via API
- [ ] Check deals table has record
- [ ] Check deal_transactions has record
- [ ] Verify transaction_type = "deal_origination"
- [ ] Verify amount matches principal
- [ ] Verify metadata has originator and investor IDs

**DealPayment:**
- [ ] Make payment via API
- [ ] Check deal_payments table has record
- [ ] Check deal outstanding_balance updated
- [ ] Check deal_status transitioned correctly
- [ ] Check deal_transactions has record
- [ ] Try payment exceeding balance (should fail)
- [ ] Try payment on closed deal (should fail)

**DealCloseout:**
- [ ] Close paid-off deal via API
- [ ] Check deal status = "closed"
- [ ] Check deal_transactions has record
- [ ] Try closing unpaid deal (should fail)
- [ ] Try making payment after closeout (should fail)

---

## API Documentation

### Swagger UI Updates

**Verify these endpoints appear in Swagger UI:**

1. Navigate to `http://localhost:8080/swagger-ui/`
2. Find "Deals" section
3. Verify endpoints:
   - `POST /api/v1/deal` - Create deal (existing, now with transaction recording)
   - `POST /api/v1/deal/{deal_id}/payment` - Make payment (NEW)
   - `POST /api/v1/deal/{deal_id}/closeout` - Close deal (NEW)

### Request/Response Examples

**Add to Swagger annotations:**

**Payment Example:**
```json
// Request
{
  "amount": "25000.00",
  "payment_type": "regular",
  "notes": "Q4 payment"
}

// Response
{
  "payment_id": "uuid-123",
  "deal_id": "uuid-456",
  "amount": "25000.00",
  "payment_type": "regular",
  "new_outstanding_balance": "75000.00",
  "deal_status": "active",
  "transaction_id": "uuid-789",
  "created_at": "2024-12-12T10:30:00Z"
}
```

**Closeout Example:**
```json
// Request
{
  "confirm_paid_off": true,
  "notes": "Deal completed successfully"
}

// Response
{
  "deal_id": "uuid-456",
  "deal_status": "closed",
  "transaction_id": "uuid-abc",
  "closed_at": "2024-12-12T11:00:00Z"
}
```

---

## Integration Validation

### End-to-End Validation

**Complete flow test:**

```bash
#!/bin/bash
# test_deal_operations.sh

set -e  # Exit on error

echo "=== Testing Complete Deal Operations Flow ==="

# 1. Create user
echo "1. Creating user..."
USER_RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","user_type":"investor"}')

USER_ID=$(echo $USER_RESPONSE | jq -r '.user_id')
echo "   User created: $USER_ID"

# Verify transaction
psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE user_id = '$USER_ID' AND transaction_type = 'user_registration'
" | grep -q "1" && echo "   ✅ UserRegistration transaction recorded"

# 2. Create deal
echo "2. Creating deal..."
DEAL_RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{
    \"originator_id\":\"$ORIGINATOR_ID\",
    \"investor_id\":\"$INVESTOR_ID\",
    \"principal_amount\":\"10000.00\",
    \"interest_rate_bps\":500
  }")

DEAL_ID=$(echo $DEAL_RESPONSE | jq -r '.deal_id')
echo "   Deal created: $DEAL_ID"

# Verify transaction
psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE deal_id = '$DEAL_ID' AND transaction_type = 'deal_origination'
" | grep -q "1" && echo "   ✅ DealOrigination transaction recorded"

# 3. Make payment
echo "3. Making payment..."
PAYMENT_RESPONSE=$(curl -s -X POST "http://localhost:8080/api/v1/deal/$DEAL_ID/payment" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"amount":"10000.00"}')

PAYMENT_ID=$(echo $PAYMENT_RESPONSE | jq -r '.payment_id')
echo "   Payment created: $PAYMENT_ID"
echo "   New balance: $(echo $PAYMENT_RESPONSE | jq -r '.new_outstanding_balance')"
echo "   Deal status: $(echo $PAYMENT_RESPONSE | jq -r '.deal_status')"

# Verify transaction
psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE deal_id = '$DEAL_ID' AND transaction_type = 'deal_payment'
" | grep -q "1" && echo "   ✅ DealPayment transaction recorded"

# 4. Close deal
echo "4. Closing deal..."
CLOSEOUT_RESPONSE=$(curl -s -X POST "http://localhost:8080/api/v1/deal/$DEAL_ID/closeout" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"confirm_paid_off":true,"notes":"Completed"}')

CLOSEOUT_STATUS=$(echo $CLOSEOUT_RESPONSE | jq -r '.deal_status')
echo "   Deal status: $CLOSEOUT_STATUS"

# Verify transaction
psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE deal_id = '$DEAL_ID' AND transaction_type = 'deal_closeout'
" | grep -q "1" && echo "   ✅ DealCloseout transaction recorded"

# 5. Verify complete transaction history
echo "5. Verifying transaction history..."
TRANSACTION_COUNT=$(psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions WHERE deal_id = '$DEAL_ID'
" | tr -d ' ')

echo "   Total transactions: $TRANSACTION_COUNT"
[[ "$TRANSACTION_COUNT" == "3" ]] && echo "   ✅ Complete lifecycle recorded" || echo "   ❌ Missing transactions"

# 6. Trigger batch processing
echo "6. Triggering batch processing..."
curl -s -X POST http://localhost:8080/api/v1/batch/trigger \
  -H "Authorization: Bearer $TOKEN" > /dev/null

sleep 2

# Verify transactions were batched
BATCHED=$(psql $DATABASE_URL -t -c "
  SELECT COUNT(*) FROM deal_transactions 
  WHERE deal_id = '$DEAL_ID' AND included_in_batch_id IS NOT NULL
" | tr -d ' ')

echo "   Batched transactions: $BATCHED"
[[ "$BATCHED" -gt "0" ]] && echo "   ✅ Transactions batched" || echo "   ⚠️  Batch pending"

echo ""
echo "=== Test Complete ==="
echo "Deal ID: $DEAL_ID"
echo "Transaction count: $TRANSACTION_COUNT"
echo "Status: PASSED"
```

**Run:**
```bash
chmod +x test_deal_operations.sh
./test_deal_operations.sh
```

---

## Common Implementation Patterns

### Pattern 1: Basic Transaction Recording (No Special Metadata)

**Use for:** UserRegistration, DealCloseout

```rust
state.transaction_recording
    .record_transaction(
        TransactionType::UserRegistration,
        user_id,
        None,
        Decimal::ZERO,
        Some(json!({
            "email": email,
        })),
    )
    .await?
```

### Pattern 2: Deal Transaction with Amount

**Use for:** DealOrigination, DealPayment

```rust
state.transaction_recording
    .record_transaction(
        TransactionType::DealPayment,
        payer_id,
        Some(deal_id),
        payment_amount,
        Some(json!({
            "payment_id": payment_id.to_string(),
            "payment_type": "regular",
        })),
    )
    .await?
```

### Pattern 3: Transaction with Rollback on Failure

**Use when:** Transaction recording is critical

```rust
// Start database transaction
let mut db_tx = pool.begin().await?;

// Create payment record
sqlx::query!("INSERT INTO deal_payments ...")
    .execute(&mut *db_tx)
    .await?;

// Update deal
sqlx::query!("UPDATE deals SET ...")
    .execute(&mut *db_tx)
    .await?;

// Commit database changes
db_tx.commit().await?;

// Record for zkVM (outside DB transaction)
let tx_result = state.transaction_recording
    .record_transaction(...)
    .await;

// If recording fails, decide whether to rollback
if let Err(e) = tx_result {
    // Could rollback payment here, or log and continue
    error!("Transaction recording failed: {}", e);
    // For MVP: Return error, let user retry
    return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
}
```

---

## Error Handling Guidelines

### Error Categories

**1. Validation Errors (400 Bad Request)**
```rust
// Examples:
- Invalid UUID format
- Amount <= 0
- Payment exceeds balance
- Deal in wrong status

// Response:
(StatusCode::BAD_REQUEST, "Clear error message for user")
```

**2. Authentication Errors (401 Unauthorized)**
```rust
// Examples:
- No auth token
- Invalid token
- Expired session

// Response:
(StatusCode::UNAUTHORIZED, "Authentication required")
```

**3. Permission Errors (403 Forbidden)**
```rust
// Examples:
- User not originator or investor
- User not authorized for operation

// Response:
(StatusCode::FORBIDDEN, "You don't have permission for this operation")
```

**4. Not Found Errors (404)**
```rust
// Examples:
- Deal doesn't exist
- Payment doesn't exist

// Response:
(StatusCode::NOT_FOUND, "Deal not found")
```

**5. Server Errors (500 Internal Server Error)**
```rust
// Examples:
- Database connection failure
- Transaction recording failure
- Unexpected errors

// Response:
(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")

// And ALWAYS log:
error!("Failed to create payment: {}", e);
```

### Error Handling Template

```rust
pub async fn endpoint(...) -> Result<Json<Response>, (StatusCode, String)> {
    // Parse inputs
    let input = parse_input().map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Invalid input: {}", e))
    })?;
    
    // Validate business rules
    validate_business_rules().map_err(|e| {
        (StatusCode::BAD_REQUEST, e.to_string())
    })?;
    
    // Check permissions
    check_permissions().map_err(|e| {
        (StatusCode::FORBIDDEN, e.to_string())
    })?;
    
    // Database operations
    let result = database_operation()
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database operation failed".to_string())
        })?;
    
    // Record transaction
    let tx_id = state.transaction_recording
        .record_transaction(...)
        .await
        .map_err(|e| {
            error!("Transaction recording failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Transaction recording failed".to_string())
        })?;
    
    Ok(Json(response))
}
```

---

## Deployment Considerations

### Feature Flags (Optional but Recommended)

**Consider adding feature flag for transaction recording:**

```rust
// In config
pub struct ApiConfig {
    // ... existing fields ...
    pub enable_zkvm_transaction_recording: bool,
}

// In endpoint
if state.config.enable_zkvm_transaction_recording {
    state.transaction_recording.record_transaction(...).await?;
} else {
    info!("zkVM transaction recording disabled via config");
}
```

**Benefits:**
- Can deploy code without enabling recording
- Can enable in staging first
- Can quickly disable if issues arise
- Easier rollback path

### Database Migration Verification

**Before deploying, verify migration exists:**

```sql
-- Should exist from earlier work
-- If not, create migration:

CREATE TABLE IF NOT EXISTS deal_transactions (
    id UUID PRIMARY KEY,
    transaction_id_hash BYTEA NOT NULL,
    user_id_hash BYTEA NOT NULL,
    deal_id_hash BYTEA,
    transaction_type TEXT NOT NULL,
    amount DECIMAL NOT NULL DEFAULT 0,
    user_id UUID,
    deal_id UUID REFERENCES deals(id),
    payment_id UUID,
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_deal_transactions_pending 
ON deal_transactions(created_at) 
WHERE included_in_batch_id IS NULL;

CREATE INDEX idx_deal_transactions_deal_id 
ON deal_transactions(deal_id);

CREATE INDEX idx_deal_transactions_batch_id 
ON deal_transactions(included_in_batch_id);
```

### Rollout Strategy

**Phase 1: Deploy Infrastructure (Week 1)**
- Deploy TransactionRecordingService
- Deploy with feature flag OFF
- Verify no errors in production

**Phase 2: Enable UserRegistration (Week 1)**
- Enable recording for new users only
- Monitor for 24 hours
- Verify transactions being created

**Phase 3: Enable DealOrigination (Week 2)**
- Enable recording for new deals only
- Monitor for 24 hours
- Verify transactions being created

**Phase 4: Enable Payment & Closeout (Week 2)**
- Deploy new endpoints
- Test in staging first
- Enable in production
- Monitor for issues

**Phase 5: Verify Batch Processing (Week 3)**
- Manually trigger batch
- Verify transactions processed
- Check proof generation
- Verify contract posting

---

## Validation Checklist

### EPIC-2.1 Complete (UserRegistration)

- [ ] Code merged to main branch
- [ ] TransactionRecordingService call added to user creation
- [ ] Error handling in place
- [ ] Manual test passed
- [ ] Integration test written and passing
- [ ] No regression in user creation endpoint
- [ ] Code reviewed and approved
- [ ] Swagger docs updated

### EPIC-2.2 Complete (DealOrigination)

- [ ] Code merged to main branch
- [ ] TransactionRecordingService call added to deal creation
- [ ] Metadata properly formatted for zkVM
- [ ] Error handling in place
- [ ] Manual test passed
- [ ] Integration test written and passing
- [ ] No regression in deal creation endpoint
- [ ] Code reviewed and approved

### EPIC-2.3 Complete (Payment Endpoint)

- [ ] New endpoint created
- [ ] Route added to router
- [ ] Payment logic implemented correctly
- [ ] Deal balance updated
- [ ] Deal status transitions correctly
- [ ] TransactionRecordingService integrated
- [ ] Error cases handled
- [ ] Manual test passed
- [ ] Integration tests written and passing
- [ ] Swagger docs generated
- [ ] Code reviewed and approved

### EPIC-2.4 Complete (Closeout Endpoint)

- [ ] New endpoint created
- [ ] Route added to router
- [ ] Closeout logic implemented
- [ ] Validation logic correct
- [ ] TransactionRecordingService integrated
- [ ] Error cases handled
- [ ] Manual test passed
- [ ] Integration tests written and passing
- [ ] Swagger docs generated
- [ ] Code reviewed and approved

### EPIC-2.5 Complete (Integration Tests)

- [ ] Test file created
- [ ] All 23 tests implemented
- [ ] All tests passing
- [ ] Code coverage >80%
- [ ] Tests run in CI/CD
- [ ] Test documentation complete

### Overall WS-2 Complete

- [ ] All 4 transaction types integrated
- [ ] All endpoints working
- [ ] All tests passing
- [ ] Documentation complete
- [ ] Code reviewed
- [ ] Deployed to staging
- [ ] Validated in staging
- [ ] Ready for WS-5 (E2E testing)

---

## Timeline Breakdown

### Week 1 (After EPIC-1 Complete)

**Day 1-2:**
- EPIC-2.1: UserRegistration (4 hours)
- EPIC-2.2: DealOrigination (4 hours)
- **Deliverable:** 2/4 operations integrated

### Week 2

**Day 3-4:**
- EPIC-2.3: Payment endpoint (8 hours)
- **Deliverable:** 3/4 operations integrated

### Week 3

**Day 5:**
- EPIC-2.4: Closeout endpoint (4 hours)
- **Deliverable:** 4/4 operations integrated

**Day 6-7:**
- EPIC-2.5: Integration tests (8 hours)
- **Deliverable:** All tests passing

### Total: 3.5 days spread across 3 weeks

---

## Success Metrics

### Definition of Done

**EPIC-2 is complete when:**

1. ✅ All 4 endpoints create deal_transactions records
2. ✅ Payment endpoint accessible via `POST /api/v1/deal/{id}/payment`
3. ✅ Closeout endpoint accessible via `POST /api/v1/deal/{id}/closeout`
4. ✅ All integration tests passing (23 tests)
5. ✅ Swagger UI shows all endpoints
6. ✅ Manual validation script succeeds
7. ✅ Code reviewed and merged
8. ✅ Deployed to staging
9. ✅ No regressions in existing functionality

### Measurement

**Verify coverage:**

```bash
# Check transaction recording is happening
psql $DATABASE_URL -c "
  SELECT 
    transaction_type,
    COUNT(*) as count,
    SUM(amount) as total_amount
  FROM deal_transactions
  WHERE created_at > NOW() - INTERVAL '1 day'
  GROUP BY transaction_type
  ORDER BY count DESC;
"

# Expected output (after some API usage):
# transaction_type     | count | total_amount
# ---------------------+-------+--------------
# deal_origination     |   10  |  1000000.00
# deal_payment         |   20  |   500000.00
# user_registration    |    5  |        0.00
# deal_closeout        |    2  |        0.00
```

**Performance validation:**

```bash
# Check API latency hasn't regressed
curl -w "@curl-format.txt" -o /dev/null -s \
  -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -d '...'

# Should still be <500ms total
```

---

## Common Issues & Solutions

### Issue 1: "TransactionRecordingService not found in ApiState"

**Cause:** Service not added to ApiState or not initialized

**Solution:**
```rust
// Verify in api/src/api/router.rs
pub struct ApiState {
    // ...
    pub transaction_recording: Arc<TransactionRecordingService>,  // Should exist
}

// Verify initialization in server.rs
let transaction_recording = Arc::new(TransactionRecordingService::new(pool.clone()));
let state = ApiState { transaction_recording, /* ... */ };
```

### Issue 2: "Payment endpoint returns 404"

**Cause:** Route not added to router or path incorrect

**Solution:**
```bash
# Check route exists
grep -r "deal.*payment" api/src/api/router.rs

# Verify path format
# Should be: "/api/v1/deal/:deal_id/payment"
# NOT: "/api/v1/deals/:deal_id/payment" (note plural)
```

### Issue 3: "Metadata encoding doesn't match zkVM expectations"

**Cause:** Metadata format mismatch between API and zkVM

**Solution:**
- Document metadata format in code comments
- Add validation test that checks format
- Coordinate with batch processor team on encoding

### Issue 4: "Tests fail with 'relation deal_transactions does not exist'"

**Cause:** Test database not migrated

**Solution:**
```bash
# Run migrations on test database
export TEST_DATABASE_URL="postgresql://localhost/test_db"
sqlx migrate run --database-url $TEST_DATABASE_URL
```

### Issue 5: "Deal status doesn't transition correctly"

**Cause:** Payment logic bug

**Debug:**
```rust
// Add extensive logging
info!("Payment: amount={}, outstanding={}, new_balance={}", 
    amount, deal.outstanding_balance, new_balance);
info!("Status transition: {} → {}", deal.deal_status, new_status);

// Verify calculation
assert_eq!(new_balance, deal.outstanding_balance - amount);
```

---

## Handoff Documentation

### For WS-5 (Testing Team)

**What to test:**

1. **API Functionality**
   - All 4 endpoints work correctly
   - Error cases return appropriate status codes
   - Responses match OpenAPI spec

2. **Transaction Recording**
   - Every API call creates deal_transactions record
   - Transaction types are correct
   - Amounts match
   - Metadata is populated

3. **Deal Lifecycle**
   - Originated → Active → Paid Off → Closed
   - Status transitions happen correctly
   - Can't skip states
   - Can't reverse states

4. **Batch Processing**
   - Transactions can be batched
   - zkVM processes them correctly
   - Proofs generated successfully

**Test Data:**
- Use populate_deal_data.rs script (after WS-6 updates)
- Or manually create via API
- Focus on edge cases (zero payments, overpayments, etc.)

### For WS-3 (Collateral Team)

**What you can ignore:**

While WS-2 is integrating deals, WS-3 can work independently on collateral zkVM logic:
- Don't worry about API integration yet
- Focus on lib/src/collateral.rs
- Unit test with LoanTransaction directly
- WS-4 will handle API integration later

**What to coordinate on:**

- Metadata format for collateral transactions
- Share test cases for double-pledge prevention
- Coordinate on GlobalLoanState struct changes

---

## Documentation Requirements

### Code Comments

**Add to each endpoint:**

```rust
/// Make a payment on a deal
///
/// # Transaction Recording
///
/// This endpoint creates a `deal_payment` transaction in the deal_transactions
/// table for zkVM batch processing. The transaction will later be included in
/// a proof batch and posted to the smart contract.
///
/// # State Transitions
///
/// - originated → active (on first payment)
/// - active → paid_off (when balance reaches 0)
///
/// # zkVM Integration
///
/// The transaction is recorded synchronously. If recording fails, the entire
/// request fails to maintain consistency between the database and zkVM state.
///
/// See: docs/zk/architecture.md for architecture details
```

### API Documentation

**Update:** `docs/integrations/api-swagger.md`

Add sections for:
- Payment endpoint usage
- Closeout endpoint usage
- Transaction recording behavior
- Error responses

### Architecture Documentation

**Update:** `docs/zk/architecture.md`

Add real examples using the actual implemented endpoints:

```markdown
### Real Example: Making a Payment

Layer 1 (API): POST /api/v1/deal/{id}/payment
- Validates amount
- Creates payment record
- Updates deal balance
- Calls TransactionRecordingService

Layer 2 (Service): TransactionRecordingService
- Hashes UUIDs
- Creates deal_transactions record
- Returns transaction_id

Layer 3 (zkVM): Will process later in batch
- Validates payment against zkVM state
- Updates deal outstanding_balance in zkVM
- Generates proof
```

---

## Quick Reference

### Files Modified

1. `api/src/handlers/users/users.rs` - Add transaction recording to user creation
2. `api/src/handlers/deals/deals.rs` - Add transaction recording to deal creation + new endpoints
3. `api/src/api/router.rs` - Add payment and closeout routes
4. `api/src/api/openapi.rs` - Add new endpoints to OpenAPI spec
5. `api/tests/deal_zkvm_integration_tests.rs` - New test file

### New Endpoints

- `POST /api/v1/deal/{deal_id}/payment` - Make payment
- `POST /api/v1/deal/{deal_id}/closeout` - Close deal

### Testing Commands

```bash
# Run integration tests
cargo test --test deal_zkvm_integration_tests

# Run manual validation
./test_deal_operations.sh

# Check Swagger UI
open http://localhost:8080/swagger-ui/

# Verify transactions in database
psql $DATABASE_URL -c "SELECT transaction_type, COUNT(*) FROM deal_transactions GROUP BY transaction_type;"
```

---

## Acceptance Criteria (Final)

### Functional

- [ ] User creation records UserRegistration transaction
- [ ] Deal creation records DealOrigination transaction
- [ ] Payment endpoint works and records DealPayment transaction
- [ ] Closeout endpoint works and records DealCloseout transaction
- [ ] All transactions have correct transaction_type
- [ ] All amounts are correct
- [ ] All metadata is populated
- [ ] UUIDs are hashed with SHA256

### Quality

- [ ] All integration tests pass
- [ ] Code coverage >80% for new code
- [ ] No compiler warnings
- [ ] No clippy warnings
- [ ] Formatted with rustfmt
- [ ] Error handling comprehensive

### Performance

- [ ] Payment endpoint <500ms response time
- [ ] Closeout endpoint <300ms response time
- [ ] Transaction recording adds <50ms
- [ ] No N+1 queries
- [ ] Database queries optimized

### Documentation

- [ ] All endpoints documented with #[utoipa::path]
- [ ] Swagger UI shows new endpoints
- [ ] Code comments explain transaction recording
- [ ] Integration guide for other teams
- [ ] Error handling documented

### Integration

- [ ] Works with existing batch processor
- [ ] Transactions can be batched
- [ ] zkVM can process transactions
- [ ] No breaking changes to existing endpoints

---

**End of Work Stream 2 Implementation Plan**

**Next Work Stream:** After EPIC-2 is complete, WS-3 (Collateral zkVM) and WS-4 (Collateral API) can proceed with their implementations.
