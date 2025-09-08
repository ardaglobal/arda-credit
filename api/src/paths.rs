//! Centralized API path constants
//!
//! This module provides a single source of truth for all API paths,
//! making it easy to maintain consistency across the server routes,
//! client URLs, and OpenAPI documentation.
//!
//! Path Structure:
//! - root: /api/v1
//! - users: {root}/user and {root}/users
//! - loans: {root}/loan and {root}/loans
//! - state: {root}/state (includes batch and proofs as sub roots)

/// Current API version (for reference and documentation)
pub const API_VERSION: &str = "v1";

/// Build API paths with current version
/// Note: Due to Rust's concat! limitations, version is hardcoded in macro
/// To change version: update both API_VERSION above and version in macro below
macro_rules! api_path {
    ($path:expr) => {
        concat!("/api/v1", $path)
    };
}

// ============================================================================
// CORE API PATHS
// ============================================================================

/// Root path
pub const ROOT: &str = "/";

/// Health check path (legacy)
pub const HEALTH: &str = "/health";

/// API health check path
pub const API_HEALTH: &str = api_path!("/health");

/// API info path
pub const API_INFO: &str = api_path!("/info");

// ============================================================================
// USER MANAGEMENT PATHS
// ============================================================================

/// Users collection (list users)
pub const USERS: &str = api_path!("/users");

/// User creation (single user)
pub const USER: &str = api_path!("/user");

/// Individual user operations
pub const USER_BY_ID: &str = api_path!("/user/{user_id}");

/// User KYC submission
pub const USER_KYC: &str = api_path!("/user/{user_id}/kyc");

/// User KYC status update (admin)
pub const USER_KYC_STATUS: &str = api_path!("/user/{user_id}/kyc-status");

/// List pending KYC applications (admin)
pub const ADMIN_KYC_PENDING: &str = api_path!("/admin/kyc/pending");

/// KYC statistics (admin)
pub const ADMIN_KYC_STATS: &str = api_path!("/admin/kyc/stats");

/// Bulk KYC approval (admin)
pub const ADMIN_KYC_BULK_APPROVE: &str = api_path!("/admin/kyc/bulk-approve");

/// User balance by wallet address
pub const USER_BALANCE_BY_ADDRESS: &str = api_path!("/user/{user_id}/{address}/balance");

/// User balance by user ID
pub const USER_BALANCE_BY_ID: &str = api_path!("/user/{user_id}/balance");

/// User transactions by user ID
pub const USER_TRANSACTIONS_BY_ID: &str = api_path!("/user/{user_id}/transactions");

/// Transaction by hash
pub const TRANSACTION_BY_HASH: &str = api_path!("/transactions/{tx_hash}");

// ============================================================================
// INVESTOR MANAGEMENT PATHS
// ============================================================================

/// Investor registration
pub const REGISTER_INVESTOR: &str = api_path!("/auth/register/investor");

/// Merchant registration  
pub const REGISTER_MERCHANT: &str = api_path!("/auth/register/merchant");

/// Investor profile operations
pub const INVESTOR_PROFILE: &str = api_path!("/investor/profile");

/// Merchant profile operations
pub const MERCHANT_PROFILE: &str = api_path!("/merchant/profile");

/// Merchant turnover operations
pub const MERCHANT_TURNOVER: &str = api_path!("/merchant/turnover");

/// Merchant turnover by date
pub const MERCHANT_TURNOVER_BY_DATE: &str = api_path!("/merchant/turnover/{date}");

/// Merchant turnover analytics
pub const MERCHANT_TURNOVER_ANALYTICS: &str = api_path!("/merchant/turnover/analytics");

/// Merchant profile completion
pub const MERCHANT_PROFILE_COMPLETION: &str = api_path!("/merchant/profile/completion");

// ============================================================================
// WITHDRAWAL MANAGEMENT PATHS
// ============================================================================

/// Initiate withdrawal request
pub const WITHDRAWALS: &str = api_path!("/withdrawals");

/// Get withdrawal status by request ID
pub const WITHDRAWAL_BY_ID: &str = api_path!("/withdrawals/{request_id}");

/// Get user's available balance for withdrawals
pub const USER_AVAILABLE_BALANCE: &str = api_path!("/users/{address}/available-balance");

/// Get user's withdrawal history by address  
pub const USER_WITHDRAWALS: &str = api_path!("/users/{address}/withdrawals");

/// Cancel withdrawal request
pub const WITHDRAWAL_CANCEL: &str = api_path!("/withdrawals/{request_id}/cancel");

/// Admin endpoint: Get pending withdrawal requests
pub const ADMIN_PENDING_WITHDRAWALS: &str = api_path!("/admin/withdrawals/pending");

// ============================================================================
// INVITE CODE MANAGEMENT PATHS
// ============================================================================

/// Generate new invite code
pub const INVITE_CODE_GENERATE: &str = api_path!("/invite-code");

/// Validate invite code (doesn't mark as used)
pub const INVITE_CODE_VALIDATE: &str = api_path!("/invite-code/{code}/validate");

// ============================================================================
// LOAN MANAGEMENT PATHS
// ============================================================================

/// Loans collection (list loans)
pub const LOANS: &str = api_path!("/loans");

/// Loan creation (single loan)
pub const LOAN: &str = api_path!("/loan");

/// Individual loan operations
pub const LOAN_BY_ID: &str = api_path!("/loan/{loan_id}");

/// Loan payment history
pub const LOAN_PAYMENTS: &str = api_path!("/loan/{loan_id}/payments");

/// Loan statistics
pub const LOANS_STATS: &str = api_path!("/loans/stats");

/// Loan audit trail
pub const LOAN_AUDIT_TRAIL: &str = api_path!("/loan/{loan_id}/audit-trail");

/// Loan lifecycle summary
pub const LOAN_LIFECYCLE_SUMMARY: &str = api_path!("/loan/{loan_id}/lifecycle-summary");

// ============================================================================
// POOL MANAGEMENT PATHS
// ============================================================================

/// Pool collection (list pools)
pub const POOLS: &str = api_path!("/pools");

/// Individual pool operations
pub const POOL_BY_ID: &str = api_path!("/pools/{pool_id}");

/// Pool utilization
pub const POOL_UTILIZATION: &str = api_path!("/pools/{pool_id}/utilization");

/// Pool transactions
pub const POOL_TRANSACTIONS: &str = api_path!("/pools/{pool_id}/transactions");

// ============================================================================
// STATE MANAGEMENT PATHS (transactions, batches, proofs)
// ============================================================================

/// Current state
pub const STATE_CURRENT: &str = api_path!("/state/current");

/// State summary (comprehensive overview)
pub const STATE_SUMMARY: &str = api_path!("/state/summary");

/// State history
pub const STATE_HISTORY: &str = api_path!("/state/history");

// Transaction Operations
/// Submit transaction
pub const STATE_TRANSACTIONS: &str = api_path!("/state/transactions");

/// Get pending transactions
pub const STATE_TRANSACTIONS_PENDING: &str = api_path!("/state/transactions/pending");

// Batch Operations
/// Batch operations root (list batches, create batch)
pub const STATE_BATCHES: &str = api_path!("/state/batches");

/// Get specific batch
pub const STATE_BATCH_BY_ID: &str = api_path!("/state/batch/{batch_id}");

/// Update batch proof
pub const STATE_BATCH_PROOF: &str = api_path!("/state/batch/{batch_id}/proof");

/// Batch trigger
pub const STATE_BATCHES_TRIGGER: &str = api_path!("/state/batches/trigger");

/// Batch processor statistics
pub const STATE_BATCHES_STATS: &str = api_path!("/state/batches/stats");

/// Contract data for batch
pub const STATE_CONTRACT: &str = api_path!("/state/{batch_id}/contract");

/// Create batch from loan transactions
pub const STATE_BATCH_CREATE: &str = api_path!("/state/batch/create");

/// Get loan batch details
pub const STATE_LOAN_BATCH_BY_ID: &str = api_path!("/state/loan-batch/{batch_id}");

/// Get pending loan transactions ready for batching
pub const STATE_BATCH_PENDING: &str = api_path!("/state/batch/pending");

// Payment Operations
/// Record payment
pub const STATE_PAYMENTS: &str = api_path!("/state/payments");

/// Get payment details
pub const STATE_PAYMENT_BY_ID: &str = api_path!("/state/payment/{transaction_id}");

// Proof Operations
/// Generate loan operation proof
pub const STATE_PROOFS_LOAN_OPERATION: &str = api_path!("/state/proofs/loan-operation");

/// Generate loan payment proof
pub const STATE_PROOFS_LOAN_PAYMENT: &str =
    api_path!("/state/proofs/loan-payment/{user_id}/{loan_id}/{payment_id}/{batch_id}");

/// Verify proof
pub const STATE_PROOFS_VERIFY: &str = api_path!("/state/proofs/verify");

/// Generate proof report
pub const STATE_PROOFS_REPORT: &str = api_path!("/state/proofs/report");

/// Batch verify proofs
pub const STATE_PROOFS_BATCH_VERIFY: &str = api_path!("/state/proofs/batch-verify");

/// Generate transaction inclusion proof
pub const STATE_PROOF: &str = api_path!("/state/proof/{transaction_type}/{transaction_id}");

/// Verify transaction inclusion
pub const STATE_VERIFY: &str = api_path!("/state/verify");

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Replace path parameter with actual value
pub fn replace_param(path: &str, param: &str, value: &str) -> String {
    path.replace(&format!("{{{}}}", param), value)
}

// User Management Helper Functions
/// Build user path with user_id parameter
pub fn user_path(user_id: &str) -> String {
    replace_param(USER_BY_ID, "user_id", user_id)
}

/// Build user KYC path with user_id parameter
pub fn user_kyc_path(user_id: &str) -> String {
    replace_param(USER_KYC, "user_id", user_id)
}

/// Build user KYC status path with user_id parameter
pub fn user_kyc_status_path(user_id: &str) -> String {
    replace_param(USER_KYC_STATUS, "user_id", user_id)
}

/// Build admin KYC pending path
pub fn admin_kyc_pending_path() -> String {
    ADMIN_KYC_PENDING.to_string()
}

/// Build admin KYC stats path
pub fn admin_kyc_stats_path() -> String {
    ADMIN_KYC_STATS.to_string()
}

/// Build admin bulk KYC approval path
pub fn admin_kyc_bulk_approve_path() -> String {
    ADMIN_KYC_BULK_APPROVE.to_string()
}

// Invite Code Management Helper Functions
/// Build invite code validate path with code parameter
pub fn invite_code_validate_path(code: &str) -> String {
    replace_param(INVITE_CODE_VALIDATE, "code", code)
}

// Loan Management Helper Functions
/// Build loan path with loan_id parameter
pub fn loan_path(loan_id: &str) -> String {
    replace_param(LOAN_BY_ID, "loan_id", loan_id)
}

/// Build loan payments path with loan_id parameter
pub fn loan_payments_path(loan_id: &str) -> String {
    replace_param(LOAN_PAYMENTS, "loan_id", loan_id)
}

/// Build loan audit trail path with loan_id parameter
pub fn loan_audit_trail_path(loan_id: &str) -> String {
    replace_param(LOAN_AUDIT_TRAIL, "loan_id", loan_id)
}

/// Build loan lifecycle summary path with loan_id parameter
pub fn loan_lifecycle_summary_path(loan_id: &str) -> String {
    replace_param(LOAN_LIFECYCLE_SUMMARY, "loan_id", loan_id)
}

// State Management Helper Functions
/// Build batch path with batch_id parameter
pub fn batch_path(batch_id: i32) -> String {
    replace_param(STATE_BATCH_BY_ID, "batch_id", &batch_id.to_string())
}

/// Build loan batch path with batch_id parameter
pub fn loan_batch_path(batch_id: i32) -> String {
    replace_param(STATE_LOAN_BATCH_BY_ID, "batch_id", &batch_id.to_string())
}

/// Build batch proof path with batch_id parameter
pub fn batch_proof_path(batch_id: i32) -> String {
    replace_param(STATE_BATCH_PROOF, "batch_id", &batch_id.to_string())
}

/// Build state contract path with batch_id parameter
pub fn state_contract_path(batch_id: i32) -> String {
    replace_param(STATE_CONTRACT, "batch_id", &batch_id.to_string())
}

/// Build payment path with transaction_id parameter
pub fn payment_path(transaction_id: &str) -> String {
    replace_param(STATE_PAYMENT_BY_ID, "transaction_id", transaction_id)
}

/// Build loan payment proof path with parameters
pub fn loan_payment_proof_path(
    user_id: &str,
    loan_id: &str,
    payment_id: &str,
    batch_id: i32,
) -> String {
    let path = replace_param(STATE_PROOFS_LOAN_PAYMENT, "user_id", user_id);
    let path = replace_param(&path, "loan_id", loan_id);
    let path = replace_param(&path, "payment_id", payment_id);
    replace_param(&path, "batch_id", &batch_id.to_string())
}

/// Build transaction proof path with parameters
pub fn transaction_proof_path(transaction_type: &str, transaction_id: &str) -> String {
    let path = replace_param(STATE_PROOF, "transaction_type", transaction_type);
    replace_param(&path, "transaction_id", transaction_id)
}

/// Generate user balance by address path
pub fn user_balance_by_address_path(address: &str) -> String {
    replace_param(USER_BALANCE_BY_ADDRESS, "address", address)
}

/// Generate user balance by ID path
pub fn user_balance_by_id_path(user_id: &str) -> String {
    replace_param(USER_BALANCE_BY_ID, "user_id", user_id)
}

/// Generate user transactions by ID path
pub fn user_transactions_by_id_path(user_id: &str) -> String {
    replace_param(USER_TRANSACTIONS_BY_ID, "user_id", user_id)
}

// Withdrawal Management Helper Functions
/// Build withdrawal path with request_id parameter
pub fn withdrawal_path(request_id: &str) -> String {
    replace_param(WITHDRAWAL_BY_ID, "request_id", request_id)
}

/// Build user available balance path with address parameter
pub fn user_available_balance_path(address: &str) -> String {
    replace_param(USER_AVAILABLE_BALANCE, "address", address)
}

/// Build user withdrawals path with address parameter
pub fn user_withdrawals_path(address: &str) -> String {
    replace_param(USER_WITHDRAWALS, "address", address)
}

/// Build withdrawal cancel path with request_id parameter
pub fn withdrawal_cancel_path(request_id: &str) -> String {
    replace_param(WITHDRAWAL_CANCEL, "request_id", request_id)
}

/// Generate transaction by hash path
pub fn transaction_by_hash_path(tx_hash: &str) -> String {
    replace_param(TRANSACTION_BY_HASH, "tx_hash", tx_hash)
}

// ============================================================================
// PATH LISTS FOR DOCUMENTATION
// ============================================================================

/// All endpoint information for API documentation
pub fn get_all_endpoints() -> Vec<crate::rest::EndpointInfo> {
    vec![
        // Core API Endpoints
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: API_HEALTH.to_string(),
            description: "Health check".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: API_INFO.to_string(),
            description: "API information".to_string(),
        },
        // User Management
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: USERS.to_string(),
            description: "List user identities with filters".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: USERS.to_string(),
            description: "Register new user identity".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: USER_BY_ID.to_string(),
            description: "Get user details by ID".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: USER_KYC.to_string(),
            description: "Submit KYC information".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: USER_KYC_STATUS.to_string(),
            description: "Update KYC status (admin only)".to_string(),
        },
        // Admin KYC Management
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: ADMIN_KYC_PENDING.to_string(),
            description: "List pending KYC applications (admin only)".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: ADMIN_KYC_STATS.to_string(),
            description: "Get KYC statistics (admin only)".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: ADMIN_KYC_BULK_APPROVE.to_string(),
            description: "Bulk approve KYC applications (admin only)".to_string(),
        },
        // Invite Code Management
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: INVITE_CODE_GENERATE.to_string(),
            description: "Generate new invite code".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: INVITE_CODE_VALIDATE.to_string(),
            description: "Validate invite code (doesn't mark as used)".to_string(),
        },
        // Loan Management
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOANS.to_string(),
            description: "List loans with filters".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: LOAN.to_string(),
            description: "Create new loan".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOAN_BY_ID.to_string(),
            description: "Get loan details by ID".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "DELETE".to_string(),
            path: LOAN_BY_ID.to_string(),
            description: "Close loan by ID".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOAN_PAYMENTS.to_string(),
            description: "Get loan payment history".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOANS_STATS.to_string(),
            description: "Get loan statistics".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOAN_AUDIT_TRAIL.to_string(),
            description: "Get loan audit trail".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: LOAN_LIFECYCLE_SUMMARY.to_string(),
            description: "Get loan lifecycle summary".to_string(),
        },
        // State Management - Core State
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_CURRENT.to_string(),
            description: "Get current system state".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_SUMMARY.to_string(),
            description: "Get comprehensive state summary with loan and user stats".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_HISTORY.to_string(),
            description: "Get state transition history".to_string(),
        },
        // State Management - Transactions
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_TRANSACTIONS.to_string(),
            description: "Submit a new transaction".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_TRANSACTIONS_PENDING.to_string(),
            description: "Get pending transactions".to_string(),
        },
        // State Management - Batches
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCHES.to_string(),
            description: "List batches".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCHES.to_string(),
            description: "Create new batch from pending transactions".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCH_BY_ID.to_string(),
            description: "Get batch details by ID".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCH_PROOF.to_string(),
            description: "Update batch with ZK proof".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCHES_TRIGGER.to_string(),
            description: "Manually trigger batch processing".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCHES_STATS.to_string(),
            description: "Get batch processor statistics".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCH_CREATE.to_string(),
            description: "Create batch from loan transactions".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_LOAN_BATCH_BY_ID.to_string(),
            description: "Get loan batch details by ID".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCH_PENDING.to_string(),
            description: "Get pending loan transactions ready for batching".to_string(),
        },
        // State Management - Payments
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PAYMENTS.to_string(),
            description: "Record loan payment".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PAYMENT_BY_ID.to_string(),
            description: "Get payment details".to_string(),
        },
        // State Management - Proofs
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PROOF.to_string(),
            description: "Generate transaction inclusion proof".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_VERIFY.to_string(),
            description: "Verify transaction inclusion".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_LOAN_OPERATION.to_string(),
            description: "Generate loan operation proof".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PROOFS_LOAN_PAYMENT.to_string(),
            description: "Generate loan payment proof".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_VERIFY.to_string(),
            description: "Verify loan proof".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_REPORT.to_string(),
            description: "Generate proof report".to_string(),
        },
        crate::rest::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_BATCH_VERIFY.to_string(),
            description: "Batch verify proofs".to_string(),
        },
    ]
}
