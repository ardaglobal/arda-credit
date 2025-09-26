# MVP API Endpoints Strategy

## Overview

This document outlines the API endpoint strategy for the MVP secondary trading platform. The goal is to focus the OpenAPI documentation on critical path features while keeping non-MVP endpoints in the codebase but commented out from public documentation.

## MVP Core Feature Set

### Primary User Journeys
1. **User Onboarding**: Registration → KYC → Profile Setup
2. **Deal Trading**: Browse → Purchase → Manage
3. **Portfolio Management**: View holdings → List for sale → Transfer ownership
4. **Financial Operations**: Deposit USDC → Withdraw proceeds

## MVP Endpoint Classification

### ✅ MVP Critical (Keep in OpenAPI)

#### Authentication & Session Management
```yaml
POST /api/v1/auth/request-magic-link
  description: "Request passwordless login link via email"
  required: true
  complexity: low
  
POST /api/v1/auth/verify-magic-link  
  description: "Verify magic link token and create session"
  required: true
  complexity: low
  
GET /api/v1/auth/me
  description: "Get current authenticated user details"
  required: true
  complexity: low
  
POST /api/v1/auth/logout
  description: "End user session"
  required: true  
  complexity: low
```

#### User Management Core
```yaml
POST /api/v1/users/register
  description: "Register new user account with invite code"
  required: true
  complexity: medium
  
GET /api/v1/users/{id}
  description: "Get user profile and KYC status"
  required: true
  complexity: low
  
PUT /api/v1/users/{id}  
  description: "Update user profile information"
  required: true
  complexity: low
  
POST /api/v1/users/{id}/kyc
  description: "Submit KYC documentation"
  required: true
  complexity: medium
```

#### Deal CRUD Operations  
```yaml
POST /api/v1/deals
  description: "Create new deal (originators only)"
  required: true
  complexity: high
  notes: "Core business logic - must be bulletproof"
  
GET /api/v1/deals
  description: "List deals with filtering/pagination"
  required: true
  complexity: medium
  parameters:
    - status: [active, for_sale, closed]
    - investor_id: UUID
    - originator_id: UUID
    - limit: integer
    - offset: integer
  
GET /api/v1/deals/{id}
  description: "Get detailed deal information"
  required: true
  complexity: medium
  
PUT /api/v1/deals/{id}
  description: "Update deal details (owner only)"
  required: true
  complexity: medium
  
DELETE /api/v1/deals/{id}
  description: "Delete deal (if no payments made)"
  required: true
  complexity: medium
```

#### Deal Payments
```yaml  
POST /api/v1/deals/{id}/payments
  description: "Record payment against deal"
  required: true
  complexity: high
  notes: "Critical - affects deal balance and status"
  
GET /api/v1/deals/{id}/payments
  description: "Get payment history for deal"
  required: true
  complexity: low
  
GET /api/v1/payments/{id}
  description: "Get specific payment details"
  required: true
  complexity: low
```

#### Secondary Market Trading
```yaml
GET /api/v1/deals/for-sale
  description: "Browse deals available for purchase"
  required: true
  complexity: medium
  notes: "Core marketplace functionality"
  
POST /api/v1/deals/{id}/list-for-sale
  description: "List deal for sale in marketplace"
  required: true
  complexity: medium
  
DELETE /api/v1/deals/{id}/remove-from-sale  
  description: "Remove deal from marketplace"
  required: true
  complexity: low
  
GET /api/v1/deals/marketplace/stats
  description: "Basic marketplace statistics"
  required: true
  complexity: low
```

#### Deal Ownership Transfers
```yaml
POST /api/v1/deals/{id}/transfer
  description: "Transfer deal ownership to another investor"
  required: true
  complexity: high
  notes: "Critical business logic - handle with care"
  
GET /api/v1/deals/{id}/transfer-history
  description: "View ownership transfer history for deal"
  required: true
  complexity: low
  
POST /api/v1/deals/{id}/transfer/validate
  description: "Validate if transfer is possible"
  required: true
  complexity: medium
```

#### Financial Operations
```yaml
GET /api/v1/balances/address/{address}
  description: "Get USDC balance for wallet address"
  required: true
  complexity: low
  
POST /api/v1/withdrawals/initiate
  description: "Request USDC withdrawal"
  required: true
  complexity: medium
  
GET /api/v1/withdrawals/{id}/status
  description: "Check withdrawal request status"
  required: true
  complexity: low
```

#### System Operations  
```yaml
GET /api/v1/health
  description: "Service health check"
  required: true
  complexity: low
  
GET /api/v1/batches
  description: "List ZK proof batches"
  required: true
  complexity: low
  
POST /api/v1/batches/trigger
  description: "Manually trigger batch creation"
  required: true
  complexity: medium
```

### 🚫 Non-MVP (Comment Out from OpenAPI)

#### Advanced Analytics
```yaml
# COMMENT OUT - Not MVP critical
GET /api/v1/analytics/platform-metrics
GET /api/v1/analytics/user-activity  
GET /api/v1/analytics/deal-performance
GET /api/v1/metrics/prometheus
GET /api/v1/metrics/custom
```

#### AI & Document Analysis
```yaml  
# COMMENT OUT - Advanced feature
POST /api/v1/documents/{id}/analyze
GET /api/v1/ai/chat
POST /api/v1/ai/chat
GET /api/v1/ai/analytics/dashboard
POST /api/v1/ai/notifications/configure
```

#### Credit Line Management
```yaml
# COMMENT OUT - Separate product feature
POST /api/v1/credit-lines/request
GET /api/v1/credit-lines
PUT /api/v1/credit-lines/{id}/status
GET /api/v1/credit-lines/{id}/utilization
POST /api/v1/credit-lines/{id}/drawdown
```

#### Advanced Pool Operations
```yaml
# COMMENT OUT - Basic pool creation may stay, advanced features out
GET /api/v1/pools/{id}/metrics
GET /api/v1/pools/{id}/performance-analytics
POST /api/v1/pools/{id}/borough-limits
GET /api/v1/pools/{id}/enhanced-metrics
GET /api/v1/pools/{id}/roi-projections
```

#### Merchant Portal
```yaml
# COMMENT OUT - Different use case entirely  
POST /api/v1/merchants/register
GET /api/v1/merchants/{id}/profile
POST /api/v1/merchants/{id}/turnover
GET /api/v1/merchants/{id}/completion-score
```

#### Advanced Financial Transactions
```yaml
# COMMENT OUT - Basic deposit/withdrawal stays, advanced tracking out
GET /api/v1/financial-transactions/account/{id}
POST /api/v1/financial-transactions/account
GET /api/v1/financial-transactions/pool/{id}
GET /api/v1/financial-transactions/credit-line/{id}
GET /api/v1/users/{id}/financial-summary
```

## Implementation Strategy

### OpenAPI Conditional Compilation

#### Option 1: Feature Flags (Recommended)
```rust
// api/src/openapi.rs
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        // Always include MVP critical paths
        crate::authentication_handlers::request_magic_link,
        crate::deal_handlers::create_deal_endpoint,
        crate::deal_handlers::list_deals_endpoint,
        crate::deal_handlers::browse_deals_for_sale_endpoint,
        
        // Conditionally include advanced features
        #[cfg(feature = "advanced-analytics")]
        crate::analytics_handlers::get_platform_metrics,
        
        #[cfg(feature = "ai-features")]
        crate::ai_analysis_handlers::analyze_document,
        
        #[cfg(feature = "credit-lines")]
        crate::credit_line_handlers::submit_credit_line_request,
        
        #[cfg(feature = "advanced-pools")]
        crate::pool_handlers::get_enhanced_investor_metrics_handler,
    ),
    components(schemas(
        // MVP schemas always included
        crate::deal_handlers::CreateDealRequest,
        crate::deal_handlers::DealDetailsResponse,
        
        // Advanced schemas conditionally included
        #[cfg(feature = "advanced-analytics")]
        crate::analytics_handlers::PlatformMetricsResponse,
    ))
)]
pub struct ApiDoc;
```

#### Option 2: Environment-Based Rendering
```rust
// api/src/openapi.rs
pub fn create_openapi_spec(include_advanced: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = ApiDoc::openapi();
    
    if !include_advanced {
        // Remove non-MVP paths
        openapi.paths.paths.retain(|path, _| {
            !path.contains("/analytics/") &&
            !path.contains("/ai/") && 
            !path.contains("/credit-lines/") &&
            !path.contains("/merchants/")
        });
    }
    
    openapi
}

// In router setup
let include_advanced = std::env::var("INCLUDE_ADVANCED_ENDPOINTS")
    .map(|v| v == "true")
    .unwrap_or(false);

let spec = create_openapi_spec(include_advanced);
```

#### Option 3: Comment-Based Approach (Simplest)
```rust
// api/src/openapi.rs
#[derive(OpenApi)]  
#[openapi(
    paths(
        // ✅ MVP CRITICAL - Authentication
        crate::authentication_handlers::request_magic_link,
        crate::authentication_handlers::verify_magic_link,
        
        // ✅ MVP CRITICAL - Deal Management  
        crate::deal_handlers::create_deal_endpoint,
        crate::deal_handlers::list_deals_endpoint,
        crate::deal_handlers::browse_deals_for_sale_endpoint,
        
        // 🚫 NON-MVP - Advanced Analytics (commented out for MVP)
        // crate::analytics_handlers::get_platform_metrics,
        // crate::analytics_handlers::get_user_activity_metrics,
        
        // 🚫 NON-MVP - AI Features (commented out for MVP)  
        // crate::ai_analysis_handlers::analyze_document,
        // crate::ai_analysis_handlers::ai_chat_endpoint,
        
        // 🚫 NON-MVP - Credit Lines (commented out for MVP)
        // crate::credit_line_handlers::submit_credit_line_request,
        // crate::credit_line_handlers::list_credit_line_requests,
    )
)]
pub struct ApiDoc;
```

### Cargo Feature Configuration

Add to `Cargo.toml`:
```toml
[features]
default = ["mvp-only"]
mvp-only = []
advanced-analytics = []
ai-features = []  
credit-lines = []
advanced-pools = []
full-feature-set = ["advanced-analytics", "ai-features", "credit-lines", "advanced-pools"]
```

### Build Commands
```bash
# MVP build (default)
cargo build --release

# Full feature build
cargo build --release --features full-feature-set

# Development with specific features
cargo run --features "advanced-analytics,ai-features"
```

## Endpoint Documentation Standards

### MVP Endpoint Requirements
Each MVP endpoint must include:

1. **Clear Purpose**: One-line description of business function
2. **Request/Response Examples**: Real-world JSON examples
3. **Error Scenarios**: All possible error responses documented
4. **Authentication Requirements**: Which user types can access
5. **Rate Limiting**: If applicable
6. **Side Effects**: What state changes occur

### Example MVP Endpoint Documentation
```yaml
/api/v1/deals/{id}/transfer:
  post:
    summary: "Transfer deal ownership to another investor"
    description: |
      Transfers ownership of a deal from the current investor to a different investor.
      This operation:
      - Validates the transfer is legally permissible
      - Updates deal ownership records
      - Creates a deal_transaction record for ZK proof inclusion
      - Sends notifications to both parties
      
      ⚠️  CRITICAL: This endpoint modifies core business state and must be called
      within a database transaction to ensure data consistency.
      
    parameters:
      - name: id
        in: path
        required: true
        schema:
          type: string
          format: uuid
        example: "123e4567-e89b-12d3-a456-426614174000"
        
    requestBody:
      required: true
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/TransferDealOwnershipRequest'
          examples:
            standard_transfer:
              summary: "Standard ownership transfer"
              value:
                from_investor_id: "123e4567-e89b-12d3-a456-426614174000"
                to_investor_id: "987fcdeb-51d2-4321-b567-426614174999"
                transfer_price: 47500.00
                notes: "Strategic portfolio rebalancing"
                
    responses:
      200:
        description: "Transfer completed successfully"
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/DealTransferResponse'
      400:
        description: "Invalid transfer request"
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ErrorResponse'
            examples:
              invalid_ownership:
                summary: "User doesn't own deal"
                value:
                  error: "INVALID_OWNERSHIP"
                  message: "User is not the current investor for this deal"
      403:
        description: "Transfer not permitted"
        examples:
          kyc_required:
            summary: "KYC verification required"  
            value:
              error: "KYC_REQUIRED"
              message: "Both parties must have approved KYC status"
      404:
        description: "Deal not found"
      422:
        description: "Deal not transferable (e.g., closed, defaulted)"
        
    security:
      - BearerAuth: []
    tags:
      - Deal Trading
```

## Testing Strategy for MVP Endpoints

### Test Coverage Requirements
- **Unit Tests**: 90%+ coverage for MVP endpoints
- **Integration Tests**: End-to-end user journey tests
- **Load Tests**: Performance under realistic load
- **Security Tests**: Authorization and input validation

### Critical Path Testing
1. **User Registration → KYC → Deal Creation**: Full flow test
2. **Deal Listing → Browse → Purchase → Transfer**: Trading flow test  
3. **USDC Deposit → Deal Investment → Withdrawal**: Financial flow test
4. **ZK Proof Generation**: State transition completeness test

### Test Data Strategy
```yaml
# MVP Test Scenarios
test_users:
  - investor_kyc_approved: "Has funds, can trade"
  - investor_pending_kyc: "Limited functionality"  
  - originator_verified: "Can create deals"
  - admin_user: "Full platform access"

test_deals:
  - active_deal: "Available for trading"
  - for_sale_deal: "Listed in marketplace"
  - paid_off_deal: "Completed transaction"
  - defaulted_deal: "Edge case handling"
```

## Deployment Configuration

### Environment-Specific OpenAPI
```bash
# Development - Full API documentation
INCLUDE_ADVANCED_ENDPOINTS=true

# Staging - MVP focus but some advanced for testing  
INCLUDE_ADVANCED_ENDPOINTS=false
INCLUDE_ANALYTICS_ENDPOINTS=true  

# Production MVP - Only critical endpoints
INCLUDE_ADVANCED_ENDPOINTS=false
INCLUDE_ANALYTICS_ENDPOINTS=false
```

### Monitoring & Observability
Track MVP endpoint usage:
```rust
// Key metrics for MVP endpoints
- Request volume per endpoint
- Response time percentiles  
- Error rates by endpoint
- User journey completion rates
- ZK proof generation success rates
```

## Migration Plan

### Phase 1: Comment Out Non-MVP (Week 1)
1. Identify all non-MVP endpoints in current OpenAPI
2. Comment out from OpenAPI spec (keep code intact)
3. Update documentation to reflect MVP scope
4. Test MVP functionality completeness

### Phase 2: Enhance MVP Documentation (Week 2)  
1. Add detailed examples for all MVP endpoints
2. Document error scenarios comprehensively
3. Create integration test suite for MVP flows
4. Performance test MVP critical path

### Phase 3: Validation & Launch (Week 3)
1. External review of MVP API documentation
2. User acceptance testing with limited beta users
3. Load testing under realistic conditions
4. Final security review of exposed endpoints

This approach ensures the MVP API documentation is focused and professional while maintaining the codebase's flexibility for future feature releases.
