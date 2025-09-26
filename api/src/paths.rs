//! Centralized API path constants
//!
//! This module provides a single source of truth for all API paths,
//! making it easy to maintain consistency across the server routes,
//! client URLs, and OpenAPI documentation.
//!
//! Path Structure:
//! - root: /api/v1
//! - users: {root}/user and {root}/users
//! - deals: {root}/deal and {root}/deals (converted from loans)
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

// Import for EndpointInfo type
use crate::health_handlers::EndpointInfo;

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

/// Platform metrics (admin)
pub const ADMIN_PLATFORM_METRICS: &str = api_path!("/admin/platform-metrics");

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

/// Investor dashboard
pub const INVESTOR_DASHBOARD: &str = api_path!("/investor/dashboard");

/// Investor pool summaries
pub const INVESTOR_POOL_SUMMARIES: &str = api_path!("/investor/pools/summary");

/// Investor pool metrics
pub const INVESTOR_POOL_METRICS: &str = api_path!("/investor/pool-metrics");

/// Investor enhanced metrics (commented out in router)
pub const INVESTOR_METRICS: &str = api_path!("/investor/metrics");

/// Authentication login
pub const AUTH_LOGIN: &str = api_path!("/auth/login");

/// Authentication logout
pub const AUTH_LOGOUT: &str = api_path!("/auth/logout");

/// Magic link authentication endpoints
pub const AUTH_MAGIC_LINK: &str = api_path!("/auth/magic-link");
pub const AUTH_VERIFY: &str = api_path!("/auth/verify");
pub const AUTH_ME: &str = api_path!("/auth/me");

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

/// Admin endpoint: Approve withdrawal request
pub const ADMIN_APPROVE_WITHDRAWAL: &str = api_path!("/admin/withdrawals/{request_id}/approve");

/// Admin endpoint: Reject withdrawal request
pub const ADMIN_REJECT_WITHDRAWAL: &str = api_path!("/admin/withdrawals/{request_id}/reject");

/// Admin endpoint: Get withdrawal execution stats
pub const ADMIN_WITHDRAWAL_STATS: &str = api_path!("/admin/withdrawals/stats");

/// Admin endpoint: Execute withdrawal manually
pub const ADMIN_EXECUTE_WITHDRAWAL: &str = api_path!("/admin/withdrawals/{request_id}/execute");

/// Admin endpoint: Get withdrawal dashboard
pub const ADMIN_WITHDRAWAL_DASHBOARD: &str = api_path!("/admin/withdrawals/dashboard");

// ============================================================================
// INVITE CODE MANAGEMENT PATHS
// ============================================================================

/// Generate new invite code
pub const INVITE_CODE_GENERATE: &str = api_path!("/invite-code");

/// Validate invite code (doesn't mark as used)
pub const INVITE_CODE_VALIDATE: &str = api_path!("/invite-code/{code}/validate");

// ============================================================================
// DEAL MANAGEMENT PATHS (converted from LOAN MANAGEMENT PATHS)
// ============================================================================

/// Deals collection (list deals) - was LOANS
pub const DEALS: &str = api_path!("/deals");

/// Deal creation (single deal) - was LOAN
pub const DEAL: &str = api_path!("/deal");

/// Individual deal operations - was LOAN_BY_ID
pub const DEAL_BY_ID: &str = api_path!("/deal/{deal_id}");

/// Deal payment history - was LOAN_PAYMENTS
pub const DEAL_PAYMENTS: &str = api_path!("/deal/{deal_id}/payments");

/// Deal statistics - was LOANS_STATS
pub const DEALS_STATS: &str = api_path!("/deals/stats");

/// Deals by user ID - get all deals for a user (as originator or investor)
pub const DEALS_BY_USER_ID: &str = api_path!("/deals/{user_id}");

/// Deal audit trail - was LOAN_AUDIT_TRAIL
pub const DEAL_AUDIT_TRAIL: &str = api_path!("/deal/{deal_id}/audit-trail");

/// Deal lifecycle summary - was LOAN_LIFECYCLE_SUMMARY
pub const DEAL_LIFECYCLE_SUMMARY: &str = api_path!("/deal/{deal_id}/lifecycle-summary");

// ============================================================================
// DEAL MARKETPLACE PATHS (consolidated from separate marketplace paths)
// ============================================================================

/// List deals for sale (consolidated from LOANS_FOR_SALE)
pub const DEALS_FOR_SALE: &str = api_path!("/deals/for-sale");

/// List a deal for sale (consolidated from LOAN_LIST_FOR_SALE)
pub const DEAL_LIST_FOR_SALE: &str = api_path!("/deal/{deal_id}/list-for-sale");

/// Remove deal from sale (consolidated from LOAN_LIST_FOR_SALE with DELETE)
pub const DEAL_REMOVE_FROM_SALE: &str = api_path!("/deal/{deal_id}/remove-from-sale");

/// Deal marketplace statistics (consolidated from MARKETPLACE_STATS)
pub const DEAL_MARKETPLACE_STATS: &str = api_path!("/deals/marketplace/stats");

// ============================================================================
// DEAL TRANSFER PATHS (consolidated from separate transfer paths)
// ============================================================================

/// Transfer deal ownership (consolidated from LOAN_TRANSFER)
pub const DEAL_TRANSFER: &str = api_path!("/deal/{deal_id}/transfer");

/// Deal transfer history (consolidated from LOAN_TRANSFER_HISTORY)
pub const DEAL_TRANSFER_HISTORY: &str = api_path!("/deal/{deal_id}/transfers");

/// User's deal transfer history (consolidated from USER_TRANSFER_HISTORY)
pub const USER_DEAL_TRANSFER_HISTORY: &str = api_path!("/user/{user_id}/deal-transfers");

/// Validate deal transfer (consolidated from LOAN_TRANSFER_VALIDATE)
pub const DEAL_TRANSFER_VALIDATE: &str = api_path!("/deal/{deal_id}/transfer/validate");

// ============================================================================
// DEPRECATED PATHS - REMOVED IN DEAL CONVERSION
// ============================================================================

// REMOVED: Loan management paths
// pub const LOANS: &str = api_path!("/loans");
// pub const LOAN: &str = api_path!("/loan");
// pub const LOAN_BY_ID: &str = api_path!("/loan/{loan_id}");
// pub const LOAN_PAYMENTS: &str = api_path!("/loan/{loan_id}/payments");

// REMOVED: Separate marketplace paths (consolidated into deal paths)
// pub const LOANS_FOR_SALE: &str = api_path!("/loans/for-sale");
// pub const LOAN_LIST_FOR_SALE: &str = api_path!("/loan/{loan_id}/list-for-sale");
// pub const MARKETPLACE_STATS: &str = api_path!("/loans/marketplace/stats");

// REMOVED: Separate transfer paths (consolidated into deal paths)
// pub const LOAN_TRANSFER: &str = api_path!("/loan/{loan_id}/transfer");
// pub const LOAN_TRANSFER_HISTORY: &str = api_path!("/loan/{loan_id}/transfers");
// pub const USER_TRANSFER_HISTORY: &str = api_path!("/user/{user_id}/transfers");
// pub const LOAN_TRANSFER_VALIDATE: &str = api_path!("/loan/{loan_id}/transfer/validate");

// ============================================================================
// POOL MANAGEMENT PATHS
// ============================================================================

/// Pool collection (list pools)
pub const POOLS: &str = api_path!("/pools");

/// Individual pool operations
pub const POOL_BY_ID: &str = api_path!("/pools/{pool_id}");

/// Pool deposit operation
pub const POOL_DEPOSIT: &str = api_path!("/pools/{pool_id}/deposit");

/// Pool utilization
pub const POOL_UTILIZATION: &str = api_path!("/pools/{pool_id}/utilization");

/// Pool transactions
pub const POOL_TRANSACTIONS: &str = api_path!("/pools/{pool_id}/transactions");

/// Pool matching endpoint for auto-deployment
pub const POOL_MATCH: &str = api_path!("/pools/match");

/// Pool metrics overview endpoint
pub const POOL_METRICS_OVERVIEW: &str = api_path!("/pools/metrics/overview");

/// Investor-specific pool metrics endpoint
pub const POOL_METRICS_INVESTOR: &str = api_path!("/pools/metrics/investor/{investor_id}");

/// Pool borough limits endpoint
pub const POOL_BOROUGH_LIMITS: &str = api_path!("/pools/{pool_id}/borough-limits");

/// Individual pool borough limit endpoint
pub const POOL_BOROUGH_LIMIT_BY_BOROUGH: &str =
    api_path!("/pools/{pool_id}/borough-limits/{borough}");

/// Pool borough summary endpoint
pub const POOL_BOROUGH_SUMMARY: &str = api_path!("/pools/{pool_id}/borough-summary");

/// Pool borough validation endpoint
pub const POOL_BOROUGH_VALIDATION: &str = api_path!("/pools/{pool_id}/borough-validation");

/// Analytics market overview
pub const ANALYTICS_MARKET_OVERVIEW: &str = api_path!("/analytics/market-overview");

/// Analytics pool performance
pub const ANALYTICS_POOL_PERFORMANCE: &str = api_path!("/analytics/pool-performance");

// ============================================================================
// CREDIT LINE MANAGEMENT PATHS
// ============================================================================

/// Credit line requests collection
pub const CREDIT_LINES: &str = api_path!("/credit-lines");

/// Browse available pools for credit lines
pub const CREDIT_LINES_AVAILABLE_POOLS: &str = api_path!("/credit-lines/available-pools");

/// Check credit eligibility
pub const CREDIT_LINES_ELIGIBILITY: &str = api_path!("/credit-lines/eligibility");

/// Submit credit line request
pub const CREDIT_LINES_REQUEST: &str = api_path!("/credit-lines/request");

/// Submit credit line request (v2)
pub const CREDIT_LINES_REQUEST_V2: &str = api_path!("/credit-lines/request-v2");

/// List active credit lines with pool details
pub const CREDIT_LINES_ACTIVE: &str = api_path!("/credit-lines/active");

// ============================================================================
// FINANCIAL TRANSACTION PATHS
// ============================================================================

/// Account financial transactions for user
pub const USER_ACCOUNT_FINANCIAL: &str = api_path!("/user/{user_id}/financial/account");

/// Account deposit recording
pub const USER_ACCOUNT_FINANCIAL_DEPOSIT: &str =
    api_path!("/user/{user_id}/financial/account/deposit");

/// Account withdrawal recording
pub const USER_ACCOUNT_FINANCIAL_WITHDRAWAL: &str =
    api_path!("/user/{user_id}/financial/account/withdrawal");

/// Pool funding recording
pub const POOL_FINANCIAL_FUND: &str = api_path!("/pool/{pool_id}/financial/fund");

/// Pool withdrawal recording
pub const POOL_FINANCIAL_WITHDRAW: &str = api_path!("/pool/{pool_id}/financial/withdraw");

/// Pool financial transactions
pub const POOL_FINANCIAL_TRANSACTIONS: &str = api_path!("/pool/{pool_id}/financial/transactions");

/// Credit line draw recording
pub const CREDIT_LINE_FINANCIAL_DRAW: &str =
    api_path!("/credit-line/{credit_line_id}/financial/draw");

/// Credit line payment recording
pub const CREDIT_LINE_FINANCIAL_PAYMENT: &str =
    api_path!("/credit-line/{credit_line_id}/financial/payment");

/// Credit line financial transactions
pub const CREDIT_LINE_FINANCIAL_TRANSACTIONS: &str =
    api_path!("/credit-line/{credit_line_id}/financial/transactions");

/// User financial transactions (unified view)
pub const USER_FINANCIAL_TRANSACTIONS: &str = api_path!("/user/{user_id}/financial/transactions");

/// User financial summary
pub const USER_FINANCIAL_SUMMARY: &str = api_path!("/user/{user_id}/financial/summary");

/// User pool financial transactions
pub const USER_POOL_FINANCIAL: &str = api_path!("/user/{user_id}/financial/pools");

/// Merchant credit line financial transactions
pub const MERCHANT_CREDIT_LINE_FINANCIAL: &str =
    api_path!("/merchant/{merchant_id}/financial/credit-lines");

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
// CONTRACT CONFIGURATION PATHS
// ============================================================================

/// Get Arda contract configuration (address, network details)
pub const CONTRACT_CONFIG: &str = api_path!("/contract/config");

/// Get real-time transaction updates via Server-Sent Events (SSE)
pub const TRANSACTION_UPDATES_SSE: &str = api_path!("/transactions/updates");

// ============================================================================
// COMPANY MANAGEMENT PATHS
// ============================================================================

/// Company registration
pub const REGISTER_COMPANY: &str = api_path!("/companies/register");

/// Company details
pub const COMPANY_BY_ID: &str = api_path!("/companies/{company_id}");

/// Company dashboard
pub const COMPANY_DASHBOARD: &str = api_path!("/companies/{company_id}/dashboard");

/// KYB submission
pub const SUBMIT_KYB: &str = api_path!("/companies/{company_id}/kyb");

/// KYB status
pub const KYB_STATUS: &str = api_path!("/companies/{company_id}/kyb/status");

/// Team member invitation
pub const INVITE_TEAM_MEMBER: &str = api_path!("/companies/{company_id}/team/invite");

/// Team member list
pub const LIST_TEAM_MEMBERS: &str = api_path!("/companies/{company_id}/team");

/// Update team member role
pub const UPDATE_TEAM_MEMBER_ROLE: &str = api_path!("/companies/{company_id}/team/{user_id}/role");

/// Remove team member
pub const REMOVE_TEAM_MEMBER: &str = api_path!("/companies/{company_id}/team/{user_id}");

// ============================================================================
// DOCUMENT MANAGEMENT PATHS
// ============================================================================

/// Upload document to company
pub const DOCUMENT_UPLOAD: &str = api_path!("/companies/{company_id}/documents/upload");

/// Download document from company
pub const DOCUMENT_DOWNLOAD: &str =
    api_path!("/companies/{company_id}/documents/{document_id}/download");

/// Get document metadata
pub const DOCUMENT_BY_ID: &str = api_path!("/companies/{company_id}/documents/{document_id}");

/// List company documents
pub const DOCUMENTS: &str = api_path!("/companies/{company_id}/documents");

/// Approve document
pub const DOCUMENT_APPROVE: &str =
    api_path!("/companies/{company_id}/documents/{document_id}/approve");

/// Reject document
pub const DOCUMENT_REJECT: &str =
    api_path!("/companies/{company_id}/documents/{document_id}/reject");

/// Verify document integrity
pub const DOCUMENT_VERIFY: &str =
    api_path!("/companies/{company_id}/documents/{document_id}/verify");

/// Get document audit logs
pub const DOCUMENT_AUDIT: &str = api_path!("/companies/{company_id}/documents/audit");

/// Document service health check
pub const DOCUMENT_HEALTH: &str = api_path!("/documents/health");

// ============================================================================
// TRANSACTION HISTORY PATHS
// ============================================================================

/// Get comprehensive transaction history across all operations
pub const TRANSACTION_HISTORY: &str = api_path!("/transactions/history");

/// Get transaction history for a specific pool
pub const POOL_TRANSACTION_HISTORY: &str = api_path!("/pools/{pool_id}/transaction-history");

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

// Deal Management Helper Functions
/// Build deal path with deal_id parameter
pub fn deal_path(deal_id: &str) -> String {
    replace_param(DEAL_BY_ID, "deal_id", deal_id)
}

/// Build deal payments path with deal_id parameter
pub fn deal_payments_path(deal_id: &str) -> String {
    replace_param(DEAL_PAYMENTS, "deal_id", deal_id)
}

/// Build deals by user ID path with user_id parameter
pub fn deals_by_user_id_path(user_id: &str) -> String {
    replace_param(DEALS_BY_USER_ID, "user_id", user_id)
}

/// Build deal audit trail path with deal_id parameter
pub fn deal_audit_trail_path(deal_id: &str) -> String {
    replace_param(DEAL_AUDIT_TRAIL, "deal_id", deal_id)
}

/// Build deal lifecycle summary path with deal_id parameter
pub fn deal_lifecycle_summary_path(deal_id: &str) -> String {
    replace_param(DEAL_LIFECYCLE_SUMMARY, "deal_id", deal_id)
}

// State Management Helper Functions
/// Build batch path with batch_id parameter
pub fn batch_path(batch_id: i32) -> String {
    replace_param(STATE_BATCH_BY_ID, "batch_id", &batch_id.to_string())
}

/// Build deal batch path with batch_id parameter
pub fn deal_batch_path(batch_id: i32) -> String {
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

/// Build deal payment proof path with parameters
pub fn deal_payment_proof_path(
    user_id: &str,
    deal_id: &str,
    payment_id: &str,
    batch_id: i32,
) -> String {
    let path = replace_param(STATE_PROOFS_LOAN_PAYMENT, "user_id", user_id);
    let path = replace_param(&path, "deal_id", deal_id);
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

/// Build admin approve withdrawal path with request_id parameter
pub fn admin_approve_withdrawal_path(request_id: &str) -> String {
    replace_param(ADMIN_APPROVE_WITHDRAWAL, "request_id", request_id)
}

/// Build admin reject withdrawal path with request_id parameter
pub fn admin_reject_withdrawal_path(request_id: &str) -> String {
    replace_param(ADMIN_REJECT_WITHDRAWAL, "request_id", request_id)
}

/// Build admin execute withdrawal path with request_id parameter
pub fn admin_execute_withdrawal_path(request_id: &str) -> String {
    replace_param(ADMIN_EXECUTE_WITHDRAWAL, "request_id", request_id)
}

/// Generate transaction by hash path
pub fn transaction_by_hash_path(tx_hash: &str) -> String {
    replace_param(TRANSACTION_BY_HASH, "tx_hash", tx_hash)
}

// ============================================================================
// FINANCIAL TRANSACTION PATH HELPERS
// ============================================================================

/// Build user account financial path with user_id parameter
pub fn user_account_financial_path(user_id: &str) -> String {
    replace_param(USER_ACCOUNT_FINANCIAL, "user_id", user_id)
}

/// Build user account deposit path with user_id parameter
pub fn user_account_financial_deposit_path(user_id: &str) -> String {
    replace_param(USER_ACCOUNT_FINANCIAL_DEPOSIT, "user_id", user_id)
}

/// Build user account withdrawal path with user_id parameter
pub fn user_account_financial_withdrawal_path(user_id: &str) -> String {
    replace_param(USER_ACCOUNT_FINANCIAL_WITHDRAWAL, "user_id", user_id)
}

/// Build pool funding path with pool_id parameter
pub fn pool_financial_fund_path(pool_id: &str) -> String {
    replace_param(POOL_FINANCIAL_FUND, "pool_id", pool_id)
}

/// Build pool withdrawal path with pool_id parameter
pub fn pool_financial_withdraw_path(pool_id: &str) -> String {
    replace_param(POOL_FINANCIAL_WITHDRAW, "pool_id", pool_id)
}

/// Build pool financial transactions path with pool_id parameter
pub fn pool_financial_transactions_path(pool_id: &str) -> String {
    replace_param(POOL_FINANCIAL_TRANSACTIONS, "pool_id", pool_id)
}

// Pool Borough Management Helper Functions
/// Build pool borough limits path with pool_id parameter
pub fn pool_borough_limits_path(pool_id: &str) -> String {
    replace_param(POOL_BOROUGH_LIMITS, "pool_id", pool_id)
}

/// Build pool borough limit by borough path with pool_id and borough parameters
pub fn pool_borough_limit_by_borough_path(pool_id: &str, borough: &str) -> String {
    let path = replace_param(POOL_BOROUGH_LIMIT_BY_BOROUGH, "pool_id", pool_id);
    replace_param(&path, "borough", borough)
}

/// Build pool borough summary path with pool_id parameter
pub fn pool_borough_summary_path(pool_id: &str) -> String {
    replace_param(POOL_BOROUGH_SUMMARY, "pool_id", pool_id)
}

/// Build pool borough validation path with pool_id parameter
pub fn pool_borough_validation_path(pool_id: &str) -> String {
    replace_param(POOL_BOROUGH_VALIDATION, "pool_id", pool_id)
}

/// Build credit line draw path with credit_line_id parameter
pub fn credit_line_financial_draw_path(credit_line_id: &str) -> String {
    replace_param(CREDIT_LINE_FINANCIAL_DRAW, "credit_line_id", credit_line_id)
}

/// Build credit line payment path with credit_line_id parameter
pub fn credit_line_financial_payment_path(credit_line_id: &str) -> String {
    replace_param(
        CREDIT_LINE_FINANCIAL_PAYMENT,
        "credit_line_id",
        credit_line_id,
    )
}

/// Build credit line financial transactions path with credit_line_id parameter
pub fn credit_line_financial_transactions_path(credit_line_id: &str) -> String {
    replace_param(
        CREDIT_LINE_FINANCIAL_TRANSACTIONS,
        "credit_line_id",
        credit_line_id,
    )
}

/// Build user financial transactions path with user_id parameter
pub fn user_financial_transactions_path(user_id: &str) -> String {
    replace_param(USER_FINANCIAL_TRANSACTIONS, "user_id", user_id)
}

/// Build user financial summary path with user_id parameter
pub fn user_financial_summary_path(user_id: &str) -> String {
    replace_param(USER_FINANCIAL_SUMMARY, "user_id", user_id)
}

/// Build user pool financial path with user_id parameter
pub fn user_pool_financial_path(user_id: &str) -> String {
    replace_param(USER_POOL_FINANCIAL, "user_id", user_id)
}

/// Build merchant credit line financial path with merchant_id parameter
pub fn merchant_credit_line_financial_path(merchant_id: &str) -> String {
    replace_param(MERCHANT_CREDIT_LINE_FINANCIAL, "merchant_id", merchant_id)
}

// ============================================================================
// TRANSACTION HISTORY PATH HELPERS
// ============================================================================

/// Build pool transaction history path with pool_id parameter
pub fn pool_transaction_history_path(pool_id: &str) -> String {
    replace_param(POOL_TRANSACTION_HISTORY, "pool_id", pool_id)
}

// Document Management Helper Functions
/// Build document upload path with company_id parameter
pub fn document_upload_path(company_id: &str) -> String {
    replace_param(DOCUMENT_UPLOAD, "company_id", company_id)
}

/// Build document download path with company_id and document_id parameters
pub fn document_download_path(company_id: &str, document_id: &str) -> String {
    let path = replace_param(DOCUMENT_DOWNLOAD, "company_id", company_id);
    replace_param(&path, "document_id", document_id)
}

/// Build document by ID path with company_id and document_id parameters
pub fn document_by_id_path(company_id: &str, document_id: &str) -> String {
    let path = replace_param(DOCUMENT_BY_ID, "company_id", company_id);
    replace_param(&path, "document_id", document_id)
}

/// Build documents path with company_id parameter
pub fn documents_path(company_id: &str) -> String {
    replace_param(DOCUMENTS, "company_id", company_id)
}

/// Build document approve path with company_id and document_id parameters
pub fn document_approve_path(company_id: &str, document_id: &str) -> String {
    let path = replace_param(DOCUMENT_APPROVE, "company_id", company_id);
    replace_param(&path, "document_id", document_id)
}

/// Build document reject path with company_id and document_id parameters
pub fn document_reject_path(company_id: &str, document_id: &str) -> String {
    let path = replace_param(DOCUMENT_REJECT, "company_id", company_id);
    replace_param(&path, "document_id", document_id)
}

/// Build document verify path with company_id and document_id parameters
pub fn document_verify_path(company_id: &str, document_id: &str) -> String {
    let path = replace_param(DOCUMENT_VERIFY, "company_id", company_id);
    replace_param(&path, "document_id", document_id)
}

/// Build document audit path with company_id parameter
pub fn document_audit_path(company_id: &str) -> String {
    replace_param(DOCUMENT_AUDIT, "company_id", company_id)
}

// ============================================================================
// PATH LISTS FOR DOCUMENTATION
// ============================================================================

/// All endpoint information for API documentation
pub fn get_all_endpoints() -> Vec<crate::health_handlers::EndpointInfo> {
    vec![
        // Core API Endpoints
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: API_HEALTH.to_string(),
            description: "Health check".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: API_INFO.to_string(),
            description: "API information".to_string(),
        },
        // User Management
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USERS.to_string(),
            description: "List user identities with filters".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: USERS.to_string(),
            description: "Register new user identity".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USER_BY_ID.to_string(),
            description: "Get user details by ID".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: USER_KYC.to_string(),
            description: "Submit KYC information".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: USER_KYC_STATUS.to_string(),
            description: "Update KYC status (admin only)".to_string(),
        },
        // Admin KYC Management
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: ADMIN_KYC_PENDING.to_string(),
            description: "List pending KYC applications (admin only)".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: ADMIN_KYC_STATS.to_string(),
            description: "Get KYC statistics (admin only)".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: ADMIN_KYC_BULK_APPROVE.to_string(),
            description: "Bulk approve KYC applications (admin only)".to_string(),
        },
        // Invite Code Management
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: INVITE_CODE_GENERATE.to_string(),
            description: "Generate new invite code".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: INVITE_CODE_VALIDATE.to_string(),
            description: "Validate invite code (doesn't mark as used)".to_string(),
        },
        // Loan Management
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEALS.to_string(),
            description: "List loans with filters".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEALS_BY_USER_ID.to_string(),
            description: "Get all deals for a user (as originator or investor)".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: DEAL.to_string(),
            description: "Create new loan".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEAL_BY_ID.to_string(),
            description: "Get loan details by ID".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "DELETE".to_string(),
            path: DEAL_BY_ID.to_string(),
            description: "Close loan by ID".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEAL_PAYMENTS.to_string(),
            description: "Get loan payment history".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEALS_STATS.to_string(),
            description: "Get loan statistics".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEAL_AUDIT_TRAIL.to_string(),
            description: "Get loan audit trail".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DEAL_LIFECYCLE_SUMMARY.to_string(),
            description: "Get loan lifecycle summary".to_string(),
        },
        // State Management - Core State
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_CURRENT.to_string(),
            description: "Get current system state".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_SUMMARY.to_string(),
            description: "Get comprehensive state summary with loan and user stats".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_HISTORY.to_string(),
            description: "Get state transition history".to_string(),
        },
        // State Management - Transactions
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_TRANSACTIONS.to_string(),
            description: "Submit a new transaction".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_TRANSACTIONS_PENDING.to_string(),
            description: "Get pending transactions".to_string(),
        },
        // State Management - Batches
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCHES.to_string(),
            description: "List batches".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCHES.to_string(),
            description: "Create new batch from pending transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCH_BY_ID.to_string(),
            description: "Get batch details by ID".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCH_PROOF.to_string(),
            description: "Update batch with ZK proof".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCHES_TRIGGER.to_string(),
            description: "Manually trigger batch processing".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCHES_STATS.to_string(),
            description: "Get batch processor statistics".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_BATCH_CREATE.to_string(),
            description: "Create batch from loan transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_LOAN_BATCH_BY_ID.to_string(),
            description: "Get loan batch details by ID".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_BATCH_PENDING.to_string(),
            description: "Get pending loan transactions ready for batching".to_string(),
        },
        // State Management - Payments
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PAYMENTS.to_string(),
            description: "Record loan payment".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PAYMENT_BY_ID.to_string(),
            description: "Get payment details".to_string(),
        },
        // State Management - Proofs
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PROOF.to_string(),
            description: "Generate transaction inclusion proof".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_VERIFY.to_string(),
            description: "Verify transaction inclusion".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_LOAN_OPERATION.to_string(),
            description: "Generate loan operation proof".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: STATE_PROOFS_LOAN_PAYMENT.to_string(),
            description: "Generate loan payment proof".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_VERIFY.to_string(),
            description: "Verify loan proof".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_REPORT.to_string(),
            description: "Generate proof report".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: STATE_PROOFS_BATCH_VERIFY.to_string(),
            description: "Batch verify proofs".to_string(),
        },
        // Financial Transaction Endpoints
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: USER_ACCOUNT_FINANCIAL_DEPOSIT.to_string(),
            description: "Record account deposit from smart contract".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: USER_ACCOUNT_FINANCIAL_WITHDRAWAL.to_string(),
            description: "Record account withdrawal from smart contract".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USER_ACCOUNT_FINANCIAL.to_string(),
            description: "Get user's account financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: POOL_FINANCIAL_FUND.to_string(),
            description: "Record pool funding transaction".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: POOL_FINANCIAL_WITHDRAW.to_string(),
            description: "Record pool withdrawal transaction".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: POOL_FINANCIAL_TRANSACTIONS.to_string(),
            description: "Get pool financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USER_POOL_FINANCIAL.to_string(),
            description: "Get user's pool financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: CREDIT_LINE_FINANCIAL_DRAW.to_string(),
            description: "Record credit line draw transaction".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: CREDIT_LINE_FINANCIAL_PAYMENT.to_string(),
            description: "Record credit line payment transaction".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: CREDIT_LINE_FINANCIAL_TRANSACTIONS.to_string(),
            description: "Get credit line financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: MERCHANT_CREDIT_LINE_FINANCIAL.to_string(),
            description: "Get merchant's credit line financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USER_FINANCIAL_TRANSACTIONS.to_string(),
            description: "Get user's unified financial transactions".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: USER_FINANCIAL_SUMMARY.to_string(),
            description: "Get user's financial summary".to_string(),
        },
        // Pool Borough Limit Endpoints
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: POOL_BOROUGH_LIMITS.to_string(),
            description: "Get borough limits for a pool".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "PUT".to_string(),
            path: POOL_BOROUGH_LIMITS.to_string(),
            description: "Set borough limit for a pool".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "DELETE".to_string(),
            path: POOL_BOROUGH_LIMIT_BY_BOROUGH.to_string(),
            description: "Delete borough limit for a pool".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: POOL_BOROUGH_SUMMARY.to_string(),
            description: "Get borough summary for a pool".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: POOL_BOROUGH_VALIDATION.to_string(),
            description: "Validate borough limits for a pool".to_string(),
        },
        // Investor Query Endpoints
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: INVESTOR_DASHBOARD.to_string(),
            description: "Get investor dashboard data".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: INVESTOR_POOL_SUMMARIES.to_string(),
            description: "Get investor pool summaries".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: INVESTOR_POOL_METRICS.to_string(),
            description: "Get investor pool metrics".to_string(),
        },
        // Analytics Endpoints
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: ANALYTICS_MARKET_OVERVIEW.to_string(),
            description: "Get market overview analytics".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: ANALYTICS_POOL_PERFORMANCE.to_string(),
            description: "Get pool performance analytics".to_string(),
        },
        // Authentication Endpoints
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: AUTH_LOGIN.to_string(),
            description: "User login authentication".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: AUTH_LOGOUT.to_string(),
            description: "User logout authentication".to_string(),
        },
        // Contract Configuration Endpoints
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: CONTRACT_CONFIG.to_string(),
            description: "Get Arda contract configuration (address, network details)".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: TRANSACTION_UPDATES_SSE.to_string(),
            description: "Get real-time transaction updates via Server-Sent Events (SSE)"
                .to_string(),
        },
        // Document Management Endpoints
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: DOCUMENT_UPLOAD.to_string(),
            description: "Upload document to company".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DOCUMENT_DOWNLOAD.to_string(),
            description: "Download document from company".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DOCUMENT_BY_ID.to_string(),
            description: "Get document metadata".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "DELETE".to_string(),
            path: DOCUMENT_BY_ID.to_string(),
            description: "Delete document from company".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DOCUMENTS.to_string(),
            description: "List company documents".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: DOCUMENT_APPROVE.to_string(),
            description: "Approve document".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: DOCUMENT_REJECT.to_string(),
            description: "Reject document".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "POST".to_string(),
            path: DOCUMENT_VERIFY.to_string(),
            description: "Verify document integrity".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DOCUMENT_AUDIT.to_string(),
            description: "Get document audit logs".to_string(),
        },
        crate::health_handlers::EndpointInfo {
            method: "GET".to_string(),
            path: DOCUMENT_HEALTH.to_string(),
            description: "Document service health check".to_string(),
        },
    ]
}
