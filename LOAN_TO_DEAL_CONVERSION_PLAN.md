# Loan to Deal Conversion Action Plan

## Overview

This document outlines the complete conversion from loan-based terminology to deal-based terminology in the Arda Credit system. This is a hard cutover with no backward compatibility requirements.

## Key Changes Summary

| Current | New |
|---------|-----|
| `borrower_id` | `originator_id` |
| `lender_id` | `investor_id` |
| `loans` table | `deals` table |
| `loan_*` tables | `deal_*` tables |
| `LoanOrigination` | `DealOrigination` |
| `/loans` endpoints | `/deals` endpoints |
| `marketplace` endpoints | Consolidated into deal endpoints |
| Contract name `Loan` | Contract name `ARDA` |
| CLI `loan` commands | CLI `deal` commands |

## Sequential Task Breakdown

The following tasks must be executed in order due to dependencies. Limited parallel work opportunities are noted where applicable.

---

## Phase 1: Database Foundation (Days 1-2)

**Priority: CRITICAL - Everything depends on this**

### Task 1.1: Create New Database Schema
**Files to modify:**
- `db/migrations/` - Create new migration file (e.g., `061_convert_loans_to_deals.sql`)

**Changes:**
```sql
-- Drop old tables (hard cutover)
DROP TABLE IF EXISTS loan_transfers;
DROP TABLE IF EXISTS loan_listings; 
DROP TABLE IF EXISTS loan_transactions;
DROP TABLE IF EXISTS payments;
DROP TABLE IF EXISTS loans;

-- Create new deals table (copy of loans structure)
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    originator_id UUID NOT NULL REFERENCES identities(id),  -- was borrower_id
    investor_id UUID NOT NULL REFERENCES identities(id),    -- was lender_id
    principal_amount DECIMAL(18, 6) NOT NULL CHECK (principal_amount > 0),
    outstanding_balance DECIMAL(18, 6) NOT NULL CHECK (outstanding_balance >= 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0),
    loan_status VARCHAR(20) NOT NULL DEFAULT 'originated' CHECK (loan_status IN ('originated', 'active', 'paid_off', 'closed', 'defaulted')),
    originated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    maturity_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT originator_investor_different CHECK (originator_id != investor_id)
);

-- Create other deal tables
CREATE TABLE deal_payments; -- copy of payments structure
CREATE TABLE deal_transactions; -- copy of loan_transactions structure  
CREATE TABLE deal_listings; -- copy of loan_listings structure
CREATE TABLE deal_transfers; -- copy of loan_transfers structure

-- Create indexes
CREATE INDEX idx_deals_originator_id ON deals(originator_id);
CREATE INDEX idx_deals_investor_id ON deals(investor_id);
-- ... additional indexes
```

### Task 1.2: Update Database Types
**Files to modify:**
- `db/src/db.rs`

**Changes:**
```rust
/// Deal record with originator/investor relationship
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Deal {  // was Loan
    pub id: uuid::Uuid,
    pub originator_id: uuid::Uuid,  // was borrower_id
    pub investor_id: uuid::Uuid,    // was lender_id
    pub principal_amount: rust_decimal::Decimal,
    pub outstanding_balance: rust_decimal::Decimal,
    pub interest_rate_bps: i32,
    pub loan_status: String, // keeping same statuses
    pub originated_at: DateTime<Utc>,
    pub maturity_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Deal payment record
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealPayment {  // was Payment
    pub id: uuid::Uuid,
    pub deal_id: uuid::Uuid,  // was loan_id
    pub payer_id: uuid::Uuid,
    pub amount: rust_decimal::Decimal,
    pub payment_type: String,
    pub transaction_hash: Option<String>,
    pub processed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
```

### Task 1.3: Update Database Functions
**Files to modify:**
- `db/src/db.rs`

**Changes:**
- `create_loan` → `create_deal`
- `get_loan_by_id` → `get_deal_by_id`
- `list_loans` → `list_deals`
- `update_loan` → `update_deal`
- `create_payment` → `create_deal_payment`
- Update all SQL queries to use new table names
- Update function signatures to use `originator_id`/`investor_id`

### Task 1.4: Update Marketplace Database Types
**Files to modify:**
- `db/src/marketplace.rs`

**Changes:**
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealListing {  // was LoanListing
    pub listing_id: Uuid,
    pub deal_id: Uuid,    // was loan_id
    pub seller_id: Uuid,
    // ... rest stays same
}

pub struct MarketplaceDeal {  // was MarketplaceLoan
    // Update all internal fields
}
```

### Task 1.5: Update Transfers Database Types  
**Files to modify:**
- `db/src/transfers.rs`

**Changes:**
- All `loan_` references → `deal_`
- Update struct definitions and database queries

### Task 1.6: Update Library Exports
**Files to modify:**
- `db/src/lib.rs`

**Changes:**
- Update all function exports: `create_loan` → `create_deal`, etc.

**Estimated Time: 2 days**
**Dependencies: None**
**Parallel Work: None - this is foundational**

---

## Phase 2: Core Business Logic (Days 3-4)

**Priority: HIGH - Required before API layer**

### Task 2.1: Update Transaction Types
**Files to modify:**
- `lib/src/transactions.rs`

**Changes:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionType {
    UserRegistration = 1,
    KycSubmission = 2,
    KycVerification = 3,
    DealOrigination = 4,    // was LoanOrigination
    DealPayment = 5,        // was LoanPayment  
    DealCloseout = 6,       // was LoanCloseout
    UserDeposit = 7,
    UserWithdrawal = 8,
    // ... rest unchanged
}
```

### Task 2.2: Update Core Deal Logic
**Files to modify:**
- `lib/src/loan.rs` → `lib/src/deal.rs` (rename file)
- `lib/src/loan_processor.rs` → `lib/src/deal_processor.rs` (rename file)

**Changes:**
- Update all struct field names: `borrower_id` → `originator_id`, `lender_id` → `investor_id`
- Update function names: `process_loan_batch` → `process_deal_batch`
- Update all internal logic and validation
- Update transaction type references

### Task 2.3: Update Library Exports
**Files to modify:**  
- `lib/src/lib.rs`

**Changes:**
- Update all exports to reference deal modules and types
- Update re-exports

### Task 2.4: Update Test Files
**Files to modify:**
- `lib/tests/test_balance_integration.rs`
- `lib/src/zkvm_ads_consistency_tests.rs`
- All other test files with loan references

**Changes:**
- Update all test data structures
- Update transaction type references
- Update field names in test cases

**Estimated Time: 2 days**
**Dependencies: Phase 1 complete**  
**Parallel Work: Could start ZK program updates in parallel**

---

## Phase 3: ZK Program (Days 4-5)

**Priority: HIGH - Required for proof generation**

### Task 3.1: Update ZK Program Logic
**Files to modify:**
- `program/src/main.rs`

**Changes:**
- `process_loan_batch` → `process_deal_batch`
- Update all internal variable names and logic
- Update transaction type handling

### Task 3.2: Update Script Layer
**Files to modify:**
- `script/src/bin/main.rs`

**Changes:**
- Update all transaction type references
- Update test data generation
- Update batch processing logic

**Estimated Time: 1 day**
**Dependencies: Phase 2 (core logic) complete**
**Parallel Work: Can be done in parallel with API layer work**

---

## Phase 4: Smart Contracts (Days 4-6)

**Priority: MEDIUM - Somewhat independent**

### Task 4.1: Rename and Update Smart Contract
**Files to modify:**
- `contracts/src/Loan.sol` → `contracts/src/ARDA.sol` (rename)

**Changes:**
```solidity
// Update contract name
contract ARDA is EventHelpers {  // was Loan

// Update internal struct names  
struct DealBatchPublicValues {  // was LoanBatchPublicValues
    // ... same fields
}

// Update function names
function verifyDealProof(...) public view returns (uint128) {  // was verifyLoanProof
    // Update internal variable names but keep same logic
}
```

### Task 4.2: Update Contract Tests
**Files to modify:**
- `contracts/test/` - All test files

**Changes:**
- Update all contract interaction code
- Update test data and assertions
- Update import statements

### Task 4.3: Update Deployment Scripts
**Files to modify:**
- Any deployment scripts in `contracts/`
- `Makefile` contract deployment commands

**Changes:**
- Update contract name references
- Update deployment commands

**Estimated Time: 2 days**  
**Dependencies: Phase 2 (for understanding internal changes)**
**Parallel Work: Can be done in parallel with API/CLI work**

---

## Phase 5: API Layer (Days 6-8)

**Priority: HIGH - Required for CLI**

### Task 5.1: Update API Request/Response Types
**Files to modify:**
- `api/src/loan_handlers.rs` → `api/src/deal_handlers.rs` (rename)

**Changes:**
```rust
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateDealRequest {  // was CreateLoanRequest
    pub originator_id: String,  // was borrower_id
    pub investor_id: String,    // was lender_id
    pub principal_amount: String,
    pub interest_rate_bps: u16,
    pub term_days: Option<u32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]  
pub struct DealDetailsResponse {  // was LoanDetailsResponse
    pub deal_id: String,       // was loan_id
    pub originator_id: String, // was borrower_id
    pub investor_id: String,   // was lender_id
    // ... rest same
}
```

### Task 5.2: Update API Endpoints
**Files to modify:**
- `api/src/deal_handlers.rs` (renamed from loan_handlers.rs)

**Changes:**
- `create_loan_endpoint` → `create_deal_endpoint`
- `list_loans_endpoint` → `list_deals_endpoint`  
- `get_loan_details_endpoint` → `get_deal_details_endpoint`
- Update all function implementations to use new database functions

### Task 5.3: Update API Paths
**Files to modify:**
- `api/src/paths.rs`

**Changes:**
```rust
// Deal management paths (was loan management)
pub const DEALS: &str = api_path!("/deals");         // was "/loans"
pub const DEAL: &str = api_path!("/deal");           // was "/loan"  
pub const DEAL_BY_ID: &str = api_path!("/deal/{deal_id}");  // was "/loan/{loan_id}"
pub const DEAL_PAYMENTS: &str = api_path!("/deal/{deal_id}/payments");
pub const DEALS_STATS: &str = api_path!("/deals/stats");

// Remove marketplace-specific paths, consolidate into deal actions
// REMOVE: LOANS_FOR_SALE, LOAN_LIST_FOR_SALE, MARKETPLACE_STATS
// REPLACE WITH: DEALS_FOR_SALE, DEAL_LIST_FOR_SALE (or integrate into deals endpoints)
```

### Task 5.4: Update API Routing
**Files to modify:**
- `api/src/router.rs`

**Changes:**
- Update all route definitions to use new paths
- Update handler function references
- Consolidate marketplace routes into deal routes
- Remove separate marketplace endpoints

### Task 5.5: Update Marketplace Handlers
**Files to modify:**
- `api/src/marketplace_handlers.rs` → `api/src/deal_marketplace_handlers.rs` (rename)

**Changes:**
- Consolidate marketplace functionality into deal endpoints
- Update to use deal terminology throughout
- Update database function calls

### Task 5.6: Update Transfer Handlers  
**Files to modify:**
- `api/src/transfer_handlers.rs` → `api/src/deal_transfer_handlers.rs` (rename)

**Changes:**
- Update all loan references to deal references
- Update function signatures and implementations

### Task 5.7: Update API Client
**Files to modify:**
- `api/src/client/mod.rs`

**Changes:**
- Update all client method names
- Update URL construction
- Update request/response handling

### Task 5.8: Update OpenAPI Documentation
**Files to modify:**
- `api/src/openapi.rs`

**Changes:**
- Update all endpoint documentation
- Update schema references
- Remove marketplace-specific endpoints

**Estimated Time: 3 days**
**Dependencies: Phase 1 (database) and Phase 2 (core logic) complete**
**Parallel Work: Can work on CLI in parallel once basic API structure is done**

---

## Phase 6: CLI Layer (Days 8-9)

**Priority: MEDIUM - User interface**

### Task 6.1: Update CLI Command Structure  
**Files to modify:**
- `cli/src/cli.rs`

**Changes:**
```rust
/// Credit/deal operations (investors, merchants, credit lines)
#[command(subcommand)]  
Deal(DealCommands),  // was Loan(LoanCommands)

// Remove separate Marketplace and Transfer commands - consolidate into Deal
// REMOVE: Marketplace(MarketplaceCommands), Transfer(TransferCommands)
```

### Task 6.2: Update Deal Commands
**Files to modify:**
- `cli/src/loans/` → `cli/src/deals/` (rename directory)
- `cli/src/loans/mod.rs` → `cli/src/deals/mod.rs` (rename)

**Changes:**
```rust
#[derive(Subcommand)]
pub enum DealCommands {  // was LoanCommands
    /// Originate a new deal between parties
    OriginateDeal {  // was OriginateLoan
        /// Originator user ID
        #[arg(long)]
        originator_id: String,  // was borrower_id
        /// Investor user ID  
        #[arg(long)]
        investor_id: String,    // was lender_id
        /// Deal amount in USDC
        #[arg(long)]
        amount: f64,
        // ... rest same
    },

    /// Make a payment toward a deal
    MakePayment {
        /// Deal ID to make payment to
        #[arg(long)]
        deal_id: String,  // was loan_id
        // ... rest same
    },

    /// Close out a deal
    CloseoutDeal {  // was CloseoutLoan  
        /// Deal ID to close out
        #[arg(long)]
        deal_id: String,  // was loan_id
        // ... rest same
    },

    // Add consolidated marketplace/transfer commands here
    /// List deal for sale
    ListDealForSale {
        // ... parameters
    },
    
    /// Transfer deal ownership
    TransferDeal {
        // ... parameters  
    },
}
```

### Task 6.3: Update CLI Implementation
**Files to modify:**
- `cli/src/deals/mod.rs` (renamed from loans/mod.rs)

**Changes:**
- Update all function implementations
- Update API client calls
- Update parameter mapping
- Update output formatting

### Task 6.4: Update CLI Integration
**Files to modify:**
- `cli/src/cli.rs`

**Changes:**
- Update command dispatcher
- Remove separate marketplace/transfer handling
- Update imports

### Task 6.5: Remove/Consolidate Old CLI Modules
**Files to remove/modify:**
- `cli/src/marketplace.rs` - Consolidate into deals module
- `cli/src/transfers.rs` - Consolidate into deals module

**Estimated Time: 2 days**
**Dependencies: Phase 5 (API layer) complete**
**Parallel Work: Can work on documentation in parallel**

---

## Phase 7: Documentation & Final Cleanup (Days 9-10)

**Priority: LOW - But important for maintainability**

### Task 7.1: Update Agent Documentation
**Files to modify:**
- `AGENTS.md`

**Changes:**
- Update all loan references to deal references
- Update CLI command examples
- Update API endpoint examples
- Update database schema documentation

### Task 7.2: Update README Files
**Files to modify:**
- `README.md`
- `cli/README.md`
- `contracts/README.md`
- Any other README files

**Changes:**
- Update terminology throughout
- Update example commands
- Update architecture descriptions

### Task 7.3: Update Architecture Documentation
**Files to modify:**
- `docs/architecture/` - All architecture files

**Changes:**
- Update terminology and concepts
- Update diagrams if any exist
- Update flow descriptions

### Task 7.4: Update Integration Examples
**Files to modify:**
- `docs/integrations/` - All integration documentation

**Changes:**
- Update API examples
- Update frontend integration examples
- Update authentication examples

**Estimated Time: 1-2 days**
**Dependencies: All implementation phases complete**
**Parallel Work: Can be done in parallel with final testing**

---


---

## Summary

**Total Timeline: 10-11 days**

**Critical Path:**
1. Database (2 days) 
2. Core Logic (2 days)
3. API Layer (3 days)
4. CLI Layer (2 days)
5. Testing (2 days)

**Parallel Opportunities:**
- ZK Program (Phase 3) can start once Phase 2 begins
- Smart Contracts (Phase 4) can be done parallel to API/CLI work
- Documentation (Phase 7) can be done parallel to later phases
- Some testing can be done incrementally during development

**Risk Mitigation:**
- Database changes are the highest risk - should be tested thoroughly
- Keep database migration reversible during initial testing
- Test each phase incrementally before moving to the next
- Maintain comprehensive test coverage throughout

## File Count Estimate

**Total Files to Modify: ~60-80 files**
- Database: ~10 files
- Core Logic: ~15 files  
- ZK Program: ~5 files
- Smart Contracts: ~8 files
- API Layer: ~20 files
- CLI Layer: ~10 files
- Documentation: ~15 files
- Tests: ~15 files

This plan provides a structured approach to completely converting the loan-based system to a deal-based system while maintaining all functionality and ensuring a smooth transition.
