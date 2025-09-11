use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use humantime::format_duration;

use serde::{Deserialize, Serialize};
use sqlx::{Error as SqlxError, PgPool};
use std::str::FromStr;
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, OpenApi, ToSchema};

// OpenTelemetry imports
use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer},
    Context, KeyValue,
};
use utoipa_swagger_ui::SwaggerUi;
use uuid;

use crate::auth::AuthService;
use crate::admin_handlers::{
    KycStatisticsResponse, ListPendingKycQuery, ListPendingKycResponse,
};
use crate::authentication_handlers::{
    GenerateInviteCodeRequest, GenerateInviteCodeResponse, InviteCodeResponse, LoginRequest,
    LoginResponse, LogoutRequest, LogoutResponse, UserSummary, ValidateInviteCodeResponse,
};
use crate::batch_processor::BatchProcessorHandle;
// Credit line handlers
use crate::credit_line_handlers::{
    browse_available_pools, check_credit_eligibility, list_credit_line_requests,
    submit_credit_line_request,
};
// Financial transaction handlers
use crate::financial_transaction_handlers::{
    get_account_financial_transactions,
    get_credit_line_financial_transactions,
    get_merchant_credit_line_financial_transactions,
    get_pool_financial_transactions,
    get_user_financial_summary,
    get_user_financial_transactions,
    get_user_pool_financial_transactions,
    record_account_deposit,
    record_account_withdrawal,
    record_credit_line_draw,
    record_credit_line_payment,
    record_pool_funding,
    record_pool_withdrawal,

    // Types for OpenAPI documentation
    AccountFinancialTransactionResponse,
    CreateAccountFinancialTransactionRequest,
    CreateCreditLineFinancialTransactionRequest,
    CreatePoolFinancialTransactionRequest,
    CreditLineFinancialTransactionResponse,
    FinancialSummaryResponse,
    PaginatedFinancialTransactionsResponse,
    PoolFinancialTransactionResponse,
    UnifiedFinancialTransactionResponse,
};
use crate::paths;
use crate::pool_handlers::{
    create_pool, get_investor_dashboard, get_investor_pool_summaries, get_market_overview,
    get_pool_details, get_pool_performance, get_pool_transactions, get_pool_utilization,
    list_pools, update_pool_status,
};
use crate::user_handlers::{
    KycData, RegisterUserRequest, RegisterUserResponse, SubmitKycRequest,
    SubmitKycResponse, UpdateKycStatusRequest, UserDetailsResponse,
};
use crate::merchant_handlers::{
    InvestorProfileQuery, InvestorProfileResponse, MerchantProfileResponse,
    RegisterInvestorRequest, RegisterInvestorResponse, RegisterMerchantRequest,
    RegisterMerchantResponse, UpdateInvestorProfileRequest, UpdateMerchantProfileRequest,
};
use crate::{BalanceVerificationService, WithdrawalTransactionService};
use arda_credit_db::{
    create_investor_profile,
    create_liquidity_pool,
    create_loan,
    create_merchant,
    // Merchant turnover types
    create_or_update_turnover,
    find_matching_liquidity_pools,
    generate_invite_code,
    get_all_batches,
    get_batch_by_id,
    get_contract_submission_data,
    get_current_state,
    get_deposit_by_tx_hash,
    get_deposits_by_user_address,
    get_identity_by_email,
    get_identity_by_id,
    // get_investor_profile_by_user_id, // unused
    get_investor_profile_by_user_id_or_email,
    get_investor_profile_with_identity,
    get_invite_code_info,
    get_kyc_statistics,
    get_liquidity_pool_by_id,
    get_liquidity_pools_by_investor,
    get_loan_by_id,
    get_merchant_by_user_id,
    get_merchant_profile_by_merchant_id,
    get_merchant_profile_completion,
    get_merchant_turnover,
    get_merchant_turnover_analytics,
    get_pending_transactions,
    get_user_balance_by_address,
    get_user_balance_by_id,
    list_loans,
    list_pending_kyc_applications,
    register_or_reregister_user,
    store_ads_state_commit,
    submit_kyc_data,
    submit_transaction,
    update_batch_proof,
    update_identity_profile,
    update_investor_profile,
    update_kyc_status,
    update_last_login,
    update_liquidity_pool_status,
    update_loan,
    update_merchant_profile,
    update_turnover,
    use_invite_code,
    AuditEvent,
    CreateInvestorProfileRequest,
    CreateLiquidityPoolRequest,
    CreateMerchantRequest,
    CreateTurnoverRequest,
    Identity,
    IndexedMerkleTreeADS,
    // InvestorProfile, // unused
    LoanInclusionProof,
    LoanLifecycleSummary,
    // Merchant, // unused
    // MerchantProfile, // unused
    MerchantProfileCompletion,
    MerchantTurnoverAnalytics,
    MerchantTurnoverDaily,
    PoolMatchingCriteria,
    RegisterResult,
    UpdateInvestorProfileRequest as DbUpdateInvestorProfileRequest,
    UpdateMerchantProfileRequest as DbUpdateMerchantProfileRequest,
    UpdateTurnoverRequest,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to parse UUID from string
fn parse_user_id(user_id: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(user_id).map_err(|_| format!("Invalid user ID format: {}", user_id))
}

/// Extract authenticated user info from request headers (set by auth middleware)
#[derive(Debug)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub user_type: String,
}

/// Convert database Identity to API UserDetailsResponse
fn identity_to_user_details(identity: Identity) -> UserDetailsResponse {
    UserDetailsResponse {
        user_id: identity.id.to_string(),
        email: identity.email,
        user_type: identity.user_type.parse().unwrap_or(UserType::User),
        kyc_status: identity.kyc_status,
        kyc_submitted_at: identity.kyc_submitted_at,
        kyc_verified_at: identity.kyc_verified_at,
        created_at: identity.created_at,
        updated_at: Some(identity.updated_at),
        first_name: identity.first_name,
        last_name: identity.last_name,
        jurisdiction: identity.jurisdiction,
        wallet_address: identity.wallet_address,
    }
}


// ============================================================================
// API STATE
// ============================================================================

/// API state containing the database pool, configuration, and ADS service
#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub config: ApiConfig,
    pub batch_processor: Option<BatchProcessorHandle>,
    pub ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
    pub auth_service: Arc<AuthService>,
    pub withdrawal_service: WithdrawalTransactionService,
    pub balance_service: BalanceVerificationService,
}

/// Configuration for API server
#[derive(Debug, Clone, ToSchema)]
pub struct ApiConfig {
    pub server_name: String,
    pub version: String,
    pub max_batch_size: u32,
    pub enable_debug_endpoints: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            server_name: "Batch Processing API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 50,
            enable_debug_endpoints: false,
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE MODELS
// ============================================================================

/// Request to submit a single transaction
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitTransactionRequest {
    pub amount: i32,
}

/// Response from transaction submission
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitTransactionResponse {
    pub transaction_id: i32,
    pub amount: i32,
    pub status: String, // "pending"
    pub created_at: DateTime<Utc>,
}

/// Request to create a batch
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBatchRequest {
    pub batch_size: Option<i32>,
}

/// Response from batch creation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBatchResponse {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub created_at: DateTime<Utc>,
}

/// Response for pending transactions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingTransactionsResponse {
    pub transactions: Vec<TransactionInfo>,
    pub total_count: usize,
    pub total_amount: i32,
}

/// Transaction info for API responses
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    pub id: i32,
    pub amount: i32,
    pub created_at: DateTime<Utc>,
}

/// Response for batch listing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchListResponse {
    pub batches: Vec<BatchInfo>,
    pub total_count: usize,
}

/// Batch info for API responses
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchInfo {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub sindri_proof_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub proven_at: Option<DateTime<Utc>>,
}

/// Response for current state
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CurrentStateResponse {
    pub counter_value: i64,
    pub has_merkle_root: bool,
    pub last_batch_id: Option<i32>,
    pub last_proven_batch_id: Option<i32>,
}

/// Request to update batch with proof
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateBatchProofRequest {
    pub sindri_proof_id: String,
    pub status: String,              // "proven", "failed"
    pub merkle_root: Option<String>, // hex encoded
}

/// Query parameters for batch listing
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchListQuery {
    pub limit: Option<i32>,
}

/// Contract submission data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractSubmissionData {
    pub public: ContractPublicData,
    pub private: ContractPrivateData,
}

/// Contract public data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractPublicData {
    pub prev_merkle_root: String,
    pub new_merkle_root: String,
    pub zk_proof: String,
}

/// Contract private data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractPrivateData {
    pub prev_counter_value: i64,
    pub new_counter_value: i64,
    pub transactions: Vec<i32>,
}

/// API information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiInfoResponse {
    pub server_name: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub description: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub database_connected: bool,
    pub api_version: String,
    pub git_tag: String,
    pub git_sha: String,
    pub git_sha_short: String,
    pub git_dirty: bool,
}

/// Batch processor stats response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchProcessorStatsResponse {
    pub total_batches_created: u64,
    pub total_transactions_processed: u64,
    pub timer_triggers: u64,
    pub count_triggers: u64,
    pub manual_triggers: u64,
    pub errors: u64,
    pub last_batch_time: Option<String>,
}

/// Manual batch trigger response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TriggerBatchResponse {
    pub triggered: bool,
    pub message: String,
}

// ============================================================================
// LOAN PROCESSING MODELS
// ============================================================================

/// User type enumeration for allowed user types
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    User,
    Investor,
    Originator,
    Admin,
    Merchant,
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Investor => write!(f, "investor"),
            Self::Originator => write!(f, "originator"),
            Self::Admin => write!(f, "admin"),
            Self::Merchant => write!(f, "merchant"),
        }
    }
}

impl std::str::FromStr for UserType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "investor" => Ok(Self::Investor),
            "originator" => Ok(Self::Originator),
            "merchant" => Ok(Self::Merchant),
            "admin" => Ok(Self::Admin),
            _ => Err(format!("Invalid user type: {}", s)),
        }
    }
}

/// Request to register a new user identity
#[derive(Debug, Serialize, Deserialize, ToSchema)]









// UpdateUserRequest moved to user_handlers.rs

// UpdateUserResponse moved to user_handlers.rs

/// Delete user response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteUserResponse {
    pub user_id: String,
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
}

/// Delete loan response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteLoanResponse {
    pub loan_id: String,
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
    pub final_status: String,
}

/// List identities query parameters
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListIdentitiesQuery {
    pub user_type: Option<UserType>,
    pub kyc_status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List identities response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListIdentitiesResponse {
    pub identities: Vec<UserDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Create new loan request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanRequest {
    pub borrower_id: String,
    pub lender_id: String,
    pub principal_amount: String,            // Amount in cents as string
    pub interest_rate_bps: u16,              // Interest rate in basis points
    pub term_days: Option<u32>,              // Optional loan term
    pub metadata: Option<serde_json::Value>, // Optional additional data
}

/// Loan details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanDetailsResponse {
    pub loan_id: String,
    pub borrower_id: String,
    pub lender_id: String,
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub loan_status: String,
    pub originated_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
}

/// List loans query parameters
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListLoansQuery {
    pub status: Option<String>,
    pub borrower_id: Option<String>,
    pub lender_id: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List loans response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoansResponse {
    pub loans: Vec<LoanDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Loan statistics response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanStatsResponse {
    pub total_loans: u64,
    pub total_principal: String,   // Total principal amount as string
    pub total_outstanding: String, // Total outstanding balance as string
    pub average_interest_rate_bps: u32,
    pub status_breakdown: std::collections::HashMap<String, u64>,
}

/// Update loan request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLoanRequest {
    pub principal_amount: Option<String>, // Amount in cents as string
    pub outstanding_balance: Option<String>, // Outstanding balance in cents as string
    pub interest_rate_bps: Option<u16>,   // Interest rate in basis points
    pub loan_status: Option<String>, // "originated", "active", "paid_off", "closed", "defaulted"
    pub maturity_date: Option<Option<String>>, // Optional maturity date (ISO 8601), Option<Option<T>> to handle NULL
}

/// Update loan response
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateLoanResponse {
    pub loan_id: String,
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub loan_details: LoanDetailsResponse,
}

/// Record loan payment request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecordPaymentRequest {
    pub loan_id: String,
    pub amount: String,                    // Amount in cents as string
    pub payment_type: String,              // "Principal" | "Interest" | "Mixed"
    pub payment_reference: Option<String>, // Optional external reference
    pub notes: Option<String>,
}

/// Payment response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub transaction_id: String,
    pub loan_id: String,
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String, // "Pending" | "Processed" | "Failed"
    pub created_at: DateTime<Utc>,
}

/// Payment details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentDetailsResponse {
    pub transaction_id: String,
    pub loan_id: String,
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String,
    pub batch_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Loan payment history response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanPaymentHistoryResponse {
    pub loan_id: String,
    pub payments: Vec<PaymentDetailsResponse>,
    pub total_payments: usize,
    pub total_amount_paid: String,
}

/// Current state summary response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateSummaryResponse {
    pub current_state_root: String,
    pub transaction_count: u64,
    pub last_batch_id: Option<String>,
    pub last_updated: DateTime<Utc>,
    pub loan_summary: LoanStatsSummary,
    pub user_summary: UserStatsSummary,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanStatsSummary {
    pub total_loans: u64,
    pub total_principal: String,
    pub total_outstanding: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserStatsSummary {
    pub total_users: u64,
    pub kyc_approved_users: u64,
}

/// Transaction inclusion proof response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionProofResponse {
    pub transaction_id: String,
    pub transaction_type: String,
    pub proof: ProofData,
    pub verification_data: VerificationData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProofData {
    pub merkle_proof: String,
    pub batch_proof: String,
    pub state_root: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationData {
    pub batch_id: String,
    pub batch_timestamp: u64,
    pub transaction_index: u32,
}

/// Verify transaction inclusion request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifyTransactionRequest {
    pub transaction_id: String,
    pub proof_data: ProofData,
    pub expected_transaction_type: String,
}

/// Transaction verification response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionVerificationResponse {
    pub is_valid: bool,
    pub verification_result: VerificationResult,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationResult {
    pub merkle_proof_valid: bool,
    pub batch_proof_valid: bool,
    pub transaction_included: bool,
    pub transaction_type_matches: bool,
}

/// State transition history response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateHistoryResponse {
    pub state_transitions: Vec<StateTransition>,
    pub total_batches: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateTransition {
    pub batch_id: String,
    pub previous_state_root: String,
    pub new_state_root: String,
    pub transaction_count: u32,
    pub batch_timestamp: u64,
    pub proof_id: Option<String>,
    pub verification_status: String,
}

/// Batch creation request for loans
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanBatchRequest {
    pub max_transaction_count: Option<u32>,
    pub force_create: Option<bool>,
    pub include_transaction_types: Option<Vec<String>>,
}

/// Batch creation response for loans
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanBatchResponse {
    pub batch_id: String,
    pub transaction_count: u32,
    pub batch_status: String,
    pub estimated_proof_time: String,
    pub created_at: DateTime<Utc>,
}

/// Pending transactions ready for batching
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingLoanTransactionsResponse {
    pub pending_transactions: Vec<PendingTransactionInfo>,
    pub total_pending: usize,
    pub total_amount: String,
    pub oldest_pending: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingTransactionInfo {
    pub transaction_id: String,
    pub transaction_type: String,
    pub user_id: String,
    pub amount: String,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// DEPOSIT TRANSACTION TYPES
// ============================================================================

/// Response for user balance query
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserBalanceResponse {
    pub user_id: Option<String>,
    pub wallet_address: String,
    pub available_balance: String,
    pub currency: String,
}

/// Response for user transaction history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserTransactionsResponse {
    pub transactions: Vec<DepositTransactionInfo>,
    pub total_count: usize,
}

/// Deposit transaction information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DepositTransactionInfo {
    pub id: i32,
    pub user_address: String,
    pub amount: String,
    pub deposit_amount_usdc: Option<String>,
    pub transaction_hash: String,
    pub status: String, // pending, confirmed, processed, failed
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub block_number: i64,
    pub created_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Response for transaction by hash query
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionStatusResponse {
    pub found: bool,
    pub transaction: Option<DepositTransactionInfo>,
}

// ============================================================================
// LIQUIDITY POOL API MODELS
// ============================================================================

/// Request to create a new liquidity pool
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolRequest {
    pub name: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    /// Investor ID (required for pool creation)
    pub investor_id: Option<String>,
}

/// Response from pool creation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolResponse {
    pub pool_id: String,
    pub investor_id: String,
    pub name: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Pool details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PoolDetailsResponse {
    pub pool_id: String,
    pub investor_id: String,
    pub name: String,
    /// Total deposits as decimal string
    pub total_deposits: String,
    /// Available liquidity as decimal string
    pub available_liquidity: String,
    /// Utilized liquidity as decimal string
    pub utilized_liquidity: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pool list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PoolListResponse {
    pub pools: Vec<PoolDetailsResponse>,
    pub total_count: usize,
}

/// Pool status update request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatePoolStatusRequest {
    pub status: String, // ACTIVE, PAUSED, CLOSED
}

/// Pool matching criteria for auto-deployment
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PoolMatchRequest {
    /// Required liquidity amount as decimal string
    pub required_liquidity: String,
    /// Maximum LTV ratio as decimal string
    pub max_ltv: String,
    /// Maximum interest rate as decimal string
    pub max_rate: String,
    pub min_repayment_days: i32,
}

// ============================================================================
// ROUTER SETUP
// ============================================================================

/// Create the main API router with all batch processing and loan endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints
        .route(paths::HEALTH, get(health_check))
        .route(paths::ROOT, get(api_info))
        .route(paths::API_HEALTH, get(health_check))
        .route(paths::API_INFO, get(api_info))
        // OpenAPI documentation routes - Swagger UI handles both the UI and JSON spec
        .merge(swagger_ui())
        // Authentication endpoints
        .route(
            "/api/auth/login",
            post(crate::authentication_handlers::login_endpoint),
        )
        .route(
            "/api/auth/logout",
            post(crate::authentication_handlers::logout_endpoint),
        )
        // Admin KYC endpoints
        .route(
            paths::ADMIN_KYC_PENDING,
            get(crate::admin_handlers::list_pending_kyc_endpoint),
        )
        .route(
            paths::ADMIN_KYC_STATS,
            get(crate::admin_handlers::get_kyc_statistics_endpoint),
        )
        .route(
            paths::ADMIN_KYC_BULK_APPROVE,
            post(crate::admin_handlers::bulk_kyc_approval_endpoint),
        )
        .route(
            paths::INVITE_CODE_GENERATE,
            post(crate::authentication_handlers::generate_invite_code_endpoint),
        )
        .route(
            paths::INVITE_CODE_VALIDATE,
            post(crate::authentication_handlers::validate_invite_code_endpoint),
        )
        // Transaction operations
        .route(
            paths::STATE_TRANSACTIONS,
            post(crate::state_handlers::submit_transaction_endpoint),
        )
        .route(
            paths::STATE_TRANSACTIONS_PENDING,
            get(crate::state_handlers::get_pending_transactions_endpoint),
        )
        // Batch operations
        .route(
            paths::STATE_BATCHES,
            post(crate::state_handlers::create_batch_endpoint),
        )
        .route(paths::STATE_BATCHES, get(get_batches_endpoint))
        .route(paths::STATE_BATCH_BY_ID, get(get_batch_endpoint))
        .route(paths::STATE_BATCH_PROOF, post(update_batch_proof_endpoint))
        .route(paths::STATE_BATCHES_TRIGGER, post(trigger_batch_endpoint))
        .route(
            paths::STATE_BATCHES_STATS,
            get(get_batch_processor_stats_endpoint),
        )
        // State operations
        .route(paths::STATE_CURRENT, get(get_current_state_endpoint))
        .route(paths::STATE_CONTRACT, get(get_contract_data_endpoint))
        // ========================================================================
        // LOAN PROCESSING ENDPOINTS
        // ========================================================================
        // User Management
        .route(
            paths::USER,
            post(crate::user_handlers::register_user_endpoint),
        )
        .route(
            paths::USERS,
            get(crate::user_handlers::list_identities_endpoint),
        )
        .route(
            paths::USER_BY_ID,
            get(crate::user_handlers::get_user_details_endpoint),
        )
        .route(paths::USER_BY_ID, put(update_user_endpoint))
        .route(paths::USER_BY_ID, delete(delete_user_endpoint))
        .route(paths::USER_KYC, post(submit_kyc_endpoint))
        .route(paths::USER_KYC_STATUS, post(update_kyc_status_endpoint))
        // Investor Management endpoints
        .route(
            paths::REGISTER_INVESTOR,
            post(crate::merchant_handlers::register_investor_endpoint),
        )
        .route(
            paths::INVESTOR_PROFILE,
            get(crate::merchant_handlers::get_investor_profile_endpoint),
        )
        // Merchant Management endpoints
        .route(
            paths::REGISTER_MERCHANT,
            post(crate::merchant_handlers::register_merchant_endpoint),
        )
        .route(
            paths::MERCHANT_PROFILE,
            get(crate::merchant_handlers::get_merchant_profile_endpoint)
                .put(crate::merchant_handlers::update_merchant_profile_endpoint),
        )
        .route(
            paths::INVESTOR_PROFILE,
            put(crate::merchant_handlers::update_investor_profile_endpoint),
        )
        // User Balance & Transaction endpoints
        .route(
            paths::USER_BALANCE_BY_ADDRESS,
            get(get_user_balance_by_address_endpoint),
        )
        .route(
            paths::USER_BALANCE_BY_ID,
            get(get_user_balance_by_id_endpoint),
        )
        .route(
            paths::USER_TRANSACTIONS_BY_ID,
            get(get_user_transactions_endpoint),
        )
        .route(
            paths::TRANSACTION_BY_HASH,
            get(get_transaction_by_hash_endpoint),
        )
        // ========================================================================
        // WITHDRAWAL MANAGEMENT ENDPOINTS
        // ========================================================================
        .route(paths::WITHDRAWALS, post(initiate_withdrawal_endpoint))
        .route(paths::WITHDRAWAL_BY_ID, get(get_withdrawal_status_endpoint))
        .route(
            paths::USER_AVAILABLE_BALANCE,
            get(get_user_available_balance_endpoint),
        )
        .route(
            paths::USER_WITHDRAWALS,
            get(get_user_withdrawal_history_endpoint),
        )
        .route(paths::WITHDRAWAL_CANCEL, post(cancel_withdrawal_endpoint))
        .route(
            paths::ADMIN_PENDING_WITHDRAWALS,
            get(get_pending_withdrawals_endpoint),
        )
        // ========================================================================
        // FINANCIAL TRANSACTION ENDPOINTS
        // ========================================================================
        // Account Financial Transactions
        .route(
            paths::USER_ACCOUNT_FINANCIAL_DEPOSIT,
            post(record_account_deposit),
        )
        .route(
            paths::USER_ACCOUNT_FINANCIAL_WITHDRAWAL,
            post(record_account_withdrawal),
        )
        .route(
            paths::USER_ACCOUNT_FINANCIAL,
            get(get_account_financial_transactions),
        )
        // Pool Financial Transactions
        .route(paths::POOL_FINANCIAL_FUND, post(record_pool_funding))
        .route(paths::POOL_FINANCIAL_WITHDRAW, post(record_pool_withdrawal))
        .route(
            paths::POOL_FINANCIAL_TRANSACTIONS,
            get(get_pool_financial_transactions),
        )
        .route(
            paths::USER_POOL_FINANCIAL,
            get(get_user_pool_financial_transactions),
        )
        // Credit Line Financial Transactions
        .route(
            paths::CREDIT_LINE_FINANCIAL_DRAW,
            post(record_credit_line_draw),
        )
        .route(
            paths::CREDIT_LINE_FINANCIAL_PAYMENT,
            post(record_credit_line_payment),
        )
        .route(
            paths::CREDIT_LINE_FINANCIAL_TRANSACTIONS,
            get(get_credit_line_financial_transactions),
        )
        .route(
            paths::MERCHANT_CREDIT_LINE_FINANCIAL,
            get(get_merchant_credit_line_financial_transactions),
        )
        // Unified Financial Views
        .route(
            paths::USER_FINANCIAL_TRANSACTIONS,
            get(get_user_financial_transactions),
        )
        .route(
            paths::USER_FINANCIAL_SUMMARY,
            get(get_user_financial_summary),
        )
        // ========================================================================
        // LOAN MANAGEMENT ENDPOINTS
        // ========================================================================
        // Loan Management
        .route(
            paths::LOAN,
            post(crate::loan_handlers::create_loan_endpoint),
        )
        .route(paths::LOANS, get(crate::loan_handlers::list_loans_endpoint))
        .route(
            paths::LOAN_BY_ID,
            get(crate::loan_handlers::get_loan_details_endpoint),
        )
        .route(
            paths::LOAN_BY_ID,
            put(crate::loan_handlers::update_loan_endpoint),
        )
        .route(
            paths::LOAN_BY_ID,
            delete(crate::loan_handlers::delete_loan_endpoint),
        )
        .route(
            paths::LOANS_STATS,
            get(crate::loan_handlers::get_loan_stats_endpoint),
        )
        // Payment Processing
        .route(
            paths::STATE_PAYMENTS,
            post(crate::loan_handlers::record_payment_endpoint),
        )
        .route(
            paths::STATE_PAYMENT_BY_ID,
            get(crate::loan_handlers::get_payment_details_endpoint),
        )
        .route(
            paths::LOAN_PAYMENTS,
            get(crate::loan_handlers::get_loan_payment_history_endpoint),
        )
        // State Management & ZK Proofs
        .route(paths::STATE_SUMMARY, get(get_state_summary_endpoint))
        .route(paths::STATE_PROOF, get(get_transaction_proof_endpoint))
        .route(paths::STATE_VERIFY, post(verify_transaction_endpoint))
        .route(paths::STATE_HISTORY, get(get_state_history_endpoint))
        // Batch Operations for Loans
        .route(paths::STATE_BATCH_CREATE, post(create_loan_batch_endpoint))
        .route(paths::STATE_LOAN_BATCH_BY_ID, get(get_loan_batch_endpoint))
        .route(
            paths::STATE_BATCH_PENDING,
            get(get_pending_loan_transactions_endpoint),
        )
        // ========================================================================
        // LOAN OPERATION INCLUSION PROOF ENDPOINTS
        // ========================================================================
        .route(
            paths::STATE_PROOFS_LOAN_OPERATION,
            post(generate_loan_operation_proof_endpoint),
        )
        .route(
            paths::STATE_PROOFS_LOAN_PAYMENT,
            get(generate_loan_payment_proof_endpoint),
        )
        .route(paths::STATE_PROOFS_VERIFY, post(verify_loan_proof_endpoint))
        .route(
            paths::STATE_PROOFS_REPORT,
            post(generate_proof_report_endpoint),
        )
        .route(
            paths::STATE_PROOFS_BATCH_VERIFY,
            post(batch_verify_proofs_endpoint),
        )
        .route(paths::LOAN_AUDIT_TRAIL, get(get_loan_audit_trail_endpoint))
        .route(
            paths::LOAN_LIFECYCLE_SUMMARY,
            get(get_loan_lifecycle_summary_endpoint),
        )
        // ========================================================================
        // LIQUIDITY POOL QUERY ENDPOINTS
        // ========================================================================
        .route(paths::POOLS, get(list_pools).post(create_pool))
        .route(
            paths::POOL_BY_ID,
            get(get_pool_details).put(update_pool_status),
        )
        .route(paths::POOL_UTILIZATION, get(get_pool_utilization))
        .route(paths::POOL_TRANSACTIONS, get(get_pool_transactions))
        .route(paths::POOL_MATCH, post(find_matching_pools_endpoint))
        // Investor Query Endpoints
        .route("/api/investor/dashboard", get(get_investor_dashboard))
        .route(
            "/api/investor/pools/summary",
            get(get_investor_pool_summaries),
        )
        // Analytics Endpoints
        .route("/api/analytics/market-overview", get(get_market_overview))
        .route("/api/analytics/pool-performance", get(get_pool_performance))
        // ========================================================================
        // MERCHANT TURNOVER ENDPOINTS
        // ========================================================================
        .route(
            paths::MERCHANT_TURNOVER,
            post(crate::turnover_handlers::create_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER,
            get(crate::turnover_handlers::get_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER_BY_DATE,
            put(crate::turnover_handlers::update_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER_ANALYTICS,
            get(crate::turnover_handlers::get_turnover_analytics_endpoint),
        )
        .route(
            paths::MERCHANT_PROFILE_COMPLETION,
            get(crate::turnover_handlers::get_profile_completion_endpoint),
        )
        // ========================================================================
        // CREDIT LINE ENDPOINTS
        // ========================================================================
        .route(paths::CREDIT_LINES, get(list_credit_line_requests))
        .route(
            paths::CREDIT_LINES_AVAILABLE_POOLS,
            get(browse_available_pools),
        )
        .route(
            paths::CREDIT_LINES_ELIGIBILITY,
            get(check_credit_eligibility),
        )
        .route(
            paths::CREDIT_LINES_REQUEST,
            post(submit_credit_line_request),
        )
        // ========================================================================
        // CREDIT LINE REQUEST ENDPOINTS (V2)
        // ========================================================================
        .route(
            paths::CREDIT_LINES_REQUEST_V2,
            post(create_credit_line_request_endpoint),
        )
        .with_state(state)
}

// ============================================================================
// ENDPOINT HANDLERS
// ============================================================================

/// Health check endpoint with OpenTelemetry tracing
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Health check passed", body = HealthResponse),
        (status = 500, description = "Health check failed")
    ),
    tag = "Info"
)]
#[instrument(skip(state), level = "info")]
async fn health_check(
    State(state): State<ApiState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    // Get the global tracer
    let tracer = global::tracer("arda-credit-api");

    // Create a span for the health check operation
    let span = tracer.start("health_check");
    let cx = Context::current_with_span(span);

    // Set attributes on the span
    cx.span().set_attributes(vec![
        KeyValue::new("endpoint", "/api/v1/health"),
        KeyValue::new("operation", "health_check"),
        KeyValue::new("component", "api"),
    ]);

    info!("🔍 API: Health check requested");

    // Add event for the start of health check
    cx.span().add_event(
        "health_check_started",
        vec![KeyValue::new("timestamp", Utc::now().to_rfc3339())],
    );

    // Test database connection with tracing
    let db_connected = {
        let db_span = tracer.start("database_health_check");
        let db_cx = cx.with_span(db_span);

        db_cx.span().set_attributes(vec![
            KeyValue::new("db.system", "postgresql"),
            KeyValue::new("db.operation", "health_check"),
            KeyValue::new("db.statement", "SELECT 1"),
        ]);

        match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
            Ok(_) => {
                db_cx
                    .span()
                    .set_attribute(KeyValue::new("db.success", true));
                db_cx.span().add_event(
                    "database_connection_success",
                    vec![KeyValue::new("query", "SELECT 1")],
                );
                true
            }
            Err(e) => {
                db_cx.span().set_attributes(vec![
                    KeyValue::new("db.success", false),
                    KeyValue::new("error.message", e.to_string()),
                    KeyValue::new("error.type", "database_connection_error"),
                ]);
                db_cx.span().add_event(
                    "database_connection_failed",
                    vec![
                        KeyValue::new("error", e.to_string()),
                        KeyValue::new("query", "SELECT 1"),
                    ],
                );
                error!("Database health check failed: {}", e);
                false
            }
        }
    };

    let response = HealthResponse {
        status: if db_connected {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Utc::now(),
        database_connected: db_connected,
        api_version: state.config.version.clone(),
        git_tag: env!("GIT_TAG").to_string(),
        git_sha: env!("GIT_SHA").to_string(),
        git_sha_short: env!("GIT_SHA_SHORT").to_string(),
        git_dirty: env!("GIT_DIRTY").parse().unwrap_or(false),
    };

    // Final span attributes
    cx.span().set_attributes(vec![
        KeyValue::new("health.status", response.status.clone()),
        KeyValue::new("database.connected", db_connected),
        KeyValue::new("api.version", response.api_version.clone()),
    ]);

    if db_connected {
        info!("✅ API: Health check passed");
        cx.span().add_event(
            "health_check_completed",
            vec![KeyValue::new("result", "success")],
        );
        Ok(Json(response))
    } else {
        error!("❌ API: Health check failed - database not connected");
        cx.span().set_attributes(vec![
            KeyValue::new("error", true),
            KeyValue::new("error.message", "Database not available"),
        ]);
        cx.span().add_event(
            "health_check_completed",
            vec![KeyValue::new("result", "failure")],
        );
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available".to_string(),
        ))
    }
}

/// API information endpoint
#[instrument(skip(state), level = "info")]
/// Get API information
#[utoipa::path(
    get,
    path = "/api/v1/info",
    responses(
        (status = 200, description = "API information", body = ApiInfoResponse),
    ),
    tag = "Info"
)]
async fn api_info(State(state): State<ApiState>) -> Json<ApiInfoResponse> {
    info!("📋 API: API info requested");

    let endpoints = paths::get_all_endpoints();

    let response = ApiInfoResponse {
        server_name: state.config.server_name.clone(),
        version: state.config.version.clone(),
        timestamp: Utc::now(),
        endpoints,
    };

    info!("✅ API: API info returned");
    Json(response)
}

/// Submit a new transaction
#[utoipa::path(
    post,
    path = "/api/v1/state/transactions",
    request_body = SubmitTransactionRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully", body = SubmitTransactionResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transactions"
)]
#[instrument(skip(state), level = "info")]
async fn submit_transaction_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Result<Json<SubmitTransactionResponse>, (StatusCode, String)> {
    info!("💰 API: Submitting transaction: amount={}", request.amount);

    match submit_transaction(&state.pool, request.amount).await {
        Ok(transaction) => {
            let response = SubmitTransactionResponse {
                transaction_id: transaction.id,
                amount: transaction.amount,
                status: "pending".to_string(),
                created_at: transaction.created_at,
            };

            info!("✅ API: Transaction submitted: id={}", transaction.id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to submit transaction: {}", e),
            ))
        }
    }
}

/// Get pending transactions
#[utoipa::path(
    get,
    path = "/api/v1/state/transactions/pending",
    responses(
        (status = 200, description = "List of pending transactions", body = PendingTransactionsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transactions"
)]
#[instrument(skip(state), level = "info")]
async fn get_pending_transactions_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<PendingTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting pending transactions");

    match get_pending_transactions(&state.pool).await {
        Ok(transactions) => {
            let total_amount: i32 = transactions.iter().map(|t| t.amount).sum();
            let transaction_infos: Vec<TransactionInfo> = transactions
                .into_iter()
                .map(|t| TransactionInfo {
                    id: t.id,
                    amount: t.amount,
                    created_at: t.created_at,
                })
                .collect();

            let response = PendingTransactionsResponse {
                total_count: transaction_infos.len(),
                total_amount,
                transactions: transaction_infos,
            };

            info!(
                "✅ API: Found {} pending transactions",
                response.total_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get pending transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pending transactions: {}", e),
            ))
        }
    }
}

/// Create a new batch using unified ADS-integrated workflow
#[utoipa::path(
    post,
    path = "/api/v1/state/batches",
    request_body = CreateBatchRequest,
    responses(
        (status = 200, description = "Batch created successfully", body = CreateBatchResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn create_batch_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateBatchRequest>,
) -> Result<Json<CreateBatchResponse>, (StatusCode, String)> {
    // Validate and clamp batch_size to ensure it's in the valid range [1, max_batch_size]
    let requested_size = request
        .batch_size
        .unwrap_or(state.config.max_batch_size as i32);
    let batch_size = requested_size
        .max(1)
        .min(state.config.max_batch_size as i32);

    info!(
        "🔄 UNIFIED API: Creating batch with size: {} (using ADS integration)",
        batch_size
    );

    // Use unified batch service for consistent ADS integration
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool.clone(),
        state.ads_service.clone(),
        state.config.max_batch_size,
    );

    match unified_service
        .create_batch_with_ads(Some(batch_size), "api")
        .await
    {
        Ok(Some(result)) => {
            let response = CreateBatchResponse {
                batch_id: result.batch_id,
                previous_counter_value: result.previous_counter_value,
                final_counter_value: result.final_counter_value,
                transaction_count: result.transaction_count,
                proof_status: "pending".to_string(), // New batches start as pending
                created_at: chrono::Utc::now(),
            };

            info!(
                "✅ UNIFIED API: Batch created with ADS integration: id={}, transactions={}, nullifiers={}, merkle_root=0x{}",
                result.batch_id,
                result.transaction_count,
                result.nullifier_count,
                hex::encode(&result.merkle_root[..8])
            );

            // Trigger proof generation for the newly created batch
            if state.batch_processor.is_some() {
                info!(
                    "🚀 Triggering proof generation for batch {}",
                    result.batch_id
                );
                tokio::spawn({
                    let pool = state.pool.clone();
                    let batch_id = result.batch_id;
                    async move {
                        if let Err(e) = crate::batch_processor::BackgroundBatchProcessor::generate_proof_for_batch(&pool, batch_id).await {
                            error!("Failed to generate proof for unified batch {}: {}", batch_id, e);
                        }
                    }
                });
            } else {
                warn!("Batch processor not available - proof generation skipped");
            }

            Ok(Json(response))
        }
        Ok(None) => {
            info!("ℹ️ UNIFIED API: No transactions available to batch");
            Err((
                StatusCode::BAD_REQUEST,
                "No pending transactions available to batch".to_string(),
            ))
        }
        Err(e) => {
            error!("UNIFIED API: Failed to create batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create batch: {}", e),
            ))
        }
    }
}

/// Get all batches (paginated)
#[utoipa::path(
    get,
    path = "/api/v1/state/batches",
    params(
        ("limit" = Option<i32>, Query, description = "Limit the number of batches returned")
    ),
    responses(
        (status = 200, description = "List of batches", body = BatchListResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batches_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<BatchListQuery>,
) -> Result<Json<BatchListResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(20);
    info!("📋 API: Getting batches with limit: {}", limit);

    match get_all_batches(&state.pool, Some(limit)).await {
        Ok(batches) => {
            let batch_infos: Vec<BatchInfo> = batches
                .into_iter()
                .map(|b| BatchInfo {
                    id: b.id,
                    previous_counter_value: b.previous_counter_value,
                    final_counter_value: b.final_counter_value,
                    transaction_count: b.transaction_ids.len(),
                    proof_status: b.proof_status,
                    sindri_proof_id: b.sindri_proof_id,
                    created_at: b.created_at,
                    proven_at: b.proven_at,
                })
                .collect();

            let response = BatchListResponse {
                total_count: batch_infos.len(),
                batches: batch_infos,
            };

            info!("✅ API: Found {} batches", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get batches: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batches: {}", e),
            ))
        }
    }
}

/// Get specific batch by ID
#[utoipa::path(
    get,
    path = "/api/v1/state/batch/{batch_id}",
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    responses(
        (status = 200, description = "Batch details", body = BatchInfo),
        (status = 404, description = "Batch not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batch_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!("🔍 API: Getting batch: id={}", batch_id);

    match get_batch_by_id(&state.pool, batch_id).await {
        Ok(batch) => {
            let batch_info = BatchInfo {
                id: batch.id,
                previous_counter_value: batch.previous_counter_value,
                final_counter_value: batch.final_counter_value,
                transaction_count: batch.transaction_ids.len(),
                proof_status: batch.proof_status,
                sindri_proof_id: batch.sindri_proof_id,
                created_at: batch.created_at,
                proven_at: batch.proven_at,
            };

            info!("✅ API: Found batch: id={}", batch.id);
            Ok(Json(batch_info))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batch: {}", e),
            ))
        }
    }
}

/// Update batch with ZK proof information
///
/// Validates:
/// - Status must be one of: "pending", "proven", "failed"
/// - When status is "proven", merkle_root is required and must be exactly 32 bytes
/// - When status is not "proven", merkle_root is optional but if provided must be valid hex and 32 bytes
/// - Hex values can optionally start with "0x" prefix
#[utoipa::path(
    post,
    path = "/api/v1/state/batch/{batch_id}/proof",
    request_body = UpdateBatchProofRequest,
    responses(
        (status = 200, description = "Batch proof updated successfully", body = BatchInfo),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "Batch not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn update_batch_proof_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
    Json(request): Json<UpdateBatchProofRequest>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!(
        "🔐 API: Updating batch proof: id={}, proof={}",
        batch_id, request.sindri_proof_id
    );

    // Validate status against allowed values
    const ALLOWED_STATUSES: &[&str] = &["pending", "proven", "failed"];
    if !ALLOWED_STATUSES.contains(&request.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid status '{}'. Allowed values: {}",
                request.status,
                ALLOWED_STATUSES.join(", ")
            ),
        ));
    }

    // Validate merkle_root requirements based on status
    let validated_merkle_root = if request.status == "proven" {
        // For "proven" status, merkle_root is required
        match &request.merkle_root {
            Some(merkle_root_hex) => {
                // Decode hex string to bytes
                let merkle_root =
                    hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid merkle root hex: {}", e),
                        )
                    })?;

                // Validate that merkle_root is exactly 32 bytes
                if merkle_root.len() != 32 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Merkle root must be exactly 32 bytes, got {} bytes",
                            merkle_root.len()
                        ),
                    ));
                }

                Some(merkle_root)
            }
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Merkle root is required when status is 'proven'".to_string(),
                ));
            }
        }
    } else {
        // For other statuses, merkle_root is not required (but allowed)
        if let Some(merkle_root_hex) = &request.merkle_root {
            // If provided, still validate the format
            let merkle_root =
                hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid merkle root hex: {}", e),
                    )
                })?;

            // Validate that merkle_root is exactly 32 bytes
            if merkle_root.len() != 32 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Merkle root must be exactly 32 bytes, got {} bytes",
                        merkle_root.len()
                    ),
                ));
            }

            Some(merkle_root)
        } else {
            None
        }
    };

    // Update batch with proof
    match update_batch_proof(
        &state.pool,
        batch_id,
        &request.sindri_proof_id,
        &request.status,
    )
    .await
    {
        Ok(()) => {
            // Store Merkle root if validated and provided
            if let Some(merkle_root) = validated_merkle_root {
                if let Err(e) = store_ads_state_commit(&state.pool, batch_id, &merkle_root).await {
                    error!("Failed to store ADS state commit: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to store ADS state commit: {}", e),
                    ));
                }
            }

            // Return updated batch info
            match get_batch_by_id(&state.pool, batch_id).await {
                Ok(batch) => {
                    let batch_info = BatchInfo {
                        id: batch.id,
                        previous_counter_value: batch.previous_counter_value,
                        final_counter_value: batch.final_counter_value,
                        transaction_count: batch.transaction_ids.len(),
                        proof_status: batch.proof_status,
                        sindri_proof_id: batch.sindri_proof_id,
                        created_at: batch.created_at,
                        proven_at: batch.proven_at,
                    };

                    info!("✅ API: Batch proof updated: id={}", batch_id);
                    Ok(Json(batch_info))
                }
                Err(e) => {
                    error!("Failed to get updated batch: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to get updated batch: {}", e),
                    ))
                }
            }
        }
        Err(e) => {
            error!("Failed to update batch proof: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update batch proof: {}", e),
            ))
        }
    }
}

/// Get current counter state
#[utoipa::path(
    get,
    path = "/api/v1/state/current",
    responses(
        (status = 200, description = "Current state information", body = CurrentStateResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
async fn get_current_state_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<CurrentStateResponse>, (StatusCode, String)> {
    info!("🎯 API: Getting current state");

    match get_current_state(&state.pool).await {
        Ok(state_info) => {
            // Find last proven batch
            let last_proven_batch_id = if state_info.merkle_root.is_some() {
                state_info.last_batch_id
            } else {
                None
            };

            let response = CurrentStateResponse {
                counter_value: state_info.counter_value,
                has_merkle_root: state_info.merkle_root.is_some(),
                last_batch_id: state_info.last_batch_id,
                last_proven_batch_id,
            };

            info!(
                "✅ API: Current state: counter={}, has_root={}",
                response.counter_value, response.has_merkle_root
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get current state: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get current state: {}", e),
            ))
        }
    }
}

/// Get contract submission data (dry run)
#[utoipa::path(
    get,
    path = "/api/v1/state/{batch_id}/contract",
    responses(
        (status = 200, description = "Contract submission data", body = ContractSubmissionData),
        (status = 404, description = "Batch not found or not proven"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
async fn get_contract_data_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<ContractSubmissionData>, (StatusCode, String)> {
    info!("📄 API: Getting contract data for batch: id={}", batch_id);

    match get_contract_submission_data(&state.pool, batch_id).await {
        Ok(db_contract_data) => {
            info!("✅ API: Contract data prepared for batch: id={}", batch_id);

            // Convert from db type to API type
            let contract_data = ContractSubmissionData {
                public: ContractPublicData {
                    prev_merkle_root: db_contract_data.public.prev_merkle_root,
                    new_merkle_root: db_contract_data.public.new_merkle_root,
                    zk_proof: db_contract_data.public.zk_proof,
                },
                private: ContractPrivateData {
                    prev_counter_value: db_contract_data.private.prev_counter_value,
                    new_counter_value: db_contract_data.private.new_counter_value,
                    transactions: db_contract_data.private.transactions,
                },
            };

            Ok(Json(contract_data))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found or not proven: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found or not proven: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get contract data: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get contract data: {}", e),
            ))
        }
    }
}

/// Manually trigger batch processing
#[utoipa::path(
    post,
    path = "/api/v1/state/batches/trigger",
    responses(
        (status = 200, description = "Batch processing triggered successfully", body = TriggerBatchResponse),
        (status = 500, description = "Failed to trigger batch processing")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn trigger_batch_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<TriggerBatchResponse>, (StatusCode, String)> {
    info!("🔄 API: Manual batch trigger requested");

    if let Some(batch_processor) = &state.batch_processor {
        match batch_processor.trigger_batch() {
            Ok(()) => {
                info!("✅ API: Batch trigger sent successfully");
                Ok(Json(TriggerBatchResponse {
                    triggered: true,
                    message: "Batch processing triggered successfully".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to trigger batch processing: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to trigger batch processing: {}", e),
                ))
            }
        }
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}

/// Get batch processor statistics
#[utoipa::path(
    get,
    path = "/api/v1/state/batches/stats",
    responses(
        (status = 200, description = "Batch processor statistics", body = BatchProcessorStatsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batch_processor_stats_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<BatchProcessorStatsResponse>, (StatusCode, String)> {
    info!("📊 API: Batch processor stats requested");

    if let Some(batch_processor) = &state.batch_processor {
        let processor_stats = batch_processor.get_stats().await;

        let response = BatchProcessorStatsResponse {
            total_batches_created: processor_stats.total_batches_created,
            total_transactions_processed: processor_stats.total_transactions_processed,
            timer_triggers: processor_stats.timer_triggers,
            count_triggers: processor_stats.count_triggers,
            manual_triggers: processor_stats.manual_triggers,
            errors: processor_stats.errors,
            last_batch_time: processor_stats
                .last_batch_time
                .map(|t| format!("{} ago", format_duration(t.elapsed()))),
        };

        info!("✅ API: Batch processor stats returned");
        Ok(Json(response))
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}

// ============================================================================
// LOAN PROCESSING ENDPOINT HANDLERS
// ============================================================================

// ---- USER MANAGEMENT ENDPOINTS ----

// register_user_endpoint moved to user_handlers.rs

// login_endpoint moved to authentication_handlers.rs

// logout_endpoint moved to authentication_handlers.rs

// validate_invite_code_endpoint moved to authentication_handlers.rs

fn extract_authenticated_user_id(
    request: &axum::extract::Request<axum::body::Body>,
) -> Result<uuid::Uuid, String> {
    let user_id_header = request
        .headers()
        .get("x-user-id")
        .ok_or_else(|| "No authenticated user ID found".to_string())?;

    let user_id_str = user_id_header
        .to_str()
        .map_err(|_| "Invalid user ID header format".to_string())?;

    parse_user_id(user_id_str)
}

pub fn extract_authenticated_user(
    request: &axum::extract::Request<axum::body::Body>,
) -> Result<AuthenticatedUser, String> {
    let user_id = extract_authenticated_user_id(request)?;

    let email = request
        .headers()
        .get("x-user-email")
        .ok_or_else(|| "No authenticated user email found".to_string())?
        .to_str()
        .map_err(|_| "Invalid user email header format".to_string())?
        .to_string();

    let user_type = request
        .headers()
        .get("x-user-type")
        .ok_or_else(|| "No authenticated user type found".to_string())?
        .to_str()
        .map_err(|_| "Invalid user type header format".to_string())?
        .to_string();

    Ok(AuthenticatedUser {
        user_id,
        email,
        user_type,
    })
}

// generate_invite_code_endpoint moved to authentication_handlers.rs

/// List user identities
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(
        ListIdentitiesQuery
    ),
    responses(
        (status = 200, description = "List of user identities", body = ListIdentitiesResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn list_identities_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListIdentitiesQuery>,
) -> Result<Json<ListIdentitiesResponse>, (StatusCode, String)> {
    info!("📋 API: Listing identities with filters");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Basic identity listing using direct SQL (excludes soft deleted users)
    let identities_result = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, created_at, updated_at, first_name, last_name, jurisdiction, wallet_address
        FROM identities
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(&state.pool)
    .await;

    match identities_result {
        Ok(rows) => {
            let identities = rows
                .into_iter()
                .map(|row| UserDetailsResponse {
                    user_id: row.id.to_string(),
                    email: row.email,
                    user_type: row.user_type.parse().unwrap_or(UserType::User),
                    kyc_status: row.kyc_status,
                    kyc_submitted_at: row.kyc_submitted_at,
                    kyc_verified_at: row.kyc_verified_at,
                    created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
                    updated_at: Some(row.updated_at.unwrap_or_else(chrono::Utc::now)),
                    first_name: row.first_name,
                    last_name: row.last_name,
                    jurisdiction: row.jurisdiction,
                    wallet_address: row.wallet_address,
                })
                .collect::<Vec<_>>();

            let total_count = identities.len() as u64;

            let response = ListIdentitiesResponse {
                identities,
                total_count: total_count as usize,
                has_more: total_count >= limit as u64,
            };

            info!("✅ API: Listed {} identities", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list identities: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list identities: {}", e),
            ))
        }
    }
}

/// Get user details by ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}",
    responses(
        (status = 200, description = "User details retrieved successfully", body = UserDetailsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn get_user_details_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user details: user_id={}", user_id);

    let uuid = match parse_user_id(&user_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            warn!("❌ API: Invalid user ID format: {}", e);
            return Err((StatusCode::BAD_REQUEST, e));
        }
    };

    match get_identity_by_id(&state.pool, uuid).await {
        Ok(Some(identity)) => {
            info!("✅ API: User details retrieved for ID: {}", identity.id);
            Ok(Json(identity_to_user_details(identity)))
        }
        Ok(None) => {
            warn!("❌ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Database error retrieving user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// Submit KYC information
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc",
    request_body = SubmitKycRequest,
    responses(
        (status = 200, description = "KYC information submitted successfully", body = SubmitKycResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn submit_kyc_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<SubmitKycRequest>,
) -> Result<Json<SubmitKycResponse>, (StatusCode, String)> {
    info!(
        "📄 API: KYC submission: user_id={}, doc_type={}",
        user_id, request.kyc_data.document_type
    );

    let uuid = match parse_user_id(&user_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            warn!("❌ API: Invalid user ID format: {}", e);
            return Err((StatusCode::BAD_REQUEST, e));
        }
    };

    match submit_kyc_data(
        &state.pool,
        uuid,
        &request.kyc_data.document_type,
        &request.kyc_data.document_hash,
        request.kyc_data.jurisdiction.as_deref(),
        request.kyc_data.kyc_investor_type.as_deref(),
    )
    .await
    {
        Ok(identity) => {
            info!(
                "✅ API: KYC submitted successfully for user: {}",
                identity.id
            );
            let response = SubmitKycResponse {
                user_id: identity.id.to_string(),
                kyc_status: identity.kyc_status,
                submitted_at: identity.kyc_submitted_at.unwrap_or_else(Utc::now),
            };
            Ok(Json(response))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error submitting KYC: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to submit KYC".to_string(),
                ))
            }
        }
    }
}

/// Update KYC status (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc-status",
    request_body = UpdateKycStatusRequest,
    responses(
        (status = 200, description = "KYC status updated successfully", body = UserDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn update_kyc_status_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateKycStatusRequest>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!(
        "⚖️ API: Updating KYC status: user_id={}, new_status={}",
        user_id, request.status
    );

    let uuid = match parse_user_id(&user_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            warn!("❌ API: Invalid user ID format: {}", e);
            return Err((StatusCode::BAD_REQUEST, e));
        }
    };

    match update_kyc_status(&state.pool, uuid, &request.status, request.notes.as_deref()).await {
        Ok(identity) => {
            info!(
                "✅ API: KYC status updated successfully for user: {}",
                identity.id
            );
            Ok(Json(identity_to_user_details(identity)))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error updating KYC status: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update KYC status".to_string(),
                ))
            }
        }
    }
}

/// List pending KYC applications (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/kyc/pending",
    params(ListPendingKycQuery),
    responses(
        (status = 200, description = "List of pending KYC applications", body = ListPendingKycResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
async fn list_pending_kyc_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListPendingKycQuery>,
) -> Result<Json<ListPendingKycResponse>, (StatusCode, String)> {
    info!("🔍 API: Listing pending KYC applications");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match list_pending_kyc_applications(&state.pool, Some(limit as i64), Some(offset as i64)).await
    {
        Ok(applications) => {
            let total_count = applications.len();
            let has_more = total_count >= limit as usize;

            let application_responses: Vec<UserDetailsResponse> = applications
                .into_iter()
                .map(identity_to_user_details)
                .collect();

            let response = ListPendingKycResponse {
                applications: application_responses,
                total_count,
                has_more,
            };

            info!(
                "✅ API: Found {} pending KYC applications",
                response.total_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list pending KYC applications: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve pending KYC applications".to_string(),
            ))
        }
    }
}

/// Get KYC statistics (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/kyc/stats",
    responses(
        (status = 200, description = "KYC statistics", body = KycStatisticsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
async fn get_kyc_statistics_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<KycStatisticsResponse>, (StatusCode, String)> {
    info!("📊 API: Getting KYC statistics");

    match get_kyc_statistics(&state.pool).await {
        Ok(stats) => {
            let total_processed = stats.approved_count + stats.rejected_count;
            let approval_rate = if total_processed > 0 {
                (stats.approved_count as f64 / total_processed as f64) * 100.0
            } else {
                0.0
            };

            let response = KycStatisticsResponse {
                total_users: stats.total_users,
                approved_count: stats.approved_count,
                pending_count: stats.pending_count,
                rejected_count: stats.rejected_count,
                not_submitted_count: stats.not_submitted_count,
                approval_rate,
            };

            info!(
                "✅ API: KYC stats - Total: {}, Approved: {}, Pending: {}, Rejected: {}, Not Submitted: {}, Approval Rate: {:.1}%",
                response.total_users,
                response.approved_count,
                response.pending_count,
                response.rejected_count,
                response.not_submitted_count,
                response.approval_rate
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to get KYC statistics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve KYC statistics".to_string(),
            ))
        }
    }
}

/// Update user profile
#[utoipa::path(
    put,
    path = "/api/v1/user/{user_id}",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated successfully", body = UpdateUserResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn update_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UpdateUserResponse>, (StatusCode, String)> {
    info!("🔄 API: Updating user profile: user_id={}", user_id);

    let uuid = match parse_user_id(&user_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            warn!("❌ API: Invalid user ID format: {}", e);
            return Err((StatusCode::BAD_REQUEST, e));
        }
    };

    // Build list of fields being updated
    let mut updated_fields = Vec::new();
    if request.email.is_some() {
        updated_fields.push("email".to_string());
    }
    if request.user_type.is_some() {
        updated_fields.push("user_type".to_string());
    }
    if request.jurisdiction.is_some() {
        updated_fields.push("jurisdiction".to_string());
    }
    if request.first_name.is_some() {
        updated_fields.push("first_name".to_string());
    }
    if request.last_name.is_some() {
        updated_fields.push("last_name".to_string());
    }
    if request.wallet_address.is_some() {
        updated_fields.push("wallet_address".to_string());
    }

    match update_identity_profile(
        &state.pool,
        uuid,
        request.email.as_deref(),
        request
            .user_type
            .as_ref()
            .map(std::string::ToString::to_string)
            .as_deref(),
        request.jurisdiction.as_deref(),
        request.first_name.as_deref(),
        request.last_name.as_deref(),
        request.wallet_address.as_deref(),
    )
    .await
    {
        Ok(identity) => {
            info!(
                "✅ API: User profile updated successfully for ID: {}",
                identity.id
            );
            let response = UpdateUserResponse {
                user_id: identity.id.to_string(),
                updated_fields,
                updated_at: identity.updated_at,
            };
            Ok(Json(response))
        }
        Err(e) => {
            if e.to_string().contains("violates foreign key constraint")
                || e.to_string().contains("not found")
            {
                warn!("❌ API: User not found: {}", user_id);
                Err((StatusCode::NOT_FOUND, "User not found".to_string()))
            } else {
                error!("❌ API: Database error updating user: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update user".to_string(),
                ))
            }
        }
    }
}

/// Delete user
#[utoipa::path(
    delete,
    path = "/api/v1/user/{user_id}",
    responses(
        (status = 200, description = "User deleted successfully", body = DeleteUserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn delete_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<DeleteUserResponse>, (StatusCode, String)> {
    info!("🗑️ API: Soft deleting user: user_id={}", user_id);

    let user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Check if user exists and is not already deleted
    let existing_user = sqlx::query!(
        "SELECT id, deleted_at FROM identities WHERE id = $1",
        user_uuid
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to check user existence: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to check user existence".to_string(),
        )
    })?;

    let _user = match existing_user {
        Some(user) if user.deleted_at.is_some() => {
            return Err((StatusCode::CONFLICT, "User is already deleted".to_string()));
        }
        Some(user) => user,
        None => {
            return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
        }
    };

    // Perform soft delete by setting deleted_at timestamp
    let deleted_at = Utc::now();
    let update_result = sqlx::query!(
        "UPDATE identities SET deleted_at = $1, updated_at = $1 WHERE id = $2",
        deleted_at,
        user_uuid
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to soft delete user: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to delete user".to_string(),
        )
    })?;

    if update_result.rows_affected() == 0 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update user deletion status".to_string(),
        ));
    }

    let response = DeleteUserResponse {
        user_id,
        deleted: true,
        deleted_at,
    };

    info!("✅ API: User soft deleted successfully");
    Ok(Json(response))
}

// ---- INVESTOR MANAGEMENT ENDPOINTS - MOVED TO merchant_handlers.rs ----

/// Update merchant profile
#[utoipa::path(
    put,
    path = "/api/v1/merchant/profile",
    request_body = UpdateMerchantProfileRequest,
    responses(
        (status = 200, description = "Merchant profile updated successfully", body = MerchantProfileResponse),
        (status = 404, description = "Merchant profile not found"),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Merchants"
)]
#[instrument(skip(state), level = "info")]
async fn update_merchant_profile_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<UpdateMerchantProfileRequest>,
) -> Result<Json<MerchantProfileResponse>, (StatusCode, String)> {
    // For this implementation, we'll use a hardcoded user_id for testing
    // In a real implementation, this would come from authentication middleware
    let test_user_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid UUID".to_string(),
            )
        })?;

    info!(
        "🔄 API: Updating merchant profile for test user: {}",
        test_user_id
    );

    // Get the merchant by user ID first to ensure it exists
    let merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
        Ok(Some(merchant)) => merchant,
        Ok(None) => {
            warn!("❌ API: Merchant not found for user: {}", test_user_id);
            return Err((StatusCode::NOT_FOUND, "Merchant not found".to_string()));
        }
        Err(e) => {
            error!("❌ API: Failed to get merchant: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get merchant".to_string(),
            ));
        }
    };

    // Convert API request to database request
    let db_request = DbUpdateMerchantProfileRequest {
        legal_name: request.legal_name,
        dba: request.dba,
        mcc: request.mcc,
        country: request.country,
        business_type: request.business_type,
        industry: request.industry,
        years_in_business: request.years_in_business,
        employee_count: request.employee_count,
        monthly_revenue_estimate: request.monthly_revenue_estimate,
        business_description: request.business_description,
    };

    // Update the merchant profile
    match update_merchant_profile(&state.pool, merchant.id, &db_request).await {
        Ok(_) => {
            info!(
                "✅ API: Merchant profile updated successfully: {}",
                merchant.id
            );

            // Return the updated profile by calling the get endpoint
            // Get the user identity for additional info
            let identity = match get_identity_by_id(&state.pool, merchant.user_id).await {
                Ok(Some(identity)) => identity,
                Ok(None) => {
                    warn!(
                        "❌ API: Identity not found for merchant user: {}",
                        merchant.user_id
                    );
                    return Err((StatusCode::NOT_FOUND, "User identity not found".to_string()));
                }
                Err(e) => {
                    error!("❌ API: Failed to get identity for merchant: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get user identity".to_string(),
                    ));
                }
            };

            // Get the updated merchant (refetch from database)
            let updated_merchant = match get_merchant_by_user_id(&state.pool, test_user_id).await {
                Ok(Some(merchant)) => merchant,
                Ok(None) => {
                    warn!(
                        "❌ API: Updated merchant not found for user: {}",
                        test_user_id
                    );
                    return Err((
                        StatusCode::NOT_FOUND,
                        "Updated merchant not found".to_string(),
                    ));
                }
                Err(e) => {
                    error!("❌ API: Failed to get updated merchant: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get updated merchant".to_string(),
                    ));
                }
            };

            // Get the updated merchant profile (optional)
            let profile =
                match get_merchant_profile_by_merchant_id(&state.pool, updated_merchant.id).await {
                    Ok(profile_opt) => profile_opt,
                    Err(e) => {
                        warn!(
                        "⚠️ API: Failed to get updated merchant profile (continuing without): {}",
                        e
                    );
                        None
                    }
                };

            let response = MerchantProfileResponse {
                profile_id: profile
                    .as_ref()
                    .map(|p| p.id.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                merchant_id: updated_merchant.id.to_string(),
                user_id: updated_merchant.user_id.to_string(),
                email: identity.email,
                user_type: UserType::Merchant,
                kyc_status: identity.kyc_status,
                kyc_submitted_at: identity.kyc_submitted_at,
                kyc_verified_at: identity.kyc_verified_at,
                legal_name: updated_merchant.legal_name,
                dba: updated_merchant.dba,
                tax_id: updated_merchant.tax_id,
                mcc: updated_merchant.mcc,
                country: updated_merchant.country,
                business_type: updated_merchant.business_type,
                industry: profile.as_ref().and_then(|p| p.industry.clone()),
                years_in_business: profile.as_ref().and_then(|p| p.years_in_business),
                employee_count: profile.as_ref().and_then(|p| p.employee_count),
                monthly_revenue_estimate: profile.as_ref().and_then(|p| p.monthly_revenue_estimate),
                business_description: profile
                    .as_ref()
                    .and_then(|p| p.business_description.clone()),
                profile_completed: profile
                    .as_ref()
                    .map(|p| p.profile_completed)
                    .unwrap_or(false),
                merchant_created_at: updated_merchant.created_at,
                merchant_updated_at: updated_merchant.updated_at,
                profile_created_at: profile.as_ref().map(|p| p.created_at),
                profile_updated_at: profile.as_ref().map(|p| p.updated_at),
                jurisdiction: None,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to update merchant profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update merchant profile".to_string(),
            ))
        }
    }
}

/// Get investor profile
#[utoipa::path(
    get,
    path = "/api/v1/investor/profile",
    params(InvestorProfileQuery),
    responses(
        (status = 200, description = "Investor profile retrieved successfully", body = InvestorProfileResponse),
        (status = 400, description = "Invalid request parameters - exactly one of user_id or email must be provided"),
        (status = 404, description = "Investor profile not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Investors",
    security(
        ("jwt_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
async fn get_investor_profile_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<InvestorProfileQuery>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    // Validate that exactly one of user_id or email is provided
    match (&query.user_id, &query.email) {
        (Some(_), Some(_)) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Exactly one of user_id or email must be provided, not both".to_string(),
            ))
        }
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Exactly one of user_id or email must be provided".to_string(),
            ))
        }
        _ => {}
    }

    // Parse user_id if provided
    let user_id_option = if let Some(user_id_str) = &query.user_id {
        Some(uuid::Uuid::parse_str(user_id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid user ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    let email_option = query.email.as_deref();

    info!(
        "🔍 API: Getting investor profile for user_id: {:?}, email: {:?}",
        user_id_option, email_option
    );

    match get_investor_profile_by_user_id_or_email(&state.pool, user_id_option, email_option).await
    {
        Ok(Some((profile, identity))) => Ok(Json(InvestorProfileResponse {
            profile_id: profile.id.to_string(),
            user_id: profile.user_id.to_string(),
            email: identity.email,
            user_type: UserType::Investor,
            kyc_status: identity.kyc_status,
            kyc_submitted_at: identity.kyc_submitted_at,
            kyc_verified_at: identity.kyc_verified_at,
            kyc_investor_type: identity.kyc_investor_type.map(|t| t.to_string()),
            accredited_investor: profile.accredited_investor,
            investment_capacity: profile.investment_capacity,
            regulatory_jurisdiction: profile.regulatory_jurisdiction,
            profile_created_at: profile.created_at,
            profile_updated_at: profile.updated_at,
            first_name: identity.first_name,
            last_name: identity.last_name,
            jurisdiction: identity.jurisdiction,
            wallet_address: identity.wallet_address,
        })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            "Investor profile not found".to_string(),
        )),
        Err(e) => {
            error!("❌ API: Failed to get investor profile: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get investor profile".to_string(),
            ))
        }
    }
}

/// Update investor profile
#[utoipa::path(
    put,
    path = "/api/v1/investor/profile",
    request_body = UpdateInvestorProfileRequest,
    responses(
        (status = 200, description = "Investor profile updated successfully", body = InvestorProfileResponse),
        (status = 404, description = "Investor profile not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Investors",
    security(
        ("jwt_auth" = [])
    )
)]
#[instrument(skip(state), level = "info")]
async fn update_investor_profile_endpoint(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<UpdateInvestorProfileRequest>,
) -> Result<Json<InvestorProfileResponse>, (StatusCode, String)> {
    // Extract user ID from X-User-ID header (temporary until JWT middleware is implemented)
    let user_id_str = headers
        .get("X-User-ID")
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing user ID header".to_string(),
        ))?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid user ID header".to_string(),
            )
        })?;

    let user_id = uuid::Uuid::parse_str(user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    info!("📝 API: Updating investor profile for user: {}", user_id);

    // Convert to database update request
    let db_request = DbUpdateInvestorProfileRequest {
        accredited_investor: request.accredited_investor,
        investment_capacity: request.investment_capacity,
        regulatory_jurisdiction: request.regulatory_jurisdiction,
    };

    match update_investor_profile(&state.pool, user_id, &db_request).await {
        Ok(profile) => {
            // Get the updated profile with identity info for full response
            match get_investor_profile_with_identity(&state.pool, user_id).await {
                Ok(Some((_, identity))) => Ok(Json(InvestorProfileResponse {
                    profile_id: profile.id.to_string(),
                    user_id: profile.user_id.to_string(),
                    email: identity.email,
                    user_type: UserType::Investor,
                    kyc_status: identity.kyc_status,
                    kyc_submitted_at: identity.kyc_submitted_at,
                    kyc_verified_at: identity.kyc_verified_at,
                    kyc_investor_type: identity.kyc_investor_type.map(|t| t.to_string()),
                    accredited_investor: profile.accredited_investor,
                    investment_capacity: profile.investment_capacity,
                    regulatory_jurisdiction: profile.regulatory_jurisdiction,
                    profile_created_at: profile.created_at,
                    profile_updated_at: profile.updated_at,
                    first_name: identity.first_name,
                    last_name: identity.last_name,
                    jurisdiction: identity.jurisdiction,
                    wallet_address: identity.wallet_address,
                })),
                Ok(None) => Err((
                    StatusCode::NOT_FOUND,
                    "Investor profile not found".to_string(),
                )),
                Err(e) => {
                    error!("❌ API: Failed to get updated investor profile: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to get updated investor profile".to_string(),
                    ))
                }
            }
        }
        Err(e) => {
            if e.to_string().contains("not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    "Investor profile not found".to_string(),
                ))
            } else {
                error!("❌ API: Failed to update investor profile: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update investor profile".to_string(),
                ))
            }
        }
    }
}

// ---- DEPOSIT & BALANCE ENDPOINTS ----

/// Get user balance by wallet address
#[utoipa::path(
    get,
    path = "/api/v1/user/{address}/balance",
    responses(
        (status = 200, description = "User balance retrieved successfully", body = UserBalanceResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("address" = String, Path, description = "User's wallet address")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_balance_by_address_endpoint(
    Path(address): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalanceResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user balance by address: {}", address);

    match get_user_balance_by_address(&state.pool, &address).await {
        Ok(Some(balance)) => {
            let response = UserBalanceResponse {
                user_id: None, // We don't have user_id when querying by address directly
                wallet_address: address,
                available_balance: balance.to_string(),
                currency: "USDC".to_string(),
            };
            info!("✅ API: User balance retrieved successfully");
            Ok(Json(response))
        }
        Ok(None) => {
            warn!("⚠️ API: User not found for address: {}", address);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user balance: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user balance".to_string(),
            ))
        }
    }
}

/// Get user balance by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/balance",
    responses(
        (status = 200, description = "User balance retrieved successfully", body = UserBalanceResponse),
        (status =404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_balance_by_id_endpoint(
    Path(user_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserBalanceResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user balance by ID: {}", user_id);

    let user_uuid = match parse_user_id(&user_id) {
        Ok(id) => id,
        Err(err) => {
            warn!("⚠️ API: Invalid user ID format: {}", err);
            return Err((StatusCode::BAD_REQUEST, err));
        }
    };

    // First get user details to get wallet address
    match get_identity_by_id(&state.pool, user_uuid).await {
        Ok(Some(identity)) => match get_user_balance_by_id(&state.pool, user_uuid).await {
            Ok(Some(balance)) => {
                let response = UserBalanceResponse {
                    user_id: Some(user_id),
                    wallet_address: identity
                        .wallet_address
                        .unwrap_or_else(|| "Not set".to_string()),
                    available_balance: balance.to_string(),
                    currency: "USDC".to_string(),
                };
                info!("✅ API: User balance retrieved successfully");
                Ok(Json(response))
            }
            Ok(None) => {
                warn!("⚠️ API: Balance not found for user ID: {}", user_id);
                Err((StatusCode::NOT_FOUND, "Balance not found".to_string()))
            }
            Err(e) => {
                error!("❌ API: Failed to get user balance: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to retrieve user balance".to_string(),
                ))
            }
        },
        Ok(None) => {
            warn!("⚠️ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user identity: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// Get user transactions by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/transactions",
    responses(
        (status = 200, description = "User transactions retrieved successfully", body = UserTransactionsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID (UUID)")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_transactions_endpoint(
    Path(user_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<UserTransactionsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user transactions by ID: {}", user_id);

    let user_uuid = match parse_user_id(&user_id) {
        Ok(id) => id,
        Err(err) => {
            warn!("⚠️ API: Invalid user ID format: {}", err);
            return Err((StatusCode::BAD_REQUEST, err));
        }
    };

    // First get user details to get wallet address
    match get_identity_by_id(&state.pool, user_uuid).await {
        Ok(Some(identity)) => {
            if let Some(wallet_address) = &identity.wallet_address {
                match get_deposits_by_user_address(&state.pool, wallet_address).await {
                    Ok(deposits) => {
                        let transaction_infos: Vec<DepositTransactionInfo> = deposits
                            .into_iter()
                            .map(|deposit| DepositTransactionInfo {
                                id: deposit.id,
                                user_address: deposit.user_address,
                                amount: deposit.amount.to_string(),
                                deposit_amount_usdc: deposit
                                    .deposit_amount_usdc
                                    .map(|d| d.to_string()),
                                transaction_hash: deposit.transaction_hash,
                                status: deposit.status,
                                confirmation_count: deposit.confirmation_count,
                                required_confirmations: deposit.required_confirmations,
                                block_number: deposit.block_number,
                                created_at: deposit.created_at,
                                processed_at: deposit.processed_at,
                            })
                            .collect();

                        let response = UserTransactionsResponse {
                            total_count: transaction_infos.len(),
                            transactions: transaction_infos,
                        };

                        info!("✅ API: User transactions retrieved successfully");
                        Ok(Json(response))
                    }
                    Err(e) => {
                        error!("❌ API: Failed to get user transactions: {}", e);
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to retrieve user transactions".to_string(),
                        ))
                    }
                }
            } else {
                warn!("⚠️ API: User has no wallet address set: {}", user_id);
                Err((
                    StatusCode::BAD_REQUEST,
                    "User has no wallet address set".to_string(),
                ))
            }
        }
        Ok(None) => {
            warn!("⚠️ API: User not found: {}", user_id);
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            error!("❌ API: Failed to get user identity: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user".to_string(),
            ))
        }
    }
}

/// Get transaction by hash
#[utoipa::path(
    get,
    path = "/api/v1/transactions/{tx_hash}",
    responses(
        (status = 200, description = "Transaction retrieved successfully", body = TransactionStatusResponse),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("tx_hash" = String, Path, description = "Transaction hash")
    ),
    tag = "Transactions"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_transaction_by_hash_endpoint(
    Path(tx_hash): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<TransactionStatusResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting transaction by hash: {}", tx_hash);

    match get_deposit_by_tx_hash(&state.pool, &tx_hash).await {
        Ok(Some(deposit)) => {
            let transaction_info = DepositTransactionInfo {
                id: deposit.id,
                user_address: deposit.user_address,
                amount: deposit.amount.to_string(),
                deposit_amount_usdc: deposit.deposit_amount_usdc.map(|d| d.to_string()),
                transaction_hash: deposit.transaction_hash,
                status: deposit.status,
                confirmation_count: deposit.confirmation_count,
                required_confirmations: deposit.required_confirmations,
                block_number: deposit.block_number,
                created_at: deposit.created_at,
                processed_at: deposit.processed_at,
            };

            let response = TransactionStatusResponse {
                found: true,
                transaction: Some(transaction_info),
            };

            info!("✅ API: Transaction retrieved successfully");
            Ok(Json(response))
        }
        Ok(None) => {
            info!("ℹ️ API: Transaction not found: {}", tx_hash);
            let response = TransactionStatusResponse {
                found: false,
                transaction: None,
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to get transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve transaction".to_string(),
            ))
        }
    }
}

// ---- LOAN MANAGEMENT ENDPOINTS ----

/// Create a new loan
#[utoipa::path(
    post,
    path = "/api/v1/loan",
    request_body = CreateLoanRequest,
    responses(
        (status = 200, description = "Loan created successfully", body = LoanDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
async fn create_loan_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateLoanRequest>,
) -> Result<Json<LoanDetailsResponse>, (StatusCode, String)> {
    info!(
        "💰 API: Creating loan: borrower={}, lender={}, amount={}",
        request.borrower_id, request.lender_id, request.principal_amount
    );

    // Parse UUIDs
    let borrower_id = uuid::Uuid::parse_str(&request.borrower_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid borrower ID format".to_string(),
        )
    })?;
    let lender_id = uuid::Uuid::parse_str(&request.lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid lender ID format".to_string(),
        )
    })?;

    // Parse principal amount
    let principal_amount = request
        .principal_amount
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?;

    // Parse optional maturity date
    let maturity_date = if let Some(term_days) = request.term_days {
        Some(chrono::Utc::now() + chrono::Duration::days(term_days as i64))
    } else {
        None
    };

    // Create loan in database
    let loan = create_loan(
        &state.pool,
        borrower_id,
        lender_id,
        principal_amount,
        request.interest_rate_bps as i32,
        maturity_date,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create loan: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create loan".to_string(),
        )
    })?;

    let response = LoanDetailsResponse {
        loan_id: loan.id.to_string(),
        borrower_id: loan.borrower_id.to_string(),
        lender_id: loan.lender_id.to_string(),
        principal_amount: loan.principal_amount.to_string(),
        outstanding_balance: loan.outstanding_balance.to_string(),
        interest_rate_bps: loan.interest_rate_bps as u16,
        loan_status: loan.loan_status,
        originated_at: loan.originated_at,
        last_updated: Some(loan.updated_at),
    };

    info!("✅ API: Loan created successfully: {}", response.loan_id);
    Ok(Json(response))
}

/// List loans
#[utoipa::path(
    get,
    path = "/api/v1/loans",
    responses(
        (status = 200, description = "Loans listed successfully", body = ListLoansResponse),
        (status = 500, description = "Internal server error")
    ),
    params(ListLoansQuery),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
async fn list_loans_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListLoansQuery>,
) -> Result<Json<ListLoansResponse>, (StatusCode, String)> {
    info!("📋 API: Listing loans with filters");

    // Parse optional UUIDs
    let borrower_id = if let Some(id_str) = &params.borrower_id {
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid borrower ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    let lender_id = if let Some(id_str) = &params.lender_id {
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid lender ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    // Get loans from database
    let loans = list_loans(
        &state.pool,
        params.limit.map(|l| l as i64),
        params.offset.map(|o| o as i64),
        params.status.as_deref(),
        borrower_id,
        lender_id,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to list loans: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list loans".to_string(),
        )
    })?;

    // Convert to response format
    let loan_responses: Vec<LoanDetailsResponse> = loans
        .into_iter()
        .map(|loan| LoanDetailsResponse {
            loan_id: loan.id.to_string(),
            borrower_id: loan.borrower_id.to_string(),
            lender_id: loan.lender_id.to_string(),
            principal_amount: loan.principal_amount.to_string(),
            outstanding_balance: loan.outstanding_balance.to_string(),
            interest_rate_bps: loan.interest_rate_bps as u16,
            loan_status: loan.loan_status,
            originated_at: loan.originated_at,
            last_updated: Some(loan.updated_at),
        })
        .collect();

    let total_count = loan_responses.len();
    let limit = params.limit.unwrap_or(100) as usize;
    let has_more = total_count >= limit;

    let response = ListLoansResponse {
        loans: loan_responses,
        total_count,
        has_more,
    };

    info!("✅ API: Listed {} loans", response.total_count);
    Ok(Json(response))
}

/// Get loan details by ID
#[utoipa::path(
    get,
    path = "/api/v1/loan/{loan_id}",
    responses(
        (status = 200, description = "Loan details retrieved successfully", body = LoanDetailsResponse),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("loan_id" = String, Path, description = "Loan ID")
    ),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]

/// Update loan
#[instrument(skip(state), level = "info")]
async fn update_loan_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<String>,
    Json(request): Json<UpdateLoanRequest>,
) -> Result<Json<UpdateLoanResponse>, (StatusCode, String)> {
    info!("📝 API: Updating loan: {}", loan_id);

    // Parse loan ID
    let loan_uuid = uuid::Uuid::parse_str(&loan_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Convert request fields to database types
    let principal_amount = if let Some(amount_str) = &request.principal_amount {
        Some(amount_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?)
    } else {
        None
    };

    let outstanding_balance = if let Some(balance_str) = &request.outstanding_balance {
        Some(balance_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid outstanding balance".to_string(),
            )
        })?)
    } else {
        None
    };

    let interest_rate_bps = request.interest_rate_bps.map(|rate| rate as i32);

    let maturity_date = if let Some(date_opt) = &request.maturity_date {
        Some(match date_opt {
            Some(date_str) => Some(
                chrono::DateTime::parse_from_rfc3339(date_str)
                    .map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            "Invalid maturity date format".to_string(),
                        )
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        })
    } else {
        None
    };

    // Update the loan in the database
    let updated_loan = update_loan(
        &state.pool,
        loan_uuid,
        principal_amount,
        outstanding_balance,
        interest_rate_bps,
        request.loan_status.as_deref(),
        maturity_date,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to update loan: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update loan".to_string(),
        )
    })?;

    match updated_loan {
        Some(loan) => {
            // Track which fields were updated
            let mut updated_fields = Vec::new();
            if request.principal_amount.is_some() {
                updated_fields.push("principal_amount".to_string());
            }
            if request.outstanding_balance.is_some() {
                updated_fields.push("outstanding_balance".to_string());
            }
            if request.interest_rate_bps.is_some() {
                updated_fields.push("interest_rate_bps".to_string());
            }
            if request.loan_status.is_some() {
                updated_fields.push("loan_status".to_string());
            }
            if request.maturity_date.is_some() {
                updated_fields.push("maturity_date".to_string());
            }

            // Convert to response format
            let loan_details = LoanDetailsResponse {
                loan_id: loan.id.to_string(),
                borrower_id: loan.borrower_id.to_string(),
                lender_id: loan.lender_id.to_string(),
                principal_amount: loan.principal_amount.to_string(),
                outstanding_balance: loan.outstanding_balance.to_string(),
                interest_rate_bps: loan.interest_rate_bps as u16,
                loan_status: loan.loan_status,
                originated_at: loan.originated_at,
                last_updated: Some(loan.updated_at),
            };

            let response = UpdateLoanResponse {
                loan_id: loan.id.to_string(),
                updated_fields,
                updated_at: loan.updated_at,
                loan_details,
            };

            info!("✅ API: Loan updated successfully: {}", loan_id);
            Ok(Json(response))
        }
        None => {
            warn!("❌ API: Loan not found: {}", loan_id);
            Err((StatusCode::NOT_FOUND, "Loan not found".to_string()))
        }
    }
}

// ---- PAYMENT PROCESSING ENDPOINTS ----

// ---- STATE MANAGEMENT & ZK PROOF ENDPOINTS ----

/// Get current state summary
#[instrument(skip(_state), level = "info")]
async fn get_state_summary_endpoint(
    State(_state): State<ApiState>,
) -> Result<Json<StateSummaryResponse>, (StatusCode, String)> {
    info!("🎯 API: Getting current state summary");

    // TODO: Implement state summary with database integration
    let response = StateSummaryResponse {
        current_state_root: "0x789abc...".to_string(),
        transaction_count: 1250,
        last_batch_id: Some(uuid::Uuid::new_v4().to_string()),
        last_updated: Utc::now(),
        loan_summary: LoanStatsSummary {
            total_loans: 150,
            total_principal: "50000000".to_string(),
            total_outstanding: "48500000".to_string(),
        },
        user_summary: UserStatsSummary {
            total_users: 85,
            kyc_approved_users: 72,
        },
    };

    info!("✅ API: State summary retrieved");
    Ok(Json(response))
}

/// Generate transaction inclusion proof
#[instrument(skip(_state), level = "info")]
async fn get_transaction_proof_endpoint(
    State(_state): State<ApiState>,
    Path((transaction_type, transaction_id)): Path<(String, String)>,
) -> Result<Json<TransactionProofResponse>, (StatusCode, String)> {
    info!(
        "🔐 API: Generating inclusion proof: type={}, id={}",
        transaction_type, transaction_id
    );

    // TODO: Implement proof generation with database integration
    let response = TransactionProofResponse {
        transaction_id,
        transaction_type: transaction_type.clone(),
        proof: ProofData {
            merkle_proof: "0x123abc...".to_string(),
            batch_proof: "0x456def...".to_string(),
            state_root: "0x789abc...".to_string(),
        },
        verification_data: VerificationData {
            batch_id: uuid::Uuid::new_v4().to_string(),
            batch_timestamp: 1642681500,
            transaction_index: 42,
        },
    };

    info!("✅ API: Transaction proof generated");
    Ok(Json(response))
}

/// Verify transaction inclusion
#[instrument(skip(_state), level = "info")]
async fn verify_transaction_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<VerifyTransactionRequest>,
) -> Result<Json<TransactionVerificationResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Verifying transaction inclusion: id={}",
        request.transaction_id
    );

    // TODO: Implement transaction verification with database integration
    let response = TransactionVerificationResponse {
        is_valid: true,
        verification_result: VerificationResult {
            merkle_proof_valid: true,
            batch_proof_valid: true,
            transaction_included: true,
            transaction_type_matches: true,
        },
        verified_at: Utc::now(),
    };

    info!("✅ API: Transaction verified successfully");
    Ok(Json(response))
}

/// Get state transition history
#[instrument(skip(_state), level = "info")]
async fn get_state_history_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<BatchListQuery>, // Reuse query params for limit
) -> Result<Json<StateHistoryResponse>, (StatusCode, String)> {
    info!("📜 API: Getting state transition history");

    // TODO: Implement state history with database integration
    let response = StateHistoryResponse {
        state_transitions: vec![],
        total_batches: 0,
    };

    info!("✅ API: State history retrieved");
    Ok(Json(response))
}

// ---- BATCH OPERATIONS FOR LOANS ----

/// Create batch from pending loan transactions
#[instrument(skip(_state), level = "info")]
async fn create_loan_batch_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<CreateLoanBatchRequest>,
) -> Result<Json<CreateLoanBatchResponse>, (StatusCode, String)> {
    info!(
        "🔄 API: Creating loan batch: max_count={:?}, force={:?}",
        request.max_transaction_count, request.force_create
    );

    // TODO: Implement loan batch creation with database integration
    let batch_id = uuid::Uuid::new_v4().to_string();

    let response = CreateLoanBatchResponse {
        batch_id,
        transaction_count: 25,
        batch_status: "Created".to_string(),
        estimated_proof_time: "8m".to_string(),
        created_at: Utc::now(),
    };

    info!("✅ API: Loan batch created successfully");
    Ok(Json(response))
}

/// Get loan batch details
#[instrument(skip(state), level = "info")]
async fn get_loan_batch_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<String>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!("🔍 API: Getting loan batch details: batch_id={}", batch_id);

    // TODO: Implement loan batch lookup with database integration
    // For now, delegate to existing batch endpoint
    let batch_id_int: i32 = batch_id.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid batch ID format".to_string(),
        )
    })?;

    get_batch_endpoint(State(state), Path(batch_id_int)).await
}

/// Get pending loan transactions ready for batching
#[instrument(skip(_state), level = "info")]
async fn get_pending_loan_transactions_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<BatchListQuery>, // Reuse query params for limit
) -> Result<Json<PendingLoanTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting pending loan transactions");

    // TODO: Implement pending loan transactions lookup with database integration
    let response = PendingLoanTransactionsResponse {
        pending_transactions: vec![],
        total_pending: 0,
        total_amount: "0".to_string(),
        oldest_pending: None,
    };

    info!("✅ API: Pending loan transactions retrieved");
    Ok(Json(response))
}

// ============================================================================
// LOAN OPERATION INCLUSION PROOF ENDPOINT HANDLERS
// ============================================================================

/// Request body for generating loan operation inclusion proof
#[derive(Debug, Deserialize, ToSchema)]
struct GenerateLoanOperationProofRequest {
    user_id: uuid::Uuid,
    loan_id: Option<uuid::Uuid>,
    payment_id: Option<uuid::Uuid>,
    operation_type: String,
    batch_id: Option<i32>,
    user_role: String,
}

/// Response for loan operation inclusion proof generation
#[derive(Debug, Serialize, ToSchema)]
struct LoanOperationProofResponse {
    #[schema(value_type = Object)]
    proof: LoanInclusionProof,
    generated_at: DateTime<Utc>,
    verification_summary: serde_json::Value,
}

/// Generate loan operation inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn generate_loan_operation_proof_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<GenerateLoanOperationProofRequest>,
) -> Result<Json<LoanOperationProofResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Generating loan operation inclusion proof for user {}",
        request.user_id
    );

    // Use the unified batch service to generate proof
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool,
        state.ads_service,
        state.config.max_batch_size,
    );

    let proof = unified_service
        .generate_loan_inclusion_proof(
            request.user_id,
            request.loan_id,
            request.payment_id,
            request.operation_type,
            request.batch_id,
            request.user_role,
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Generate verification summary
    let verification_summary = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = LoanOperationProofResponse {
        proof,
        generated_at: Utc::now(),
        verification_summary,
    };

    info!("✅ API: Loan operation inclusion proof generated successfully");
    Ok(Json(response))
}

/// Generate loan payment inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn generate_loan_payment_proof_endpoint(
    State(state): State<ApiState>,
    Path((user_id, loan_id, payment_id, batch_id)): Path<(uuid::Uuid, uuid::Uuid, uuid::Uuid, i32)>,
) -> Result<Json<LoanOperationProofResponse>, (StatusCode, String)> {
    info!(
        "💰 API: Generating loan payment proof for payment {} in batch {}",
        payment_id, batch_id
    );

    // Use the unified batch service to generate proof
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let proof = unified_service
        .generate_loan_payment_proof(user_id, loan_id, payment_id, batch_id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Generate verification summary
    let verification_summary = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = LoanOperationProofResponse {
        proof,
        generated_at: Utc::now(),
        verification_summary,
    };

    info!("✅ API: Loan payment inclusion proof generated successfully");
    Ok(Json(response))
}

/// Verify loan inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn verify_loan_proof_endpoint(
    State(state): State<ApiState>,
    Json(proof): Json<LoanInclusionProof>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🔍 API: Verifying loan inclusion proof");

    // Use the unified batch service to verify proof
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool,
        state.ads_service,
        state.config.max_batch_size,
    );

    let is_valid = unified_service
        .verify_loan_proof(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let report = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = serde_json::json!({
        "valid": is_valid,
        "verified_at": Utc::now(),
        "proof_details": report
    });

    info!(
        "✅ API: Loan inclusion proof verification completed: valid={}",
        is_valid
    );
    Ok(Json(response))
}

/// Generate proof verification report endpoint
#[instrument(skip(state), level = "info")]
async fn generate_proof_report_endpoint(
    State(state): State<ApiState>,
    Json(proof): Json<LoanInclusionProof>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("📊 API: Generating proof verification report");

    // Use the unified batch service to generate report
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let report = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    info!("✅ API: Proof verification report generated");
    Ok(Json(report))
}

/// Batch verify multiple loan proofs endpoint
#[instrument(skip(state), level = "info")]
async fn batch_verify_proofs_endpoint(
    State(state): State<ApiState>,
    Json(proofs): Json<Vec<LoanInclusionProof>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!(
        "🔍 API: Batch verifying {} loan inclusion proofs",
        proofs.len()
    );

    // Use the unified batch service to batch verify proofs
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let results = unified_service
        .batch_verify_proofs(&proofs)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let valid_count = results.iter().filter(|&&v| v).count();
    let response = serde_json::json!({
        "total_proofs": proofs.len(),
        "valid_proofs": valid_count,
        "invalid_proofs": proofs.len() - valid_count,
        "results": results,
        "verified_at": Utc::now()
    });

    info!(
        "✅ API: Batch verification completed: {}/{} proofs valid",
        valid_count,
        proofs.len()
    );
    Ok(Json(response))
}

/// Get loan audit trail endpoint
#[instrument(skip(state), level = "info")]
async fn get_loan_audit_trail_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<AuditEvent>>, (StatusCode, String)> {
    info!("📋 API: Getting loan audit trail for loan {}", loan_id);

    // Use the unified batch service to get audit trail
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let audit_trail = unified_service
        .get_loan_audit_trail(loan_id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    info!(
        "✅ API: Retrieved {} audit events for loan",
        audit_trail.len()
    );
    Ok(Json(audit_trail))
}

/// Get loan lifecycle summary endpoint
#[instrument(skip(state), level = "info")]
async fn get_loan_lifecycle_summary_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<uuid::Uuid>,
) -> Result<Json<LoanLifecycleSummary>, (StatusCode, String)> {
    info!(
        "📈 API: Getting loan lifecycle summary for loan {}",
        loan_id
    );

    // Use the unified batch service to get lifecycle summary
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let summary = unified_service
        .get_loan_lifecycle_summary(loan_id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    info!(
        "✅ API: Generated loan lifecycle summary: {} operations, {} payments",
        summary.total_operations, summary.total_payments
    );
    Ok(Json(summary))
}

// ============================================================================
// CREDIT LINE REQUEST ENDPOINT HANDLERS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateCreditLineRequestBody {
    pub merchant_id: String,
    pub requested_amount: u64,     // Amount in cents
    pub requested_rate_bps: u64,   // Rate in basis points
    pub requested_tenor_days: u32, // Term in days
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreditLineRequestResponse {
    pub request_id: String,
    pub merchant_id: String,
    pub requested_amount: String,
    pub requested_rate_apr: String,
    pub requested_tenor_days: u32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Create a credit line request
#[utoipa::path(
    post,
    path = "/api/credit-lines/request",
    request_body = CreateCreditLineRequestBody,
    responses(
        (status = 200, description = "Credit line request created successfully", body = CreditLineRequestResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Credit Lines"
)]
#[instrument(skip(state), level = "info")]
async fn create_credit_line_request_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateCreditLineRequestBody>,
) -> Result<Json<CreditLineRequestResponse>, (StatusCode, String)> {
    info!(
        "📝 API: Creating credit line request for merchant: {}",
        request.merchant_id
    );

    // Parse merchant ID
    let merchant_id = uuid::Uuid::parse_str(&request.merchant_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid merchant ID format".to_string(),
        )
    })?;

    // Convert values to database format
    let amount_decimal =
        rust_decimal::Decimal::from(request.requested_amount) / rust_decimal::Decimal::from(100); // cents to dollars
    let rate_decimal = rust_decimal::Decimal::from(request.requested_rate_bps)
        / rust_decimal::Decimal::from(10000); // bps to decimal

    // Create the request data
    let request_data = arda_credit_db::CreateCreditLineRequestData {
        merchant_id,
        requested_amount: amount_decimal,
        requested_rate_apr: Some(rate_decimal),
        requested_tenor_days: request.requested_tenor_days as i32,
        request_description: Some("Credit line request via API".to_string()),
    };

    // Create credit line request
    match arda_credit_db::create_credit_line_request(&state.pool, request_data).await {
        Ok(credit_request) => {
            info!("✅ API: Credit line request created: {}", credit_request.id);
            Ok(Json(CreditLineRequestResponse {
                request_id: credit_request.id.to_string(),
                merchant_id: credit_request.merchant_id.to_string(),
                requested_amount: credit_request.requested_amount.to_string(),
                requested_rate_apr: credit_request
                    .requested_rate_apr
                    .unwrap_or_default()
                    .to_string(),
                requested_tenor_days: credit_request.requested_tenor_days.unwrap_or_default()
                    as u32,
                status: credit_request.status.unwrap_or_default(),
                created_at: credit_request
                    .created_at
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(chrono::Utc::now),
            }))
        }
        Err(e) => {
            error!("❌ API: Failed to create credit line request: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create credit line request".to_string(),
            ))
        }
    }
}

// ============================================================================
// WITHDRAWAL ENDPOINT HANDLERS
// ============================================================================

/// Initiate a new withdrawal request
#[utoipa::path(
    post,
    path = "/api/v1/withdrawals",
    responses(
        (status = 200, description = "Withdrawal initiated successfully"),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
async fn initiate_withdrawal_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<crate::InitiateWithdrawalRequest>,
) -> Result<Json<crate::InitiateWithdrawalResponse>, StatusCode> {
    match state.withdrawal_service.initiate_withdrawal(request).await {
        Ok(response) => {
            info!("Withdrawal initiated successfully: {}", response.request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to initiate withdrawal: {:?}", e);
            match e {
                crate::WithdrawalServiceError::UserNotFound(_) => Err(StatusCode::NOT_FOUND),
                crate::WithdrawalServiceError::InvalidAmount(_) => Err(StatusCode::BAD_REQUEST),
                crate::WithdrawalServiceError::KycValidationFailed(_) => {
                    Err(StatusCode::BAD_REQUEST)
                }
                crate::WithdrawalServiceError::InsufficientBalance(_) => {
                    Err(StatusCode::BAD_REQUEST)
                }
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get withdrawal request status
#[utoipa::path(
    get,
    path = "/api/v1/withdrawals/{request_id}",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID")
    ),
    responses(
        (status = 200, description = "Withdrawal status retrieved"),
        (status = 404, description = "Withdrawal request not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
async fn get_withdrawal_status_endpoint(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
) -> Result<Json<crate::WithdrawalStatusResponse>, StatusCode> {
    match state
        .withdrawal_service
        .get_withdrawal_status(request_id)
        .await
    {
        Ok(response) => {
            info!("Retrieved withdrawal status for request {}", request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get withdrawal status: {:?}", e);
            match e {
                crate::WithdrawalServiceError::Sql(sqlx::Error::RowNotFound) => {
                    Err(StatusCode::NOT_FOUND)
                }
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get user's available balance for withdrawals
#[utoipa::path(
    get,
    path = "/api/v1/users/{address}/available-balance",
    params(
        ("address" = String, Path, description = "User's wallet address")
    ),
    responses(
        (status = 200, description = "Available balance retrieved"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
async fn get_user_available_balance_endpoint(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<crate::AvailableBalance>, StatusCode> {
    match state.balance_service.get_available_balance(&address).await {
        Ok(Some(balance)) => {
            info!("Retrieved available balance for address {}", address);
            Ok(Json(balance))
        }
        Ok(None) => {
            warn!("No balance found for address {}", address);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to get available balance: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get user's withdrawal history
#[utoipa::path(
    get,
    path = "/api/v1/users/{address}/withdrawals",
    params(
        ("address" = String, Path, description = "User's wallet address")
    ),
    responses(
        (status = 200, description = "Withdrawal history retrieved"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
async fn get_user_withdrawal_history_endpoint(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<Vec<crate::WithdrawalStatusResponse>>, StatusCode> {
    match state
        .withdrawal_service
        .get_user_withdrawal_history(&address)
        .await
    {
        Ok(history) => {
            info!("Retrieved withdrawal history for address {}", address);
            Ok(Json(history))
        }
        Err(e) => {
            error!("Failed to get withdrawal history: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Cancel a withdrawal request
#[utoipa::path(
    post,
    path = "/api/v1/withdrawals/{request_id}/cancel",
    params(
        ("request_id" = i32, Path, description = "Withdrawal request ID to cancel")
    ),
    responses(
        (status = 200, description = "Withdrawal cancelled successfully"),
        (status = 400, description = "Cannot cancel withdrawal"),
        (status = 404, description = "Withdrawal request not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Withdrawals"
)]
#[instrument(skip(state), level = "info")]
async fn cancel_withdrawal_endpoint(
    State(state): State<ApiState>,
    Path(request_id): Path<i32>,
) -> Result<Json<crate::WithdrawalStatusResponse>, StatusCode> {
    match state.withdrawal_service.cancel_withdrawal(request_id).await {
        Ok(response) => {
            info!("Cancelled withdrawal request {}", request_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to cancel withdrawal: {:?}", e);
            match e {
                crate::WithdrawalServiceError::Sql(sqlx::Error::RowNotFound) => {
                    Err(StatusCode::NOT_FOUND)
                }
                crate::WithdrawalServiceError::InvalidAmount(_) => Err(StatusCode::BAD_REQUEST),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get pending withdrawal requests (admin endpoint)
#[utoipa::path(
    get,
    path = "/api/v1/admin/withdrawals/pending",
    params(
        ("limit" = Option<i32>, Query, description = "Maximum number of requests to return")
    ),
    responses(
        (status = 200, description = "Pending withdrawals retrieved"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Admin"
)]
#[instrument(skip(state), level = "info")]
async fn get_pending_withdrawals_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::WithdrawalStatusResponse>>, StatusCode> {
    let limit = params.get("limit").and_then(|s| s.parse().ok());

    match state
        .withdrawal_service
        .get_pending_withdrawals(limit)
        .await
    {
        Ok(withdrawals) => {
            info!("Retrieved {} pending withdrawals", withdrawals.len());
            Ok(Json(withdrawals))
        }
        Err(e) => {
            error!("Failed to get pending withdrawals: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// OPENAPI SPECIFICATION
// ============================================================================

/// OpenAPI specification for the Arda Credit API
#[derive(OpenApi)]
#[openapi(
    paths(
        // Core endpoints
        health_check,
        api_info,

        // Transaction endpoints
        // submit_transaction_endpoint, // moved to state_handlers.rs
        // get_pending_transactions_endpoint, // moved to state_handlers.rs

        // Batch endpoints
        // create_batch_endpoint, // moved to state_handlers.rs
        get_batches_endpoint,
        get_batch_endpoint,
        update_batch_proof_endpoint,
        trigger_batch_endpoint,
        get_batch_processor_stats_endpoint,

        // State endpoints
        get_current_state_endpoint,
        get_contract_data_endpoint,

        // Authentication endpoints
        // login_endpoint, // moved to authentication_handlers.rs
        // logout_endpoint, // moved to authentication_handlers.rs
        // validate_invite_code_endpoint, // moved to authentication_handlers.rs
        // generate_invite_code_endpoint, // moved to authentication_handlers.rs

        // User Management endpoints
        // register_user_endpoint, // moved to user_handlers.rs
        // list_identities_endpoint, // moved to user_handlers.rs  
        // get_user_details_endpoint, // moved to user_handlers.rs
        update_user_endpoint,
        delete_user_endpoint,
        submit_kyc_endpoint,
        update_kyc_status_endpoint,
        // list_pending_kyc_endpoint, // moved to admin_handlers.rs
        // get_kyc_statistics_endpoint, // moved to admin_handlers.rs

        // Investor Management endpoints - MOVED to merchant_handlers.rs
        // register_investor_endpoint,
        // get_investor_profile_endpoint,
        // update_investor_profile_endpoint,

        // Merchant Management endpoints - MOVED to merchant_handlers.rs
        // register_merchant_endpoint,
        // get_merchant_profile_endpoint,
        // update_merchant_profile_endpoint,

        // Loan Management endpoints
        create_loan_endpoint,
        list_loans_endpoint,
        // Liquidity Pool endpoints
        crate::pool_handlers::list_pools,
        crate::pool_handlers::get_pool_details,
        crate::pool_handlers::create_pool,
        crate::pool_handlers::update_pool_status,
        list_pools_endpoint,
        create_pool_endpoint,
        get_pool_endpoint,
        update_pool_status_endpoint,
        find_matching_pools_endpoint,
        // Credit Line endpoints
        crate::credit_line_handlers::list_credit_line_requests,
        crate::credit_line_handlers::browse_available_pools,
        crate::credit_line_handlers::check_credit_eligibility,
        crate::credit_line_handlers::submit_credit_line_request,

        // Financial Transaction endpoints
        crate::financial_transaction_handlers::record_account_deposit,
        crate::financial_transaction_handlers::record_account_withdrawal,
        crate::financial_transaction_handlers::get_account_financial_transactions,
        crate::financial_transaction_handlers::record_pool_funding,
        crate::financial_transaction_handlers::record_pool_withdrawal,
        crate::financial_transaction_handlers::get_pool_financial_transactions,
        crate::financial_transaction_handlers::record_credit_line_draw,
        crate::financial_transaction_handlers::record_credit_line_payment,
        crate::financial_transaction_handlers::get_credit_line_financial_transactions,
        crate::financial_transaction_handlers::get_user_financial_transactions,
        crate::financial_transaction_handlers::get_user_financial_summary,
        crate::financial_transaction_handlers::get_user_pool_financial_transactions,
        crate::financial_transaction_handlers::get_merchant_credit_line_financial_transactions
    ),
    components(
        schemas(
            // Health & Core
            HealthResponse,
            ApiInfoResponse,
            EndpointInfo,
            ContractSubmissionData,
            ContractPublicData,
            ContractPrivateData,

            // Transaction Models
            SubmitTransactionRequest,
            SubmitTransactionResponse,
            TransactionInfo,
            PendingTransactionsResponse,

            // Batch Models
            CreateBatchRequest,
            CreateBatchResponse,
            BatchListQuery,
            BatchListResponse,
            BatchInfo,
            UpdateBatchProofRequest,
            CurrentStateResponse,
            BatchProcessorStatsResponse,
            TriggerBatchResponse,

            // Authentication
            LoginRequest,
            LoginResponse,
            UserSummary,
            LogoutRequest,
            LogoutResponse,
            InviteCodeResponse,
            ValidateInviteCodeResponse,
            GenerateInviteCodeRequest,
            GenerateInviteCodeResponse,

            // User Management
            RegisterUserRequest,
            RegisterUserResponse,
            UserDetailsResponse,
            SubmitKycRequest,
            KycData,
            SubmitKycResponse,
            UpdateKycStatusRequest,
            UpdateUserRequest,
            UpdateUserResponse,
            DeleteUserResponse,
            ListIdentitiesQuery,
            ListIdentitiesResponse,

            // Investor Management
            RegisterInvestorRequest,
            RegisterInvestorResponse,
            InvestorProfileResponse,
            UpdateInvestorProfileRequest,

            // Merchant Management
            RegisterMerchantRequest,
            RegisterMerchantResponse,
            MerchantProfileResponse,
            UpdateMerchantProfileRequest,

            // Loan Management
            CreateLoanRequest,
            LoanDetailsResponse,
            ListLoansQuery,
            ListLoansResponse,
            LoanStatsResponse,
            UpdateLoanRequest,
            UpdateLoanResponse,
            DeleteLoanResponse,

            // Payment Processing
            RecordPaymentRequest,
            PaymentResponse,
            PaymentDetailsResponse,
            LoanPaymentHistoryResponse,

            // State Management
            StateSummaryResponse,
            LoanStatsSummary,
            UserStatsSummary,
            TransactionProofResponse,
            ProofData,
            VerificationData,
            VerifyTransactionRequest,
            TransactionVerificationResponse,
            VerificationResult,
            StateHistoryResponse,
            StateTransition,

            // Batch Operations
            CreateLoanBatchRequest,
            CreateLoanBatchResponse,
            PendingLoanTransactionsResponse,
            PendingTransactionInfo,

            // Proof Management
            GenerateLoanOperationProofRequest,
            LoanOperationProofResponse,

            // Liquidity Pool Management
            CreatePoolRequest,
            CreatePoolResponse,
            PoolDetailsResponse,
            PoolListResponse,
            UpdatePoolStatusRequest,
            PoolMatchRequest,

            // Financial Transactions
            CreateAccountFinancialTransactionRequest,
            CreatePoolFinancialTransactionRequest,
            CreateCreditLineFinancialTransactionRequest,
            AccountFinancialTransactionResponse,
            PoolFinancialTransactionResponse,
            CreditLineFinancialTransactionResponse,
            UnifiedFinancialTransactionResponse,
            FinancialSummaryResponse,
            PaginatedFinancialTransactionsResponse<AccountFinancialTransactionResponse>,
            PaginatedFinancialTransactionsResponse<PoolFinancialTransactionResponse>,
            PaginatedFinancialTransactionsResponse<CreditLineFinancialTransactionResponse>,
            PaginatedFinancialTransactionsResponse<UnifiedFinancialTransactionResponse>
        )
    ),
    tags(
        (name = "Info", description = "Health check and API information endpoints"),
        (name = "Authentication", description = "User login, logout and token management"),
        (name = "Users", description = "User registration and KYC management"),
        (name = "Investors", description = "Investor registration and management"),
        (name = "Merchants", description = "Merchant registration and management"),
        (name = "Credit Lines", description = "Credit line requests and eligibility evaluation"),
        (name = "Loans", description = "Loan creation and management"),
        (name = "Liquidity Pools", description = "Liquidity pool creation and management for auto-deployment"),
        (name = "Transactions", description = "Transaction submission and management"),
        (name = "Financial Transactions", description = "Account, pool, and credit line financial transaction management"),
        (name = "Batches", description = "Batch processing and proof management"),
        (name = "State", description = "System state and proof verification"),
        (name = "Admin", description = "Administrative operations and management")
    ),
    info(
        title = "Arda Credit API",
        version = "1.0.0",
        description = "Zero-knowledge proof system for batched transaction processing and loan management",
        contact(
            name = "Arda Global",
            url = "https://ardaglobal.com"
        )
    ),
    servers(
        (url = "/", description = "Default server")
    )
)]
pub struct ApiDoc;

/// Get the OpenAPI specification as JSON
pub fn get_openapi_spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Axum handler for serving the OpenAPI specification as JSON
pub fn get_openapi_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Create a Swagger UI router
fn swagger_ui() -> Router<ApiState> {
    SwaggerUi::new("/api-docs")
        .url("/api-docs/openapi.json", get_openapi_spec())
        .into()
}

// ============================================================================
// LIQUIDITY POOL ENDPOINT HANDLERS
// ============================================================================

/// Create a new liquidity pool
#[utoipa::path(
    post,
    path = "/api/v1/pools",
    request_body = CreatePoolRequest,
    responses(
        (status = 201, description = "Pool created successfully", body = CreatePoolResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only investors can create pools"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Liquidity Pools"
)]
#[instrument(skip(state), level = "info")]
async fn create_pool_endpoint(
    State(state): State<ApiState>,
    axum::extract::Extension(user): axum::extract::Extension<AuthenticatedUser>,
    Json(request): Json<CreatePoolRequest>,
) -> Result<Json<CreatePoolResponse>, (StatusCode, String)> {
    info!("🏦 Creating liquidity pool: name={}", request.name);

    // Validate user is an investor
    if user.user_type != "investor" {
        warn!("Non-investor {} attempted to create pool", user.email);
        return Err((
            StatusCode::FORBIDDEN,
            "Only investors can create liquidity pools".to_string(),
        ));
    }

    // Validate parameters
    if request.name.trim().is_empty() || request.name.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Pool name must be between 1 and 255 characters".to_string(),
        ));
    }

    // Parse and validate decimal parameters
    let max_ltv_ratio = match rust_decimal::Decimal::from_str(&request.max_ltv_ratio) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid LTV ratio format, must be a decimal string".to_string(),
            ))
        }
    };

    let max_interest_rate = match rust_decimal::Decimal::from_str(&request.max_interest_rate) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid interest rate format, must be a decimal string".to_string(),
            ))
        }
    };

    if max_ltv_ratio < rust_decimal::Decimal::from_str("0.1").unwrap()
        || max_ltv_ratio > rust_decimal::Decimal::from_str("0.95").unwrap()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "LTV ratio must be between 10% (0.1) and 95% (0.95)".to_string(),
        ));
    }

    if max_interest_rate < rust_decimal::Decimal::from_str("0.01").unwrap()
        || max_interest_rate > rust_decimal::Decimal::from_str("0.50").unwrap()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Interest rate must be between 1% (0.01) and 50% (0.50)".to_string(),
        ));
    }

    if !(7..=365).contains(&request.repayment_period_days) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Repayment period must be between 7 and 365 days".to_string(),
        ));
    }

    // Create database request
    let db_request = CreateLiquidityPoolRequest {
        investor_id: user.user_id,
        name: request.name.trim().to_string(),
        max_ltv_ratio,
        max_interest_rate,
        repayment_period_days: request.repayment_period_days,
    };

    // Create pool in database
    let pool = match create_liquidity_pool(&state.pool, &db_request).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to create liquidity pool: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create pool: {}", e),
            ));
        }
    };

    let response = CreatePoolResponse {
        pool_id: pool.id.to_string(),
        investor_id: pool.investor_id.to_string(),
        name: pool.name,
        max_ltv_ratio: pool.max_ltv_ratio.to_string(),
        max_interest_rate: pool.max_interest_rate.to_string(),
        repayment_period_days: pool.repayment_period_days,
        status: pool.status,
        created_at: pool.created_at,
    };

    info!("✅ Pool created: id={}", pool.id);
    Ok(Json(response))
}

/// List liquidity pools for authenticated investor
#[utoipa::path(
    get,
    path = "/api/pools",
    params(
        ("status" = Option<String>, Query, description = "Filter by pool status")
    ),
    responses(
        (status = 200, description = "Pool list retrieved successfully", body = PoolListResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Only investors can list pools"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Liquidity Pools"
)]
#[instrument(skip(state), level = "info")]
async fn list_pools_endpoint(
    State(state): State<ApiState>,
    axum::extract::Extension(user): axum::extract::Extension<AuthenticatedUser>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<PoolListResponse>, (StatusCode, String)> {
    info!("📋 Listing pools for investor: {}", user.user_id);

    // Validate user is an investor
    if user.user_type != "investor" {
        return Err((
            StatusCode::FORBIDDEN,
            "Only investors can list pools".to_string(),
        ));
    }

    let _status_filter = params.get("status").cloned();

    let pools = match get_liquidity_pools_by_investor(&state.pool, user.user_id).await {
        Ok(pools) => pools,
        Err(e) => {
            error!("Failed to list pools: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list pools: {}", e),
            ));
        }
    };

    let pool_details: Vec<PoolDetailsResponse> = pools
        .iter()
        .map(|pool| PoolDetailsResponse {
            pool_id: pool.id.to_string(),
            investor_id: pool.investor_id.to_string(),
            name: pool.name.clone(),
            total_deposits: pool.total_deposits.to_string(),
            available_liquidity: pool.available_liquidity.to_string(),
            utilized_liquidity: pool.utilized_liquidity.to_string(),
            max_ltv_ratio: pool.max_ltv_ratio.to_string(),
            max_interest_rate: pool.max_interest_rate.to_string(),
            repayment_period_days: pool.repayment_period_days,
            status: pool.status.clone(),
            created_at: pool.created_at,
            updated_at: pool.updated_at,
        })
        .collect();

    let response = PoolListResponse {
        total_count: pool_details.len(),
        pools: pool_details,
    };

    info!("✅ Listed {} pools", response.total_count);
    Ok(Json(response))
}

/// Get specific pool details
#[utoipa::path(
    get,
    path = "/api/pools/{pool_id}",
    params(
        ("pool_id" = String, Path, description = "Pool ID")
    ),
    responses(
        (status = 200, description = "Pool details retrieved successfully", body = PoolDetailsResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Access denied to this pool"),
        (status = 404, description = "Pool not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Liquidity Pools"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_pool_endpoint(
    State(state): State<ApiState>,
    axum::extract::Extension(user): axum::extract::Extension<AuthenticatedUser>,
    Path(pool_id_str): Path<String>,
) -> Result<Json<PoolDetailsResponse>, (StatusCode, String)> {
    info!("🔍 Getting pool details: id={}", pool_id_str);

    let pool_id = match uuid::Uuid::parse_str(&pool_id_str) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid pool ID format".to_string(),
            ))
        }
    };

    let pool = match get_liquidity_pool_by_id(&state.pool, pool_id).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to get pool: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pool: {}", e),
            ));
        }
    };

    // Check ownership (investors can only see their own pools)
    if user.user_type == "investor" && pool.investor_id != user.user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "Access denied to this pool".to_string(),
        ));
    }

    let response = PoolDetailsResponse {
        pool_id: pool.id.to_string(),
        investor_id: pool.investor_id.to_string(),
        name: pool.name.clone(),
        total_deposits: pool.total_deposits.to_string(),
        available_liquidity: pool.available_liquidity.to_string(),
        utilized_liquidity: pool.utilized_liquidity.to_string(),
        max_ltv_ratio: pool.max_ltv_ratio.to_string(),
        max_interest_rate: pool.max_interest_rate.to_string(),
        repayment_period_days: pool.repayment_period_days,
        status: pool.status,
        created_at: pool.created_at,
        updated_at: pool.updated_at,
    };

    info!("✅ Pool details retrieved: {}", pool.name);
    Ok(Json(response))
}

/// Update pool status
#[utoipa::path(
    put,
    path = "/api/pools/{pool_id}/status",
    params(
        ("pool_id" = String, Path, description = "Pool ID")
    ),
    request_body = UpdatePoolStatusRequest,
    responses(
        (status = 200, description = "Pool status updated successfully", body = PoolDetailsResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Access denied to this pool"),
        (status = 404, description = "Pool not found"),
        (status = 400, description = "Invalid status"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Liquidity Pools"
)]
#[instrument(skip(state), level = "info")]
async fn update_pool_status_endpoint(
    State(state): State<ApiState>,
    axum::extract::Extension(user): axum::extract::Extension<AuthenticatedUser>,
    Path(pool_id_str): Path<String>,
    Json(request): Json<UpdatePoolStatusRequest>,
) -> Result<Json<PoolDetailsResponse>, (StatusCode, String)> {
    info!(
        "🔄 Updating pool status: id={}, status={}",
        pool_id_str, request.status
    );

    let pool_id = match uuid::Uuid::parse_str(&pool_id_str) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid pool ID format".to_string(),
            ))
        }
    };

    // Validate status
    if !matches!(request.status.as_str(), "ACTIVE" | "PAUSED" | "CLOSED") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Status must be ACTIVE, PAUSED, or CLOSED".to_string(),
        ));
    }

    // Check pool exists and ownership
    let existing_pool = match get_liquidity_pool_by_id(&state.pool, pool_id).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to get pool: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pool: {}", e),
            ));
        }
    };

    // Check ownership
    if existing_pool.investor_id != user.user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "Access denied to this pool".to_string(),
        ));
    }

    // Update status
    let updated_pool =
        match update_liquidity_pool_status(&state.pool, pool_id, &request.status).await {
            Ok(pool) => pool,
            Err(e) => {
                error!("Failed to update pool status: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to update pool status: {}", e),
                ));
            }
        };

    let response = PoolDetailsResponse {
        pool_id: updated_pool.id.to_string(),
        investor_id: updated_pool.investor_id.to_string(),
        name: updated_pool.name,
        total_deposits: updated_pool.total_deposits.to_string(),
        available_liquidity: updated_pool.available_liquidity.to_string(),
        utilized_liquidity: updated_pool.utilized_liquidity.to_string(),
        max_ltv_ratio: updated_pool.max_ltv_ratio.to_string(),
        max_interest_rate: updated_pool.max_interest_rate.to_string(),
        repayment_period_days: updated_pool.repayment_period_days,
        status: updated_pool.status,
        created_at: updated_pool.created_at,
        updated_at: updated_pool.updated_at,
    };

    info!("✅ Pool status updated: {} -> {}", pool_id, request.status);
    Ok(Json(response))
}

/// Find matching pools for auto-deployment
#[utoipa::path(
    post,
    path = "/api/pools/match",
    request_body = PoolMatchRequest,
    responses(
        (status = 200, description = "Matching pools found", body = PoolListResponse),
        (status = 400, description = "Invalid matching criteria"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Liquidity Pools"
)]
#[instrument(skip(state), level = "info")]
async fn find_matching_pools_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<PoolMatchRequest>,
) -> Result<Json<PoolListResponse>, (StatusCode, String)> {
    info!(
        "🔍 Finding matching pools: required={}",
        request.required_liquidity
    );

    // Parse decimal strings
    let required_liquidity = match rust_decimal::Decimal::from_str(&request.required_liquidity) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid required liquidity format".to_string(),
            ))
        }
    };

    let max_ltv = match rust_decimal::Decimal::from_str(&request.max_ltv) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid max LTV format".to_string(),
            ))
        }
    };

    let max_rate = match rust_decimal::Decimal::from_str(&request.max_rate) {
        Ok(val) => val,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid max rate format".to_string(),
            ))
        }
    };

    let criteria = PoolMatchingCriteria {
        required_liquidity,
        max_ltv,
        max_rate,
        min_repayment_days: request.min_repayment_days,
    };

    let pools = match find_matching_liquidity_pools(&state.pool, &criteria).await {
        Ok(pools) => pools,
        Err(e) => {
            error!("Failed to find matching pools: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to find matching pools: {}", e),
            ));
        }
    };

    let pool_details: Vec<PoolDetailsResponse> = pools
        .iter()
        .map(|pool| PoolDetailsResponse {
            pool_id: pool.id.to_string(),
            investor_id: pool.investor_id.to_string(),
            name: pool.name.clone(),
            total_deposits: pool.total_deposits.to_string(),
            available_liquidity: pool.available_liquidity.to_string(),
            utilized_liquidity: pool.utilized_liquidity.to_string(),
            max_ltv_ratio: pool.max_ltv_ratio.to_string(),
            max_interest_rate: pool.max_interest_rate.to_string(),
            repayment_period_days: pool.repayment_period_days,
            status: pool.status.clone(),
            created_at: pool.created_at,
            updated_at: pool.updated_at,
        })
        .collect();

    let response = PoolListResponse {
        total_count: pool_details.len(),
        pools: pool_details,
    };

    info!("✅ Found {} matching pools", response.total_count);
    Ok(Json(response))
}

// ============================================================================

// ============================================================================
// MERCHANT TURNOVER RESPONSE TYPES (for OpenAPI)
// ============================================================================

// ============================================================================
// MERCHANT TURNOVER API ENDPOINTS
// ============================================================================
