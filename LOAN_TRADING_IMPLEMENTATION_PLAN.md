# 🚀 **Loan Trading Implementation Plan**

## 📋 **Executive Summary**

**Goal**: Implement loan trading marketplace functionality to enable users to list loans for sale and transfer ownership.

**Scope**: MVP loan trading with 3 core endpoints, leveraging existing infrastructure.

**Timeline**: 3-week phased approach with weekly deliverables.

---

## ✅ **Design Decisions (Confirmed)**

### **Infrastructure Integration**
- **Account Balance**: Use existing `user_balances.available_balance` system for loan purchases
- **User Management**: Continue using `identities` table (not users table)
- **Payment Settlement**: Simple debit/credit approach for MVP
- **KYC Validation**: Auto-approve approach (consistent with current system)

### **Data Model Strategy**
- **Loan Status**: Additive approach - keep existing statuses, add new ones
- **API Alignment**: Design responses to match frontend `Deal` interface
- **Database**: Add new tables, extend existing schemas minimally

---

## 🗓️ **3-Phase Implementation Plan**

## **Phase 1: Core Trading MVP (Week 1)**
*Priority: Get frontend working with basic trading functionality*

### **Database Changes**
```sql
-- 1. Add new loan trading tables
CREATE TABLE loan_listings (
    listing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(loan_id) UNIQUE,
    seller_id UUID NOT NULL REFERENCES identities(id), 
    asking_price DECIMAL(15,2) NOT NULL,
    description TEXT,
    listed_at TIMESTAMP DEFAULT NOW(),
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'sold', 'cancelled')),
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE loan_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    loan_id UUID NOT NULL REFERENCES loans(loan_id),
    from_lender_id UUID NOT NULL REFERENCES identities(id),
    to_lender_id UUID NOT NULL REFERENCES identities(id),
    purchase_price DECIMAL(15,2) NOT NULL,
    transfer_date TIMESTAMP DEFAULT NOW(),
    notes TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

-- 2. Add new loan statuses (additive - keep existing)
-- Current: originated, active, paid_off, closed, defaulted
-- Adding: for_sale, transfer_pending
-- Implementation: Update CHECK constraint on loans table
ALTER TABLE loans DROP CONSTRAINT IF EXISTS loans_loan_status_check;
ALTER TABLE loans ADD CONSTRAINT loans_loan_status_check 
CHECK (loan_status IN (
    'originated', 'active', 'paid_off', 'closed', 'defaulted',  -- Existing
    'for_sale', 'transfer_pending'                               -- New
));

-- 3. Add marketplace-friendly fields to loans (optional for Phase 1)
ALTER TABLE loans ADD COLUMN IF NOT EXISTS title VARCHAR(255);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS description TEXT;

-- 4. Add performance indexes
CREATE INDEX idx_loan_listings_status_listed ON loan_listings(status, listed_at);
CREATE INDEX idx_loan_transfers_loan ON loan_transfers(loan_id);
CREATE INDEX idx_loans_for_sale ON loans(loan_status) WHERE loan_status = 'for_sale';
```

### **API Endpoints (3 Core)**

#### **1. Browse Marketplace**
```typescript
GET /api/v1/loans/for-sale
Query Parameters:
- limit?: number (default: 20)
- offset?: number (default: 0)
- min_amount?: string
- max_amount?: string  
- max_price?: string

Response: LoansForSaleResponse {
  listings: MarketplaceDeal[]
  total_count: number
  has_more: boolean
}

MarketplaceDeal {
  // Align with frontend Deal interface
  id: string                    // loan_id
  title: string                 // loan.title or generated
  description: string           // listing.description
  merchantId: string            // loan.borrower_id  
  loanAmountRange: {
    min: string
    max: string                 // Same value for single loan
  }
  interestRateRange: {
    min: string 
    max: string                 // Same value for single loan
  }
  status: string                // 'for_sale'
  askingPrice: string           // listing.asking_price
  listedAt: string             // listing.listed_at
  sellerId: string             // listing.seller_id
  sellerEmail: string          // identity.email
}
```

#### **2. List Loan for Sale**
```typescript
POST /api/v1/loan/{loan_id}/list-for-sale

Request Body: {
  asking_price: string          // Required: "50000.00"
  description?: string          // Optional listing description
}

Response: {
  listing_id: string
  loan_id: string  
  asking_price: string
  listed_at: string
  status: "active"
}

Business Logic:
1. Verify caller owns loan (lender_id === request.user_id)
2. Verify loan_status === 'active' or 'originated'
3. Verify caller KYC status (auto-approve for now)
4. Insert loan_listings record
5. Update loans SET loan_status = 'for_sale'
```

#### **3. Transfer Loan Ownership**
```typescript
POST /api/v1/loan/{loan_id}/transfer

Request Body: {
  new_lender_id: string         // Buyer's identity ID
  purchase_price: string        // Purchase amount
  transfer_notes?: string       // Optional notes
}

Response: {
  transfer_id: string
  loan_id: string
  previous_lender_id: string
  new_lender_id: string
  purchase_price: string
  transfer_date: string
  updated_loan: MarketplaceDeal  // Updated loan details
}

Business Logic:
1. Validate loan exists and is 'for_sale'
2. Validate buyer !== current owner
3. Validate buyer KYC (auto-approve)
4. Check buyer balance >= purchase_price
5. Begin transaction:
   - Debit buyer account (user_balances)
   - Credit seller account
   - Update loans.lender_id = new_lender_id
   - Update loans.loan_status = 'active'
   - Update loan_listings.status = 'sold'
   - Insert loan_transfers record
6. Commit transaction
```

### **File Structure**
```
api/src/
├── loan_marketplace_handlers.rs    // New: Marketplace endpoints
├── loan_transfer_handlers.rs       // New: Transfer endpoints  
├── paths.rs                        // Add new route paths
└── router.rs                       // Wire up new routes

db/src/
├── marketplace.rs                  // New: Marketplace queries
├── transfers.rs                    // New: Transfer operations
└── migrations/
    └── XXX_add_loan_trading.sql    // New migration

cli/src/
└── loans/
    ├── marketplace.rs              // New: CLI marketplace commands
    └── transfers.rs                // New: CLI transfer commands
```

### **Testing Strategy**
```bash
# CLI Testing Commands
make cli ARGS="list-loan-for-sale --loan-id <uuid> --asking-price 50000.00"
make cli ARGS="browse-marketplace --limit 10"  
make cli ARGS="transfer-loan --loan-id <uuid> --buyer-id <uuid> --price 45000.00"

# Integration Tests
- List/transfer happy path
- Insufficient balance scenarios
- Ownership validation
- KYC gating (even if auto-approved)
```

---

## **Phase 2: Enhanced Trading (Week 2)**
*Priority: Complete marketplace functionality*

### **Additional Endpoints**
```typescript
// Remove from marketplace
DELETE /api/v1/loan/{loan_id}/list-for-sale

// Transfer history
GET /api/v1/loan/{loan_id}/transfers

// Enhanced loan model
PUT /api/v1/loan/{loan_id}  // Add title, description fields
```

### **Enhanced Data Model**
```sql
-- Add more Deal-aligned fields
ALTER TABLE loans ADD COLUMN IF NOT EXISTS risk_level VARCHAR(10) 
CHECK (risk_level IN ('low', 'medium', 'high'));
ALTER TABLE loans ADD COLUMN IF NOT EXISTS asset_class VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS region VARCHAR(100);
ALTER TABLE loans ADD COLUMN IF NOT EXISTS minimum_investment DECIMAL(15,2);

-- Enhanced transfer tracking
ALTER TABLE loan_transfers ADD COLUMN transfer_status VARCHAR(20) DEFAULT 'completed'
CHECK (transfer_status IN ('pending', 'completed', 'failed', 'cancelled'));
```

### **Business Logic Enhancements**
- Proper error handling with standardized error codes
- Enhanced validation (duplicate listings, transfer limits)
- Audit logging for all marketplace operations

---

## **Phase 3: Polish & Optimization (Week 3)**
*Priority: Production readiness*

### **Advanced Features**
```typescript
// Advanced marketplace search
GET /api/v1/loans/for-sale/search
Query: risk_level, asset_class, region, price_range

// Marketplace analytics
GET /api/v1/loans/marketplace/stats

// User portfolio with trading data
GET /api/v1/user/{id}/portfolio/trading
```

### **Performance Optimizations**
- Query optimization with proper indexes
- Response pagination for large result sets
- Caching frequently accessed marketplace data

### **Enhanced Business Rules**
- Transfer limits per user/timeframe
- Marketplace listing fees (if needed)
- Enhanced KYC validation when ready

---

## 🏗️ **Technical Implementation Details**

### **Database Migration Strategy**
```sql
-- Migration file: db/migrations/XXX_add_loan_trading.sql
-- Safe to run on existing data:
-- 1. Creates new tables (no data loss)
-- 2. Adds new enum values (backwards compatible)
-- 3. Adds optional columns with defaults
```

### **API Integration Points**
```rust
// Reuse existing infrastructure:
use arda_credit_db::{
    get_identity_by_id,           // KYC validation
    get_user_balance_by_id,       // Balance checks
    update_user_balance_*,        // Payment settlement
    get_loan_by_id,              // Loan operations
};

use crate::{
    kyc_service::KycValidationService,    // Existing KYC
    balance_service::BalanceService,      // Existing balance management
};
```

### **Error Handling Pattern**
```rust
// Follow existing pattern in loan_handlers.rs
pub enum MarketplaceError {
    LoanNotFound,
    InsufficientBalance,
    UnauthorizedOwner,
    InvalidKycStatus,
    LoanNotForSale,
    DatabaseError(String),
}

// Convert to HTTP status codes consistently
impl Into<(StatusCode, String)> for MarketplaceError { ... }
```

---

## 🧪 **Testing Strategy**

### **Phase 1 Testing**
```bash
# Unit Tests
cargo test marketplace
cargo test loan_transfer

# Integration Tests  
make test   # Full test suite with database

# CLI Happy Path Testing
make cli ARGS="register-user --user-type lender --email seller@test.com"
make cli ARGS="register-user --user-type investor --email buyer@test.com"
make cli ARGS="originate-loan --borrower-id <b> --lender-id <s> --amount 100000"
make cli ARGS="list-loan-for-sale --loan-id <loan> --asking-price 95000"
make cli ARGS="browse-marketplace"
make cli ARGS="transfer-loan --loan-id <loan> --buyer-id <b> --price 95000"
```

### **Test Data Setup**
```sql
-- Create test scenario with:
-- 1. 2 KYC'd users with sufficient balances
-- 2. 1 active loan owned by user 1
-- 3. Test listing and transfer flow
```

---

## 🔒 **Security Considerations**

### **Authorization**
- Loan ownership validation (only lender can list/transfer)
- Balance sufficiency checks (atomic transactions)
- KYC status validation (ready for real checks later)

### **Transaction Safety**
- All financial operations in database transactions
- Rollback on any failure in transfer process
- Audit trail for all marketplace operations

### **Input Validation**
- Decimal precision validation for prices
- UUID format validation
- SQL injection prevention (using parameterized queries)

---

## 📊 **Monitoring & Observability**

### **Metrics to Add**
```rust
// Following existing pattern in loan_handlers.rs
metrics::counter!("marketplace_listings_total").increment(1);
metrics::counter!("loan_transfers_total").increment(1);
metrics::histogram!("loan_transfer_amount").record(purchase_price);
```

### **Logging Strategy**
```rust
// Following existing pattern
info!("💰 MARKETPLACE: Listed loan {} for ${}", loan_id, asking_price);
info!("🔄 TRANSFER: Loan {} from {} to {} for ${}", loan_id, seller, buyer, price);
error!("❌ TRANSFER FAILED: {}", error);
```

---

## 🚨 **Risk Assessment**

### **Low Risk** ✅
- Database schema changes (additive, backwards compatible)
- New API endpoints (isolated functionality)
- Existing infrastructure reuse (proven patterns)

### **Medium Risk** ⚠️
- Financial transaction logic (needs careful testing)
- Account balance integration (existing but complex)
- Data consistency during transfers (transaction management)

### **Mitigation Strategy**
- Comprehensive integration testing before deployment
- Transaction rollback on any failure
- Gradual rollout with monitoring

---

## 🎯 **Success Criteria**

### **Phase 1 MVP**
- [ ] Frontend can browse empty marketplace (returns 200 with empty results)
- [ ] Users can list loans for sale via API
- [ ] Users can transfer loan ownership with payment
- [ ] All operations are transactionally safe
- [ ] CLI commands work for testing

### **Phase 2 Complete**
- [ ] Full marketplace CRUD operations
- [ ] Enhanced Deal-format responses
- [ ] Transfer history tracking
- [ ] Error handling for edge cases

### **Phase 3 Production**
- [ ] Performance optimized for 1000+ listings
- [ ] Advanced filtering and search
- [ ] Comprehensive monitoring
- [ ] Production-ready validation

---

## 🤝 **Next Steps**

1. **Approve this plan** and confirm frontend Deal interface details
2. **Start Phase 1** with database migration
3. **Implement 3 core endpoints** in parallel
4. **Test integration** with frontend
5. **Iterate based on feedback**

---

## 📞 **Questions/Blockers**

1. **Frontend Deal interface**: Do you have the exact TypeScript interface definition I should match?
2. **User registration flow**: Should marketplace users go through existing CLI registration, or do we need API-driven registration?
3. **Testing data**: Should I create test data population scripts for marketplace testing?
4. **Deployment strategy**: Any specific requirements for database migration in production?

**Ready to start implementation once approved!** 🚀
