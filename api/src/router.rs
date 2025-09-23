use axum::{
    extract::State,
    response::{Json, Response},
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info, instrument, warn};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid;

use crate::admin_handlers::{
    KycStatisticsResponse, ListPendingKycQuery, ListPendingKycResponse, PlatformMetricsResponse,
};
use crate::auth::AuthService;
use crate::authentication_handlers::{
    generate_invite_code_endpoint, logout_endpoint, validate_invite_code_endpoint,
    GenerateInviteCodeRequest, GenerateInviteCodeResponse, InviteCodeResponse, LogoutRequest,
    LogoutResponse, ValidateInviteCodeResponse,
};
use crate::batch_processor::BatchProcessorHandle;
use crate::credit_line_handlers::{
    browse_available_pools, check_credit_eligibility, list_active_credit_lines_with_pool_details,
    list_credit_line_requests, submit_credit_line_request,
};
use crate::financial_transaction_handlers::{
    get_account_financial_transactions, get_credit_line_financial_transactions,
    get_merchant_credit_line_financial_transactions, get_pool_financial_transactions,
    get_user_financial_summary, get_user_financial_transactions,
    get_user_pool_financial_transactions, record_account_deposit, record_account_withdrawal,
    record_credit_line_draw, record_credit_line_payment, record_pool_funding,
    record_pool_withdrawal,
};
use crate::health_handlers::EndpointInfo;
use crate::loan_handlers::DeleteLoanResponse;
use crate::merchant_handlers::{
    InvestorProfileQuery, InvestorProfileResponse, MerchantProfileResponse,
    RegisterInvestorRequest, RegisterInvestorResponse, RegisterMerchantRequest,
    RegisterMerchantResponse, UpdateInvestorProfileRequest, UpdateMerchantProfileRequest,
};
use crate::openapi::ApiDoc;
use crate::paths;
use crate::pool_borough_handlers::{
    delete_borough_limit, get_borough_limits, get_borough_summary, set_borough_limit,
    validate_borough_limit,
};
use crate::pool_handlers::{
    create_pool, deposit_to_pool,
    /* get_enhanced_investor_metrics_handler, */ get_investor_dashboard,
    get_investor_pool_metrics_handler, get_investor_pool_metrics_overview,
    get_investor_pool_summaries, get_market_overview, get_pool_details, get_pool_metrics_overview,
    get_pool_performance, get_pool_transactions, get_pool_utilization, list_pools,
    update_pool_status,
};
use crate::session_service::SessionService;
use crate::state_handlers::{
    create_batch_endpoint, get_batch_details_endpoint, get_batches_endpoint,
    get_contract_submission_data_endpoint, get_current_state_endpoint,
    get_pending_transactions_endpoint, submit_transaction_endpoint, trigger_batch_endpoint,
    update_batch_proof_endpoint,
};
use crate::transaction_history_handlers::{get_pool_transaction_history, get_transaction_history};
use crate::turnover_handlers::{
    create_turnover_endpoint, get_profile_completion_endpoint, get_turnover_analytics_endpoint,
    get_turnover_endpoint, update_turnover_endpoint,
};
use crate::types::UserType;
use crate::user_handlers::{
    DeleteUserResponse, KycData, RegisterUserRequest, RegisterUserResponse, SubmitKycRequest,
    SubmitKycResponse, UpdateKycStatusRequest, UpdateUserRequest, UpdateUserResponse,
    UserDetailsResponse,
};
use crate::withdrawal_handlers;
use crate::{BalanceVerificationService, WithdrawalTransactionService};
use arda_credit_db::{Identity, IndexedMerkleTreeADS};
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
    pub session_service: Arc<SessionService>,
    pub magic_link_service: Option<Arc<crate::magic_link_service::MagicLinkService>>,
    pub withdrawal_service: WithdrawalTransactionService,
    pub balance_service: BalanceVerificationService,
    pub withdrawal_executor:
        Option<crate::simple_withdrawal_executor::SimpleWithdrawalExecutorHandle>,
    pub metrics_handle: Option<Arc<crate::metrics::MetricsHandle>>,
    pub email_rate_limiter: Option<Arc<crate::middleware::EmailRateLimiter>>,
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
// ROUTER CREATION
// ============================================================================

/// Create the main API router with all batch processing and loan endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints
        .route(paths::HEALTH, get(crate::health_handlers::health_check))
        .route(paths::ROOT, get(crate::health_handlers::api_info))
        .route(paths::API_HEALTH, get(crate::health_handlers::health_check))
        .route(paths::API_INFO, get(crate::health_handlers::api_info))
        // Prometheus metrics endpoint
        .route("/metrics", get(metrics_handler))
        .route("/metrics/health", get(metrics_health_handler))
        // OpenAPI documentation routes - Swagger UI handles both the UI and JSON spec
        .merge(swagger_ui())
        // Magic Link Authentication endpoints
        .route(
            paths::AUTH_MAGIC_LINK,
            post(crate::magic_link_handlers::request_magic_link),
        )
        .route(
            paths::AUTH_VERIFY,
            post(crate::magic_link_handlers::verify_magic_link),
        )
        .route(
            paths::AUTH_ME,
            get(crate::magic_link_handlers::get_current_user),
        )
        .route(
            paths::AUTH_LOGOUT,
            post(crate::magic_link_handlers::logout_endpoint),
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
            paths::ADMIN_PLATFORM_METRICS,
            get(crate::admin_handlers::get_platform_metrics_endpoint),
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
        .route(
            paths::STATE_BATCHES,
            get(crate::state_handlers::get_batches_endpoint),
        )
        .route(
            paths::STATE_BATCH_BY_ID,
            get(crate::state_handlers::get_batch_details_endpoint),
        )
        .route(
            paths::STATE_BATCH_PROOF,
            post(crate::state_handlers::update_batch_proof_endpoint),
        )
        .route(
            paths::STATE_BATCHES_TRIGGER,
            post(crate::state_handlers::trigger_batch_endpoint),
        )
        // State operations
        .route(
            paths::STATE_CURRENT,
            get(crate::state_handlers::get_current_state_endpoint),
        )
        .route(
            paths::STATE_CONTRACT,
            get(crate::state_handlers::get_contract_submission_data_endpoint),
        )
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
        .route(
            paths::USER_BY_ID,
            put(crate::user_handlers::update_user_endpoint),
        )
        .route(
            paths::USER_BY_ID,
            delete(crate::user_handlers::delete_user_endpoint),
        )
        .route(
            paths::USER_KYC,
            post(crate::user_handlers::submit_kyc_endpoint),
        )
        .route(
            paths::USER_KYC_STATUS,
            post(crate::user_handlers::update_kyc_status_endpoint),
        )
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
            get(crate::user_handlers::get_user_balance_by_address_endpoint),
        )
        .route(
            paths::USER_BALANCE_BY_ID,
            get(crate::user_handlers::get_user_balance_by_id_endpoint),
        )
        .route(
            paths::USER_TRANSACTIONS_BY_ID,
            get(crate::user_handlers::get_user_transactions_endpoint),
        )
        // ========================================================================
        // WITHDRAWAL MANAGEMENT ENDPOINTS
        // ========================================================================
        .route(
            paths::WITHDRAWALS,
            post(crate::withdrawal_handlers::initiate_withdrawal_endpoint),
        )
        .route(
            paths::WITHDRAWAL_BY_ID,
            get(crate::withdrawal_handlers::get_withdrawal_status_endpoint),
        )
        .route(
            paths::USER_AVAILABLE_BALANCE,
            get(crate::withdrawal_handlers::get_user_available_balance_endpoint),
        )
        .route(
            paths::USER_WITHDRAWALS,
            get(crate::withdrawal_handlers::get_user_withdrawal_history_endpoint),
        )
        .route(
            paths::WITHDRAWAL_CANCEL,
            post(crate::withdrawal_handlers::cancel_withdrawal_endpoint),
        )
        .route(
            paths::ADMIN_PENDING_WITHDRAWALS,
            get(crate::withdrawal_handlers::get_pending_withdrawals_endpoint),
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
        // TRANSACTION HISTORY ENDPOINTS
        // ========================================================================
        .route(paths::TRANSACTION_HISTORY, get(get_transaction_history))
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
        // ========================================================================
        // LIQUIDITY POOL QUERY ENDPOINTS
        // ========================================================================
        .route(paths::POOLS, get(list_pools).post(create_pool))
        .route(
            paths::POOL_BY_ID,
            get(get_pool_details).put(update_pool_status),
        )
        .route(paths::POOL_DEPOSIT, post(deposit_to_pool))
        .route(paths::POOL_UTILIZATION, get(get_pool_utilization))
        .route(paths::POOL_TRANSACTIONS, get(get_pool_transactions))
        .route(
            paths::POOL_TRANSACTION_HISTORY,
            get(get_pool_transaction_history),
        )
        // ========================================================================
        // POOL BOROUGH LIMIT ENDPOINTS
        // ========================================================================
        .route(
            paths::POOL_BOROUGH_LIMITS,
            get(get_borough_limits).put(set_borough_limit),
        )
        .route(
            paths::POOL_BOROUGH_LIMIT_BY_BOROUGH,
            delete(delete_borough_limit),
        )
        .route(paths::POOL_BOROUGH_SUMMARY, get(get_borough_summary))
        .route(paths::POOL_BOROUGH_VALIDATION, get(validate_borough_limit))
        // ========================================================================
        // POOL METRICS ENDPOINTS
        // ========================================================================
        .route(paths::POOL_METRICS_OVERVIEW, get(get_pool_metrics_overview))
        .route(
            paths::POOL_METRICS_INVESTOR,
            get(get_investor_pool_metrics_overview),
        )
        // Investor Query Endpoints
        .route(paths::INVESTOR_DASHBOARD, get(get_investor_dashboard))
        .route(
            paths::INVESTOR_POOL_SUMMARIES,
            get(get_investor_pool_summaries),
        )
        .route(
            paths::INVESTOR_POOL_METRICS,
            get(get_investor_pool_metrics_handler),
        )
        /* .route(
            paths::INVESTOR_METRICS,
            get(get_enhanced_investor_metrics_handler),
        ) */
        // Analytics Endpoints
        .route(paths::ANALYTICS_MARKET_OVERVIEW, get(get_market_overview))
        .route(paths::ANALYTICS_POOL_PERFORMANCE, get(get_pool_performance))
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
        .route(
            paths::CREDIT_LINES_ACTIVE,
            get(list_active_credit_lines_with_pool_details),
        )
        // ========================================================================
        // CONTRACT CONFIGURATION ENDPOINTS
        // ========================================================================
        .route(
            paths::CONTRACT_CONFIG,
            get(crate::contract_handlers::get_contract_config),
        )
        .route(
            paths::TRANSACTION_UPDATES_SSE,
            get(crate::contract_handlers::get_transaction_updates),
        )
        // ========================================================================
        // ADDITIONAL ADMIN WITHDRAWAL ENDPOINTS (using path constants)
        // ========================================================================
        .route(
            paths::ADMIN_APPROVE_WITHDRAWAL,
            post(withdrawal_handlers::approve_withdrawal_request),
        )
        .route(
            paths::ADMIN_REJECT_WITHDRAWAL,
            post(withdrawal_handlers::reject_withdrawal_request),
        )
        .route(
            paths::ADMIN_EXECUTE_WITHDRAWAL,
            post(withdrawal_handlers::manual_execute_withdrawal),
        )
        .route(
            paths::ADMIN_WITHDRAWAL_DASHBOARD,
            get(withdrawal_handlers::get_admin_withdrawal_dashboard),
        )
        .with_state(state)
}

// ============================================================================
// SWAGGER UI SETUP
// ============================================================================

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

/// Prometheus metrics endpoint handler
#[instrument(skip(state))]
async fn metrics_handler(State(state): State<ApiState>) -> impl axum::response::IntoResponse {
    // Get metrics from the metrics handle stored in state
    let metrics = match &state.metrics_handle {
        Some(handle) => handle.render(),
        None => {
            tracing::warn!("No metrics handle found in state");
            "# No metrics handle available\n".to_string()
        }
    };

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
        .body(metrics)
        .unwrap()
}

/// Prometheus metrics health check endpoint handler
#[instrument(skip(state))]
async fn metrics_health_handler(
    State(state): State<ApiState>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use serde_json::json;

    let (status, health_status) = match &state.metrics_handle {
        Some(handle) => {
            if handle.health_check() {
                (StatusCode::OK, "healthy")
            } else {
                (StatusCode::SERVICE_UNAVAILABLE, "unhealthy")
            }
        }
        None => {
            tracing::warn!("No metrics handle found in state for health check");
            (StatusCode::SERVICE_UNAVAILABLE, "no_metrics_handle")
        }
    };

    let response_body = json!({
        "status": health_status,
        "service": "prometheus_metrics",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    (status, axum::Json(response_body))
}
