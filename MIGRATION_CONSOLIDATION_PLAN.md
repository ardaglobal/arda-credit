# Database Migration Consolidation Plan

## Current State Analysis

**Problem**: 66 migration files accumulated from proof-of-concept development  
**Goal**: Consolidate to 8-10 focused, well-documented migrations  
**Approach**: Group related functionality into logical migration units

## Proposed Migration Structure

### 1. `001_core_infrastructure.sql`
**Purpose**: Foundation tables for ZK proof and state management

```sql
-- Core processing tables
CREATE TABLE processor_state (...);
CREATE TABLE global_state (...);
CREATE TABLE counter_state (...);
CREATE TABLE proof_batches (...);
CREATE TABLE arithmetic_transactions (...);

-- ADS and Merkle tree infrastructure  
CREATE TABLE indexed_merkle_tree (...);
CREATE TABLE nullifiers (...);
CREATE TABLE audit_events (...);

-- Base indexes and constraints
CREATE INDEX idx_proof_batches_status ON proof_batches(status);
-- ... other core indexes
```

**Consolidates**: Migrations 001-012 (core architecture)

### 2. `002_authentication_system.sql`  
**Purpose**: User management and authentication

```sql
-- User identity and authentication
CREATE TABLE identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    user_type VARCHAR(20) NOT NULL CHECK (user_type IN ('investor', 'originator', 'admin')),
    kyc_status VARCHAR(20) DEFAULT 'not_submitted' 
        CHECK (kyc_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    kyc_verified_at TIMESTAMP WITH TIME ZONE,
    wallet_address VARCHAR(42), -- Ethereum address
    jurisdiction VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE -- Soft delete support
);

-- Magic link authentication (passwordless)
CREATE TABLE magic_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL, -- SHA-256 hash (never store plaintext)
    state VARCHAR(32) NOT NULL, -- CSRF protection
    redirect_uri VARCHAR(500),
    client_ip INET,
    user_agent TEXT,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    consumed_at TIMESTAMP WITH TIME ZONE, -- NULL = unused
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Session management
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    session_token VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    client_ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Invite code system
CREATE TABLE invite_codes (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) UNIQUE NOT NULL,
    user_type VARCHAR(20) NOT NULL CHECK (user_type IN ('investor', 'originator', 'admin')),
    created_by UUID REFERENCES identities(id),
    uses_remaining INTEGER DEFAULT 1,
    max_uses INTEGER DEFAULT 1,
    expires_at TIMESTAMP WITH TIME ZONE,
    description TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

**Consolidates**: Migrations 023-029, 055-059 (authentication & user management)

### 3. `003_deal_system.sql`
**Purpose**: Core business objects for deal management

```sql  
-- Main deals table (converted from loans)
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    originator_id UUID NOT NULL REFERENCES identities(id),
    investor_id UUID NOT NULL REFERENCES identities(id),
    principal_amount DECIMAL(18, 6) NOT NULL CHECK (principal_amount > 0),
    outstanding_balance DECIMAL(18, 6) NOT NULL CHECK (outstanding_balance >= 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0),
    status VARCHAR(20) NOT NULL DEFAULT 'originated' 
        CHECK (status IN ('originated', 'active', 'paid_off', 'closed', 'defaulted', 'for_sale')),
    originated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    maturity_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Marketplace fields
    title VARCHAR(255),
    description TEXT,
    risk_level VARCHAR(10) CHECK (risk_level IN ('low', 'medium', 'high')),
    asset_class VARCHAR(100),
    region VARCHAR(100),
    minimum_investment DECIMAL(15,2),
    custom_deal_id VARCHAR(100), -- External system mapping
    industry VARCHAR(100),
    document_ids JSONB, -- Array of document UUIDs
    
    CONSTRAINT originator_investor_different CHECK (originator_id != investor_id)
);

-- Deal payments tracking
CREATE TABLE deal_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),
    payer_id UUID NOT NULL REFERENCES identities(id),
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    payment_type VARCHAR(20) NOT NULL DEFAULT 'regular' 
        CHECK (payment_type IN ('regular', 'principal', 'interest', 'penalty')),
    transaction_hash VARCHAR(66), -- Ethereum tx hash if on-chain
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Deal state transitions for ZK circuit
CREATE TABLE deal_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id_hash BYTEA NOT NULL UNIQUE,
    user_id_hash BYTEA NOT NULL,
    deal_id_hash BYTEA, -- NULL for non-deal transactions
    transaction_type VARCHAR(30) NOT NULL CHECK (transaction_type IN (
        'user_registration', 'kyc_submission', 'kyc_verification',
        'user_deposit', 'user_withdrawal',
        'deal_origination', 'deal_payment', 'deal_closeout'
    )),
    amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
    
    -- Foreign key references
    identity_id UUID REFERENCES identities(id),
    deal_id UUID REFERENCES deals(id),
    payment_id UUID REFERENCES deal_payments(id),
    
    -- Batch processing
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

**Consolidates**: Migrations 060-068 (deal system, loan conversion)

### 4. `004_trading_marketplace.sql`
**Purpose**: Secondary market for deal trading

```sql
-- Deal marketplace listings
CREATE TABLE deal_listings (
    listing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),
    seller_id UUID NOT NULL REFERENCES identities(id),
    asking_price DECIMAL(15,2) NOT NULL CHECK (asking_price > 0),
    description TEXT,
    listed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,
    status VARCHAR(20) NOT NULL DEFAULT 'active' 
        CHECK (status IN ('active', 'sold', 'cancelled', 'expired', 'delisted')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT deal_listings_unique_deal UNIQUE(deal_id)
);

-- Deal ownership transfers
CREATE TABLE deal_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id),
    from_investor_id UUID NOT NULL REFERENCES identities(id),
    to_investor_id UUID NOT NULL REFERENCES identities(id),
    transfer_price DECIMAL(15,2) NOT NULL CHECK (transfer_price > 0),
    transfer_status VARCHAR(20) NOT NULL DEFAULT 'pending' 
        CHECK (transfer_status IN ('pending', 'completed', 'failed', 'cancelled')),
    transfer_date TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT deal_transfers_no_self_transfer CHECK (from_investor_id != to_investor_id)
);

-- Marketplace view for browsing
CREATE VIEW marketplace_deals AS
SELECT
    dl.listing_id,
    dl.asking_price,
    dl.description as listing_description,
    dl.listed_at,
    dl.status as listing_status,
    
    d.id as deal_id,
    d.originator_id,
    d.investor_id,
    d.principal_amount,
    d.outstanding_balance,
    d.interest_rate_bps,
    d.status as deal_status,
    d.title,
    d.risk_level,
    d.asset_class,
    d.region,
    
    i.email as seller_email,
    i.kyc_status as seller_kyc_status
FROM deal_listings dl
JOIN deals d ON dl.deal_id = d.id
JOIN identities i ON dl.seller_id = i.id  
WHERE dl.status = 'active' AND d.status = 'for_sale';
```

**Consolidates**: Trading functionality spread across multiple migrations

### 5. `005_financial_system.sql`
**Purpose**: USDC deposits, withdrawals, and balance management

```sql
-- USDC deposit tracking from smart contract events
CREATE TABLE usdc_deposits (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL, -- Ethereum address
    amount DECIMAL(18, 6) NOT NULL, -- Amount in wei (6 decimals for USDC)
    transaction_hash VARCHAR(66) UNIQUE NOT NULL,
    status VARCHAR(20) DEFAULT 'pending' 
        CHECK (status IN ('pending', 'confirmed', 'processed', 'failed')),
    confirmation_count INTEGER DEFAULT 0,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE
);

-- User balance aggregation (updated by triggers)
CREATE TABLE user_deposit_balances (
    user_address VARCHAR(42) PRIMARY KEY,
    total_deposited DECIMAL(18, 6) DEFAULT 0,
    available_balance DECIMAL(18, 6) DEFAULT 0,
    reserved_balance DECIMAL(18, 6) DEFAULT 0, -- For pending transactions
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- USDC withdrawal tracking
CREATE TABLE usdc_withdrawals (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL,
    recipient_address VARCHAR(42) NOT NULL,
    amount DECIMAL(18, 6) NOT NULL,
    transaction_hash VARCHAR(66),
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    gas_limit BIGINT DEFAULT 100000,
    gas_price DECIMAL(18, 6),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE
);

-- Withdrawal request queue
CREATE TABLE withdrawal_requests (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL,
    recipient_address VARCHAR(42) NOT NULL,
    amount DECIMAL(18, 6) NOT NULL CHECK (amount > 0),
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'processing', 'completed', 'rejected', 'cancelled')),
    requester_ip TEXT,
    kyc_verified BOOLEAN DEFAULT FALSE,
    admin_notes TEXT,
    requested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    approved_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE
);
```

**Consolidates**: Migrations 027, 030-037 (USDC & balance management)

### 6. `006_liquidity_pools.sql`
**Purpose**: Pool creation and management (if needed for MVP)

```sql
-- Liquidity pool definitions
CREATE TABLE liquidity_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    creator_id UUID NOT NULL REFERENCES identities(id),
    max_ltv_ratio DECIMAL(5,4) NOT NULL CHECK (max_ltv_ratio > 0 AND max_ltv_ratio <= 1),
    max_interest_rate DECIMAL(6,4) NOT NULL CHECK (max_interest_rate > 0),
    repayment_period_days INTEGER NOT NULL CHECK (repayment_period_days > 0),
    total_deposits DECIMAL(18, 6) DEFAULT 0,
    available_liquidity DECIMAL(18, 6) DEFAULT 0,
    utilized_liquidity DECIMAL(18, 6) DEFAULT 0,
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'PAUSED', 'CLOSED')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Pool geographic limits (if required)
CREATE TABLE pool_borough_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES liquidity_pools(id) ON DELETE CASCADE,
    borough VARCHAR(100) NOT NULL,
    max_allocation_percentage DECIMAL(5,2) NOT NULL CHECK (max_allocation_percentage > 0 AND max_allocation_percentage <= 100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(pool_id, borough)
);
```

**Consolidates**: Migrations 038-044, 051 (liquidity pool system)

### 7. `007_user_profiles.sql`  
**Purpose**: Extended user profile data

```sql
-- Investor-specific profile data
CREATE TABLE investor_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE UNIQUE,
    investment_focus TEXT[],
    min_investment_amount DECIMAL(15,2),
    max_investment_amount DECIMAL(15,2),  
    preferred_deal_duration_days INTEGER,
    risk_tolerance VARCHAR(20) CHECK (risk_tolerance IN ('conservative', 'moderate', 'aggressive')),
    investment_thesis TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Future: Originator profiles, Company/Organization tables
```

**Consolidates**: Migrations 039-040 (profile management)

### 8. `008_indexes_and_functions.sql`
**Purpose**: Performance optimization and helper functions

```sql
-- Performance indexes for deals
CREATE INDEX idx_deals_originator_id ON deals(originator_id);
CREATE INDEX idx_deals_investor_id ON deals(investor_id);  
CREATE INDEX idx_deals_status ON deals(status);
CREATE INDEX idx_deals_outstanding_balance ON deals(outstanding_balance) WHERE outstanding_balance > 0;

-- Marketplace indexes
CREATE INDEX idx_deal_listings_status_listed ON deal_listings(status, listed_at);
CREATE INDEX idx_deal_listings_price ON deal_listings(asking_price);

-- Transaction processing indexes
CREATE INDEX idx_deal_transactions_batch_id ON deal_transactions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_deal_transactions_not_batched ON deal_transactions(id) WHERE included_in_batch_id IS NULL;

-- Helper functions
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Auto-update timestamps
CREATE TRIGGER update_deals_updated_at BEFORE UPDATE ON deals 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Deal balance update on payment
CREATE OR REPLACE FUNCTION update_deal_balance_on_payment()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE deals SET
        outstanding_balance = outstanding_balance - NEW.amount,
        status = CASE WHEN (outstanding_balance - NEW.amount) <= 0 THEN 'paid_off' ELSE status END,
        updated_at = NOW()
    WHERE id = NEW.deal_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_deal_on_payment AFTER INSERT ON deal_payments
    FOR EACH ROW EXECUTE FUNCTION update_deal_balance_on_payment();
```

## Migration Execution Plan

### Phase 1: Backup & Analysis
1. **Create full database backup**
2. **Document current schema** with pg_dump --schema-only
3. **Export all current data** 
4. **Verify no data loss scenarios**

### Phase 2: Create Clean Schema
1. **Create new database** with consolidated migrations
2. **Test migration sequence** on empty database
3. **Validate all constraints and indexes**

### Phase 3: Data Migration
1. **Create data migration scripts** to move from old to new schema
2. **Handle any data transformations** (e.g., loan_status → status)
3. **Verify data integrity** after migration

### Phase 4: Deployment  
1. **Deploy to development environment** first
2. **Run full test suite** against new schema
3. **Deploy to staging** with production-like data
4. **Final production deployment** during maintenance window

## Benefits of Consolidation

### Developer Experience
- **Clearer database structure** - logical grouping of related tables
- **Easier onboarding** - new developers understand schema faster
- **Reduced complexity** - fewer files to manage and understand

### Performance
- **Optimized indexes** - consolidated index creation reduces overhead
- **Better query planning** - related tables created together
- **Reduced migration overhead** - fewer migration checks on startup

### Maintenance
- **Easier schema changes** - clear ownership of table groups
- **Better documentation** - each migration has clear purpose
- **Simplified rollbacks** - logical boundaries for schema changes

## Validation Checklist

### Pre-Migration
- [ ] All current functionality works with existing schema
- [ ] Full database backup created and tested
- [ ] Migration scripts tested on development data

### Post-Migration  
- [ ] All MVP endpoints work correctly
- [ ] ZK proof generation still functions
- [ ] Marketplace operations complete successfully
- [ ] User authentication flows work
- [ ] USDC deposit/withdrawal processes correctly
- [ ] Deal CRUD operations maintain data integrity
- [ ] Performance metrics match or improve over baseline

### Testing Requirements
- [ ] Full integration test suite passes
- [ ] Load testing with realistic data volumes
- [ ] Endpoint response times within acceptable ranges
- [ ] ZK proof generation times unchanged or improved
