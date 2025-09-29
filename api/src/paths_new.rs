//! Reorganized API path constants
//!
//! This module provides a restructured API with logical endpoint grouping:
//! - /auth/* (authentication and authorization)
//! - /users/* (user profile management)
//! - /companies/* (company management and team operations)
//! - /deals/* (deal CRUD, marketplace, transfers)
//! - /state/* (ZK proof system and state transitions)
//! - /admin/* (platform administration)
//! - /analytics/* (reports and metrics)
//! - /payments/* (balances, transactions, withdrawals)

/// Current API version
pub const API_VERSION: &str = "v1";

/// Build API paths with current version
macro_rules! api_path {
    ($path:expr) => {
        concat!("/api/v1", $path)
    };
}

// ============================================================================
// CORE SYSTEM PATHS
// ============================================================================

/// Root path
pub const ROOT: &str = "/";

/// Health check
pub const HEALTH: &str = "/health";

/// API health check
pub const API_HEALTH: &str = api_path!("/health");

/// API info
pub const API_INFO: &str = api_path!("/info");

// ============================================================================
// AUTHENTICATION & AUTHORIZATION (/auth/*)
// ============================================================================

/// Magic link request
pub const AUTH_MAGIC_LINK: &str = api_path!("/auth/magic-link");

/// Magic link verification
pub const AUTH_VERIFY: &str = api_path!("/auth/verify");

/// Get current authenticated user
pub const AUTH_ME: &str = api_path!("/auth/me");

/// Logout
pub const AUTH_LOGOUT: &str = api_path!("/auth/logout");

/// Generate invite code
pub const AUTH_INVITE_GENERATE: &str = api_path!("/auth/invite/generate");

/// Validate invite code
pub const AUTH_INVITE_VALIDATE: &str = api_path!("/auth/invite/validate");

// ============================================================================
// USER MANAGEMENT (/users/*)
// ============================================================================

/// User collection - list users
pub const USERS: &str = api_path!("/users");

/// Create new user
pub const USERS_CREATE: &str = api_path!("/users");

/// Get user details
pub const USER_BY_ID: &str = api_path!("/users/{user_id}");

/// Update user
pub const USER_UPDATE: &str = api_path!("/users/{user_id}");

/// Delete user (soft delete)
pub const USER_DELETE: &str = api_path!("/users/{user_id}");

/// Submit KYC for user
pub const USER_KYC_SUBMIT: &str = api_path!("/users/{user_id}/kyc");

/// Update KYC status (admin only)
pub const USER_KYC_STATUS: &str = api_path!("/users/{user_id}/kyc/status");

/// User balance by ID
pub const USER_BALANCE: &str = api_path!("/users/{user_id}/balance");

/// User transaction history
pub const USER_TRANSACTIONS: &str = api_path!("/users/{user_id}/transactions");

// ============================================================================
// COMPANY MANAGEMENT (/companies/*)
// ============================================================================

/// Company collection - list companies
pub const COMPANIES: &str = api_path!("/companies");

/// Create new company
pub const COMPANIES_CREATE: &str = api_path!("/companies");

/// Get company details
pub const COMPANY_BY_ID: &str = api_path!("/companies/{company_id}");

/// Update company
pub const COMPANY_UPDATE: &str = api_path!("/companies/{company_id}");

/// Delete company
pub const COMPANY_DELETE: &str = api_path!("/companies/{company_id}");

/// Company dashboard
pub const COMPANY_DASHBOARD: &str = api_path!("/companies/{company_id}/dashboard");

/// Submit KYB for company
pub const COMPANY_KYB_SUBMIT: &str = api_path!("/companies/{company_id}/kyb");

/// Update KYB status (admin only)
pub const COMPANY_KYB_STATUS: &str = api_path!("/companies/{company_id}/kyb/status");

/// Company team members
pub const COMPANY_MEMBERS: &str = api_path!("/companies/{company_id}/members");

/// Add team member
pub const COMPANY_MEMBERS_ADD: &str = api_path!("/companies/{company_id}/members");

/// Get team member details
pub const COMPANY_MEMBER_BY_ID: &str = api_path!("/companies/{company_id}/members/{user_id}");

/// Update team member role/permissions
pub const COMPANY_MEMBER_UPDATE: &str = api_path!("/companies/{company_id}/members/{user_id}");

/// Remove team member
pub const COMPANY_MEMBER_DELETE: &str = api_path!("/companies/{company_id}/members/{user_id}");

/// Invite team member
pub const COMPANY_INVITE_MEMBER: &str = api_path!("/companies/{company_id}/invites");

/// Company documents
pub const COMPANY_DOCUMENTS: &str = api_path!("/companies/{company_id}/documents");

// ============================================================================
// DEAL MANAGEMENT (/deals/*)
// ============================================================================

/// Deal collection - list deals
pub const DEALS: &str = api_path!("/deals");

/// Create new deal
pub const DEALS_CREATE: &str = api_path!("/deals");

/// Get deal details
pub const DEAL_BY_ID: &str = api_path!("/deals/{deal_id}");

/// Update deal
pub const DEAL_UPDATE: &str = api_path!("/deals/{deal_id}");

/// Delete deal
pub const DEAL_DELETE: &str = api_path!("/deals/{deal_id}");

/// Deal statistics
pub const DEALS_STATS: &str = api_path!("/deals/stats");

/// Deals for sale (marketplace)
pub const DEALS_MARKETPLACE: &str = api_path!("/deals/marketplace");

/// List deal for sale (POST to create listing)
pub const DEAL_LIST_FOR_SALE: &str = api_path!("/deals/{deal_id}/listings");

/// Remove deal from sale (DELETE listing)
pub const DEAL_REMOVE_FROM_SALE: &str = api_path!("/deals/{deal_id}/listings");

/// Deal payments
pub const DEAL_PAYMENTS: &str = api_path!("/deals/{deal_id}/payments");

/// Record deal payment
pub const DEAL_PAYMENTS_CREATE: &str = api_path!("/deals/{deal_id}/payments");

/// Get payment details
pub const DEAL_PAYMENT_BY_ID: &str = api_path!("/deals/{deal_id}/payments/{payment_id}");

/// Transfer deal ownership (POST to create transfer)
pub const DEAL_TRANSFER: &str = api_path!("/deals/{deal_id}/transfers");

/// Deal transfer history (GET transfers collection)
pub const DEAL_TRANSFERS: &str = api_path!("/deals/{deal_id}/transfers");

/// Validate deal transfer (POST to /transfers/validation)
pub const DEAL_TRANSFER_VALIDATE: &str = api_path!("/deals/{deal_id}/transfers/validation");

// ============================================================================
// STATE MANAGEMENT (/state/*)
// ============================================================================

/// Submit transaction to queue
pub const STATE_TRANSACTIONS: &str = api_path!("/state/transactions");

/// Get pending transactions
pub const STATE_TRANSACTIONS_PENDING: &str = api_path!("/state/transactions/pending");

/// Batch operations
pub const STATE_BATCHES: &str = api_path!("/state/batches");

/// Create new batch
pub const STATE_BATCHES_CREATE: &str = api_path!("/state/batches");

/// Get batch details
pub const STATE_BATCH_BY_ID: &str = api_path!("/state/batches/{batch_id}");

/// Update batch proof
pub const STATE_BATCH_PROOF: &str = api_path!("/state/batches/{batch_id}/proof");

/// Trigger batch processing
pub const STATE_BATCHES_TRIGGER: &str = api_path!("/state/batches/trigger");

/// Get current state
pub const STATE_CURRENT: &str = api_path!("/state/current");

/// Get contract submission data
pub const STATE_CONTRACT: &str = api_path!("/state/contract");

/// Contract configuration
pub const STATE_CONTRACT_CONFIG: &str = api_path!("/state/contract/config");

/// Transaction updates SSE
pub const STATE_TRANSACTION_UPDATES: &str = api_path!("/state/transactions/updates");

// ============================================================================
// ADMIN OPERATIONS (/admin/*)
// ============================================================================

/// KYC management - pending applications
pub const ADMIN_KYC_PENDING: &str = api_path!("/admin/kyc/pending");

/// KYC statistics
pub const ADMIN_KYC_STATS: &str = api_path!("/admin/kyc/stats");

/// Bulk KYC approval (POST to /admin/kyc/actions with action=bulk-approve)
pub const ADMIN_KYC_BULK_APPROVE: &str = api_path!("/admin/kyc/actions");

/// Platform metrics
pub const ADMIN_PLATFORM_METRICS: &str = api_path!("/admin/platform/metrics");

/// Withdrawal management - pending withdrawals
pub const ADMIN_WITHDRAWALS_PENDING: &str = api_path!("/admin/withdrawals?status=pending");

/// Approve withdrawal (PATCH to update status)
pub const ADMIN_WITHDRAWAL_APPROVE: &str = api_path!("/admin/withdrawals/{request_id}");

/// Reject withdrawal (PATCH to update status)
pub const ADMIN_WITHDRAWAL_REJECT: &str = api_path!("/admin/withdrawals/{request_id}");

/// Execute withdrawal manually (POST to /admin/withdrawals/{request_id}/actions)
pub const ADMIN_WITHDRAWAL_EXECUTE: &str = api_path!("/admin/withdrawals/{request_id}/actions");

/// Withdrawal dashboard
pub const ADMIN_WITHDRAWAL_DASHBOARD: &str = api_path!("/admin/withdrawals/dashboard");

/// System-wide user management
pub const ADMIN_USERS: &str = api_path!("/admin/users");

/// System-wide company management
pub const ADMIN_COMPANIES: &str = api_path!("/admin/companies");

// ============================================================================
// ANALYTICS & REPORTING (/analytics/*)
// ============================================================================

/// Market overview analytics
pub const ANALYTICS_MARKET: &str = api_path!("/analytics/market");

/// Pool performance analytics
pub const ANALYTICS_POOLS: &str = api_path!("/analytics/pools");

/// Deal analytics
pub const ANALYTICS_DEALS: &str = api_path!("/analytics/deals");

/// Transaction history analytics
pub const ANALYTICS_TRANSACTIONS: &str = api_path!("/analytics/transactions");

/// User metrics
pub const ANALYTICS_USERS: &str = api_path!("/analytics/users");

/// Company metrics
pub const ANALYTICS_COMPANIES: &str = api_path!("/analytics/companies");

/// Investor dashboard
pub const ANALYTICS_INVESTOR_DASHBOARD: &str = api_path!("/analytics/investor/dashboard");

/// Investor pool summaries
pub const ANALYTICS_INVESTOR_POOLS: &str = api_path!("/analytics/investor/pools");

/// Investor metrics
pub const ANALYTICS_INVESTOR_METRICS: &str = api_path!("/analytics/investor/metrics");

// ============================================================================
// PAYMENTS & FINANCIAL OPERATIONS (/payments/*)
// ============================================================================

/// User balances
pub const PAYMENTS_BALANCES: &str = api_path!("/payments/balances");

/// User balance by address
pub const PAYMENTS_BALANCE_BY_ADDRESS: &str = api_path!("/payments/balances/{address}");

/// Available balance for withdrawals
pub const PAYMENTS_BALANCE_AVAILABLE: &str = api_path!("/payments/balances/{address}/available");

/// USDC withdrawals
pub const PAYMENTS_WITHDRAWALS: &str = api_path!("/payments/withdrawals");

/// Create withdrawal request
pub const PAYMENTS_WITHDRAWALS_CREATE: &str = api_path!("/payments/withdrawals");

/// Get withdrawal status
pub const PAYMENTS_WITHDRAWAL_BY_ID: &str = api_path!("/payments/withdrawals/{request_id}");

/// Cancel withdrawal
pub const PAYMENTS_WITHDRAWAL_CANCEL: &str = api_path!("/payments/withdrawals/{request_id}/cancel");

/// User withdrawal history
pub const PAYMENTS_WITHDRAWAL_HISTORY: &str = api_path!("/payments/withdrawals/history/{address}");

/// Financial transactions
pub const PAYMENTS_TRANSACTIONS: &str = api_path!("/payments/transactions");

/// Account deposits
pub const PAYMENTS_DEPOSITS: &str = api_path!("/payments/deposits");

/// Account financial transactions
pub const PAYMENTS_ACCOUNT_TRANSACTIONS: &str = api_path!("/payments/accounts/{address}/transactions");

/// Pool financial operations
pub const PAYMENTS_POOL_FUND: &str = api_path!("/payments/pools/{pool_id}/fund");

/// Pool withdrawal
pub const PAYMENTS_POOL_WITHDRAW: &str = api_path!("/payments/pools/{pool_id}/withdraw");

/// Pool transactions
pub const PAYMENTS_POOL_TRANSACTIONS: &str = api_path!("/payments/pools/{pool_id}/transactions");

/// User pool transactions
pub const PAYMENTS_USER_POOL_TRANSACTIONS: &str = api_path!("/payments/users/{user_id}/pool-transactions");

/// Credit line draws
pub const PAYMENTS_CREDIT_LINE_DRAW: &str = api_path!("/payments/credit-lines/{credit_line_id}/draw");

/// Credit line payments
pub const PAYMENTS_CREDIT_LINE_PAYMENT: &str = api_path!("/payments/credit-lines/{credit_line_id}/payment");

/// Credit line transactions
pub const PAYMENTS_CREDIT_LINE_TRANSACTIONS: &str = api_path!("/payments/credit-lines/{credit_line_id}/transactions");

/// User financial summary
pub const PAYMENTS_USER_SUMMARY: &str = api_path!("/payments/users/{user_id}/summary");

/// User all financial transactions
pub const PAYMENTS_USER_TRANSACTIONS: &str = api_path!("/payments/users/{user_id}/transactions");

// ============================================================================
// DEPRECATED/LEGACY PATHS (for backward compatibility)
// ============================================================================

/// Legacy user endpoint (should use /users)
pub const USER_LEGACY: &str = api_path!("/user");

/// Legacy deal endpoints
pub const DEAL_LEGACY: &str = api_path!("/deal");

/// Legacy investor registration (should use /companies)
pub const REGISTER_INVESTOR_LEGACY: &str = api_path!("/auth/register/investor");

/// Legacy merchant registration (should use /companies)
pub const REGISTER_MERCHANT_LEGACY: &str = api_path!("/auth/register/merchant");

// ============================================================================
// LIQUIDITY POOL PATHS (specialized financial instruments)
// ============================================================================

/// Pool collection
pub const POOLS: &str = api_path!("/pools");

/// Create pool
pub const POOLS_CREATE: &str = api_path!("/pools");

/// Pool details
pub const POOL_BY_ID: &str = api_path!("/pools/{pool_id}");

/// Update pool status
pub const POOL_UPDATE: &str = api_path!("/pools/{pool_id}");

/// Pool utilization metrics
pub const POOL_UTILIZATION: &str = api_path!("/pools/{pool_id}/utilization");

/// Pool transaction history
pub const POOL_TRANSACTIONS: &str = api_path!("/pools/{pool_id}/transactions");

/// Pool borough limits
pub const POOL_BOROUGH_LIMITS: &str = api_path!("/pools/{pool_id}/borough-limits");

/// Pool borough limit by borough
pub const POOL_BOROUGH_LIMIT_BY_BOROUGH: &str = api_path!("/pools/{pool_id}/borough-limits/{borough}");

/// Pool borough summary
pub const POOL_BOROUGH_SUMMARY: &str = api_path!("/pools/{pool_id}/borough-summary");

/// Pool borough validation
pub const POOL_BOROUGH_VALIDATION: &str = api_path!("/pools/{pool_id}/borough-validation");

/// Pool metrics overview
pub const POOL_METRICS_OVERVIEW: &str = api_path!("/pools/metrics/overview");

/// Investor pool metrics
pub const POOL_METRICS_INVESTOR: &str = api_path!("/pools/metrics/investor");

// ============================================================================
// CREDIT LINE PATHS (merchant financing)
// ============================================================================

/// Credit line requests
pub const CREDIT_LINES: &str = api_path!("/credit-lines");

/// Submit credit line request
pub const CREDIT_LINES_CREATE: &str = api_path!("/credit-lines");

/// Available pools for credit lines
pub const CREDIT_LINES_POOLS: &str = api_path!("/credit-lines/available-pools");

/// Check credit eligibility
pub const CREDIT_LINES_ELIGIBILITY: &str = api_path!("/credit-lines/eligibility");

/// Active credit lines
pub const CREDIT_LINES_ACTIVE: &str = api_path!("/credit-lines/active");

// ============================================================================
// MERCHANT TURNOVER PATHS (merchant-specific operations)
// ============================================================================

/// Merchant turnover data
pub const MERCHANT_TURNOVER: &str = api_path!("/merchant/turnover");

/// Merchant turnover by date
pub const MERCHANT_TURNOVER_BY_DATE: &str = api_path!("/merchant/turnover/{date}");

/// Merchant turnover analytics
pub const MERCHANT_TURNOVER_ANALYTICS: &str = api_path!("/merchant/turnover/analytics");

/// Merchant profile completion
pub const MERCHANT_PROFILE_COMPLETION: &str = api_path!("/merchant/profile/completion");