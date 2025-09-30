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

use crate::api::openapi::ApiDoc;
use crate::config::paths;
use crate::config::types::UserType;
use crate::handlers::{
    // Credit lines
    browse_available_pools,
    // Deals
    browse_deals_for_sale_endpoint,
    check_credit_eligibility,
    // Companies (merchant module)
    companies as company_handlers,
    // State
    create_batch_endpoint,
    create_deal_endpoint,
    // Pools
    create_pool,
    // Turnover (merchant module)
    create_turnover_endpoint,
    // Pool boroughs
    delete_borough_limit,
    delete_deal_endpoint,
    deposit_to_pool,
    // Documents (merchant module)
    documents as document_handlers,
    // Auth
    generate_invite_code_endpoint,
    // Finance
    get_account_financial_transactions,
    get_batch_details_endpoint,
    get_batches_endpoint,
    get_borough_limits,
    get_borough_summary,
    get_contract_submission_data_endpoint,
    get_credit_line_financial_transactions,
    get_current_state_endpoint,
    // Magic Link Auth
    get_current_user,
    get_deal_details_endpoint,
    get_deal_payment_history_endpoint,
    get_deal_stats_endpoint,
    get_deal_transfer_history_endpoint,
    get_deals_by_user_id_endpoint,
    get_investor_dashboard,
    get_investor_pool_metrics_handler,
    get_investor_pool_metrics_overview,
    get_investor_pool_summaries,
    get_market_overview,
    get_merchant_credit_line_financial_transactions,
    get_payment_details_endpoint,
    get_pending_transactions_endpoint,
    get_pool_details,
    get_pool_financial_transactions,
    get_pool_metrics_overview,
    get_pool_performance,
    get_pool_transaction_history,
    get_pool_transactions,
    get_pool_utilization,
    get_profile_completion_endpoint,
    get_transaction_history,
    get_turnover_analytics_endpoint,
    get_turnover_endpoint,
    get_user_financial_summary,
    get_user_financial_transactions,
    get_user_pool_financial_transactions,
    list_active_credit_lines_with_pool_details,
    list_credit_line_requests,
    list_pools,
    logout_endpoint,
    record_account_deposit,
    record_account_withdrawal,
    record_credit_line_draw,
    record_credit_line_payment,
    record_deal_payment_endpoint,
    record_pool_funding,
    record_pool_withdrawal,
    remove_deal_from_sale_endpoint,
    request_magic_link,
    set_borough_limit,
    submit_credit_line_request,
    submit_transaction_endpoint,
    transfer_deal_ownership_endpoint,
    trigger_batch_endpoint,
    update_batch_proof_endpoint,
    update_deal_endpoint,
    update_pool_status,
    update_turnover_endpoint,
    validate_borough_limit,
    validate_deal_transfer_endpoint,
    validate_invite_code_endpoint,
    verify_magic_link,
    withdrawals as withdrawal_handlers,
    DeleteDealResponse,
    // Users
    DeleteUserResponse,
    // Health
    EndpointInfo,
    GenerateInviteCodeRequest,
    GenerateInviteCodeResponse,
    // Merchants
    InvestorProfileQuery,
    InvestorProfileResponse,
    InviteCodeResponse,
    KycData,
    // Admin
    KycStatisticsResponse,
    ListPendingKycQuery,
    ListPendingKycResponse,
    LogoutRequest,
    LogoutResponse,
    MerchantProfileResponse,
    PlatformMetricsResponse,
    RegisterInvestorRequest,
    RegisterInvestorResponse,
    RegisterMerchantRequest,
    RegisterMerchantResponse,
    RegisterUserRequest,
    RegisterUserResponse,
    SubmitKycRequest,
    SubmitKycResponse,
    UpdateInvestorProfileRequest,
    UpdateKycStatusRequest,
    UpdateMerchantProfileRequest,
    UpdateUserRequest,
    UpdateUserResponse,
    UserDetailsResponse,
    ValidateInviteCodeResponse,
};
use crate::services::auth::AuthService;
use crate::services::batch_processor::BatchProcessorHandle;
use crate::services::document_service::DocumentService;
use crate::services::session_service::SessionService;
use crate::services::{BalanceVerificationService, WithdrawalTransactionService};
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
        user_type: identity.user_type.parse().unwrap_or(UserType::Investor),
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
    pub magic_link_service: Option<Arc<crate::services::magic_link_service::MagicLinkService>>,
    pub withdrawal_service: WithdrawalTransactionService,
    pub balance_service: BalanceVerificationService,
    pub withdrawal_executor:
        Option<crate::services::simple_withdrawal_executor::SimpleWithdrawalExecutorHandle>,
    pub metrics_handle: Option<Arc<crate::utils::metrics::MetricsHandle>>,
    pub email_rate_limiter: Option<Arc<crate::middleware::middleware::EmailRateLimiter>>,
    pub document_service: Option<Arc<DocumentService>>,
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

/// Create the main API router with all batch processing and deal endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints
        .route(
            paths::HEALTH,
            get(crate::handlers::core::health::health_check),
        )
        .route(paths::ROOT, get(crate::handlers::core::health::api_info))
        .route(
            paths::API_HEALTH,
            get(crate::handlers::core::health::health_check),
        )
        .route(
            paths::API_INFO,
            get(crate::handlers::core::health::api_info),
        )
        // Prometheus metrics endpoint
        .route("/metrics", get(metrics_handler))
        .route("/metrics/health", get(metrics_health_handler))
        // OpenAPI documentation routes - Swagger UI handles both the UI and JSON spec
        .merge(swagger_ui())
        // Magic Link Authentication endpoints
        .route(paths::AUTH_MAGIC_LINK, post(request_magic_link))
        .route(paths::AUTH_VERIFY, post(verify_magic_link))
        .route(paths::AUTH_ME, get(get_current_user))
        .route(paths::AUTH_LOGOUT, post(logout_endpoint))
        // Admin KYC endpoints
        .route(
            paths::ADMIN_KYC_PENDING,
            get(crate::handlers::users::admin::list_pending_kyc_endpoint),
        )
        .route(
            paths::ADMIN_KYC_STATS,
            get(crate::handlers::users::admin::get_kyc_statistics_endpoint),
        )
        .route(
            paths::ADMIN_KYC_BULK_APPROVE,
            post(crate::handlers::users::admin::bulk_kyc_approval_endpoint),
        )
        .route(
            paths::ADMIN_PLATFORM_METRICS,
            get(crate::handlers::users::admin::get_platform_metrics_endpoint),
        )
        .route(
            paths::INVITE_CODE_GENERATE,
            post(crate::handlers::auth::authentication::generate_invite_code_endpoint),
        )
        .route(
            paths::INVITE_CODE_VALIDATE,
            post(crate::handlers::auth::authentication::validate_invite_code_endpoint),
        )
        // Transaction operations
        .route(
            paths::STATE_TRANSACTIONS,
            post(crate::handlers::core::state::submit_transaction_endpoint),
        )
        .route(
            paths::STATE_TRANSACTIONS_PENDING,
            get(crate::handlers::core::state::get_pending_transactions_endpoint),
        )
        // Batch operations
        .route(
            paths::STATE_BATCHES,
            post(crate::handlers::core::state::create_batch_endpoint),
        )
        .route(
            paths::STATE_BATCHES,
            get(crate::handlers::core::state::get_batches_endpoint),
        )
        .route(
            paths::STATE_BATCH_BY_ID,
            get(crate::handlers::core::state::get_batch_details_endpoint),
        )
        .route(
            paths::STATE_BATCH_PROOF,
            post(crate::handlers::core::state::update_batch_proof_endpoint),
        )
        .route(
            paths::STATE_BATCHES_TRIGGER,
            post(crate::handlers::core::state::trigger_batch_endpoint),
        )
        // State operations
        .route(
            paths::STATE_CURRENT,
            get(crate::handlers::core::state::get_current_state_endpoint),
        )
        .route(
            paths::STATE_CONTRACT,
            get(crate::handlers::core::state::get_contract_submission_data_endpoint),
        )
        // ========================================================================
        // LOAN PROCESSING ENDPOINTS
        // ========================================================================
        // User Management
        .route(
            paths::USER,
            post(crate::handlers::users::users::register_user_endpoint),
        )
        .route(
            paths::USERS,
            get(crate::handlers::users::users::list_identities_endpoint),
        )
        .route(
            paths::USER_BY_ID,
            get(crate::handlers::users::users::get_user_details_endpoint),
        )
        .route(
            paths::USER_BY_ID,
            put(crate::handlers::users::users::update_user_endpoint),
        )
        .route(
            paths::USER_BY_ID,
            delete(crate::handlers::users::users::delete_user_endpoint),
        )
        .route(
            paths::USER_KYC,
            post(crate::handlers::users::users::submit_kyc_endpoint),
        )
        .route(
            paths::USER_KYC_STATUS,
            post(crate::handlers::users::users::update_kyc_status_endpoint),
        )
        // Investor Management endpoints
        .route(
            paths::REGISTER_INVESTOR,
            post(crate::handlers::merchants::merchants::register_investor_endpoint),
        )
        .route(
            paths::INVESTOR_PROFILE,
            get(crate::handlers::merchants::merchants::get_investor_profile_endpoint),
        )
        // Merchant Management endpoints
        .route(
            paths::REGISTER_MERCHANT,
            post(crate::handlers::merchants::merchants::register_merchant_endpoint),
        )
        .route(
            paths::MERCHANT_PROFILE,
            get(crate::handlers::merchants::merchants::get_merchant_profile_endpoint)
                .put(crate::handlers::merchants::merchants::update_merchant_profile_endpoint),
        )
        .route(
            paths::INVESTOR_PROFILE,
            put(crate::handlers::merchants::merchants::update_investor_profile_endpoint),
        )
        // User Balance & Transaction endpoints
        .route(
            paths::USER_BALANCE_BY_ADDRESS,
            get(crate::handlers::users::users::get_user_balance_by_address_endpoint),
        )
        .route(
            paths::USER_BALANCE_BY_ID,
            get(crate::handlers::users::users::get_user_balance_by_id_endpoint),
        )
        .route(
            paths::USER_TRANSACTIONS_BY_ID,
            get(crate::handlers::users::users::get_user_transactions_endpoint),
        )
        // ========================================================================
        // WITHDRAWAL MANAGEMENT ENDPOINTS
        // ========================================================================
        .route(
            paths::WITHDRAWALS,
            post(crate::handlers::finance::withdrawals::initiate_withdrawal_endpoint),
        )
        .route(
            paths::WITHDRAWAL_BY_ID,
            get(crate::handlers::finance::withdrawals::get_withdrawal_status_endpoint),
        )
        .route(
            paths::USER_AVAILABLE_BALANCE,
            get(crate::handlers::finance::withdrawals::get_user_available_balance_endpoint),
        )
        .route(
            paths::USER_WITHDRAWALS,
            get(crate::handlers::finance::withdrawals::get_user_withdrawal_history_endpoint),
        )
        .route(
            paths::WITHDRAWAL_CANCEL,
            post(crate::handlers::finance::withdrawals::cancel_withdrawal_endpoint),
        )
        .route(
            paths::ADMIN_PENDING_WITHDRAWALS,
            get(crate::handlers::finance::withdrawals::get_pending_withdrawals_endpoint),
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
        // DEAL MANAGEMENT ENDPOINTS (converted from LOAN MANAGEMENT)
        // ========================================================================
        // Deal Management
        .route(
            paths::DEAL,
            post(crate::handlers::deals::deals::create_deal_endpoint),
        )
        .route(
            paths::DEALS,
            get(crate::handlers::deals::deals::list_deals_endpoint),
        )
        .route(
            paths::DEALS_BY_USER_ID,
            get(crate::handlers::deals::deals::get_deals_by_user_id_endpoint),
        )
        .route(
            paths::DEAL_BY_ID,
            get(crate::handlers::deals::deals::get_deal_details_endpoint),
        )
        .route(
            paths::DEAL_BY_ID,
            put(crate::handlers::deals::deals::update_deal_endpoint),
        )
        .route(
            paths::DEAL_BY_ID,
            delete(crate::handlers::deals::deals::delete_deal_endpoint),
        )
        .route(
            paths::DEALS_STATS,
            get(crate::handlers::deals::deals::get_deal_stats_endpoint),
        )
        // ========================================================================
        // DEAL MARKETPLACE ENDPOINTS (consolidated from separate marketplace)
        // ========================================================================
        .route(
            paths::DEALS_FOR_SALE,
            get(crate::handlers::deals::deals::browse_deals_for_sale_endpoint),
        )
        .route(
            paths::DEAL_LIST_FOR_SALE,
            post(crate::handlers::deals::deals::list_deal_for_sale_endpoint),
        )
        .route(
            paths::DEAL_REMOVE_FROM_SALE,
            delete(crate::handlers::deals::deals::remove_deal_from_sale_endpoint),
        )
        // ========================================================================
        // DEAL TRANSFER ENDPOINTS (consolidated from separate transfer)
        // ========================================================================
        .route(
            paths::DEAL_TRANSFER,
            post(crate::handlers::deals::deals::transfer_deal_ownership_endpoint),
        )
        .route(
            paths::DEAL_TRANSFER_HISTORY,
            get(crate::handlers::deals::deals::get_deal_transfer_history_endpoint),
        )
        .route(
            paths::DEAL_TRANSFER_VALIDATE,
            post(crate::handlers::deals::deals::validate_deal_transfer_endpoint),
        )
        // Deal Payment Processing
        .route(
            paths::STATE_PAYMENTS,
            post(crate::handlers::deals::deals::record_deal_payment_endpoint),
        )
        .route(
            paths::STATE_PAYMENT_BY_ID,
            get(crate::handlers::deals::deals::get_payment_details_endpoint),
        )
        .route(
            paths::DEAL_PAYMENTS,
            get(crate::handlers::deals::deals::get_deal_payment_history_endpoint),
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
            post(crate::handlers::merchants::turnover::create_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER,
            get(crate::handlers::merchants::turnover::get_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER_BY_DATE,
            put(crate::handlers::merchants::turnover::update_turnover_endpoint),
        )
        .route(
            paths::MERCHANT_TURNOVER_ANALYTICS,
            get(crate::handlers::merchants::turnover::get_turnover_analytics_endpoint),
        )
        .route(
            paths::MERCHANT_PROFILE_COMPLETION,
            get(crate::handlers::merchants::turnover::get_profile_completion_endpoint),
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
            get(crate::handlers::core::contract::get_contract_config),
        )
        .route(
            paths::TRANSACTION_UPDATES_SSE,
            get(crate::handlers::core::contract::get_transaction_updates),
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
        // ========================================================================
        // COMPANY MANAGEMENT ENDPOINTS
        // ========================================================================
        .route(
            paths::REGISTER_COMPANY,
            post(company_handlers::register_company_endpoint),
        )
        .route(
            paths::COMPANY_BY_ID,
            get(company_handlers::get_company_endpoint),
        )
        .route(
            paths::COMPANY_DASHBOARD,
            get(company_handlers::get_company_dashboard_endpoint),
        )
        .route(
            paths::SUBMIT_KYB,
            post(company_handlers::submit_kyb_endpoint),
        )
        .route(
            paths::KYB_STATUS,
            get(company_handlers::get_kyb_status_endpoint),
        )
        .route(
            paths::INVITE_TEAM_MEMBER,
            post(company_handlers::invite_team_member_endpoint),
        )
        .route(
            paths::LIST_TEAM_MEMBERS,
            get(company_handlers::list_team_members_endpoint),
        )
        .route(
            paths::UPDATE_TEAM_MEMBER_ROLE,
            put(company_handlers::update_team_member_role_endpoint),
        )
        .route(
            paths::REMOVE_TEAM_MEMBER,
            delete(company_handlers::remove_team_member_endpoint),
        )
        // ========================================================================
        // DOCUMENT MANAGEMENT ENDPOINTS
        // ========================================================================
        .route(
            paths::DOCUMENT_UPLOAD,
            post(document_handlers::upload_document),
        )
        .route(
            paths::DOCUMENT_DOWNLOAD,
            get(document_handlers::download_document),
        )
        .route(paths::DOCUMENT_BY_ID, get(document_handlers::get_document))
        .route(
            paths::DOCUMENT_BY_ID,
            delete(document_handlers::delete_document),
        )
        .route(paths::DOCUMENTS, get(document_handlers::list_documents))
        .route(
            paths::DOCUMENT_APPROVE,
            post(document_handlers::approve_document),
        )
        .route(
            paths::DOCUMENT_REJECT,
            post(document_handlers::reject_document),
        )
        .route(
            paths::DOCUMENT_VERIFY,
            post(document_handlers::verify_document),
        )
        .route(
            paths::DOCUMENT_AUDIT,
            get(document_handlers::get_audit_logs),
        )
        .route(paths::DOCUMENT_HEALTH, get(document_handlers::health_check))
        // AI Analysis endpoints
        .merge(crate::handlers::ai::analysis::create_ai_analysis_router())
        .with_state(state)
}

// ============================================================================
// ROUTER DOCUMENTATION
// ============================================================================

// ## API Router Configuration
//
// This module configures all HTTP routes for the Arda Credit API.
//
// ## Deal Management Routes
// - Deal CRUD operations
// - Deal payment processing
// - Deal marketplace operations (consolidated)
// - Deal transfer operations (consolidated)
//
// ## Recent Changes
// Converted from loan-based routing to deal-based routing:
// - All /loan/* routes → /deal/* routes
// - All /loans/* routes → /deals/* routes
// - Marketplace routes consolidated into deal routes
// - Transfer routes consolidated into deal routes
// - Removed separate marketplace and transfer handler modules

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
