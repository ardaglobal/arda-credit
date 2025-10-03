use axum::{
    extract::State,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::PgPool;
use tracing::instrument;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid;

use crate::api::openapi::ApiDoc;
use crate::config::paths;
use crate::config::types::UserType;
use crate::handlers::{
    // Deals
    companies as company_handlers,
    // Documents (companies module)
    documents as document_handlers,
    // Finance
    get_account_financial_transactions,
    // Magic Link Auth
    get_current_user,
    get_transaction_history,
    get_user_financial_summary,
    get_user_financial_transactions,
    logout_endpoint,
    record_account_deposit,
    record_account_withdrawal,
    request_magic_link,
    verify_magic_link,
    withdrawals as withdrawal_handlers,
    UserDetailsResponse,
};
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
            post(crate::handlers::companies::companies::register_investor_endpoint),
        )
        .route(
            paths::INVESTOR_PROFILE,
            get(crate::handlers::companies::companies::get_investor_profile_endpoint)
                .put(crate::handlers::companies::companies::update_investor_profile_endpoint),
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
        .route(
            paths::DOCUMENT_HEALTH,
            get(document_handlers::document_service_health_check),
        )
        // AI Analysis endpoints
        .merge(crate::handlers::ai::analysis::create_ai_analysis_router())
        // Faucet endpoints (local/staging only)
        .route(
            paths::FAUCET_INFO,
            get(crate::handlers::faucet::get_faucet_info),
        )
        .route(
            paths::FAUCET_STATUS,
            get(crate::handlers::faucet::check_faucet_status),
        )
        .route(
            paths::FAUCET_CONFIG,
            get(crate::handlers::faucet::get_faucet_config),
        )
        .route(
            paths::FAUCET_REQUEST,
            post(crate::handlers::faucet::request_faucet_funds),
        )
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
