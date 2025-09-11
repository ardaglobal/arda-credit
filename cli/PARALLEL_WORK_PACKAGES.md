# Parallel API Integration Work Packages

This document provides discrete, independent work packages for parallel agents to implement API integration across the CLI modules.

## 📦 Work Package 1: User Management API Integration

**Agent Assignment**: Agent 1
**Priority**: 🔴 **HIGH** (Core user functionality)
**Estimated Time**: 4-6 hours
**Files to Modify**: `cli/src/user.rs`
**Dependencies**: None (fully independent)

### Scope
Replace 9 placeholder implementations with real `BatchApiClient` calls in the user management module.

### Specific Tasks

1. **`create_user()` function** (Lines ~345-375)
   - Replace placeholder with `client.create_user()` call
   - Handle API response and display user creation details
   - Add proper error handling for email conflicts

2. **`list_users()` function** (Lines ~420-460)
   - Replace placeholder with `client.list_users()` call
   - Implement pagination display
   - Handle filtering by user_type and kyc_status

3. **`update_user()` function** (Lines ~460-510)
   - Replace placeholder with `client.update_user()` call
   - Handle optional field updates
   - Display changed fields in response

4. **`delete_user()` function** (Lines ~510-550)
   - Replace placeholder with `client.delete_user()` call
   - Maintain confirmation logic
   - Handle deletion response

5. **`submit_kyc()` function** (Lines ~550-605)
   - Replace placeholder with `client.submit_kyc()` call
   - Add document hash validation
   - Handle KYC submission response

6. **`update_kyc_status()` function** (Lines ~605-650)
   - Replace placeholder with `client.update_kyc_status()` call
   - Add admin permission context
   - Handle status update response

7. **`generate_invite_code()` function** (Lines ~650-700)
   - Replace placeholder with `client.generate_invite_code()` call
   - Handle authentication requirements
   - Display generated code details

8. **`check_invite_code()` function** (Lines ~700-730)
   - Replace placeholder with `client.validate_invite_code()` call
   - Display code validity and usage information

9. **`use_invite_code()` function** (Lines ~730-770)
   - Update deprecation message
   - Point users to registration flow

### Success Criteria
- ✅ All functions use real API calls (no placeholders)
- ✅ Zero unused `client` parameter warnings in user.rs
- ✅ Comprehensive error handling (404, 400, 401, 403)
- ✅ CLI displays real API response data
- ✅ Verbose mode shows additional API details

---

## 📦 Work Package 2: Balance Management API Integration

**Agent Assignment**: Agent 2
**Priority**: 🔴 **HIGH** (Critical financial operations)
**Estimated Time**: 3-4 hours
**Files to Modify**: `cli/src/balance.rs`
**Dependencies**: None (fully independent)

### Scope
Replace 12 placeholder implementations with real API calls in the balance management module.

### Specific Tasks

1. **`get_balance_by_address()` function** (Lines ~265-310)
   - Replace placeholder with `client.get_user_balance_by_address()` call
   - Display real balance data from API
   - Handle address not found scenarios

2. **`get_balance_by_id()` function** (Lines ~310-355)
   - Replace placeholder with `client.get_user_balance_by_id()` call
   - Display user balance information
   - Handle user not found scenarios

3. **`get_user_transactions()` function** (Lines ~355-400)
   - Replace placeholder with `client.get_user_transactions()` call
   - Display transaction history
   - Handle pagination if needed

4. **`get_transaction_status()` function** (Lines ~400-450)
   - Replace placeholder with `client.get_transaction_status()` call
   - Display transaction status details
   - Handle transaction not found

5. **`link_wallet()` function** (Lines ~450-520)
   - Replace placeholder with `client.update_user_wallet()` call
   - Maintain Ethereum address validation
   - Display wallet linking confirmation

6. **Withdrawal Operations** (Lines ~520-850) - **Custom HTTP Required**
   - `initiate_withdrawal()` - Create custom HTTP call to `/api/v1/withdrawals`
   - `get_withdrawal_status()` - Custom HTTP call to `/api/v1/withdrawals/{id}`
   - `get_available_balance()` - Custom HTTP call to `/api/v1/users/{address}/available-balance`
   - `get_withdrawal_history()` - Custom HTTP call to `/api/v1/users/{address}/withdrawals`
   - `cancel_withdrawal()` - Custom HTTP call to `/api/v1/withdrawals/{id}/cancel`
   - `get_pending_withdrawals()` - Custom HTTP call to `/api/v1/admin/withdrawals/pending`

### Special Requirements
- **Ethereum Address Validation**: Use existing `validate_ethereum_address()` helper
- **Custom HTTP Calls**: Use reqwest directly for withdrawal endpoints
- **Amount Parsing**: Handle USDC decimal amounts properly
- **Admin Functions**: Handle authentication requirements for admin endpoints

### Success Criteria
- ✅ All balance queries use real API data
- ✅ Withdrawal operations work with custom HTTP calls
- ✅ Proper Ethereum address validation throughout
- ✅ Zero unused `client` parameter warnings in balance.rs
- ✅ Real balance amounts displayed from API

---

## 📦 Work Package 3: Pool Management API Integration

**Agent Assignment**: Agent 3
**Priority**: 🟡 **MEDIUM** (Pool operations)
**Estimated Time**: 3-4 hours
**Files to Modify**: `cli/src/pools.rs`
**Dependencies**: None (fully independent)

### Scope
Replace 9 placeholder implementations with real API calls in the pool management module.

### Specific Tasks

1. **`create_pool()` function** (Lines ~235-270)
   - Replace placeholder with `client.create_pool()` call
   - Validate pool parameters (LTV, interest rate, etc.)
   - Display created pool details

2. **`list_pools()` function** (Lines ~315-360)
   - Replace placeholder with `client.list_pools()` call
   - Handle status filtering
   - Display pool summary information

3. **`get_pool()` function** (Lines ~380-430)
   - Replace placeholder with `client.get_pool()` call
   - Display detailed pool information
   - Handle pool not found scenarios

4. **`update_pool_status()` function** (Lines ~430-480)
   - Replace placeholder with `client.update_pool_status()` call
   - Validate status values (ACTIVE, PAUSED, CLOSED)
   - Display status update confirmation

5. **`find_matching_pools()` function** (Lines ~480-560)
   - Replace placeholder with `client.find_matching_pools()` call
   - Display matching criteria and results
   - Handle no matches found

6. **Pool Analytics Functions** (Lines ~560-850) - **Custom HTTP Required**
   - `get_pool_analytics()` - Custom HTTP call for analytics
   - `fund_pool()` - Custom HTTP call for funding
   - `withdraw_from_pool()` - Custom HTTP call for withdrawals
   - `get_pool_performance()` - Custom HTTP call for performance metrics

### Success Criteria
- ✅ All pool CRUD operations use real API
- ✅ Pool matching algorithm uses real data
- ✅ Analytics functions work with custom HTTP calls
- ✅ Zero unused `client` parameter warnings in pools.rs
- ✅ Proper parameter validation throughout

---

## 📦 Work Package 4: Investor Operations API Integration

**Agent Assignment**: Agent 4
**Priority**: 🟡 **MEDIUM** (Investor functionality)
**Estimated Time**: 3-4 hours
**Files to Modify**: `cli/src/loans.rs` (Investor functions only)
**Dependencies**: None (fully independent)

### Scope
Replace investor-related placeholder implementations with real API calls (3 functions).

### Specific Tasks

1. **`register_investor()` function** (Lines ~610-675)
   - Replace placeholder with `client.register_investor()` call
   - Handle investment capacity parsing (string to float)
   - Display investor registration confirmation
   - Handle invite code validation

2. **`get_investor_profile()` function** (Lines ~680-715)
   - Replace placeholder with `client.get_investor_profile()` call
   - Display real investor profile data
   - Handle profile not found scenarios

3. **`update_investor_profile()` function** (Lines ~720-760)
   - Replace placeholder with `client.update_investor_profile()` call
   - Handle optional field updates
   - Display profile update confirmation

### Special Requirements
- **Investment Capacity**: Parse string amounts to float values
- **Regulatory Compliance**: Handle jurisdiction requirements
- **Accreditation Status**: Validate boolean accreditation flag

### Success Criteria
- ✅ Investor registration works with real API
- ✅ Profile operations use real data
- ✅ Investment capacity parsing works correctly
- ✅ Zero unused `client` warnings for investor functions

---

## 📦 Work Package 5: Merchant & Credit Operations API Integration

**Agent Assignment**: Agent 5
**Priority**: 🟡 **MEDIUM** (Merchant functionality)
**Estimated Time**: 6-8 hours
**Files to Modify**: `cli/src/loans.rs` (Merchant/Credit functions)
**Dependencies**: None (fully independent)

### Scope
Implement custom HTTP calls for merchant and credit operations (15 functions) - **Most require custom endpoints**.

### Specific Tasks

1. **Merchant Registration** (Lines ~765-830)
   - Create custom HTTP call to `/api/v1/auth/register/merchant`
   - Handle business information validation
   - Display merchant registration confirmation

2. **Turnover Operations** (Lines ~830-980)
   - `submit_turnover()` - Custom HTTP to `/api/merchant/turnover`
   - `get_turnover()` - Custom HTTP to `/api/merchant/turnover`
   - `update_turnover()` - Custom HTTP to `/api/merchant/turnover/{date}`
   - `get_turnover_analytics()` - Custom HTTP to `/api/merchant/turnover/analytics`

3. **Profile Operations** (Lines ~980-1080)
   - `get_profile_completion()` - Custom HTTP to `/api/merchant/profile/completion`

4. **Credit Line Operations** (Lines ~1080-1280)
   - `check_credit_eligibility()` - Custom HTTP to `/api/credit-lines/eligibility/{merchant_id}`
   - `submit_credit_line_request()` - Custom HTTP to `/api/credit-lines/request`
   - `browse_available_pools()` - Custom HTTP to `/api/credit-lines/browse-pools`
   - `list_credit_line_requests()` - Custom HTTP to `/api/credit-lines/requests`

5. **Loan Operations** (Lines ~1280-1585)
   - `originate_loan()` - Custom HTTP to `/api/loans/originate`
   - `make_payment()` - Custom HTTP to `/api/loans/{id}/payment`
   - `closeout_loan()` - Custom HTTP to `/api/loans/{id}/closeout`
   - `list_loans()` - Custom HTTP to `/api/loans`
   - `get_loan()` - Custom HTTP to `/api/loans/{id}`

### Special Requirements
- **Custom HTTP Calls**: Most endpoints not available in BatchApiClient
- **Business Logic**: Complex validation for credit operations
- **Financial Calculations**: Handle loan payment calculations
- **Date Handling**: Parse and validate date formats

### Success Criteria
- ✅ All merchant operations use real API endpoints
- ✅ Credit line workflow works end-to-end
- ✅ Loan operations integrate with real data
- ✅ Financial calculations are accurate
- ✅ Zero unused `client` warnings for merchant/credit functions

---

## 📦 Work Package 6: Benchmark Integration & Optimization

**Agent Assignment**: Agent 6
**Priority**: 🟢 **LOW** (Performance testing)
**Estimated Time**: 2-3 hours
**Files to Modify**: `cli/src/benchmarks.rs`
**Dependencies**: Packages 1-5 completion (needs real API calls to benchmark)

### Scope
Integrate performance benchmarks with real API operations and optimize benchmark accuracy.

### Specific Tasks

1. **Performance Testing Functions** (Lines ~577-870)
   - Update `test_loan_performance()` to use real loan API calls
   - Integrate `test_batch_processing()` with real batch creation
   - Update `validate_zk_performance()` with real proof operations

2. **Verification Functions** (Lines ~633-770)
   - Integrate verification functions with real batch data
   - Update proof verification with actual proof files
   - Connect loan verification with real loan operations

3. **Analytics Functions** (Lines ~870-1570)
   - Update statistics gathering with real API data
   - Integrate test data generation with real API calls
   - Connect benchmark operations with actual system performance

### Success Criteria
- ✅ Benchmarks use real API operations
- ✅ Performance metrics reflect actual system performance
- ✅ Test data generation creates real entities
- ✅ Verification functions work with real proof data

---

## 🚀 Parallel Execution Strategy

### Phase 1: Independent Module Integration (Parallel)
**Timeline**: Week 1
**Agents**: 1, 2, 3, 4 (simultaneous execution)

- **Agent 1**: User Management (`user.rs`)
- **Agent 2**: Balance Management (`balance.rs`)  
- **Agent 3**: Pool Management (`pools.rs`)
- **Agent 4**: Investor Operations (`loans.rs` - investor functions only)

### Phase 2: Complex Operations (Sequential dependency)
**Timeline**: Week 2
**Agents**: 5, 6 (sequential after Phase 1)

- **Agent 5**: Merchant & Credit Operations (`loans.rs` - merchant/credit functions)
- **Agent 6**: Benchmark Integration (`benchmarks.rs`)

### Coordination Points

1. **API Client Usage Patterns**: All agents should follow the error handling pattern from `batch.rs`
2. **Custom HTTP Calls**: Agents 2, 3, 5 need to implement custom HTTP calls for missing endpoints
3. **Testing**: Each agent should test their module independently
4. **Documentation**: Update function doc comments to reflect real API usage

## 📋 Agent Assignment Commands

### For Agent 1 (User Management):
```
Implement API integration for User Management module

Files: cli/src/user.rs
Target functions: create_user, list_users, update_user, delete_user, submit_kyc, update_kyc_status, generate_invite_code, check_invite_code, use_invite_code
API methods: Use BatchApiClient methods (all available)
Pattern: Follow cli/src/batch.rs error handling
Success: Zero unused client warnings, real API data output
```

### For Agent 2 (Balance Management):
```
Implement API integration for Balance Management module

Files: cli/src/balance.rs  
Target functions: get_balance_by_address, get_balance_by_id, get_user_transactions, get_transaction_status, link_wallet, plus withdrawal operations
API methods: Mix of BatchApiClient and custom HTTP calls
Pattern: Follow cli/src/batch.rs for BatchApiClient, create custom HTTP for withdrawals
Success: Real balance data, working withdrawal operations
```

### For Agent 3 (Pool Management):
```
Implement API integration for Pool Management module

Files: cli/src/pools.rs
Target functions: create_pool, list_pools, get_pool, update_pool_status, find_matching_pools, plus analytics
API methods: BatchApiClient + custom HTTP for analytics  
Pattern: Follow cli/src/batch.rs error handling
Success: Real pool data, working analytics operations
```

### For Agent 4 (Investor Operations):
```
Implement API integration for Investor Operations (loans.rs subset)

Files: cli/src/loans.rs (investor functions only: lines 610-760)
Target functions: register_investor, get_investor_profile, update_investor_profile
API methods: Use BatchApiClient investor methods
Pattern: Follow cli/src/batch.rs error handling  
Success: Real investor data, investment capacity parsing
```

### For Agent 5 (Merchant & Credit Operations):
```
Implement API integration for Merchant & Credit Operations (loans.rs subset)

Files: cli/src/loans.rs (merchant/credit functions: lines 765-1585)
Dependencies: Wait for Agent 4 completion
Target functions: All merchant, turnover, credit line, and loan operations
API methods: Mostly custom HTTP calls (endpoints not in BatchApiClient)
Pattern: Create custom HTTP calls following withdrawal pattern from Agent 2
Success: End-to-end merchant and loan workflows working
```

### For Agent 6 (Benchmark Integration):
```
Implement API integration for Benchmark Operations

Files: cli/src/benchmarks.rs
Dependencies: Wait for Agents 1-5 completion  
Target functions: All performance and verification functions
API methods: Use real API calls from other modules for benchmarking
Pattern: Integrate with completed API implementations
Success: Real performance metrics, working verification tests
```

## 🔄 Testing & Validation Strategy

### Individual Module Testing
Each agent should:
1. Test with API server running (when available)
2. Test error scenarios (server down, invalid data)
3. Verify CLI output shows real data
4. Ensure verbose mode provides useful details

### Integration Testing
After all packages complete:
1. End-to-end workflow testing
2. Cross-module dependency validation
3. Performance impact assessment
4. Full CLI functionality verification

## 📊 Progress Tracking

### Completion Metrics
- [ ] Package 1: User Management (0/9 functions)
- [ ] Package 2: Balance Management (0/12 functions)  
- [ ] Package 3: Pool Management (0/9 functions)
- [ ] Package 4: Investor Operations (0/3 functions)
- [ ] Package 5: Merchant & Credit Operations (0/15 functions)
- [ ] Package 6: Benchmark Integration (0/8 functions)

### Quality Gates
- [ ] All unused client parameter warnings resolved
- [ ] All placeholder comments removed
- [ ] Real API data displayed in CLI output
- [ ] Comprehensive error handling implemented
- [ ] CLI compiles and runs successfully
- [ ] Integration tests pass

---

**Total Estimated Effort**: 20-29 hours across 6 parallel agents
**Critical Path**: Phase 1 (1 week) → Phase 2 (1 week)
**Expected Completion**: 2 weeks with parallel execution
