//! OpenAPI/Swagger documentation for the Arda Credit API
//!
//! This module contains all OpenAPI schema definitions and endpoint documentation
//! for the REST API. This is the single source of truth for API documentation.
//!
//! The schemas and endpoints are organized by functional area for maintainability.

use serde::Deserialize;
use utoipa::{IntoParams, OpenApi, ToSchema};

// Re-export types from handlers module
pub use crate::handlers::{
    get_current_user,
    logout_endpoint,
    request_magic_link,
    verify_magic_link,

    // Account types (NEW)
    AccountBalanceResponse,
    AccountDepositsResponse,
    AccountLookupQuery,
    AccountLookupResponse,
    AccountWithBalances,
    AccountWithdrawalsResponse,
    // Finance types
    // AccountFinancialTransactionResponse, // REMOVED - deprecated
    // Admin & User types
    AdminUserIdentity,
    AdminUserListResponse,
    AdminWithdrawalDashboard,
    // AI types
    AnalysisHistoryResponse,
    AnalysisSummary,
    AnalyticsDashboardResponse,
    AnalyzeDocumentQuery,
    ApiError as AiApiError,
    ApproveWithdrawalRequest,
    BalanceInfo,
    BatchInfo,
    BatchListResponse,
    BatchProcessorStatsResponse,
    BulkKycApprovalRequest,
    BulkKycApprovalResponse,
    BulkOperationResponse,
    // Core types
    ContractConfigQuery,
    ContractConfigResponse,
    ContractSubmissionData,
    CreateAccountFinancialTransactionRequest,
    CreateBatchRequest,
    CreateBatchResponse,
    // Deal types
    CreateDealRequest,
    CurrentStateResponse,
    DateRange,
    DealDetailsResponse,
    DealListingResponse,
    DealPaymentHistoryResponse,
    DealStatsResponse,
    DealTransferResponse,
    DealTransferValidationResponse,
    DeleteDealResponse,
    DepositInfo,
    DocumentActivity,
    // FinancialSummaryResponse, // REMOVED - deprecated
    // FinancialTransactionQuery, // REMOVED - deprecated
    // Auth types
    GenerateInviteCodeRequest,
    GenerateInviteCodeResponse,
    InvestorProfileQuery,
    InvestorProfileResponse,
    KycStatisticsResponse,
    ListDealForSaleRequest,
    ListDealsQuery,
    ListDealsResponse,
    ListIdentitiesQuery,
    ListIdentitiesResponse,
    ListPendingKycQuery,
    ListPendingKycResponse,
    LogoutResponse,
    MarketplaceDealResponse,
    PaginatedFinancialTransactionsResponse,
    PaginationInfo,
    PaymentDetailsResponse,
    PaymentResponse,
    PendingTransactionsResponse,
    PlatformMetricsResponse,
    RecordDealPaymentRequest,
    RegisterInvestorRequest,
    RegisterInvestorResponse,
    RegisterUserRequest,
    RegisterUserResponse,
    RejectWithdrawalRequest,
    RiskTrendPoint,
    SubmitKycRequest,
    SubmitKycResponse,
    SubmitTransactionRequest,
    SubmitTransactionResponse,
    TransactionHistoryItem,
    TransactionHistoryResponse,
    TransactionInfo,
    TransactionSummary,
    TransactionUpdateEvent,
    TransactionUpdatesQuery,
    TransferDealOwnershipRequest,
    TriggerAnalysisRequest,
    TriggerAnalysisResponse,
    TriggerBatchResponse,
    UnifiedFinancialTransactionResponse,
    UpdateBatchProofRequest,

    UpdateDealRequest,
    UpdateDealResponse,
    UpdateInvestorProfileRequest,
    UpdateKycStatusRequest,
    UpdateUserRequest,
    UserAccountsResponse,
    UserBalancesResponse,
    UserDetailsResponse,
    UserTransactionsResponse,
    ValidateDealTransferRequest,
    ValidateInviteCodeResponse,
    WithdrawalInfo,
};

pub use crate::config::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
};
pub use crate::config::types::UserType;
pub use crate::handlers::users::balance::{
    AddressBalance, AddressBalanceResponse, UserBalanceResponse,
};
pub use crate::services::ai_analysis_service::{
    AiChatMessage, AiChatRequest, AiChatResponse, ChangeAnalysis, ChangeCategory, ChangeType,
    DocumentDiffRequest, DocumentDiffResponse, ImpactLevel, NegotiationInsight, RiskFactor,
};
// AvailableBalance removed - now using consolidated AddressBalance from balance handlers
pub use crate::services::withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalStatusResponse,
};

/// Pagination parameters for list endpoints
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct PaginationParams {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Number of items per page
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

// ============================================================================
// MAIN OPENAPI DOCUMENTATION
// ============================================================================

/// Main OpenAPI documentation for the Arda Credit API
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Arda API",
        version = "v1",
        description = "Arda API",
        contact(
            name = "Arda Global",
            email = "support@arda.xyz",
            url = "https://arda.xyz"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080", description = "Development server"),
        (url = "https://api.arda.credit", description = "Production server")
    ),
    paths(
        // Health and Info endpoints
        crate::handlers::core::health::health_check,
        crate::handlers::core::health::api_info,

        // Magic Link Authentication endpoints (NEW)
        crate::handlers::auth::magic_link::request_magic_link,
        crate::handlers::auth::magic_link::verify_magic_link,
        crate::handlers::auth::magic_link::get_current_user,
        crate::handlers::auth::magic_link::magic_link_logout_endpoint,

        // Authentication utilities (keep these)
        crate::handlers::auth::authentication::validate_invite_code_endpoint,
        crate::handlers::auth::authentication::generate_invite_code_endpoint,

        // User Management endpoints
        crate::handlers::users::users::register_user_endpoint,
        crate::handlers::users::users::get_user_details_endpoint,
        crate::handlers::users::users::list_identities_endpoint,
        crate::handlers::users::users::update_user_endpoint,
        crate::handlers::users::users::delete_user_endpoint,
        crate::handlers::users::balance::get_balance_by_address_endpoint,
        crate::handlers::users::balance::get_balance_by_user_id_endpoint,
        crate::handlers::users::users::get_user_transactions_endpoint,
        crate::handlers::users::users::submit_kyc_endpoint,
        crate::handlers::users::users::update_kyc_status_endpoint,

        // Admin endpoints
        crate::handlers::users::admin::list_pending_kyc_endpoint,
        crate::handlers::users::admin::get_kyc_statistics_endpoint,
        crate::handlers::users::admin::bulk_kyc_approval_endpoint,
        crate::handlers::users::admin::get_platform_metrics_endpoint,

        // State Management endpoints
        crate::handlers::core::state::submit_transaction_endpoint,
        crate::handlers::core::state::get_pending_transactions_endpoint,
        crate::handlers::core::state::create_batch_endpoint,
        crate::handlers::core::state::get_batches_endpoint,
        crate::handlers::core::state::get_current_state_endpoint,
        crate::handlers::core::state::get_batch_endpoint,
        crate::handlers::core::state::get_batch_details_endpoint,
        crate::handlers::core::state::get_contract_submission_data_endpoint,
        crate::handlers::core::state::update_batch_proof_endpoint,
        crate::handlers::core::state::trigger_batch_endpoint,

        // Deal Management endpoints (consolidated - includes marketplace and transfer operations)
        crate::handlers::deals::deals::create_deal_endpoint,
        crate::handlers::deals::deals::list_deals_endpoint,
        crate::handlers::deals::deals::get_deals_by_user_id_endpoint,
        crate::handlers::deals::deals::get_deal_details_endpoint,
        crate::handlers::deals::deals::get_deal_stats_endpoint,
        crate::handlers::deals::deals::update_deal_endpoint,
        crate::handlers::deals::deals::delete_deal_endpoint,
        crate::handlers::deals::deals::record_deal_payment_endpoint,
        crate::handlers::deals::deals::get_payment_details_endpoint,
        crate::handlers::deals::deals::get_deal_payment_history_endpoint,

        // Consolidated Deal Marketplace endpoints (moved from marketplace_handlers)
        crate::handlers::deals::deals::browse_deals_for_sale_endpoint,
        crate::handlers::deals::deals::list_deal_for_sale_endpoint,
        crate::handlers::deals::deals::remove_deal_from_sale_endpoint,
        crate::handlers::deals::deals::deal_marketplace_stats_endpoint,

        // Consolidated Deal Transfer endpoints (moved from transfer_handlers)
        crate::handlers::deals::deals::transfer_deal_ownership_endpoint,
        crate::handlers::deals::deals::get_deal_transfer_history_endpoint,
        crate::handlers::deals::deals::get_user_deal_transfer_history_endpoint,
        crate::handlers::deals::deals::validate_deal_transfer_endpoint,

        // Withdrawal endpoints
        crate::handlers::finance::withdrawals::initiate_withdrawal_endpoint,
        crate::handlers::finance::withdrawals::get_withdrawal_status_endpoint,
        crate::handlers::finance::withdrawals::get_user_withdrawal_history_endpoint,
        crate::handlers::finance::withdrawals::cancel_withdrawal_endpoint,
        crate::handlers::finance::withdrawals::get_pending_withdrawals_endpoint,

        // AI Analysis endpoints
        crate::handlers::ai::analysis::analyze_document_diff,
        crate::handlers::ai::analysis::process_chat_message,
        crate::handlers::ai::analysis::get_negotiation_insights,
        crate::handlers::ai::analysis::get_chat_history,
        crate::handlers::ai::analysis::get_analysis_history,
        crate::handlers::ai::analysis::trigger_analysis,
        crate::handlers::ai::analysis::get_analytics_dashboard,


        // Faucet endpoints (local/staging only)
        crate::handlers::faucet::get_faucet_info,
        crate::handlers::faucet::check_faucet_status,
        crate::handlers::faucet::request_faucet_funds,

        // Admin withdrawal endpoints
        crate::handlers::finance::withdrawals::approve_withdrawal_request,
        crate::handlers::finance::withdrawals::reject_withdrawal_request,
        crate::handlers::finance::withdrawals::get_admin_withdrawal_dashboard,
        crate::handlers::finance::withdrawals::manual_execute_withdrawal,

        // Account Management endpoints (NEW - Account-Centric Model)
        crate::handlers::accounts::lookup_account,
        crate::handlers::accounts::get_account,
        crate::handlers::accounts::get_user_accounts,
        crate::handlers::accounts::get_account_balance_endpoint,
        crate::handlers::accounts::get_user_balances,
        crate::handlers::accounts::get_account_deposits_endpoint,
        crate::handlers::accounts::get_account_withdrawals_endpoint,

        // Transaction History endpoints
        crate::handlers::finance::transaction_history::get_transaction_history,

        // Company Management endpoints
        crate::handlers::companies::companies::register_company_endpoint,
        crate::handlers::companies::companies::get_company_endpoint,
        crate::handlers::companies::companies::get_company_dashboard_endpoint,
        crate::handlers::companies::companies::submit_kyb_endpoint,
        crate::handlers::companies::companies::get_kyb_status_endpoint,
        crate::handlers::companies::companies::invite_team_member_endpoint,
        crate::handlers::companies::companies::list_team_members_endpoint,
        crate::handlers::companies::companies::update_team_member_role_endpoint,
        crate::handlers::companies::companies::remove_team_member_endpoint,
        crate::handlers::companies::companies::get_investor_profile_endpoint,
        crate::handlers::companies::companies::update_investor_profile_endpoint,
        crate::handlers::companies::companies::register_investor_endpoint,

        // Contract configuration endpoints
        crate::handlers::core::contract::get_contract_config,
        crate::handlers::core::contract::get_transaction_updates,
    ),
    tags(
        // Primary API Groups (8 main groups + Dev)
        (name = "Auth", description = "Authentication, authorization, and invite code management"),
        (name = "Users", description = "User account management, KYC verification, balances, and transaction history"),
        (name = "Companies", description = "Originator and investor company profiles, documents, turnover tracking, and AI analysis"),
        (name = "Deals", description = "Deal lifecycle management, marketplace operations, ownership transfers, and payment processing"),
        (name = "State", description = "Blockchain state management, batch processing, ZK proof generation, and contract integration"),
        (name = "Admin", description = "Administrative operations including KYC approval, withdrawal management, and platform oversight"),
        (name = "Analytics", description = "Pool analytics, credit lines, transaction history, metrics, and AI-powered insights"),
        (name = "Payments", description = "Financial transactions, withdrawal processing, and payment tracking across accounts, pools, and credit lines"),
        (name = "Dev", description = "Development and testing utilities: health checks, API info, faucet operations (local/staging only)")
    ),
    components(schemas(
        // Authentication types
        RegisterUserRequest,
        RegisterUserResponse,
        UserDetailsResponse,
        UpdateUserRequest,
        LogoutResponse,
        RegisterInvestorRequest,
        RegisterInvestorResponse,
        InvestorProfileQuery,
        InvestorProfileResponse,
        UpdateInvestorProfileRequest,
        UserType,

        // User profile types (nested profile data)
        crate::handlers::users::users::InvestorProfileData,
        crate::handlers::users::users::OriginatorProfileData,

        // Company types
        crate::services::company_service::CompanyRegistrationRequest,
        crate::services::company_service::CompanyResponseDto,
        crate::services::company_service::KybSubmissionRequest,
        crate::services::company_service::TeamInvitationRequest,
        crate::services::company_service::TeamMemberResponseDto,
        crate::services::company_service::TeamMemberPermissions,
        crate::handlers::companies::companies::KybStatusResponse,
        crate::handlers::companies::companies::UpdateTeamMemberRoleRequest,
        crate::handlers::companies::companies::CompanyDashboardResponse,
        crate::handlers::companies::companies::TeamStats,
        crate::handlers::companies::companies::DocumentStats,
        crate::handlers::companies::companies::ActivityItem,

        // Document types
        crate::handlers::companies::documents::DocumentUploadResponse,
        crate::handlers::companies::documents::DocumentDownloadHeaders,
        crate::handlers::companies::documents::DocumentVerificationResponse,
        crate::handlers::companies::documents::DocumentListResponse,
        crate::handlers::companies::documents::AuditLogsResponse,
        crate::services::document_service::DocumentResponseDto,
        crate::services::document_service::DocumentAuditLogDto,

        // Admin types
        AdminUserIdentity,
        AdminUserListResponse,
        BulkKycApprovalRequest,
        BulkKycApprovalResponse,
        BulkOperationResponse,
        KycStatisticsResponse,
        ListPendingKycQuery,
        ListPendingKycResponse,

        // Account types (NEW - Account-Centric Model)
        crate::handlers::accounts::AccountLookupQuery,
        crate::handlers::accounts::AccountLookupResponse,
        crate::handlers::accounts::UserAccountsResponse,
        crate::handlers::accounts::AccountBalanceResponse,
        crate::handlers::accounts::BalanceInfo,
        crate::handlers::accounts::UserBalancesResponse,
        crate::handlers::accounts::AccountWithBalances,
        crate::handlers::accounts::AccountDepositsResponse,
        crate::handlers::accounts::DepositInfo,
        crate::handlers::accounts::AccountWithdrawalsResponse,
        crate::handlers::accounts::WithdrawalInfo,
        crate::handlers::accounts::PaginationQuery,

        // Financial transaction types (DEPRECATED - removed)
        // FinancialTransactionQuery,
        // CreateAccountFinancialTransactionRequest,
        // AccountFinancialTransactionResponse,
        // UnifiedFinancialTransactionResponse,
        // FinancialSummaryResponse,

        // Deal types (consolidated - includes marketplace and transfer operations)
        CreateDealRequest,
        DeleteDealResponse,
        ListDealsQuery,
        ListDealsResponse,
        DealDetailsResponse,
        DealPaymentHistoryResponse,
        DealStatsResponse,
        PaymentDetailsResponse,
        PaymentResponse,
        RecordDealPaymentRequest,
        UpdateDealRequest,
        UpdateDealResponse,

        // Consolidated Deal Marketplace types (moved from marketplace_handlers)
        MarketplaceDealResponse,
        ListDealForSaleRequest,
        DealListingResponse,

        // Consolidated Deal Transfer types (moved from transfer_handlers)
        TransferDealOwnershipRequest,
        DealTransferResponse,
        ValidateDealTransferRequest,
        DealTransferValidationResponse,

        // State management types
        BatchInfo,
        BatchListResponse,
        BatchProcessorStatsResponse,
        ContractSubmissionData,
        CreateBatchRequest,
        CreateBatchResponse,
        CurrentStateResponse,
        PendingTransactionsResponse,
        SubmitTransactionRequest,
        SubmitTransactionResponse,
        TransactionInfo,
        TriggerBatchResponse,
        UpdateBatchProofRequest,

        // User types
        ListIdentitiesQuery,
        ListIdentitiesResponse,
        SubmitKycRequest,
        SubmitKycResponse,
        UpdateKycStatusRequest,
        UserTransactionsResponse,

        // Consolidated Balance types
        AddressBalance,
        AddressBalanceResponse,
        UserBalanceResponse,

        // Withdrawal types
        PaginationParams,
        InitiateWithdrawalRequest,
        InitiateWithdrawalResponse,
        WithdrawalStatusResponse,
        // AvailableBalance removed - replaced with consolidated AddressBalance,
        // Admin withdrawal types
        AdminWithdrawalDashboard,
        ApproveWithdrawalRequest,
        RejectWithdrawalRequest,

        // Faucet types (local/staging only)
        crate::handlers::faucet::FaucetInfoResponse,
        crate::handlers::faucet::FaucetStatusQuery,
        crate::handlers::faucet::FaucetStatusResponse,
        crate::handlers::faucet::FaucetRequestBody,
        crate::handlers::faucet::FaucetRequestResponse,
        crate::handlers::faucet::NetworkConfig,

        // Contract configuration types
        ContractConfigResponse,
        ContractConfigQuery,
        TransactionUpdateEvent,
        TransactionUpdatesQuery,

        // AI Analysis types
        AnalyzeDocumentQuery,
        AnalysisHistoryResponse,
        AnalysisSummary,
        AnalyticsDashboardResponse,
        AiApiError,
        DocumentActivity,
        RiskTrendPoint,
        TriggerAnalysisRequest,
        TriggerAnalysisResponse,

        // AI Service types
        AiChatMessage,
        AiChatRequest,
        AiChatResponse,
        ChangeAnalysis,
        ChangeCategory,
        ChangeType,
        DocumentDiffRequest,
        DocumentDiffResponse,
        ImpactLevel,
        NegotiationInsight,
        RiskFactor,

        // Magic Link Authentication schemas (NEW)
        MagicLinkRequest,
        MagicLinkResponse,
        VerifyMagicLinkRequest,
        VerifyMagicLinkResponse,
        UserInfo,
        MagicLinkErrorResponse,

        // REMOVE these old schemas (password-based auth removed):
        // LoginRequest, (DELETED)
        // LoginResponse, (DELETED)
        // UserSummary, (DELETED)
    )),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

/// Security addon to configure session cookie authentication
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "session_cookie",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Cookie(
                        utoipa::openapi::security::ApiKeyValue::new(String::from("session_token")),
                    ),
                ),
            );
        }
    }
}
