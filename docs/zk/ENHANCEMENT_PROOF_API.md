# Enhancement: Audit Trail API with ZK Proof Information

**Feature Request:** Enrich transaction audit trail endpoints to include ZK proof metadata  
**Priority:** HIGH - Improves developer experience and transparency  
**Effort:** 5 hours  
**Status:** Recommended enhancement to implemented system  

---

## Executive Summary

### Your Questions Answered

**Q1: Are ZK proofs stored in the database?**

✅ **YES** - Proof metadata is stored in `proof_batches` table:
- `sindri_proof_id` - Reference to proof on Sindri service
- `proof_status` - "pending", "proven", "failed"
- `posted_to_contract` - Whether proof is on-chain
- `contract_tx_hash` - Blockchain transaction hash
- `proven_at`, `posted_to_contract_at` - Timestamps

**Q2: Should API endpoints return proof information with transactions?**

✅ **EXCELLENT IDEA** - Significantly improves user experience

**Current Problem:**
- Transactions and proofs stored in different tables
- Requires 3+ API calls to get complete picture
- Frontend must manually correlate data

**Your Vision:**
Single endpoint returns:
- Transactions grouped by batch
- Proof status for each batch
- Whether proven and posted on-chain
- Actual proof data if requested

---

## Current State

### Existing Endpoints

**1. GET /api/v1/deal/{deal_id}/audit-trail**
- Returns: Transactions with `included_in_batch_id`
- Missing: Proof metadata, batch status

**2. GET /api/v1/state/batch/{batch_id}**
- Returns: Batch info with proof status
- Missing: Transaction details

**3. GET /api/v1/state/batches/{batch_id}/contract-data**
- Returns: Merkle roots and proof reference
- Missing: Transaction context

### Database Schema

**proof_batches table:**
```sql
CREATE TABLE proof_batches (
    id SERIAL PRIMARY KEY,
    previous_counter_value BIGINT,
    final_counter_value BIGINT,
    transaction_ids INTEGER[],
    sindri_proof_id TEXT,              -- Proof ID
    proof_status TEXT,                 -- Status
    created_at TIMESTAMP,
    proven_at TIMESTAMP,
    posted_to_contract BOOLEAN,        -- On-chain?
    posted_to_contract_at TIMESTAMP,
    contract_tx_hash TEXT              -- TX hash
);
```

**deal_transactions table:**
```sql
CREATE TABLE deal_transactions (
    id UUID PRIMARY KEY,
    transaction_type TEXT,
    amount DECIMAL,
    included_in_batch_id INTEGER REFERENCES proof_batches(id),  -- ← Links!
    created_at TIMESTAMP,
    metadata JSONB
);
```

**ads_state_commits table:**
```sql
CREATE TABLE ads_state_commits (
    id SERIAL PRIMARY KEY,
    batch_id INTEGER REFERENCES proof_batches(id),
    prev_merkle_root TEXT,             -- Previous state root
    new_merkle_root TEXT,              -- New state root
    created_at TIMESTAMP
);
```

---

## Proposed Enhancement

### New Endpoint: audit-trail-with-proofs

**Route:** `GET /api/v1/deal/{deal_id}/audit-trail-with-proofs`

**Query Parameters:**
- `include_proof_data` (boolean, default: false) - Include actual proof bytes

**Response Structure:**

```typescript
{
  deal_id: string;
  deal_status: string;
  total_transactions: number;
  
  // Transactions grouped by batch with proof info
  batches: [{
    batch_id: number;
    batch_status: "pending" | "proving" | "proven" | "posted";
    created_at: string;
    proven_at?: string;
    posted_at?: string;
    
    transactions: [{
      id: string;
      transaction_type: string;
      amount: string;
      created_at: string;
      metadata?: object;
    }];
    
    proof?: {
      sindri_proof_id: string;
      proof_status: string;
      posted_to_contract: boolean;
      contract_tx_hash?: string;
      
      merkle_roots: {
        previous_state_root: string;
        new_state_root: string;
      };
      
      // Useful links
      proof_url: string;           // Link to Sindri
      contract_url?: string;       // Link to Etherscan
      
      // Optional: Include actual proof bytes (base64)
      proof_data?: {
        proof_bytes_size: number;
        public_values_size: number;
        proof_bytes_base64?: string;
        public_values_base64?: string;
      };
    };
  }];
  
  // Transactions not yet batched
  unbatched_transactions: [{
    id: string;
    transaction_type: string;
    amount: string;
    created_at: string;
    status: "pending_batch";
  }];
}
```

---

## Implementation Guide

### Step 1: Database Query (1 hour)

**File:** `db/src/db.rs`

**Create optimized query:**

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TransactionWithProofInfo {
    // Transaction fields
    pub id: Uuid,
    pub transaction_type: String,
    pub amount: Decimal,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    
    // Batch and proof fields (nullable)
    pub batch_id: Option<i32>,
    pub proof_status: Option<String>,
    pub sindri_proof_id: Option<String>,
    pub proven_at: Option<DateTime<Utc>>,
    pub posted_to_contract: Option<bool>,
    pub posted_to_contract_at: Option<DateTime<Utc>>,
    pub contract_tx_hash: Option<String>,
    pub prev_merkle_root: Option<String>,
    pub new_merkle_root: Option<String>,
}

/// Get deal transactions with proof information (single query)
pub async fn get_deal_transactions_with_proofs(
    pool: &PgPool,
    deal_id: Uuid,
) -> Result<Vec<TransactionWithProofInfo>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT 
            dt.id,
            dt.transaction_type,
            dt.amount,
            dt.created_at,
            dt.metadata,
            dt.included_in_batch_id as batch_id,
            pb.proof_status,
            pb.sindri_proof_id,
            pb.proven_at,
            pb.posted_to_contract,
            pb.posted_to_contract_at,
            pb.contract_tx_hash,
            ads.prev_merkle_root,
            ads.new_merkle_root
        FROM deal_transactions dt
        LEFT JOIN proof_batches pb ON dt.included_in_batch_id = pb.id
        LEFT JOIN (
            SELECT DISTINCT ON (batch_id)
                batch_id,
                prev_merkle_root,
                new_merkle_root
            FROM ads_state_commits
            ORDER BY batch_id, id DESC
        ) ads ON pb.id = ads.batch_id
        WHERE dt.deal_id = $1
        ORDER BY dt.created_at ASC
        "#,
    )
    .bind(deal_id)
    .fetch_all(pool)
    .await?;
    
    let transactions = rows.into_iter().map(|row| TransactionWithProofInfo {
        id: row.get("id"),
        transaction_type: row.get("transaction_type"),
        amount: row.get("amount"),
        created_at: row.get("created_at"),
        metadata: row.get("metadata"),
        batch_id: row.get("batch_id"),
        proof_status: row.get("proof_status"),
        sindri_proof_id: row.get("sindri_proof_id"),
        proven_at: row.get("proven_at"),
        posted_to_contract: row.get("posted_to_contract"),
        posted_to_contract_at: row.get("posted_to_contract_at"),
        contract_tx_hash: row.get("contract_tx_hash"),
        prev_merkle_root: row.get("prev_merkle_root"),
        new_merkle_root: row.get("new_merkle_root"),
    }).collect();
    
    Ok(transactions)
}
```

### Step 2: API Response Types (30 min)

**File:** `api/src/handlers/deals/deals.rs`

```rust
#[derive(Debug, Serialize, ToSchema)]
pub struct EnhancedDealAuditTrailResponse {
    pub deal_id: String,
    pub deal_status: String,
    pub total_transactions: usize,
    pub batches: Vec<BatchWithTransactions>,
    pub unbatched_transactions: Vec<UnbatchedTransaction>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchWithTransactions {
    pub batch_id: i32,
    pub batch_status: String,
    pub created_at: String,
    pub proven_at: Option<String>,
    pub posted_at: Option<String>,
    pub transactions: Vec<DealTransactionEvent>,
    pub proof: Option<ProofMetadata>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProofMetadata {
    pub sindri_proof_id: String,
    pub proof_status: String,
    pub posted_to_contract: bool,
    pub contract_tx_hash: Option<String>,
    pub merkle_roots: MerkleRootInfo,
    pub proof_url: String,
    pub contract_url: Option<String>,
    pub proof_data: Option<ProofDataInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MerkleRootInfo {
    pub previous_state_root: String,
    pub new_state_root: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProofDataInfo {
    pub proof_bytes_size: usize,
    pub public_values_size: usize,
    pub proof_bytes_base64: Option<String>,
    pub public_values_base64: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnbatchedTransaction {
    pub id: String,
    pub transaction_type: String,
    pub amount: String,
    pub created_at: String,
    pub status: String,  // Always "pending_batch"
}
```

### Step 3: Endpoint Implementation (2-3 hours)

See complete implementation in document above.

### Step 4: Add Route (5 min)

**File:** `api/src/api/router.rs`

```rust
.route(
    "/api/v1/deal/:deal_id/audit-trail-with-proofs",
    get(deals::get_deal_audit_trail_with_proofs_endpoint)
)
```

### Step 5: Testing (1 hour)

**File:** `api/tests/deal_zkvm_integration_tests.rs`

Add test that verifies enhanced endpoint returns proof info

---

## Benefits

### For Developers

- ✅ Single API call instead of 3+
- ✅ Data already grouped and correlated
- ✅ Clear status indicators
- ✅ Easy to display in UI

### For Users

- ✅ See transaction lifecycle (pending → batched → proven → on-chain)
- ✅ Click links to view proofs
- ✅ Verify on blockchain
- ✅ Transparency and trust

### For Compliance

- ✅ Complete audit trail in one response
- ✅ Proof status visible
- ✅ Cryptographic commitments shown
- ✅ Blockchain verification links

---

## Timeline

**Week 1 (After compilation bugs fixed):**
- Implement database query (1 hour)
- Create response types (30 min)
- Implement endpoint (2-3 hours)
- Add route and test (1 hour)
- **Total: 5 hours**

**Week 2 (Optional enhancements):**
- Add caching for batch info
- Add similar endpoints for user transactions
- Add WebSocket for real-time updates

---

## Recommendation

✅ **IMPLEMENT THIS** - It's a high-value enhancement that:
- Takes only 5 hours
- Significantly improves UX
- Provides transparency
- Aligns with zkVM architecture goals

This enhancement makes the ZK proof system much more accessible and transparent to end users, which is a key value proposition of the entire system.

---

**Next Step:** Should I create a detailed implementation plan for this as "WS-7: Proof API Enhancement"?
