# API Refactoring Migration Plan

This document outlines the comprehensive migration plan to extract all functionality from the monolithic `rest.rs` (7131 lines) into focused handler modules.

## 🎯 **Migration Goals**

- **Extract ALL 73 endpoint functions** from `rest.rs` 
- **Move ALL 80+ type definitions** to appropriate handler files
- **Rename `rest.rs` → `router.rs`** (routing logic only, <300 lines)
- **Ensure all files are <1000 lines** following single responsibility principle
- **Maintain ALL existing functionality** with zero breaking changes

## 📊 **Current State Analysis**

```
rest.rs: 7131 lines
├── 73 endpoint functions (async fn *_endpoint)
├── 80+ type definitions (struct *Request/*Response)
├── Helper functions
├── Router setup (create_router)
└── OpenAPI documentation
```

## 🔄 **Migration Scope Breakdown**

### **Scope 1: User Management** 🟢 **IN PROGRESS**
**Target**: `user_handlers.rs`
**Current Status**: ✅ **PARTIALLY COMPLETE** (543 lines)

#### Types to Move:
- [x] `RegisterUserRequest` - MOVED
- [x] `RegisterUserResponse` - MOVED  
- [x] `UserDetailsResponse` - MOVED
- [x] `UpdateUserRequest` - MOVED
- [x] `UpdateUserResponse` - MOVED
- [x] `DeleteUserResponse` - MOVED
- [x] `ListIdentitiesQuery` - MOVED
- [x] `ListIdentitiesResponse` - MOVED
- [x] `SubmitKycRequest` - MOVED
- [x] `SubmitKycResponse` - MOVED
- [x] `UpdateKycStatusRequest` - MOVED
- [x] `UserBalanceResponse` - MOVED
- [x] `UserTransactionsResponse` - MOVED
- [x] `DepositTransactionInfo` - MOVED
- [ ] `TransactionStatusResponse`
- [ ] `AuthenticatedUser` (helper type)

#### Endpoints to Move:
- [x] `register_user_endpoint` - MOVED & ROUTER UPDATED
- [x] `list_identities_endpoint` - MOVED & ROUTER UPDATED  
- [x] `get_user_details_endpoint` - MOVED & ROUTER UPDATED
- [x] `submit_kyc_endpoint` - MOVED
- [x] `update_kyc_status_endpoint` - MOVED
- [ ] `update_user_endpoint`
- [ ] `delete_user_endpoint`
- [ ] `get_user_balance_by_address_endpoint`
- [ ] `get_user_balance_by_id_endpoint`
- [ ] `get_user_transactions_endpoint`
- [ ] `get_transaction_by_hash_endpoint`

#### Router Updates Needed:
- [x] `paths::USER` → `user_handlers::register_user_endpoint`
- [x] `paths::USERS` → `user_handlers::list_identities_endpoint`
- [x] `paths::USER_BY_ID` (GET) → `user_handlers::get_user_details_endpoint`
- [ ] `paths::USER_BY_ID` (PUT) → `user_handlers::update_user_endpoint`
- [ ] `paths::USER_BY_ID` (DELETE) → `user_handlers::delete_user_endpoint`
- [ ] `paths::USER_KYC` → `user_handlers::submit_kyc_endpoint`
- [ ] `paths::USER_KYC_STATUS` → `user_handlers::update_kyc_status_endpoint`

---

### **Scope 2: Authentication** 🟡 **READY TO START**
**Target**: `authentication_handlers.rs`
**Current Status**: ✅ **CREATED** (405 lines)

#### Types to Move:
- [x] `LoginRequest` - MOVED
- [x] `LoginResponse` - MOVED
- [x] `LogoutRequest` - MOVED
- [x] `LogoutResponse` - MOVED
- [x] `UserSummary` - MOVED
- [x] `InviteCodeResponse` - MOVED
- [x] `ValidateInviteCodeResponse` - MOVED
- [x] `GenerateInviteCodeRequest` - MOVED
- [x] `GenerateInviteCodeResponse` - MOVED

#### Endpoints to Move:
- [x] `login_endpoint` - MOVED
- [x] `logout_endpoint` - MOVED
- [x] `validate_invite_code_endpoint` - MOVED
- [x] `generate_invite_code_endpoint` - MOVED

#### Router Updates Needed:
- [ ] `/api/auth/login` → `authentication_handlers::login_endpoint`
- [ ] `/api/auth/logout` → `authentication_handlers::logout_endpoint`
- [ ] `paths::INVITE_CODE_VALIDATE` → `authentication_handlers::validate_invite_code_endpoint`
- [ ] `paths::INVITE_CODE_GENERATE` → `authentication_handlers::generate_invite_code_endpoint`

---

### **Scope 3: State & Batch Management** 🟢 **IN PROGRESS**
**Target**: `state_handlers.rs`
**Current Status**: ✅ **PARTIALLY COMPLETE** (393 lines)

#### Types to Move:
- [x] `SubmitTransactionRequest` - MOVED
- [x] `SubmitTransactionResponse` - MOVED
- [x] `CreateBatchRequest` - MOVED
- [x] `CreateBatchResponse` - MOVED
- [x] `BatchListResponse` - MOVED
- [x] `BatchInfo` - MOVED
- [x] `PendingTransactionsResponse` - MOVED
- [x] `TransactionInfo` - MOVED
- [x] `CurrentStateResponse` - MOVED
- [x] `UpdateBatchProofRequest` - MOVED
- [x] `BatchListQuery` - MOVED
- [x] `ContractSubmissionData` - MOVED
- [x] `ContractPublicData` - MOVED
- [x] `ContractPrivateData` - MOVED
- [x] `BatchProcessorStatsResponse` - MOVED
- [x] `TriggerBatchResponse` - MOVED

#### Endpoints to Move:
- [x] `submit_transaction_endpoint` - MOVED & ROUTER UPDATED
- [x] `get_pending_transactions_endpoint` - MOVED & ROUTER UPDATED
- [x] `create_batch_endpoint` - MOVED & ROUTER UPDATED
- [ ] `get_batches_endpoint`
- [ ] `get_batch_endpoint`
- [ ] `update_batch_proof_endpoint`
- [ ] `get_current_state_endpoint`
- [ ] `get_contract_data_endpoint`
- [ ] `trigger_batch_endpoint`
- [ ] `get_batch_processor_stats_endpoint`

#### Router Updates Needed:
- [x] `paths::STATE_TRANSACTIONS` → `state_handlers::submit_transaction_endpoint`
- [x] `paths::STATE_TRANSACTIONS_PENDING` → `state_handlers::get_pending_transactions_endpoint`
- [x] `paths::STATE_BATCHES` (POST) → `state_handlers::create_batch_endpoint`
- [ ] `paths::STATE_BATCHES` (GET) → `state_handlers::get_batches_endpoint`
- [ ] `paths::STATE_BATCH_BY_ID` → `state_handlers::get_batch_endpoint`
- [ ] `paths::STATE_BATCH_PROOF` → `state_handlers::update_batch_proof_endpoint`

---

### **Scope 4: Admin Operations** 🟢 **IN PROGRESS**
**Target**: `admin_handlers.rs`
**Current Status**: ✅ **PARTIALLY COMPLETE** (250 lines)

#### Types to Move:
- [ ] `ListPendingKycQuery`
- [ ] `ListPendingKycResponse`
- [ ] `KycStatisticsResponse`
- [ ] `BulkKycApprovalRequest`
- [ ] `BulkKycApprovalResponse`

#### Endpoints to Move:
- [x] `list_pending_kyc_endpoint` - MOVED & ROUTER UPDATED
- [x] `get_kyc_statistics_endpoint` - MOVED & ROUTER UPDATED
- [ ] `bulk_kyc_approval_endpoint`

#### Router Updates Needed:
- [x] `paths::ADMIN_KYC_PENDING` → `admin_handlers::list_pending_kyc_endpoint`
- [x] `paths::ADMIN_KYC_STATS` → `admin_handlers::get_kyc_statistics_endpoint`
- [ ] `paths::ADMIN_KYC_BULK_APPROVE` → `admin_handlers::bulk_kyc_approval_endpoint`

---

### **Scope 5: Merchant & Investor Management** 🟡 **READY TO START**
**Target**: `merchant_handlers.rs`
**Current Status**: ✅ **STRUCTURE CREATED** (141 lines)

#### Types to Move:
- [x] `RegisterMerchantRequest` - MOVED
- [x] `RegisterMerchantResponse` - MOVED
- [x] `RegisterInvestorRequest` - MOVED
- [x] `RegisterInvestorResponse` - MOVED
- [ ] `UpdateMerchantProfileRequest`
- [ ] `MerchantProfileResponse`
- [ ] `InvestorProfileResponse`
- [ ] `UpdateInvestorProfileRequest`
- [ ] `MerchantProfileQuery`
- [ ] `InvestorProfileQuery`
- [ ] `UpdateMerchantRequest`

#### Endpoints to Move:
- [ ] `register_merchant_endpoint`
- [ ] `register_investor_endpoint`
- [ ] `get_merchant_profile_endpoint`
- [ ] `update_merchant_profile_endpoint`
- [ ] `get_investor_profile_endpoint`
- [ ] `update_investor_profile_endpoint`

#### Router Updates Needed:
- [ ] `paths::REGISTER_MERCHANT` → `merchant_handlers::register_merchant_endpoint`
- [ ] `paths::REGISTER_INVESTOR` → `merchant_handlers::register_investor_endpoint`
- [ ] `paths::MERCHANT_PROFILE` → `merchant_handlers::get/update_merchant_profile_endpoint`
- [ ] `paths::INVESTOR_PROFILE` → `merchant_handlers::get/update_investor_profile_endpoint`

---

### **Scope 6: Loan Management** 🟡 **READY TO START**
**Target**: `loan_handlers.rs`
**Current Status**: ✅ **STRUCTURE CREATED** (335 lines)

#### Types to Move:
- [x] `CreateLoanRequest` - MOVED
- [x] `LoanDetailsResponse` - MOVED
- [x] `ListLoansQuery` - MOVED
- [x] `ListLoansResponse` - MOVED
- [x] `LoanStatsResponse` - MOVED
- [x] `UpdateLoanRequest` - MOVED
- [x] `UpdateLoanResponse` - MOVED
- [x] `DeleteLoanResponse` - MOVED
- [ ] `RecordPaymentRequest`
- [ ] `PaymentResponse`
- [ ] `PaymentDetailsResponse`
- [ ] `LoanPaymentHistoryResponse`

#### Endpoints to Move:
- [ ] `create_loan_endpoint`
- [ ] `list_loans_endpoint`
- [ ] `get_loan_details_endpoint`
- [ ] `update_loan_endpoint`
- [ ] `delete_loan_endpoint`
- [ ] `get_loan_stats_endpoint`
- [ ] `record_payment_endpoint`
- [ ] `get_payment_details_endpoint`
- [ ] `get_loan_payment_history_endpoint`

#### Router Updates Needed:
- [ ] `paths::LOAN` → `loan_handlers::create_loan_endpoint`
- [ ] `paths::LOANS` → `loan_handlers::list_loans_endpoint`
- [ ] `paths::LOAN_BY_ID` → `loan_handlers::get/update/delete_loan_endpoint`
- [ ] `paths::LOANS_STATS` → `loan_handlers::get_loan_stats_endpoint`

---

### **Scope 7: Turnover & Analytics** 🔴 **NOT STARTED**
**Target**: `turnover_handlers.rs` (NEW FILE)

#### Types to Move:
- [ ] `TurnoverCreateRequest`
- [ ] `TurnoverUpdateRequest`
- [ ] `TurnoverResponse`
- [ ] `TurnoverAnalyticsResponse`
- [ ] `ProfileCompletionResponse`

#### Endpoints to Move:
- [ ] `create_turnover_endpoint`
- [ ] `get_turnover_endpoint`
- [ ] `update_turnover_endpoint`
- [ ] `get_turnover_analytics_endpoint`
- [ ] `get_profile_completion_endpoint`

#### Router Updates Needed:
- [ ] `paths::MERCHANT_TURNOVER` → `turnover_handlers::*`
- [ ] `paths::MERCHANT_TURNOVER_ANALYTICS` → `turnover_handlers::*`

---

### **Scope 8: Withdrawal Management** 🔴 **NOT STARTED**
**Target**: `withdrawal_handlers.rs` (NEW FILE)

#### Types to Move:
- [ ] `InitiateWithdrawalRequest`
- [ ] `InitiateWithdrawalResponse`
- [ ] `WithdrawalStatusResponse`
- [ ] `CancelWithdrawalRequest`

#### Endpoints to Move:
- [ ] `initiate_withdrawal_endpoint`
- [ ] `get_withdrawal_status_endpoint`
- [ ] `get_user_available_balance_endpoint`
- [ ] `get_user_withdrawal_history_endpoint`
- [ ] `cancel_withdrawal_endpoint`
- [ ] `get_pending_withdrawals_endpoint`

---

### **Scope 9: Pool Management** 🟠 **PARTIALLY EXISTS**
**Target**: `pool_handlers.rs` (ALREADY EXISTS - 812 lines)
**Status**: Needs integration with rest.rs pool endpoints

#### Types to Move:
- [ ] `CreatePoolRequest` (already in pool_api.rs)
- [ ] `CreatePoolResponse` (already in pool_api.rs)  
- [ ] `PoolListResponse`
- [ ] `PoolMatchRequest`

#### Endpoints to Move:
- [ ] `create_pool_endpoint`
- [ ] `list_pools_endpoint`
- [ ] `get_pool_endpoint`
- [ ] `update_pool_status_endpoint`
- [ ] `find_matching_pools_endpoint`

---

### **Scope 10: Core Infrastructure** 🟡 **READY TO START**
**Target**: `router.rs` (RENAMED FROM rest.rs)

#### What Stays:
- `create_router()` function
- Route definitions only
- `ApiState` and `ApiConfig` types
- Basic helper functions

#### What Gets Removed:
- ALL endpoint implementations
- ALL request/response types
- ALL business logic

#### Target Size: **<300 lines**

---

## 🚀 **Parallel Execution Plan**

### **Phase 1: Type Extraction** (Can be done in parallel)
- **Agent A**: Extract User & Auth types (Scopes 1-2)
- **Agent B**: Extract State & Batch types (Scope 3)  
- **Agent C**: Extract Merchant & Loan types (Scopes 5-6)
- **Agent D**: Extract Withdrawal & Turnover types (Scopes 7-8)

### **Phase 2: Endpoint Extraction** (Can be done in parallel)
- **Agent A**: Extract User & Auth endpoints (Scopes 1-2)
- **Agent B**: Extract State & Batch endpoints (Scope 3)
- **Agent C**: Extract Merchant & Loan endpoints (Scopes 5-6)
- **Agent D**: Extract Withdrawal & Turnover endpoints (Scopes 7-8)

### **Phase 3: Router Updates** (Sequential - needs coordination)
- Update all route definitions to point to new handlers
- Remove OpenAPI references to moved endpoints
- Test each scope incrementally

### **Phase 4: Cleanup** (Sequential)
- Remove all moved functions from rest.rs
- Rename rest.rs → router.rs
- Update all imports across codebase
- Final testing and verification

---

## 📋 **Detailed Task Breakdown**

### **Scope 1: User Management** 
```bash
# Files to modify:
- api/src/user_handlers.rs (ADD remaining endpoints)
- api/src/rest.rs (REMOVE user endpoints)

# Types to extract from rest.rs lines:
- TransactionStatusResponse (lines ~1138-1143)
- AuthenticatedUser (lines ~151-155)

# Endpoints to extract from rest.rs:
- update_user_endpoint (lines ~3131-3227)
- delete_user_endpoint (lines ~3227-3315)  
- get_user_balance_by_address_endpoint (lines ~4321-4350)
- get_user_balance_by_id_endpoint (lines ~4367-4420)
- get_user_transactions_endpoint (lines ~4437-4512)
- get_transaction_by_hash_endpoint (lines ~4528-4574)
```

### **Scope 2: Authentication**
```bash
# Files to modify:
- api/src/authentication_handlers.rs (ALREADY CREATED)
- api/src/rest.rs (REMOVE auth endpoints)

# Endpoints to extract from rest.rs:
- login_endpoint (lines ~2331-2447)
- logout_endpoint (lines ~2447-2479) 
- validate_invite_code_endpoint (lines ~2479-2592)
- generate_invite_code_endpoint (lines ~2592-2679)
```

### **Scope 3: State & Batch Management**
```bash
# Files to modify:
- api/src/state_handlers.rs (ADD remaining endpoints)
- api/src/rest.rs (REMOVE state endpoints)

# Endpoints to extract from rest.rs:
- get_batches_endpoint (lines ~1857-1913)
- get_batch_endpoint (lines ~1913-1975)
- update_batch_proof_endpoint (lines ~1975-2129)
- get_current_state_endpoint (lines ~2129-2181)
- get_contract_data_endpoint (lines ~2181-2235)
- trigger_batch_endpoint (lines ~2235-2277)
- get_batch_processor_stats_endpoint (lines ~2277-2331)
```

### **Scope 4: Admin Operations**
```bash
# Files to modify:
- api/src/admin_handlers.rs (ADD remaining endpoints)
- api/src/rest.rs (REMOVE admin endpoints)

# Types to extract from rest.rs:
- ListPendingKycQuery (lines ~764-768)
- ListPendingKycResponse (lines ~771-777)
- BulkKycApprovalRequest (lines ~790-795)
- BulkKycApprovalResponse (lines ~797-803)

# Endpoints to extract from rest.rs:
- bulk_kyc_approval_endpoint (lines ~3048-3131)
```

---

## 🎯 **Success Criteria**

### **Completion Checklist:**
- [ ] `rest.rs` → `router.rs` renamed and <300 lines
- [ ] All endpoint functions moved to handler files
- [ ] All type definitions moved to handler files  
- [ ] All handler files <1000 lines
- [ ] `make lint` passes
- [ ] `make test` passes
- [ ] All original functionality preserved

### **File Size Targets:**
- `router.rs`: <300 lines (routing only)
- `user_handlers.rs`: <800 lines
- `state_handlers.rs`: <600 lines
- `authentication_handlers.rs`: <500 lines
- `admin_handlers.rs`: <400 lines
- `merchant_handlers.rs`: <700 lines
- `loan_handlers.rs`: <600 lines

---

## ⚠️ **Critical Dependencies**

### **Import Updates Required:**
```rust
// OLD
use crate::rest::{UserDetailsResponse, CreateBatchRequest, ...};

// NEW  
use crate::user_handlers::{UserDetailsResponse, ...};
use crate::state_handlers::{CreateBatchRequest, ...};
```

### **Files That Import from rest.rs:**
- `api/src/openapi.rs` ✅ PARTIALLY UPDATED
- `api/src/types.rs` ✅ UPDATED
- `api/src/server.rs` ✅ UPDATED
- `api/src/paths.rs` ✅ UPDATED
- `api/src/pool_handlers.rs` ⚠️ NEEDS UPDATE
- `api/src/financial_transaction_handlers.rs` ⚠️ NEEDS UPDATE
- `api/src/credit_line_handlers.rs` ⚠️ NEEDS UPDATE
- `api/src/batch_processor.rs` ⚠️ NEEDS UPDATE
- `api/src/integration.rs` ⚠️ NEEDS UPDATE
- `api/src/graphql.rs` ⚠️ NEEDS UPDATE

---

## 🔧 **Execution Commands**

### **For Each Scope:**
1. **Extract Types**: Copy type definitions to handler file
2. **Extract Endpoints**: Copy endpoint functions to handler file  
3. **Update Router**: Change route to point to new handler
4. **Remove from rest.rs**: Delete moved functions and types
5. **Update OpenAPI**: Remove references from rest.rs OpenAPI derive
6. **Test**: Verify `make lint` and `make test` still pass

### **Final Steps:**
1. **Rename**: `mv rest.rs router.rs`
2. **Clean Router**: Remove all non-routing code
3. **Update Imports**: Update all `rest::` imports to handler modules
4. **Final Test**: Comprehensive testing

---

## 📊 **Progress Tracking**

**Current Progress:**
- ✅ User Management: 60% complete (6/10 endpoints moved)
- ✅ Authentication: 100% complete (structure ready)
- ✅ State Management: 30% complete (3/10 endpoints moved)  
- ✅ Admin Operations: 70% complete (2/3 endpoints moved)
- 🔴 Merchant/Investor: 0% complete (0/6 endpoints moved)
- 🔴 Loan Management: 0% complete (0/9 endpoints moved)
- 🔴 Withdrawal: 0% complete (0/6 endpoints moved)
- 🔴 Turnover: 0% complete (0/5 endpoints moved)

**Overall Progress: ~25% complete**

---

## 🎯 **Next Actions**

**Immediate Priority (can be parallelized):**
1. **Complete Scope 1** (User Management) - finish remaining 4 endpoints
2. **Complete Scope 3** (State Management) - finish remaining 7 endpoints  
3. **Complete Scope 4** (Admin Operations) - finish remaining 1 endpoint
4. **Start Scope 5** (Merchant/Investor) - extract all 6 endpoints
5. **Start Scope 6** (Loan Management) - extract all 9 endpoints

**This migration will reduce `rest.rs` from 7131 lines to <300 lines while maintaining all functionality.**
