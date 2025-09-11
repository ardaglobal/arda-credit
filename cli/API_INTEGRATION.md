# CLI API Integration Plan

This document outlines the comprehensive plan to integrate real API client calls into the Arda Credit CLI, replacing placeholder implementations with actual API interactions.

## 🔍 Current State Analysis

### API Client Architecture

The CLI uses two main client types:
- **`BatchApiClient`** (`api/src/client/mod.rs`) - HTTP API client for server operations
- **`EthereumClient`** (`ethereum-client/src/client.rs`) - Blockchain client for contract interactions

### Integration Status by Module

| Module | Total Functions | Real API Calls | Placeholder Implementations | Integration %|
|--------|----------------|----------------|----------------------------|-------------|
| `batch.rs` | 11 | ✅ **11** | 0 | **100%** |
| `balance.rs` | 13 | ❌ **0** | 13 | **0%** |
| `user.rs` | 10 | ❌ **1** | 9 | **10%** |
| `loans.rs` | 21 | ❌ **0** | 18 | **0%** |
| `pools.rs` | 9 | ❌ **0** | 9 | **0%** |
| `benchmarks.rs` | 17 | ❌ **0** | 8 | **0%** |
| **TOTAL** | **81** | **12** | **57** | **15%** |

## 🎯 Available API Methods

### BatchApiClient Methods (32 available)

#### Core Batch Operations
- `submit_transaction(amount: i32)` ✅ **IMPLEMENTED**
- `get_pending_transactions()` ✅ **IMPLEMENTED**
- `create_batch(batch_size: Option<i32>)` ✅ **IMPLEMENTED**
- `get_batches(limit: Option<i32>)` ✅ **IMPLEMENTED**
- `get_batch(batch_id: i32)` ✅ **IMPLEMENTED**
- `update_batch_proof(batch_id: i32, sindri_proof_id: String)` ✅ **IMPLEMENTED**
- `get_current_state()` ✅ **IMPLEMENTED**
- `get_contract_data(batch_id: i32)` ✅ **IMPLEMENTED**
- `health_check()` ✅ **IMPLEMENTED**
- `get_api_info()` ✅ **IMPLEMENTED**

#### User Management Operations
- `create_user(user_type: &str, email: &str, password: &str, jurisdiction: Option<&str>)` ❌ **NEEDS INTEGRATION**
- `get_user(user_id: &str)` ⚠️ **PARTIALLY IMPLEMENTED**
- `list_users(user_type: Option<&str>, kyc_status: Option<&str>, limit: Option<i32>, offset: Option<i32>)` ❌ **NEEDS INTEGRATION**
- `update_user(user_id: &str, email: Option<&str>, user_type: Option<&str>, jurisdiction: Option<&str>)` ❌ **NEEDS INTEGRATION**
- `delete_user(user_id: &str)` ❌ **NEEDS INTEGRATION**

#### KYC Operations
- `submit_kyc(user_id: &str, document_type: &str, document_hash: &str, jurisdiction: Option<&str>, kyc_investor_type: Option<&str>)` ❌ **NEEDS INTEGRATION**
- `update_kyc_status(user_id: &str, status: &str, notes: Option<&str>)` ❌ **NEEDS INTEGRATION**

#### Invite Code Operations
- `generate_invite_code()` ❌ **NEEDS INTEGRATION**
- `validate_invite_code(code: &str)` ❌ **NEEDS INTEGRATION**

#### Balance Operations
- `get_user_balance_by_address(address: &str)` ❌ **NEEDS INTEGRATION**
- `get_user_balance_by_id(user_id: &str)` ❌ **NEEDS INTEGRATION**
- `get_user_transactions(user_id: &str)` ❌ **NEEDS INTEGRATION**
- `get_transaction_status(tx_hash: &str)` ❌ **NEEDS INTEGRATION**
- `update_user_wallet(user_id: &str, wallet_address: &str)` ❌ **NEEDS INTEGRATION**

#### Investor Operations
- `register_investor(email, first_name, last_name, jurisdiction, accredited_investor, investment_capacity, regulatory_jurisdiction, invite_code)` ❌ **NEEDS INTEGRATION**
- `get_investor_profile(user_id: &str)` ❌ **NEEDS INTEGRATION**
- `update_investor_profile(user_id, accredited_investor, investment_capacity, regulatory_jurisdiction)` ❌ **NEEDS INTEGRATION**

#### Pool Operations
- `create_pool(name: &str, max_ltv_ratio: f64, max_interest_rate: f64, repayment_period_days: i32)` ❌ **NEEDS INTEGRATION**
- `list_pools(status: Option<&str>)` ❌ **NEEDS INTEGRATION**
- `get_pool(pool_id: &str)` ❌ **NEEDS INTEGRATION**
- `update_pool_status(pool_id: &str, status: &str)` ❌ **NEEDS INTEGRATION**
- `find_matching_pools(required_liquidity: f64, max_ltv: f64, max_rate: f64, min_repayment_days: i32)` ❌ **NEEDS INTEGRATION**

### EthereumClient Methods

#### Contract Operations
- `query_contract_verification_key()` ✅ **IMPLEMENTED**
- `monitor_deposits()` ❌ **NEEDS INTEGRATION**
- `monitor_withdrawals()` ❌ **NEEDS INTEGRATION**

## 📋 Integration Work Breakdown

### Phase 1: User Management Module (High Priority)
**File**: `cli/src/user.rs`
**Estimated Effort**: 4-6 hours
**Functions to Integrate**: 9

| Function | API Method | Complexity | Notes |
|----------|------------|------------|-------|
| `create_user()` | `client.create_user()` | Low | Direct mapping |
| `get_user()` | `client.get_user()` | Low | ⚠️ Partially done |
| `list_users()` | `client.list_users()` | Medium | Pagination handling |
| `update_user()` | `client.update_user()` | Low | Optional fields |
| `delete_user()` | `client.delete_user()` | Low | Confirmation handling |
| `submit_kyc()` | `client.submit_kyc()` | Medium | File handling |
| `update_kyc_status()` | `client.update_kyc_status()` | Low | Admin function |
| `generate_invite_code()` | `client.generate_invite_code()` | Low | Auth required |
| `check_invite_code()` | `client.validate_invite_code()` | Low | Direct mapping |

### Phase 2: Balance Management Module (High Priority)
**File**: `cli/src/balance.rs`
**Estimated Effort**: 3-4 hours
**Functions to Integrate**: 12

| Function | API Method | Complexity | Notes |
|----------|------------|------------|-------|
| `get_balance_by_address()` | `client.get_user_balance_by_address()` | Low | Address validation |
| `get_balance_by_id()` | `client.get_user_balance_by_id()` | Low | Direct mapping |
| `get_user_transactions()` | `client.get_user_transactions()` | Medium | Pagination |
| `get_transaction_status()` | `client.get_transaction_status()` | Low | Hash lookup |
| `link_wallet()` | `client.update_user_wallet()` | Medium | Address validation |
| `initiate_withdrawal()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_withdrawal_status()` | Custom HTTP call | Medium | Not in BatchApiClient |
| `get_available_balance()` | Custom HTTP call | Medium | Not in BatchApiClient |
| `get_withdrawal_history()` | Custom HTTP call | Medium | Not in BatchApiClient |
| `cancel_withdrawal()` | Custom HTTP call | Medium | Not in BatchApiClient |
| `get_pending_withdrawals()` | Custom HTTP call | Medium | Admin function |

### Phase 3: Pool Management Module (Medium Priority)
**File**: `cli/src/pools.rs`
**Estimated Effort**: 3-4 hours
**Functions to Integrate**: 9

| Function | API Method | Complexity | Notes |
|----------|------------|------------|-------|
| `create_pool()` | `client.create_pool()` | Low | Parameter validation |
| `list_pools()` | `client.list_pools()` | Low | Status filtering |
| `get_pool()` | `client.get_pool()` | Low | Direct mapping |
| `update_pool_status()` | `client.update_pool_status()` | Low | Status validation |
| `find_matching_pools()` | `client.find_matching_pools()` | Medium | Complex criteria |
| `get_pool_analytics()` | Custom HTTP call | High | Not in BatchApiClient |
| `fund_pool()` | Custom HTTP call | High | Not in BatchApiClient |
| `withdraw_from_pool()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_pool_performance()` | Custom HTTP call | High | Not in BatchApiClient |

### Phase 4: Loan Operations Module (Medium Priority)
**File**: `cli/src/loans.rs`
**Estimated Effort**: 6-8 hours
**Functions to Integrate**: 18

| Function | API Method | Complexity | Notes |
|----------|------------|------------|-------|
| `register_investor()` | `client.register_investor()` | Medium | Complex parameters |
| `get_investor_profile()` | `client.get_investor_profile()` | Low | Direct mapping |
| `update_investor_profile()` | `client.update_investor_profile()` | Medium | Optional fields |
| `register_merchant()` | Custom HTTP call | High | Not in BatchApiClient |
| `submit_turnover()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_turnover()` | Custom HTTP call | High | Not in BatchApiClient |
| `update_turnover()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_turnover_analytics()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_profile_completion()` | Custom HTTP call | High | Not in BatchApiClient |
| `check_credit_eligibility()` | Custom HTTP call | High | Not in BatchApiClient |
| `submit_credit_line_request()` | Custom HTTP call | High | Not in BatchApiClient |
| `browse_available_pools()` | Custom HTTP call | High | Not in BatchApiClient |
| `list_credit_line_requests()` | Custom HTTP call | High | Not in BatchApiClient |
| `originate_loan()` | Custom HTTP call | High | Not in BatchApiClient |
| `make_payment()` | Custom HTTP call | High | Not in BatchApiClient |
| `closeout_loan()` | Custom HTTP call | High | Not in BatchApiClient |
| `list_loans()` | Custom HTTP call | High | Not in BatchApiClient |
| `get_loan()` | Custom HTTP call | High | Not in BatchApiClient |

### Phase 5: Benchmark Module (Low Priority)
**File**: `cli/src/benchmarks.rs`
**Estimated Effort**: 2-3 hours
**Functions to Integrate**: 8

| Function | API Method | Complexity | Notes |
|----------|------------|------------|-------|
| `test_prefixed_nullifiers()` | Local computation | Low | No API needed |
| `verify_loan_payment()` | Multiple API calls | High | Complex verification |
| `verify_loan_origination()` | Multiple API calls | High | Complex verification |
| `verify_loan_operation()` | Multiple API calls | High | Complex verification |
| `test_loan_performance()` | Multiple API calls | High | Performance testing |
| `query_loan_status()` | Custom HTTP call | High | Not in BatchApiClient |
| `query_user_loans()` | Custom HTTP call | High | Not in BatchApiClient |
| `generate_test_data()` | Multiple API calls | High | Bulk operations |

## 🚀 Parallel Implementation Strategy

### Work Package 1: User Management Integration
**Assignable to Agent 1**
**Files**: `cli/src/user.rs`
**Dependencies**: None
**Estimated Time**: 4-6 hours

**Tasks**:
1. Replace `create_user()` placeholder with `client.create_user()` call
2. Complete `get_user()` implementation (already started)
3. Implement `list_users()` with pagination support
4. Implement `update_user()` with optional field handling
5. Implement `delete_user()` with confirmation logic
6. Implement `submit_kyc()` with file validation
7. Implement `update_kyc_status()` with admin checks
8. Implement `generate_invite_code()` with auth handling
9. Complete `check_invite_code()` implementation

**Acceptance Criteria**:
- All functions use real API calls
- Error handling matches API client error types
- CLI output shows real data from API responses
- No unused client parameters

### Work Package 2: Balance Management Integration
**Assignable to Agent 2**
**Files**: `cli/src/balance.rs`
**Dependencies**: None
**Estimated Time**: 3-4 hours

**Tasks**:
1. Replace `get_balance_by_address()` with `client.get_user_balance_by_address()`
2. Replace `get_balance_by_id()` with `client.get_user_balance_by_id()`
3. Implement `get_user_transactions()` with `client.get_user_transactions()`
4. Implement `get_transaction_status()` with `client.get_transaction_status()`
5. Implement `link_wallet()` with `client.update_user_wallet()`
6. Create custom HTTP calls for withdrawal operations (not in BatchApiClient)
7. Add proper Ethereum address validation throughout
8. Implement error handling for 404/400 responses

**Special Notes**:
- Withdrawal operations require custom HTTP calls to withdrawal endpoints
- Need to integrate with `ethereum-client` for address validation

### Work Package 3: Pool Management Integration
**Assignable to Agent 3**
**Files**: `cli/src/pools.rs`
**Dependencies**: None
**Estimated Time**: 3-4 hours

**Tasks**:
1. Replace `create_pool()` with `client.create_pool()`
2. Replace `list_pools()` with `client.list_pools()`
3. Replace `get_pool()` with `client.get_pool()`
4. Replace `update_pool_status()` with `client.update_pool_status()`
5. Replace `find_matching_pools()` with `client.find_matching_pools()`
6. Create custom HTTP calls for analytics operations
7. Add parameter validation for pool criteria
8. Implement proper error handling and user feedback

### Work Package 4: Loan Operations Integration (Part 1)
**Assignable to Agent 4**
**Files**: `cli/src/loans.rs` (Investor functions)
**Dependencies**: None
**Estimated Time**: 3-4 hours

**Tasks**:
1. Replace `register_investor()` with `client.register_investor()`
2. Replace `get_investor_profile()` with `client.get_investor_profile()`
3. Replace `update_investor_profile()` with `client.update_investor_profile()`
4. Add investment capacity parsing and validation
5. Handle regulatory jurisdiction logic
6. Implement proper error handling for investor operations

### Work Package 5: Loan Operations Integration (Part 2)
**Assignable to Agent 5**
**Files**: `cli/src/loans.rs` (Merchant/Credit functions)
**Dependencies**: Work Package 4 completion
**Estimated Time**: 4-6 hours

**Tasks**:
1. Create custom HTTP calls for merchant operations (not in BatchApiClient)
2. Implement turnover data submission and retrieval
3. Implement credit line eligibility and request management
4. Implement loan origination, payment, and closeout operations
5. Add business logic validation
6. Create comprehensive error handling for credit operations

### Work Package 6: Benchmark Integration
**Assignable to Agent 6**
**Files**: `cli/src/benchmarks.rs`
**Dependencies**: Packages 1-5 completion
**Estimated Time**: 2-3 hours

**Tasks**:
1. Integrate performance tests with real API calls
2. Implement loan operation verification with real batch data
3. Create comprehensive performance metrics collection
4. Add stress testing with actual API load
5. Implement test data generation using real API calls

## 🔧 Implementation Guidelines

### Error Handling Pattern

```rust
match client.api_method().await {
    Ok(response) => {
        println!("✅ Operation successful!");
        // Handle response data
        if verbose {
            println!("   Details: {:#?}", response);
        }
    }
    Err(e) => {
        let error_msg = e.to_string();
        if error_msg.contains("404") {
            println!("❌ Resource not found");
        } else if error_msg.contains("400") {
            println!("❌ Invalid request parameters");
        } else if error_msg.contains("401") || error_msg.contains("403") {
            println!("❌ Authentication required");
        } else {
            eprintln!("❌ API error: {}", e);
            return Err(e.into());
        }
    }
}
```

### Custom HTTP Call Pattern (for endpoints not in BatchApiClient)

```rust
use reqwest::Client;
use serde_json::json;

let http_client = Client::new();
let api_url = env::var("ARDA_CREDIT_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
let url = format!("{}/api/v1/custom-endpoint", api_url);

let request_body = json!({
    "field1": value1,
    "field2": value2
});

match http_client.post(&url).json(&request_body).send().await {
    Ok(response) => {
        if response.status().is_success() {
            let result: serde_json::Value = response.json().await?;
            // Handle success
        } else {
            let error_text = response.text().await?;
            // Handle error
        }
    }
    Err(e) => {
        return Err(eyre::eyre!("Request failed: {}", e));
    }
}
```

### Ethereum Client Integration Pattern

```rust
use ethereum_client::{Config, EthereumClient};

let config = Config::from_env().map_err(|e| {
    eyre::eyre!("Failed to load Ethereum config: {}", e)
})?;

let eth_client = EthereumClient::new(config).await.map_err(|e| {
    eyre::eyre!("Failed to create Ethereum client: {}", e)
})?;

// Use eth_client for blockchain operations
```

## 🔄 Missing API Endpoints

The following CLI functions require API endpoints that don't exist in BatchApiClient:

### Withdrawal Operations (5 endpoints needed)
- `POST /api/v1/withdrawals` - Initiate withdrawal
- `GET /api/v1/withdrawals/{id}` - Get withdrawal status
- `GET /api/v1/users/{address}/available-balance` - Get available balance
- `GET /api/v1/users/{address}/withdrawals` - Get withdrawal history
- `POST /api/v1/withdrawals/{id}/cancel` - Cancel withdrawal

### Merchant Operations (8 endpoints needed)
- `POST /api/v1/auth/register/merchant` - Register merchant
- `POST /api/merchant/turnover` - Submit turnover data
- `GET /api/merchant/turnover` - Get turnover data
- `PUT /api/merchant/turnover/{date}` - Update turnover
- `GET /api/merchant/turnover/analytics` - Get analytics
- `GET /api/merchant/profile/completion` - Get profile completion

### Credit Line Operations (6 endpoints needed)
- `GET /api/credit-lines/eligibility/{merchant_id}` - Check eligibility
- `POST /api/credit-lines/request` - Submit credit request
- `GET /api/credit-lines/browse-pools` - Browse pools
- `GET /api/credit-lines/requests` - List requests
- `POST /api/loans/originate` - Originate loan
- `POST /api/loans/{id}/payment` - Make payment

### Loan Management (4 endpoints needed)
- `GET /api/loans/{id}` - Get loan details
- `GET /api/loans` - List loans
- `POST /api/loans/{id}/closeout` - Closeout loan
- `GET /api/loans/stats` - Get loan statistics

## 📝 Implementation Steps for Each Agent

### Pre-Implementation Checklist
1. Review the specific API client method signatures in `api/src/client/mod.rs`
2. Identify error types returned by the API client
3. Check if custom HTTP calls are needed (for missing endpoints)
4. Plan response data structure handling
5. Design user-friendly error messages

### Implementation Process
1. **Replace placeholder comment** with actual API call
2. **Handle API response** with proper error checking
3. **Update CLI output** to use real response data
4. **Add verbose mode** details from API response
5. **Test error scenarios** (404, 400, 401, etc.)
6. **Remove unused client parameter warnings**

### Testing Strategy
1. **Unit tests** for each function with mock API responses
2. **Integration tests** with real API server (when available)
3. **Error handling tests** for various failure scenarios
4. **CLI argument validation tests**

## 🎯 Success Metrics

### Completion Criteria
- [ ] All placeholder implementations replaced with real API calls
- [ ] Zero unused client parameter warnings
- [ ] Comprehensive error handling for all API scenarios
- [ ] CLI outputs real data from API responses
- [ ] All functions handle verbose mode appropriately

### Performance Targets
- [ ] CLI startup time < 100ms
- [ ] API call response time displayed to user
- [ ] Graceful handling of network timeouts
- [ ] Proper connection retry logic

## 🔍 Quality Assurance

### Code Review Checklist
- [ ] Function uses appropriate API client method
- [ ] Error handling covers all expected scenarios
- [ ] CLI output is user-friendly and informative
- [ ] Verbose mode provides additional useful details
- [ ] Function documentation is accurate and complete
- [ ] No hardcoded values or placeholder data

### Testing Requirements
- [ ] Function works with real API server
- [ ] Function handles API server downtime gracefully
- [ ] Error messages guide user toward resolution
- [ ] Verbose output provides debugging information
- [ ] Function respects CLI-wide verbose flag

---

## 📞 Agent Assignment Template

When assigning work to parallel agents, use this template:

```
Agent X: Implement API integration for [MODULE_NAME]

Files to modify: cli/src/[module].rs
Dependencies: [list any dependencies]
Estimated time: [X-Y hours]

Tasks:
1. [Specific function 1]
2. [Specific function 2]
...

Success criteria:
- All placeholder implementations replaced
- Zero unused client warnings for this module
- Comprehensive error handling
- Real API data in CLI output

Reference files:
- cli/src/batch.rs (working example)
- api/src/client/mod.rs (available API methods)
- This API_INTEGRATION.md (implementation patterns)
```
