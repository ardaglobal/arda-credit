//! OpenAPI/Swagger documentation for the Arda Credit API
//!
//! This module contains all OpenAPI schema definitions and endpoint documentation
//! for the REST API. This is the single source of truth for API documentation.
//!
//! The schemas and endpoints are organized by functional area for maintainability.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};

// Re-export types from their new locations
pub use crate::admin_handlers::{
    AdminUserIdentity, AdminUserListResponse, BulkKycApprovalRequest, BulkKycApprovalResponse,
    BulkOperationResponse, KycStatisticsResponse, ListPendingKycQuery, ListPendingKycResponse,
    PlatformMetricsResponse,
};
pub use crate::authentication_handlers::{
    GenerateInviteCodeRequest, GenerateInviteCodeResponse, LogoutResponse,
    ValidateInviteCodeResponse,
};
pub use crate::balance_service::AvailableBalance;
pub use crate::contract_handlers::{
    ContractConfigQuery, ContractConfigResponse, TransactionUpdateEvent, TransactionUpdatesQuery,
};
pub use crate::credit_line_handlers::{
    ActiveCreditLineResponse, ActiveCreditLinesListResponse, ActiveCreditLinesQuery,
    ActivePoolDetails, AvailablePoolResponse, CreditLineErrorResponse,
    CreditLineRequestListResponse, CreditLineRequestQuery, CreditLineRequestResponse,
    EligibilityCheckDetail, EligibilityCheckResponse, PoolBrowsingQuery,
    SubmitCreditLineRequestBody,
};
pub use crate::financial_transaction_handlers::{
    AccountFinancialTransactionResponse, CreateAccountFinancialTransactionRequest,
    CreateCreditLineFinancialTransactionRequest, CreatePoolFinancialTransactionRequest,
    CreatePoolFundingRequest, CreditLineFinancialTransactionResponse, FinancialSummaryResponse,
    FinancialTransactionQuery, PaginatedFinancialTransactionsResponse,
    PoolFinancialTransactionResponse, UnifiedFinancialTransactionResponse,
};
pub use crate::loan_handlers::{
    CreateLoanRequest, DeleteLoanResponse, ListLoansQuery, ListLoansResponse, LoanDetailsResponse,
    LoanPaymentHistoryResponse, LoanStatsResponse, PaymentDetailsResponse, PaymentResponse,
    RecordPaymentRequest, UpdateLoanRequest, UpdateLoanResponse,
};
use crate::magic_link_handlers::{
    get_current_user, logout_endpoint, request_magic_link, verify_magic_link,
};
use crate::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
};
pub use crate::marketplace_handlers::{
    ListLoanForSaleRequest, ListLoanForSaleResponse, LoansForSaleResponse, MarketplaceDeal,
    RemoveLoanFromSaleResponse,
};
pub use crate::merchant_handlers::{
    InvestorProfileQuery, InvestorProfileResponse, MerchantProfileQuery, MerchantProfileResponse,
    RegisterInvestorRequest, RegisterInvestorResponse, RegisterMerchantRequest,
    RegisterMerchantResponse, UpdateInvestorProfileRequest, UpdateMerchantProfileRequest,
};
pub use crate::pool_api::{
    BenchmarkData, CreatePoolRequest, CreatePoolResponse, ErrorResponse, InvestorDashboardResponse,
    InvestorMetrics, InvestorPoolMetricsOverviewResponse, InvestorPoolSummariesResponse,
    InvestorPoolSummary, MarketOverviewResponse, PageInfo, PlatformMetrics, PoolDetailsResponse,
    PoolListQuery, PoolListResponse, PoolMatchRequest, PoolMetricsOverviewResponse,
    PoolPerformanceData, PoolPerformanceMetrics, PoolPerformanceResponse, PoolSummaryInfo,
    PoolTransactionInfo, PoolTransactionsResponse, PoolUtilizationResponse, RecentActivity,
    StandardPoolObject, TransactionHistoryQuery, UpdatePoolStatusRequest, UtilizationDataPoint,
    UtilizationHistoryQuery,
};
pub use crate::pool_borough_handlers::{
    BoroughErrorResponse, BoroughLimitResponse, BoroughSummaryResponse, BoroughValidationResponse,
    SetBoroughLimitRequest, ValidateBoroughQuery,
};
pub use crate::state_handlers::{
    BatchInfo, BatchListResponse, BatchProcessorStatsResponse, ContractSubmissionData,
    CreateBatchRequest, CreateBatchResponse, CurrentStateResponse, PendingTransactionsResponse,
    SubmitTransactionRequest, SubmitTransactionResponse, TransactionInfo, TriggerBatchResponse,
    UpdateBatchProofRequest,
};
pub use crate::transaction_history_handlers::{
    DateRange, PaginationInfo, TransactionHistoryItem, TransactionHistoryResponse,
    TransactionSummary,
};
pub use crate::transfer_handlers::{
    LoanTransferHistoryResponse, LoanTransferRequest, LoanTransferResponse, TransferHistoryItem,
    TransferHistoryQuery, UserTransferHistoryResponse,
};
pub use crate::turnover_handlers::{
    ProfileCompletionResponse, TurnoverAnalyticsResponse, TurnoverCreateRequest, TurnoverResponse,
    TurnoverUpdateRequest,
};
pub use crate::types::UserType;
pub use crate::user_handlers::{
    ListIdentitiesQuery, ListIdentitiesResponse, RegisterUserRequest, RegisterUserResponse,
    SubmitKycRequest, SubmitKycResponse, UpdateKycStatusRequest, UpdateUserRequest,
    UserBalanceResponse, UserDetailsResponse, UserTransactionsResponse,
};
pub use crate::withdrawal_handlers::{
    AdminWithdrawalDashboard, ApproveWithdrawalRequest, RejectWithdrawalRequest,
};
pub use crate::withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalStatusResponse,
};

// AI Analysis types
pub use crate::ai_analysis_handlers::{
    AnalysisHistoryResponse, AnalysisSummary, AnalyticsDashboardResponse, AnalyzeDocumentQuery,
    ApiError as AiApiError, DocumentActivity, RiskTrendPoint, TriggerAnalysisRequest,
    TriggerAnalysisResponse,
};
pub use crate::ai_analysis_service::{
    AiChatMessage, AiChatRequest, AiChatResponse, ChangeAnalysis, ChangeCategory, ChangeType,
    DocumentDiffRequest, DocumentDiffResponse, ImpactLevel, NegotiationInsight, RiskFactor,
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
        title = "Arda Credit API",
        version = "v1",
        description = "REST API for the Arda Credit platform with magic link authentication providing credit and investment services"
    ),
    servers(
        (url = "http://localhost:8080", description = "Development server"),
        (url = "https://api.arda.credit", description = "Production server")
    ),
    paths(
        // Health and Info endpoints
        crate::health_handlers::health_check,
        crate::health_handlers::api_info,

        // Magic Link Authentication endpoints (NEW)
        crate::magic_link_handlers::request_magic_link,
        crate::magic_link_handlers::verify_magic_link,
        crate::magic_link_handlers::get_current_user,
        crate::magic_link_handlers::logout_endpoint,

        // Authentication utilities (keep these)
        crate::authentication_handlers::validate_invite_code_endpoint,
        crate::authentication_handlers::generate_invite_code_endpoint,

        // User Management endpoints
        crate::user_handlers::register_user_endpoint,
        crate::user_handlers::get_user_details_endpoint,
        crate::user_handlers::list_identities_endpoint,
        crate::user_handlers::update_user_endpoint,
        crate::user_handlers::delete_user_endpoint,
        crate::user_handlers::get_user_balance_by_address_endpoint,
        crate::user_handlers::get_user_balance_by_id_endpoint,
        crate::user_handlers::get_user_transactions_endpoint,
        crate::user_handlers::submit_kyc_endpoint,
        crate::user_handlers::update_kyc_status_endpoint,

        // Admin endpoints
        crate::admin_handlers::list_pending_kyc_endpoint,
        crate::admin_handlers::get_kyc_statistics_endpoint,
        crate::admin_handlers::bulk_kyc_approval_endpoint,
        crate::admin_handlers::get_platform_metrics_endpoint,

        // State Management endpoints
        crate::state_handlers::submit_transaction_endpoint,
        crate::state_handlers::get_pending_transactions_endpoint,
        crate::state_handlers::create_batch_endpoint,
        crate::state_handlers::get_batches_endpoint,
        crate::state_handlers::get_current_state_endpoint,
        crate::state_handlers::get_batch_endpoint,
        crate::state_handlers::get_batch_details_endpoint,
        crate::state_handlers::get_contract_submission_data_endpoint,
        crate::state_handlers::update_batch_proof_endpoint,
        crate::state_handlers::trigger_batch_endpoint,

        // Merchant endpoints
        crate::merchant_handlers::register_merchant_endpoint,
        crate::merchant_handlers::get_merchant_profile_endpoint,
        crate::merchant_handlers::update_merchant_profile_endpoint,
        crate::merchant_handlers::get_investor_profile_endpoint,
        crate::merchant_handlers::update_investor_profile_endpoint,
        crate::merchant_handlers::register_investor_endpoint,

        // Loan Management endpoints
        crate::loan_handlers::create_loan_endpoint,
        crate::loan_handlers::list_loans_endpoint,
        crate::loan_handlers::get_loan_details_endpoint,
        crate::loan_handlers::get_loan_stats_endpoint,
        crate::loan_handlers::update_loan_endpoint,
        crate::loan_handlers::delete_loan_endpoint,
        crate::loan_handlers::record_payment_endpoint,
        crate::loan_handlers::get_payment_details_endpoint,
        crate::loan_handlers::get_loan_payment_history_endpoint,

        // Loan Trading Marketplace endpoints
        crate::marketplace_handlers::browse_marketplace_endpoint,
        crate::marketplace_handlers::list_loan_for_sale_endpoint,
        crate::marketplace_handlers::remove_loan_from_sale_endpoint,
        crate::marketplace_handlers::marketplace_stats_endpoint,

        // Loan Transfer endpoints
        crate::transfer_handlers::transfer_loan_ownership_endpoint,
        crate::transfer_handlers::get_loan_transfer_history_endpoint,
        crate::transfer_handlers::get_user_transfer_history_endpoint,
        crate::transfer_handlers::validate_transfer_endpoint,

        // Pool endpoints
        crate::pool_handlers::list_pools,
        crate::pool_handlers::get_pool_details,
        crate::pool_handlers::create_pool,
        crate::pool_handlers::update_pool_status,
        crate::pool_handlers::get_pool_metrics_overview,
        crate::pool_handlers::get_investor_pool_metrics_overview,

        // Pool Borough Limit endpoints
        crate::pool_borough_handlers::set_borough_limit,
        crate::pool_borough_handlers::get_borough_limits,
        crate::pool_borough_handlers::get_borough_summary,
        crate::pool_borough_handlers::validate_borough_limit,
        crate::pool_borough_handlers::delete_borough_limit,

        // Withdrawal endpoints
        crate::withdrawal_handlers::initiate_withdrawal_endpoint,
        crate::withdrawal_handlers::get_withdrawal_status_endpoint,
        crate::withdrawal_handlers::get_user_available_balance_endpoint,
        crate::withdrawal_handlers::get_user_withdrawal_history_endpoint,
        crate::withdrawal_handlers::cancel_withdrawal_endpoint,
        crate::withdrawal_handlers::get_pending_withdrawals_endpoint,

        // Admin withdrawal endpoints
        crate::withdrawal_handlers::approve_withdrawal_request,
        crate::withdrawal_handlers::reject_withdrawal_request,
        crate::withdrawal_handlers::get_admin_withdrawal_dashboard,
        crate::withdrawal_handlers::manual_execute_withdrawal,

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
        crate::financial_transaction_handlers::get_merchant_credit_line_financial_transactions,

        // Transaction History endpoints
        crate::transaction_history_handlers::get_transaction_history,
        crate::transaction_history_handlers::get_pool_transaction_history,

        // Credit Line endpoints
        crate::credit_line_handlers::check_credit_eligibility,
        crate::credit_line_handlers::submit_credit_line_request,
        crate::credit_line_handlers::list_credit_line_requests,
        crate::credit_line_handlers::browse_available_pools,
        crate::credit_line_handlers::list_active_credit_lines_with_pool_details,

        // Turnover endpoints
        crate::turnover_handlers::create_turnover_endpoint,
        crate::turnover_handlers::get_turnover_endpoint,
        crate::turnover_handlers::update_turnover_endpoint,
        crate::turnover_handlers::get_turnover_analytics_endpoint,
        crate::turnover_handlers::get_profile_completion_endpoint,

        // Contract configuration endpoints
        crate::contract_handlers::get_contract_config,
        crate::contract_handlers::get_transaction_updates,

        // Document Management endpoints
        crate::document_handlers::upload_document,
        crate::document_handlers::download_document,
        crate::document_handlers::get_document,
        crate::document_handlers::list_documents,
        crate::document_handlers::approve_document,
        crate::document_handlers::reject_document,
        crate::document_handlers::delete_document,
        crate::document_handlers::verify_document,
        crate::document_handlers::get_audit_logs,

        // AI Analysis endpoints
        crate::ai_analysis_handlers::analyze_document_diff,
        crate::ai_analysis_handlers::trigger_analysis,
        crate::ai_analysis_handlers::get_negotiation_insights,
        crate::ai_analysis_handlers::get_analysis_history,
        crate::ai_analysis_handlers::process_chat_message,
        crate::ai_analysis_handlers::get_chat_history,
        crate::ai_analysis_handlers::get_analytics_dashboard,
    ),
    tags(
        (name = "Authentication", description = "User authentication endpoints"),
        (name = "Documents", description = "Document upload, download, and management"),
        (name = "User Management", description = "User profile and account management"),
        (name = "Users", description = "User identity and profile operations"),
        (name = "Investor", description = "Investor-specific operations"),
        (name = "Investors", description = "Investor profile and account management"),
        (name = "Merchant", description = "Merchant profile and operations"),
        (name = "Merchants", description = "Merchant profile and account management"),
        (name = "Merchant Turnover", description = "Merchant turnover tracking and reporting"),
        (name = "Merchant Analytics", description = "Merchant analytics and insights"),
        (name = "Merchant Profile", description = "Merchant profile completion tracking"),
        (name = "KYC", description = "Know Your Customer verification"),
        (name = "State Management", description = "Blockchain state operations"),
        (name = "State", description = "Application state queries"),
        (name = "Batch Processing", description = "Transaction batch management"),
        (name = "Batches", description = "Batch creation and management"),
        (name = "Transactions", description = "Transaction operations"),
        (name = "Liquidity Pools", description = "Liquidity pool management and queries"),
        (name = "Pool Metrics", description = "Platform-wide and investor-specific pool metrics and analytics"),
        (name = "Loans", description = "Loan lifecycle management"),
        (name = "Payments", description = "Payment processing and history"),
        (name = "Marketplace", description = "Loan marketplace operations"),
        (name = "Transfer", description = "Loan ownership transfer operations"),
        (name = "Withdrawals", description = "Withdrawal operations"),
        (name = "Admin Withdrawals", description = "Administrative withdrawal approval and management"),
        (name = "Admin Dashboard", description = "Administrative dashboard and monitoring"),
        (name = "Credit Lines", description = "Credit line requests and management"),
        (name = "Financial Transactions", description = "Financial transaction tracking and reporting"),
        (name = "Transaction History", description = "Comprehensive transaction history across all operations"),
        (name = "Analytics", description = "Platform analytics and reporting"),
        (name = "Admin", description = "Administrative operations"),
        (name = "Health", description = "Health check and system information"),
        (name = "Info", description = "API information and discovery"),
        (name = "contract", description = "Contract configuration and blockchain integration"),
        (name = "transactions", description = "Real-time transaction monitoring and updates"),
        (name = "AI Analysis", description = "AI-powered document analysis and change detection"),
        (name = "AI Chat", description = "Conversational AI for document analysis and negotiation advice"),
        (name = "AI Analytics", description = "Analytics and insights from AI document analysis")
    ),
    components(schemas(
        // Authentication types
        RegisterUserRequest,
        RegisterUserResponse,
        UserDetailsResponse,
        UpdateUserRequest,
        LogoutResponse,
        RegisterInvestorRequest,
        RegisterMerchantRequest,
        MerchantProfileQuery,
        MerchantProfileResponse,
        InvestorProfileQuery,
        InvestorProfileResponse,
        UpdateMerchantProfileRequest,
        UpdateInvestorProfileRequest,
        UserType,

        // Document types
        crate::document_handlers::DocumentUploadResponse,
        crate::document_handlers::DocumentDownloadHeaders,
        crate::document_handlers::DocumentVerificationResponse,
        crate::document_handlers::DocumentListResponse,
        crate::document_handlers::AuditLogsResponse,
        crate::document_service::DocumentResponseDto,
        crate::document_service::DocumentAuditLogDto,

        // Admin types
        AdminUserIdentity,
        AdminUserListResponse,
        BulkKycApprovalRequest,
        BulkKycApprovalResponse,
        BulkOperationResponse,
        KycStatisticsResponse,
        ListPendingKycQuery,
        ListPendingKycResponse,

        // Credit line types
        SubmitCreditLineRequestBody,
        CreditLineRequestResponse,
        EligibilityCheckResponse,
        EligibilityCheckDetail,
        AvailablePoolResponse,
        PoolBrowsingQuery,
        CreditLineRequestQuery,
        CreditLineRequestListResponse,
        CreditLineErrorResponse,

        // Financial transaction types
        FinancialTransactionQuery,
        CreateAccountFinancialTransactionRequest,
        CreatePoolFinancialTransactionRequest,
        CreatePoolFundingRequest,
        CreateCreditLineFinancialTransactionRequest,
        AccountFinancialTransactionResponse,
        PoolFinancialTransactionResponse,
        CreditLineFinancialTransactionResponse,
        UnifiedFinancialTransactionResponse,
        FinancialSummaryResponse,

        // Loan types
        CreateLoanRequest,
        DeleteLoanResponse,
        ListLoansQuery,
        ListLoansResponse,
        LoanDetailsResponse,
        LoanPaymentHistoryResponse,
        LoanStatsResponse,
        PaymentDetailsResponse,
        PaymentResponse,
        RecordPaymentRequest,
        UpdateLoanRequest,
        UpdateLoanResponse,

        // Loan Trading Marketplace types
        MarketplaceDeal,
        LoansForSaleResponse,
        ListLoanForSaleRequest,
        ListLoanForSaleResponse,
        RemoveLoanFromSaleResponse,

        // Loan Transfer types
        LoanTransferRequest,
        LoanTransferResponse,
        LoanTransferHistoryResponse,
        UserTransferHistoryResponse,
        TransferHistoryItem,
        TransferHistoryQuery,

        // Pool types
        PoolListQuery,
        UtilizationHistoryQuery,
        TransactionHistoryQuery,
        StandardPoolObject,
        PoolListResponse,
        PageInfo,
        PoolDetailsResponse,
        PoolSummaryInfo,
        PoolUtilizationResponse,
        UtilizationDataPoint,
        PoolTransactionsResponse,
        PoolTransactionInfo,
        InvestorDashboardResponse,
        InvestorMetrics,
        RecentActivity,
        InvestorPoolSummariesResponse,
        InvestorPoolSummary,
        PoolPerformanceMetrics,
        MarketOverviewResponse,
        PlatformMetrics,
        PoolPerformanceResponse,
        PoolPerformanceData,
        BenchmarkData,
        CreatePoolRequest,
        CreatePoolResponse,
        UpdatePoolStatusRequest,
        PoolMatchRequest,
        PoolMetricsOverviewResponse,
        InvestorPoolMetricsOverviewResponse,
        ErrorResponse,

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

        // Turnover types
        ProfileCompletionResponse,
        TurnoverAnalyticsResponse,
        TurnoverCreateRequest,
        TurnoverResponse,
        TurnoverUpdateRequest,

        // User types
        ListIdentitiesQuery,
        ListIdentitiesResponse,
        SubmitKycRequest,
        SubmitKycResponse,
        UpdateKycStatusRequest,
        UserBalanceResponse,
        UserTransactionsResponse,

        // Withdrawal types
        PaginationParams,
        InitiateWithdrawalRequest,
        InitiateWithdrawalResponse,
        WithdrawalStatusResponse,
        AvailableBalance,
        // Admin withdrawal types
        AdminWithdrawalDashboard,
        ApproveWithdrawalRequest,
        RejectWithdrawalRequest,

        // Additional merchant types
        InvestorProfileQuery,
        InvestorProfileResponse,
        UpdateMerchantProfileRequest,
        UpdateInvestorProfileRequest,

        // Borough limit types
        SetBoroughLimitRequest,
        BoroughLimitResponse,
        BoroughSummaryResponse,
        ValidateBoroughQuery,
        BoroughValidationResponse,
        BoroughErrorResponse,

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
