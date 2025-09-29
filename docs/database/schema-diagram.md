# Arda Credit Database Schema Diagram

## Entity Relationship Overview

```mermaid
erDiagram
    companies ||--o{ users : "belongs_to"
    companies ||--o{ team_members : "has"
    users ||--o{ team_members : "member_of"
    users ||--o{ deals_originator : "originates"
    users ||--o{ deals_investor : "invests_in"
    users ||--o{ deal_payments : "makes"
    users ||--o{ deal_listings : "lists"
    users ||--o{ deal_transfers_from : "transfers_from"
    users ||--o{ deal_transfers_to : "receives"
    users ||--o{ transaction_queue : "creates"
    users ||--o{ user_balances : "has"
    users ||--o{ magic_links : "authenticates_via"
    users ||--o{ sessions : "has_sessions"
    users ||--o{ investor_profiles : "has_profile"

    deals ||--o{ deal_payments : "receives"
    deals ||--o{ deal_listings : "listed_as"
    deals ||--o{ deal_transfers : "transferred_via"
    deals ||--o{ transaction_queue : "generates"

    proof_batches ||--o{ transaction_queue : "includes"
    proof_batches ||--o{ state_snapshots : "creates"
    proof_batches ||--o{ ads_state_commits : "commits"
    proof_batches ||--o{ incoming_transactions : "includes"

    companies {
        uuid id PK
        varchar name
        varchar type "originator|investor"
        varchar kyb_status "not_submitted|pending|approved|rejected"
        varchar legal_name
        varchar tax_id UK
        varchar registration_number UK
        varchar jurisdiction
        timestamp created_at
        timestamp updated_at
    }

    users {
        uuid id PK
        uuid company_id FK
        varchar user_type "investor|originator|admin"
        varchar email UK
        varchar first_name
        varchar last_name
        varchar kyc_status "not_submitted|pending|approved|rejected"
        varchar kyc_investor_type "QIB|QP|AI"
        varchar wallet_address UK
        boolean system_admin
        timestamp deleted_at
        timestamp created_at
        timestamp updated_at
    }

    team_members {
        uuid id PK
        uuid company_id FK
        uuid user_id FK
        varchar role "admin|write|read"
        varchar status "active|inactive|suspended"
        boolean can_upload_documents
        boolean can_download_documents
        boolean can_delete_documents
        boolean can_approve_documents
        boolean can_manage_team
        boolean can_view_audit_logs
        timestamp created_at
        timestamp updated_at
    }

    deals {
        uuid id PK
        uuid originator_id FK
        uuid investor_id FK
        decimal principal_amount
        decimal outstanding_balance
        integer interest_rate_bps
        varchar status "draft|active|paid_off|closed|defaulted|for_sale|transfer_pending"
        varchar custom_deal_id UK
        varchar title
        text description
        varchar industry
        varchar risk_level "low|medium|high"
        varchar asset_class
        varchar region
        jsonb document_ids
        timestamp originated_at
        timestamp maturity_date
        timestamp created_at
        timestamp updated_at
    }

    deal_payments {
        uuid id PK
        uuid deal_id FK
        uuid payer_id FK
        decimal amount
        varchar payment_type "regular|principal|interest|penalty"
        varchar transaction_hash UK
        timestamp processed_at
        timestamp created_at
    }

    deal_listings {
        uuid id PK
        uuid deal_id FK UK
        uuid seller_id FK
        decimal asking_price
        text description
        varchar status "active|sold|cancelled|expired|delisted"
        timestamp listed_at
        timestamp expires_at
        timestamp created_at
        timestamp updated_at
    }

    deal_transfers {
        uuid id PK
        uuid deal_id FK
        uuid from_investor_id FK
        uuid to_investor_id FK
        decimal transfer_price
        varchar status "pending|completed|failed|cancelled"
        timestamp transfer_date
        timestamp completed_at
        text notes
        timestamp created_at
    }

    transaction_queue {
        uuid id PK
        bytea transaction_id_hash UK
        bytea user_id_hash
        bytea deal_id_hash
        varchar transaction_type
        decimal amount
        uuid user_id FK
        uuid deal_id FK
        uuid payment_id FK
        uuid company_id FK
        integer included_in_batch_id FK
        integer batch_position
        jsonb metadata
        timestamp created_at
    }

    proof_batches {
        integer id PK
        bigint previous_counter_value
        bigint final_counter_value
        integer[] transaction_ids
        varchar sindri_proof_id
        varchar proof_status "pending|proven|failed"
        varchar transaction_hash
        timestamp created_at
        timestamp proven_at
    }

    user_balances {
        uuid id PK
        uuid user_id FK
        varchar balance_type "available|locked|reserved"
        decimal amount
        varchar currency
        timestamp created_at
        timestamp updated_at
    }

    magic_links {
        uuid id PK
        uuid user_id FK
        bytea token_hash UK
        varchar state
        varchar redirect_uri
        inet client_ip
        text user_agent
        timestamp expires_at
        timestamp consumed_at
        timestamp created_at
    }

    sessions {
        uuid id PK
        uuid user_id FK
        varchar session_token UK
        timestamp expires_at
        text client_ip
        text user_agent
        timestamp created_at
        timestamp last_accessed_at
    }

    investor_profiles {
        uuid id PK
        uuid user_id FK UK
        boolean accredited_investor
        decimal investment_capacity
        varchar regulatory_jurisdiction
        timestamp created_at
        timestamp updated_at
    }
```

## Core Table Relationships

### 1. **Companies & Users** (Multi-tenant Foundation)
```
companies (1) ←→ (0..*) users
companies (1) ←→ (0..*) team_members ←→ (0..*) users
```
- Companies represent both originators (deal creators) and investors (capital providers)
- Users can be associated with companies or operate independently
- Team members provide granular role-based access control within companies

### 2. **Deal Lifecycle** (Core Business Logic)
```
users (originators) (1) ←→ (0..*) deals ←→ (0..*) users (investors)
deals (1) ←→ (0..*) deal_payments
deals (1) ←→ (0..1) deal_listings (marketplace)
deals (1) ←→ (0..*) deal_transfers (ownership changes)
```
- Deals represent financial agreements between originators and investors
- Payments automatically update deal balances via triggers
- Marketplace enables secondary trading
- Transfers track ownership changes with audit trail

### 3. **Privacy & State Management** (ZK Proof Integration)
```
users/deals (1..*) ←→ (1) transaction_queue (1..*) ←→ (1) proof_batches
proof_batches (1) ←→ (0..*) state_snapshots
proof_batches (1) ←→ (0..*) ads_state_commits
```
- Transaction queue stages operations for ZK proof inclusion
- Proof batches provide privacy-preserving verification
- State snapshots enable auditing and rollback capabilities
- ADS commits link to smart contract state

### 4. **Authentication & Sessions** (Security)
```
users (1) ←→ (0..*) magic_links (email-based auth)
users (1) ←→ (0..*) sessions (cookie-based sessions)
```
- Magic links provide passwordless authentication
- Sessions maintain authenticated state with security tracking

### 5. **Financial Management** (Balance & Payments)
```
users (1) ←→ (0..*) user_balances (multi-currency, multi-type)
users (1) ←→ (0..*) user_deposit_balances (wallet-specific)
```
- User balances support available, locked, and reserved funds
- Deposit balances track wallet-specific USDC deposits

## Key Performance Indexes

### Foreign Key Indexes ✅
- All foreign key relationships have corresponding indexes for join performance
- Partial indexes used where appropriate (e.g., non-null values only)

### Composite Indexes for Common Queries ✅
```sql
-- Company queries
idx_companies_type_kyb_status (type, kyb_status)

-- User queries
idx_users_company_type (company_id, user_type)
idx_users_company_kyc (company_id, kyc_status)
idx_users_type_kyc_status (user_type, kyc_status)

-- Deal queries (MVP critical patterns)
idx_deals_investor_status_originated (investor_id, status, originated_at)
idx_deals_originator_status_originated (originator_id, status, originated_at)
idx_deals_status_balance (status, outstanding_balance)
idx_deals_industry_status (industry, status)

-- State transition queries
idx_transaction_queue_type_batch_created (transaction_type, included_in_batch_id, created_at)
idx_transaction_queue_unbatched_priority (transaction_type, created_at) WHERE included_in_batch_id IS NULL
```

### Unique Constraints ✅
```sql
-- Business-critical uniqueness
companies.name, companies.type -- Unique company names per type
companies.legal_name -- Globally unique legal names
companies.tax_id -- Globally unique tax IDs
users.email -- Unique email addresses
users.wallet_address -- Unique wallet addresses
deals.custom_deal_id, deals.originator_id -- Unique custom IDs per originator
deal_listings.deal_id -- Only one active listing per deal
```

## Data Types & Constraints

### Financial Precision
- All monetary amounts use `DECIMAL(18, 6)` for precise USDC calculations (6 decimal places)
- Interest rates stored as basis points (`INTEGER`) for exact calculations
- Balance checks ensure non-negative values

### Privacy & Security
- Transaction hashes stored as `BYTEA` (SHA256) for ZK circuit compatibility
- Magic link tokens hashed before storage (never store plaintext)
- IP addresses and user agents logged for security auditing

### Status Management
- Enums used for status fields to ensure data integrity
- Check constraints prevent invalid state transitions
- Triggers automatically update related records (e.g., deal balance on payment)

## Scalability Considerations

### Partitioning Opportunities (Future)
- `transaction_queue` by `created_at` (monthly partitions)
- `deal_payments` by `processed_at` (monthly partitions)
- `proof_batches` by `created_at` (monthly partitions)

### Archive Strategy
- `magic_links` and `sessions` have automatic cleanup functions
- Soft delete pattern used for `users` table (`deleted_at`)
- Historical data maintained in `deal_transfers` and `state_snapshots`

### Performance Monitoring
- Views created for index usage tracking
- Functions for foreign key index validation
- Regular `ANALYZE` recommended for optimal query planning

## Security & Compliance

### Data Privacy
- PII encrypted at application level
- ZK proofs maintain transaction privacy
- Hashed identifiers used in privacy-critical tables

### Audit Trail
- All tables include `created_at` timestamps
- Critical tables include `updated_at` with automatic triggers
- State transitions provide complete audit history
- Transfer records maintain compliance documentation

### Access Control
- Role-based permissions in `team_members`
- Company-level data isolation
- KYC status controls access to sensitive operations
- System admin flags for privileged operations