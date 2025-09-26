# MVP Code Consolidation Implementation Checklist

## Overview

This checklist provides specific, actionable steps to implement the audit recommendations and consolidate the codebase around the MVP's core requirements. Each item includes the specific files to modify and commands to run.

## Phase 1: Critical Naming Fixes (Week 1)

### 1.1 Fix Database Field Names (High Priority)

#### Remove `loan_status` field naming
- [ ] **Database Migration**: Create migration to rename `loan_status` to `status` in `deals` table
  ```sql
  -- File: db/migrations/069_fix_deal_status_naming.sql
  ALTER TABLE deals RENAME COLUMN loan_status TO status;
  ```

- [ ] **Update Rust Structs**: Fix `Deal` struct in `db/src/db.rs`
  ```rust
  // CHANGE FROM:
  pub loan_status: String,
  
  // CHANGE TO:  
  pub status: String,
  ```

- [ ] **Update All References**: Search and replace `loan_status` with `status`
  ```bash
  # Find all files that reference loan_status
  grep -r "loan_status" --include="*.rs" --include="*.sql" .
  
  # Update each file manually to ensure context is preserved
  ```

#### Fix Stuttering Field Names
- [ ] **Update Deal struct** - Remove stuttering in `db/src/db.rs`:
  ```rust
  // CURRENT (BAD)
  pub struct Deal {
      pub deal_id: uuid::Uuid,        // Remove
      pub deal_description: String,   // Remove  
      pub deal_title: String,         // Remove
  }
  
  // RECOMMENDED (GOOD)
  pub struct Deal {
      pub id: uuid::Uuid,
      pub description: Option<String>,
      pub title: Option<String>,
  }
  ```

- [ ] **Update database migration** to reflect new field names
- [ ] **Update all API handlers** that reference these fields
- [ ] **Update serialization** in API responses

### 1.2 Files to Modify for Naming Fixes

- [ ] `db/src/db.rs` - Deal struct definition
- [ ] `api/src/deal_handlers.rs` - All deal endpoints
- [ ] `db/src/marketplace.rs` - Marketplace structs
- [ ] `db/src/transfers.rs` - Transfer structs  
- [ ] `lib/src/deal.rs` - Deal processing logic
- [ ] All migration files referencing `loan_status`

## Phase 2: Database Migration Consolidation (Week 2)

### 2.1 Create Consolidated Migrations

- [ ] **Backup current database**
  ```bash
  pg_dump $DATABASE_URL > backup_pre_consolidation.sql
  pg_dump --schema-only $DATABASE_URL > schema_backup.sql
  ```

- [ ] **Create new migration directory**
  ```bash
  mkdir db/migrations_consolidated
  ```

- [ ] **Create consolidated migration files** (see `MIGRATION_CONSOLIDATION_PLAN.md`):
  - [ ] `001_core_infrastructure.sql`
  - [ ] `002_authentication_system.sql`
  - [ ] `003_deal_system.sql`
  - [ ] `004_trading_marketplace.sql`
  - [ ] `005_financial_system.sql`
  - [ ] `006_liquidity_pools.sql`
  - [ ] `007_user_profiles.sql`
  - [ ] `008_indexes_and_functions.sql`

### 2.2 Migration Testing Strategy

- [ ] **Test on fresh database**
  ```bash
  # Create test database
  createdb arda_credit_migration_test
  
  # Run consolidated migrations
  DATABASE_URL=postgresql://postgres@localhost/arda_credit_migration_test make initDB
  
  # Verify schema matches current production
  pg_dump --schema-only arda_credit_migration_test > new_schema.sql
  diff schema_backup.sql new_schema.sql
  ```

- [ ] **Create data migration script** to move from old to new schema if needed
- [ ] **Test with actual data** using a copy of staging database

### 2.3 Migration Deployment

- [ ] **Update Makefile** to use new consolidated migrations
- [ ] **Update CI/CD pipeline** to run new migration sequence
- [ ] **Deploy to staging environment** first
- [ ] **Validate all functionality** works with new schema
- [ ] **Archive old migrations** (don't delete, move to `db/migrations_archive/`)

## Phase 3: API Endpoint Cleanup (Week 2-3)

### 3.1 OpenAPI Documentation Cleanup

- [ ] **Add feature flags to `api/src/openapi.rs`**:
  ```rust
  #[derive(OpenApi)]
  #[openapi(
      paths(
          // ✅ MVP CRITICAL - Keep these
          crate::deal_handlers::create_deal_endpoint,
          crate::deal_handlers::list_deals_endpoint,
          crate::deal_handlers::browse_deals_for_sale_endpoint,
          
          // 🚫 NON-MVP - Comment these out
          // crate::ai_analysis_handlers::analyze_document,
          // crate::credit_line_handlers::submit_credit_line_request,
          // crate::pool_handlers::get_enhanced_investor_metrics_handler,
      )
  )]
  pub struct ApiDoc;
  ```

### 3.2 Specific Endpoints to Comment Out

- [ ] **AI & Document Analysis** - Comment out in OpenAPI:
  - `POST /api/v1/documents/{id}/analyze`
  - `GET /api/v1/ai/chat`
  - `POST /api/v1/ai/chat`

- [ ] **Advanced Analytics** - Comment out in OpenAPI:
  - `GET /api/v1/analytics/*`
  - `GET /api/v1/metrics/*`

- [ ] **Credit Lines** - Comment out in OpenAPI:
  - `POST /api/v1/credit-lines/*`
  - `GET /api/v1/credit-lines/*`

- [ ] **Advanced Pool Features** - Comment out in OpenAPI:
  - `GET /api/v1/pools/{id}/metrics`
  - `GET /api/v1/pools/{id}/analytics`

### 3.3 Files to Modify for API Cleanup

- [ ] `api/src/openapi.rs` - Comment out non-MVP endpoints
- [ ] `api/src/router.rs` - Ensure routing still works but endpoints not documented
- [ ] Test that Swagger UI only shows MVP endpoints

## Phase 4: Environment Configuration (Week 3)

### 4.1 Add Environment Variables

- [ ] **Create environment config files**:
  - [ ] `.env.development`
  - [ ] `.env.staging`  
  - [ ] `.env.sandbox`
  - [ ] `.env.production`

- [ ] **Add environment detection in `api/src/config.rs`**:
  ```rust
  #[derive(Debug, Clone, Deserialize)]
  pub struct AppConfig {
      pub node_env: Environment,
      pub proof_type: ProofType,
      pub enable_contract_posting: bool,
      pub enable_sindri_proofs: bool,
      // ... other fields
  }
  
  impl AppConfig {
      pub fn should_post_to_contract(&self) -> bool {
          self.enable_contract_posting && 
          !matches!(self.node_env, Environment::Development)
      }
  }
  ```

### 4.2 Environment Variable Implementation

- [ ] **Core environment variables to add**:
  ```bash
  NODE_ENV=development|staging|sandbox|production
  PROOF_TYPE=core|groth16
  ENABLE_CONTRACT_POSTING=false|true
  ENABLE_SINDRI_PROOFS=false|true
  MVP_MODE=true|false
  ```

- [ ] **Update batch processing** to use environment config
- [ ] **Update contract posting** to check environment flags
- [ ] **Update proof generation** to use local vs Sindri based on environment

### 4.3 Files to Modify for Environment Config

- [ ] Create `api/src/config.rs` for centralized config management
- [ ] Update `api/src/lib.rs` to use new config system
- [ ] Update `api/src/unified_batch_service.rs` for conditional proof generation
- [ ] Update `Makefile` to support different environment configs

## Phase 5: Organization Structure (Week 4)

### 5.1 Add Organization Tables

- [ ] **Create organization migration**:
  ```sql
  -- File: db/migrations/009_add_organization_structure.sql
  CREATE TABLE organizations (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      name VARCHAR(255) NOT NULL,
      organization_type VARCHAR(20) NOT NULL CHECK (organization_type IN ('investment_fund', 'originator_firm')),
      contact_email VARCHAR(255),
      created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
      updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
  );
  
  ALTER TABLE identities ADD COLUMN organization_id UUID REFERENCES organizations(id);
  ALTER TABLE identities ADD COLUMN role_in_organization VARCHAR(20) 
      CHECK (role_in_organization IN ('admin', 'write', 'read'));
  ```

### 5.2 Organization API Endpoints

- [ ] **Add organization management endpoints**:
  - [ ] `POST /api/v1/organizations` - Create organization
  - [ ] `GET /api/v1/organizations/{id}` - Get organization details
  - [ ] `POST /api/v1/organizations/{id}/invite` - Invite user to organization
  - [ ] `GET /api/v1/organizations/{id}/members` - List organization members

- [ ] **Update user registration** to support organization membership
- [ ] **Update invite codes** to be organization-specific

### 5.3 Permission System Implementation

- [ ] **Add permission checking middleware**:
  ```rust
  // api/src/permissions.rs
  pub fn require_organization_role(required_role: OrganizationRole) -> impl Filter {
      // Implementation for role-based access control
  }
  ```

- [ ] **Update deal endpoints** to check organization permissions
- [ ] **Update marketplace endpoints** to respect organization boundaries

## Phase 6: Testing & Validation (Week 4-5)

### 6.1 Critical Path Testing

- [ ] **Create integration tests for MVP user journeys**:
  - [ ] User registration → KYC → Deal creation flow
  - [ ] Deal listing → Browse → Purchase → Transfer flow
  - [ ] USDC deposit → Investment → Withdrawal flow

- [ ] **Test files to create/update**:
  - [ ] `api/tests/integration_mvp_user_journey.rs`
  - [ ] `api/tests/integration_deal_trading_flow.rs`
  - [ ] `api/tests/integration_financial_operations.rs`

### 6.2 Performance Testing

- [ ] **Load test MVP endpoints**:
  ```bash
  # Test deal creation performance
  make cli ARGS="deal originate --originator-id ... --investor-id ... --amount 10000"
  
  # Test marketplace browsing
  make cli ARGS="deal browse-marketplace --limit 100"
  
  # Test batch processing under load
  make cli ARGS="trigger-batch --verbose"
  ```

### 6.3 ZK Proof Integration Testing

- [ ] **Verify all MVP transactions create deal_transactions records**
- [ ] **Test batch processing includes all transaction types**
- [ ] **Validate ZK proof generation for complete user journeys**
- [ ] **Test environment-based proof type switching**

## Phase 7: Documentation Updates (Week 5)

### 7.1 Update Core Documentation

- [ ] **Update `README.md`** with new consolidated structure
- [ ] **Update `AGENTS.md`** with latest schema and API changes  
- [ ] **Create `MVP_USER_GUIDE.md`** for external users
- [ ] **Update API documentation** examples with real MVP workflows

### 7.2 Developer Documentation

- [ ] **Update database schema documentation**
- [ ] **Document environment configuration options**
- [ ] **Create troubleshooting guide** for common MVP setup issues
- [ ] **Update deployment documentation** for new environment structure

## Implementation Commands Summary

### Quick Start Commands
```bash
# 1. Create database backup
pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql

# 2. Apply naming fixes migration  
echo "CREATE MIGRATION 069_fix_deal_status_naming.sql"

# 3. Test consolidated migrations
createdb arda_credit_test && DATABASE_URL=postgresql://localhost/arda_credit_test make initDB

# 4. Update OpenAPI documentation
# Edit api/src/openapi.rs manually

# 5. Add environment configuration
cp .env.example .env.development
# Configure each environment file

# 6. Run full test suite
make test

# 7. Deploy to staging
make up-dev  # Test locally first
```

### Verification Commands
```bash
# Verify database schema is clean
pg_dump --schema-only $DATABASE_URL | grep -i loan  # Should return minimal results

# Verify API documentation is MVP-focused
curl http://localhost:8080/swagger-ui/ | grep -c "credit-line"  # Should be 0

# Verify environment configuration works
NODE_ENV=development make run  # Should use local proofs
NODE_ENV=staging make run      # Should use Sindri proofs

# Verify MVP user journey works end-to-end
./test_mvp_user_journey.sh
```

## Success Criteria

### Phase 1 Complete ✅
- [ ] No `loan_status` references remain in codebase
- [ ] No stuttering field names (e.g., `deal_id` → `id`)
- [ ] All tests pass with new naming

### Phase 2 Complete ✅  
- [ ] Database has 8-10 focused migrations instead of 66+
- [ ] New database creation from scratch works perfectly
- [ ] All existing data migrates correctly

### Phase 3 Complete ✅
- [ ] Swagger UI shows only MVP endpoints (~25 endpoints vs current ~100+)
- [ ] All MVP functionality accessible and documented
- [ ] Advanced features remain in code but not exposed in docs

### Phase 4 Complete ✅
- [ ] Different environments use appropriate proof types
- [ ] Development skips contract posting and Sindri calls
- [ ] Production uses Groth16 proofs and mainnet contracts

### Phase 5 Complete ✅
- [ ] Organization structure supports multi-user companies
- [ ] Role-based permissions work for deal operations
- [ ] Invite codes are organization-specific

### Phase 6 Complete ✅
- [ ] All MVP user journeys tested end-to-end
- [ ] Performance meets requirements under realistic load
- [ ] ZK proof generation works in all environments

This checklist provides a clear roadmap for consolidating the codebase around the MVP while maintaining all existing functionality and preparing for future growth.
