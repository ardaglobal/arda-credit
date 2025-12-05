use axum::{
    extract::{DefaultBodyLimit, State},
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::PgPool;
// Collections import removed - not needed for current implementation
use tracing::instrument;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid;

use crate::api::openapi::ApiDoc;
use crate::config::paths;
use crate::config::types::UserType;
use crate::handlers::{
    // Companies
    companies as company_handlers,
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
use crate::services::session_service::SessionService;
use crate::services::{BalanceVerificationService, MetricsCache, WithdrawalTransactionService};
use arda_credit_db::{IndexedMerkleTreeADS, User};
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

/// Convert database User to API UserDetailsResponse (legacy - prefer users::identity_to_user_details)
#[allow(dead_code)]
fn identity_to_user_details(identity: User) -> UserDetailsResponse {
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
        // Profile fields will be None in legacy conversion
        investor_profile: None,
        originator_profile: None,
        companies: Vec::new(), // Empty companies in legacy conversion
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
    pub team_invitation_service:
        Option<Arc<crate::services::team_invitation_service::TeamInvitationService>>,
    pub withdrawal_service: WithdrawalTransactionService,
    pub balance_service: BalanceVerificationService,
    pub withdrawal_executor:
        Option<crate::services::simple_withdrawal_executor::SimpleWithdrawalExecutorHandle>,
    pub metrics_handle: Option<Arc<crate::utils::metrics::MetricsHandle>>,
    pub email_rate_limiter: Option<Arc<crate::middleware::middleware::EmailRateLimiter>>,
    pub metrics_cache: Arc<MetricsCache>,
    pub event_publisher: Option<Arc<crate::events::CollateralEventPublisher>>,
}

/// Configuration for API server
#[derive(Debug, Clone, ToSchema)]
pub struct ApiConfig {
    pub server_name: String,
    pub version: String,
    pub max_batch_size: u32,
    pub enable_debug_endpoints: bool,
    pub metrics_refresh_enabled: bool,
    pub metrics_refresh_interval_hours: u64,
    pub metrics_cache_ttl_seconds: u64,
    pub metrics_cache_max_entries: usize,
    pub metrics_cache_enabled: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            server_name: "Batch Processing API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 50,
            enable_debug_endpoints: false,
            metrics_refresh_enabled: std::env::var("METRICS_REFRESH_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            metrics_refresh_interval_hours: std::env::var("METRICS_REFRESH_INTERVAL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            metrics_cache_ttl_seconds: std::env::var("METRICS_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600), // 1 hour default
            metrics_cache_max_entries: std::env::var("METRICS_CACHE_MAX_ENTRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10000),
            metrics_cache_enabled: std::env::var("METRICS_CACHE_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
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
        // Individual API spec endpoints for each category (openapi.json handled by SwaggerUI)
        .route("/api-docs/auth.json", get(get_auth_spec_handler))
        .route("/api-docs/users.json", get(get_users_spec_handler))
        .route("/api-docs/companies.json", get(get_companies_spec_handler))
        .route("/api-docs/deals.json", get(get_deals_spec_handler))
        .route("/api-docs/collateral.json", get(get_collateral_spec_handler))
        .route("/api-docs/loan_tape.json", get(get_loan_tape_spec_handler))
        .route("/api-docs/admin.json", get(get_admin_spec_handler))
        .route("/api-docs/payments.json", get(get_payments_spec_handler))
        .route("/api-docs/state.json", get(get_state_spec_handler))
        .route("/api-docs/taxonomy.json", get(get_taxonomy_spec_handler))
        .route("/api-docs/metrics.json", get(get_metrics_spec_handler))
        .route("/api-docs/internal.json", get(get_internal_spec_handler))
        .route("/api-docs/dev.json", get(get_dev_spec_handler))
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
        // Metrics endpoints
        .route(
            paths::METRICS_MARKET_VALUE,
            get(crate::handlers::metrics::get_market_value),
        )
        .route(
            paths::METRICS_PIE_CHARTS,
            get(crate::handlers::metrics::get_pie_chart_data),
        )
        .route(
            paths::METRICS_REFRESH,
            post(crate::handlers::metrics::refresh_metrics),
        )
        .route(
            paths::METRICS_CACHE_STATS,
            get(crate::handlers::metrics::get_cache_stats),
        )
        .route(
            paths::METRICS_CACHE_MANAGE,
            post(crate::handlers::metrics::manage_cache),
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
        // User Balance & Transaction endpoints (consolidated)
        .route(
            paths::USER_BALANCE_BY_ADDRESS,
            get(crate::handlers::users::balance::get_balance_by_address_endpoint),
        )
        .route(
            paths::USER_BALANCE_BY_ID,
            get(crate::handlers::users::balance::get_balance_by_user_id_endpoint),
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
            paths::DEAL_PROCESS_DOCUMENT_EXTRACTION,
            post(crate::handlers::deals::document_extraction::process_document_extraction),
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
            paths::COMPANY_DEALS,
            get(crate::handlers::deals::deals::get_deals_by_company_endpoint),
        )
        .route(
            paths::COMPANY_DRAFT_DEALS,
            get(crate::handlers::deals::deals::get_draft_deals_by_company_endpoint),
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
        // LOAN TAPE ENDPOINTS
        // ========================================================================
        .route(
            paths::LOAN_TAPE_BULK,
            post(crate::handlers::loan_tape_handlers::create_bulk_loan_tape_handler),
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
        // COMPANY WALLET ADDRESS ENDPOINTS
        // ========================================================================
        .route(
            paths::COMPANY_WALLETS,
            post(company_handlers::create_wallet_address)
                .get(company_handlers::list_wallet_addresses),
        )
        .route(
            paths::COMPANY_WALLET_BY_ID,
            get(company_handlers::get_wallet_address).put(company_handlers::update_wallet_address),
        )
        .route(
            paths::COMPANY_USER_WALLETS,
            get(company_handlers::get_user_wallet_addresses),
        )
        .route(
            paths::COMPANY_WALLET_PERMISSIONS,
            post(company_handlers::grant_wallet_permission),
        )
        .route(
            paths::COMPANY_WALLET_PERMISSIONS_BY_WALLET,
            get(company_handlers::list_wallet_permissions),
        )
        .route(
            paths::COMPANY_WALLET_PERMISSION_BY_ID,
            delete(company_handlers::revoke_wallet_permission),
        )
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
            paths::FAUCET_REQUEST,
            post(crate::handlers::faucet::request_faucet_funds),
        )
        // ========================================================================
        // TAXONOMY ENDPOINTS - Arda Document Management
        // ========================================================================
        // Credit Agreements
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENTS,
            get(crate::handlers::taxonomy::credit_agreements::list_credit_agreements_handler),
        )
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENTS,
            post(crate::handlers::taxonomy::credit_agreements::create_credit_agreement_handler),
        )
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENTS_BATCH,
            post(crate::handlers::taxonomy::credit_agreements::create_credit_agreements_batch_handler),
        )
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENT_BY_ID,
            get(crate::handlers::taxonomy::credit_agreements::get_credit_agreement_handler),
        )
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENT_BY_ID,
            put(crate::handlers::taxonomy::credit_agreements::update_credit_agreement_handler),
        )
        .route(
            paths::TAXONOMY_CREDIT_AGREEMENT_BY_ID,
            delete(crate::handlers::taxonomy::credit_agreements::delete_credit_agreement_handler),
        )
        // Fee Letters
        .route(
            paths::TAXONOMY_FEE_LETTERS,
            get(crate::handlers::taxonomy::fee_letters::list_fee_letters_handler),
        )
        .route(
            paths::TAXONOMY_FEE_LETTERS,
            post(crate::handlers::taxonomy::fee_letters::create_fee_letter_handler),
        )
        .route(
            paths::TAXONOMY_FEE_LETTERS_BATCH,
            post(crate::handlers::taxonomy::fee_letters::create_fee_letters_batch_handler),
        )
        .route(
            paths::TAXONOMY_FEE_LETTER_BY_ID,
            get(crate::handlers::taxonomy::fee_letters::get_fee_letter_handler),
        )
        .route(
            paths::TAXONOMY_FEE_LETTER_BY_ID,
            put(crate::handlers::taxonomy::fee_letters::update_fee_letter_handler),
        )
        .route(
            paths::TAXONOMY_FEE_LETTER_BY_ID,
            delete(crate::handlers::taxonomy::fee_letters::delete_fee_letter_handler),
        )
        // Legal Opinions
        .route(
            paths::TAXONOMY_LEGAL_OPINIONS,
            get(crate::handlers::taxonomy::legal_opinions::list_legal_opinions_handler),
        )
        .route(
            paths::TAXONOMY_LEGAL_OPINIONS,
            post(crate::handlers::taxonomy::legal_opinions::create_legal_opinion_handler),
        )
        .route(
            paths::TAXONOMY_LEGAL_OPINIONS_BATCH,
            post(crate::handlers::taxonomy::legal_opinions::create_legal_opinions_batch_handler),
        )
        .route(
            paths::TAXONOMY_LEGAL_OPINION_BY_ID,
            get(crate::handlers::taxonomy::legal_opinions::get_legal_opinion_handler),
        )
        .route(
            paths::TAXONOMY_LEGAL_OPINION_BY_ID,
            put(crate::handlers::taxonomy::legal_opinions::update_legal_opinion_handler),
        )
        .route(
            paths::TAXONOMY_LEGAL_OPINION_BY_ID,
            delete(crate::handlers::taxonomy::legal_opinions::delete_legal_opinion_handler),
        )
        // Security Agreements
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENTS,
            get(crate::handlers::taxonomy::security_agreements::list_security_agreements_handler),
        )
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENTS,
            post(crate::handlers::taxonomy::security_agreements::create_security_agreement_handler),
        )
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENTS_BATCH,
            post(crate::handlers::taxonomy::security_agreements::create_security_agreements_batch_handler),
        )
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENT_BY_ID,
            get(crate::handlers::taxonomy::security_agreements::get_security_agreement_handler),
        )
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENT_BY_ID,
            put(crate::handlers::taxonomy::security_agreements::update_security_agreement_handler),
        )
        .route(
            paths::TAXONOMY_SECURITY_AGREEMENT_BY_ID,
            delete(crate::handlers::taxonomy::security_agreements::delete_security_agreement_handler),
        )
        // UCC Financing Statements
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENTS,
            get(crate::handlers::taxonomy::ucc_financing_statements::list_ucc_financing_statements_handler),
        )
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENTS,
            post(crate::handlers::taxonomy::ucc_financing_statements::create_ucc_financing_statement_handler),
        )
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENTS_BATCH,
            post(crate::handlers::taxonomy::ucc_financing_statements::create_ucc_financing_statements_batch_handler),
        )
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENT_BY_ID,
            get(crate::handlers::taxonomy::ucc_financing_statements::get_ucc_financing_statement_handler),
        )
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENT_BY_ID,
            put(crate::handlers::taxonomy::ucc_financing_statements::update_ucc_financing_statement_handler),
        )
        .route(
            paths::TAXONOMY_UCC_FINANCING_STATEMENT_BY_ID,
            delete(crate::handlers::taxonomy::ucc_financing_statements::delete_ucc_financing_statement_handler),
        )
        // Account Control Agreements
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENTS,
            get(crate::handlers::taxonomy::account_control_agreements::list_account_control_agreements_handler),
        )
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENTS,
            post(crate::handlers::taxonomy::account_control_agreements::create_account_control_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENTS_BATCH,
            post(crate::handlers::taxonomy::account_control_agreements::create_account_control_agreements_batch_handler),
        )
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENT_BY_ID,
            get(crate::handlers::taxonomy::account_control_agreements::get_account_control_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENT_BY_ID,
            put(crate::handlers::taxonomy::account_control_agreements::update_account_control_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ACCOUNT_CONTROL_AGREEMENT_BY_ID,
            delete(crate::handlers::taxonomy::account_control_agreements::delete_account_control_agreement_handler),
        )
        // Guaranties
        .route(
            paths::TAXONOMY_GUARANTIES,
            get(crate::handlers::taxonomy::guaranties::list_guaranties_handler),
        )
        .route(
            paths::TAXONOMY_GUARANTIES,
            post(crate::handlers::taxonomy::guaranties::create_guaranty_handler),
        )
        .route(
            paths::TAXONOMY_GUARANTIES_BATCH,
            post(crate::handlers::taxonomy::guaranties::create_guaranties_batch_handler),
        )
        .route(
            paths::TAXONOMY_GUARANTY_BY_ID,
            get(crate::handlers::taxonomy::guaranties::get_guaranty_handler),
        )
        .route(
            paths::TAXONOMY_GUARANTY_BY_ID,
            put(crate::handlers::taxonomy::guaranties::update_guaranty_handler),
        )
        .route(
            paths::TAXONOMY_GUARANTY_BY_ID,
            delete(crate::handlers::taxonomy::guaranties::delete_guaranty_handler),
        )
        // Assignment & Assumption Agreements routes
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENTS,
            get(crate::handlers::taxonomy::assignments::list_assignment_assumption_agreements_handler),
        )
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENTS,
            post(crate::handlers::taxonomy::assignments::create_assignment_assumption_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENTS_BATCH,
            post(crate::handlers::taxonomy::assignments::create_assignment_assumption_agreements_batch_handler),
        )
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENT_BY_ID,
            get(crate::handlers::taxonomy::assignments::get_assignment_assumption_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENT_BY_ID,
            put(crate::handlers::taxonomy::assignments::update_assignment_assumption_agreement_handler),
        )
        .route(
            paths::TAXONOMY_ASSIGNMENT_ASSUMPTION_AGREEMENT_BY_ID,
            delete(crate::handlers::taxonomy::assignments::delete_assignment_assumption_agreement_handler),
        )
        // Participation Agreements
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENTS,
            get(crate::handlers::taxonomy::participations::list_participation_agreements_handler),
        )
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENTS,
            post(crate::handlers::taxonomy::participations::create_participation_agreement_handler),
        )
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENTS_BATCH,
            post(crate::handlers::taxonomy::participations::create_participation_agreements_batch_handler),
        )
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENT_BY_ID,
            get(crate::handlers::taxonomy::participations::get_participation_agreement_handler),
        )
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENT_BY_ID,
            put(crate::handlers::taxonomy::participations::update_participation_agreement_handler),
        )
        .route(
            paths::TAXONOMY_PARTICIPATION_AGREEMENT_BY_ID,
            delete(crate::handlers::taxonomy::participations::delete_participation_agreement_handler),
        )
        // SSI Records
        .route(
            paths::TAXONOMY_SSI_RECORDS,
            get(crate::handlers::taxonomy::ssi_records::list_ssi_records_handler),
        )
        .route(
            paths::TAXONOMY_SSI_RECORDS,
            post(crate::handlers::taxonomy::ssi_records::create_ssi_record_handler),
        )
        .route(
            paths::TAXONOMY_SSI_RECORDS_BATCH,
            post(crate::handlers::taxonomy::ssi_records::create_ssi_records_batch_handler),
        )
        .route(
            paths::TAXONOMY_SSI_RECORD_BY_ID,
            get(crate::handlers::taxonomy::ssi_records::get_ssi_record_handler),
        )
        .route(
            paths::TAXONOMY_SSI_RECORD_BY_ID,
            put(crate::handlers::taxonomy::ssi_records::update_ssi_record_handler),
        )
        .route(
            paths::TAXONOMY_SSI_RECORD_BY_ID,
            delete(crate::handlers::taxonomy::ssi_records::delete_ssi_record_handler),
        )
        // Secondary Trades
        .route(
            paths::TAXONOMY_SECONDARY_TRADES,
            get(crate::handlers::taxonomy::secondary_trades::list_secondary_trades_handler),
        )
        .route(
            paths::TAXONOMY_SECONDARY_TRADES,
            post(crate::handlers::taxonomy::secondary_trades::create_secondary_trade_handler),
        )
        .route(
            paths::TAXONOMY_SECONDARY_TRADES_BATCH,
            post(crate::handlers::taxonomy::secondary_trades::create_secondary_trades_batch_handler),
        )
        .route(
            paths::TAXONOMY_SECONDARY_TRADE_BY_ID,
            get(crate::handlers::taxonomy::secondary_trades::get_secondary_trade_handler),
        )
        .route(
            paths::TAXONOMY_SECONDARY_TRADE_BY_ID,
            put(crate::handlers::taxonomy::secondary_trades::update_secondary_trade_handler),
        )
        .route(
            paths::TAXONOMY_SECONDARY_TRADE_BY_ID,
            delete(crate::handlers::taxonomy::secondary_trades::delete_secondary_trade_handler),
        )
        // Collateral
        .route(
            paths::COLLATERAL,
            get(crate::handlers::taxonomy::collateral::list_collateral_handler),
        )
        .route(
            paths::COLLATERAL,
            post(crate::handlers::taxonomy::collateral::create_collateral_handler),
        )
        .route(
            paths::COLLATERAL_BATCH,
            post(crate::handlers::taxonomy::collateral::create_collateral_batch_handler),
        )
        .route(
            paths::COLLATERAL_BY_ID,
            get(crate::handlers::taxonomy::collateral::get_collateral_handler),
        )
        .route(
            paths::COLLATERAL_BY_ID,
            put(crate::handlers::taxonomy::collateral::update_collateral_handler),
        )
        .route(
            paths::COLLATERAL_BY_ID,
            delete(crate::handlers::taxonomy::collateral::delete_collateral_handler),
        )
        .route(
            paths::COLLATERAL_WITH_DUPLICATES,
            get(crate::handlers::taxonomy::collateral::get_collateral_with_duplicates_handler),
        )
        .route(
            paths::COLLATERAL_WITH_MATCHES,
            get(crate::handlers::taxonomy::collateral::get_collateral_with_matches_handler),
        )
        .route(
            paths::COLLATERAL_BY_CUSTOM_ID,
            get(crate::handlers::taxonomy::collateral::get_collateral_by_custom_id_handler),
        )
        .route(
            paths::COLLATERAL_BY_DEAL_ID,
            get(crate::handlers::taxonomy::collateral::get_collateral_by_deal_id_handler),
        )
        // Internal API: Collateral fetch for arda-collateral-intel
        .route(
            "/api/v1/internal/collateral/{id}",
            get(crate::handlers::taxonomy::collateral::get_collateral_internal_handler),
        )
        // Internal API: Detection results from arda-collateral-intel
        .route(
            "/api/v1/internal/collateral/{id}/detection",
            post(crate::handlers::taxonomy::detection::receive_detection_result),
        )
        // Set maximum request body size to 100MB for document uploads
        // Default Axum limit is 2MB which is too small for PDFs
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100MB
        // Apply session authentication middleware BEFORE with_state
        .layer(axum::middleware::from_fn({
            let state_clone = state.clone();
            move |cookies, request, next| {
                let state = state_clone.clone();
                crate::middleware::session_middleware::session_auth_middleware(state, cookies, request, next)
            }
        }))
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
pub async fn get_openapi_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Filter OpenAPI specification by specific tags (paths, tags, and schemas)
fn filter_openapi_by_tags(
    mut spec: utoipa::openapi::OpenApi,
    allowed_tags: &[&str],
) -> utoipa::openapi::OpenApi {
    // Filter paths that have the specified tags
    let filtered_paths: std::collections::BTreeMap<String, utoipa::openapi::path::PathItem> = spec
        .paths
        .paths
        .into_iter()
        .filter(|(_, path_item)| {
            // Check if any operation in this path has one of the allowed tags
            let operations = [
                path_item.get.as_ref(),
                path_item.post.as_ref(),
                path_item.put.as_ref(),
                path_item.delete.as_ref(),
                path_item.patch.as_ref(),
            ];

            operations.into_iter().flatten().any(|op| {
                op.tags.as_ref().map_or(false, |tags| {
                    tags.iter().any(|tag| allowed_tags.contains(&tag.as_str()))
                })
            })
        })
        .collect();

    spec.paths.paths = filtered_paths;

    // Filter schemas based on category to keep individual files small
    if let Some(components) = spec.components.as_mut() {
        let original_schemas = components.schemas.clone();
        components.schemas.clear();

        // Define schema patterns for each category to minimize size
        let schema_patterns = match allowed_tags {
            tags if tags.contains(&"Auth") => vec![
                "MagicLink",
                "Auth",
                "Login",
                "Register",
                "Invite",
                "Verify",
                "UserType",
                "User",
            ],
            tags if tags.contains(&"Users") => vec![
                "User",
                "Identity",
                "Kyc",
                "Balance",
                "Profile",
                "Transaction",
                "Address",
            ],
            tags if tags.contains(&"Companies") => vec![
                "Company",
                "Membership",
                "Team",
                "Wallet",
                "Kyb",
                "Investor",
                "Permission",
                "Invitation",
            ],
            tags if tags.contains(&"Deals") => vec![
                "Deal",
                "Loan",
                "Marketplace",
                "Transfer",
                "Payment",
                "Listing",
                "Create",
                "Update",
                "List",
            ],
            tags if tags.contains(&"Admin") => vec![
                "Admin",
                "Kyc",
                "Bulk",
                "Platform",
                "Statistics",
                "Withdrawal",
                "Pending",
            ],
            tags if tags.contains(&"Payments") => vec![
                "Payment",
                "Withdrawal",
                "Transaction",
                "Financial",
                "Balance",
                "Initiate",
            ],
            tags if tags.contains(&"State") => vec![
                "State",
                "Batch",
                "Transaction",
                "Contract",
                "Proof",
                "Current",
                "Trigger",
            ],
            tags if tags.contains(&"Metrics") => {
                vec!["Metrics", "Market", "Pie", "Chart", "Value", "Refresh"]
            }
            tags if tags.contains(&"Loan Tape") => {
                vec!["LoanTape", "Loan", "Tape", "BulkLoan", "ListLoan"]
            }
            tags if tags.contains(&"Collateral") => vec![
                "Collateral",
                "Match",
                "Duplicate",
                "ErrorResponse",
                "DeleteResponse",
                "BatchOperation",
            ],
            tags if tags.contains(&"Internal") => {
                vec!["Internal", "Detection", "Collateral", "ErrorResponse"]
            }
            tags if tags.contains(&"Dev") => vec!["Faucet", "Network", "Health", "Info"],
            _ => vec![
                "Credit",
                "Fee",
                "Legal",
                "Security",
                "Ucc",
                "Account",
                "Guaranty",
                "Assignment",
                "Participation",
                "Ssi",
                "Secondary",
                "Taxonomy",
                "Delete",
                "Paginated",
                "Batch",
            ], // Taxonomy catch-all
        };

        // Only include schemas that match the category patterns
        for (name, schema) in &original_schemas {
            if schema_patterns.iter().any(|pattern| {
                name.contains(pattern) || name.to_lowercase().contains(&pattern.to_lowercase())
            }) {
                components.schemas.insert(name.clone(), schema.clone());
            }
        }

        // Always include common/base schemas if they exist in original
        let common_schemas = ["PaginationParams", "ApiError", "ErrorResponse", "UserType"];
        for schema_name in common_schemas {
            if let Some(schema) = original_schemas.get(schema_name) {
                components
                    .schemas
                    .insert(schema_name.to_string(), schema.clone());
            }
        }
    }

    // Filter tags to only include the allowed ones
    if let Some(ref mut tags) = spec.tags {
        tags.retain(|tag| allowed_tags.contains(&tag.name.as_str()));
    }

    spec
}

/// Individual API spec handlers for each category
pub async fn get_auth_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Auth"]);
    Json(spec)
}

pub async fn get_users_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Users"]);
    Json(spec)
}

pub async fn get_companies_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(
        ApiDoc::openapi(),
        &["Companies", "Company Memberships", "Company Wallets"],
    );
    Json(spec)
}

pub async fn get_deals_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Deals"]);
    Json(spec)
}

pub async fn get_loan_tape_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Loan Tape"]);
    Json(spec)
}

pub async fn get_admin_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Admin"]);
    Json(spec)
}

pub async fn get_payments_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Payments"]);
    Json(spec)
}

pub async fn get_state_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["State"]);
    Json(spec)
}

pub async fn get_collateral_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Collateral"]);
    Json(spec)
}

pub async fn get_taxonomy_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(
        ApiDoc::openapi(),
        &[
            "Taxonomy - Credit Agreements",
            "Taxonomy - Fee Letters",
            "Taxonomy - Legal Opinions",
            "Taxonomy - Security Agreements",
            "Taxonomy - UCC Financing Statements",
            "Taxonomy - Account Control Agreements",
            "Taxonomy - Guaranties",
            "Taxonomy - Assignment & Assumption Agreements",
            "Taxonomy - Participation Agreements",
            "Taxonomy - SSI Records",
            "Taxonomy - Secondary Trades",
        ],
    );
    Json(spec)
}

pub async fn get_metrics_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Metrics"]);
    Json(spec)
}

pub async fn get_internal_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Internal"]);
    Json(spec)
}

pub async fn get_dev_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    let spec = filter_openapi_by_tags(ApiDoc::openapi(), &["Dev"]);
    Json(spec)
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
