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
pub use crate::deal_handlers::{
    CreateDealRequest,
    DealDetailsResponse,
    DealListingResponse,
    DealPaymentHistoryResponse,
    DealStatsResponse,
    DealTransferResponse,
    DealTransferValidationResponse,
    DeleteDealResponse,
    // Consolidated marketplace types (moved from marketplace_handlers)
    ListDealForSaleRequest,
    ListDealsQuery,
    ListDealsResponse,
    MarketplaceDealResponse,
    PaymentDetailsResponse,
    PaymentResponse,
    RecordDealPaymentRequest,
    // Consolidated transfer types (moved from transfer_handlers)
    TransferDealOwnershipRequest,
    UpdateDealRequest,
    UpdateDealResponse,
    ValidateDealTransferRequest,
};
pub use crate::financial_transaction_handlers::{
    AccountFinancialTransactionResponse, CreateAccountFinancialTransactionRequest,
    CreateCreditLineFinancialTransactionRequest, CreatePoolFinancialTransactionRequest,
    CreatePoolFundingRequest, CreditLineFinancialTransactionResponse, FinancialSummaryResponse,
    FinancialTransactionQuery, PaginatedFinancialTransactionsResponse,
    PoolFinancialTransactionResponse, UnifiedFinancialTransactionResponse,
};
use crate::magic_link_handlers::{
    get_current_user, logout_endpoint, request_magic_link, verify_magic_link,
};
use crate::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
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
        title = "Arda Credit Deal API",
        version = "v1",
        description = r#"
# Arda Credit Deal Processing API

A privacy-preserving deal processing system built on SP1 zero-knowledge proofs that enables secure financial deal management with complete transaction privacy.

## API Categories

The API is organized into 8 main categories:

### 🔐 Auth - Authentication & Authorization
Handle user authentication, session management, and access control.
- Magic link authentication (passwordless)
- Session validation and logout
- Invite code generation and validation

**Example**: `POST /api/v1/auth/magic-link` to request authentication link

### 👤 Users - User Management & Profiles
Manage user accounts, profiles, and KYC verification.
- User registration and profile management
- KYC submission and status tracking
- User balance and transaction queries

**Example**: `POST /api/v1/user` to register new user

### 🏢 Companies - Company Management & Operations
Handle company profiles, liquidity pools, and credit lines.
- Merchant/Investor profile management
- Liquidity pool creation and management
- Credit line eligibility and requests

**Example**: `POST /api/v1/pools` to create liquidity pool

### 📊 Deals - Deal Lifecycle & Operations
Complete deal management including marketplace and transfers.
- Deal creation, updates, and payments
- Marketplace listing and browsing
- Ownership transfers between investors

**Example**: `POST /api/v1/deal` to create new deal

### ⚡ State - Blockchain State Management
Zero-knowledge proof batching and contract interactions.
- Transaction batch creation and processing
- ZK proof generation and verification
- Blockchain state synchronization

**Example**: `POST /api/v1/state/trigger-batch` to create ZK proof batch

### 🔧 Admin - Administrative Operations
Platform administration and system management.
- KYC approval workflows
- Withdrawal approvals
- Platform metrics and monitoring

**Example**: `GET /api/v1/admin/kyc/pending` to list pending KYC requests

### 📈 Analytics - Data Analysis & AI
Reporting, insights, and AI-powered document analysis.
- Transaction history and reporting
- AI-powered document analysis
- Platform analytics and insights

**Example**: `POST /api/v1/ai/analyze` for document analysis

### 💰 Payments - Financial Transactions
Payment processing, withdrawals, and financial tracking.
- Deal payments and history
- USDC deposits and withdrawals
- Financial transaction tracking

**Example**: `POST /api/v1/deal/{id}/payment` to record payment

## Core Features

- **Privacy-First Design**: All operations maintain transaction privacy through SP1 zero-knowledge proofs
- **Deal Lifecycle Management**: Complete deal origination, payment, and closeout workflows
- **Decentralized Marketplace**: List, browse, and trade deals between investors
- **Automated Compliance**: KYC verification and regulatory compliance built-in
- **Real-time Processing**: Batch processing for efficient ZK proof generation

## Authentication

The API uses **magic link authentication** (passwordless):
1. Request magic link via email: `POST /api/v1/auth/magic-link`
2. Verify link and get session: `POST /api/v1/auth/verify`
3. Use session cookie for subsequent requests
4. Logout when done: `POST /api/v1/auth/logout`

## Error Handling

All endpoints return standardized error responses:
```json
{
  "error": "validation_failed",
  "message": "Deal amount must be positive",
  "details": {
    "field": "principal_amount",
    "code": "INVALID_AMOUNT"
  }
}
```

Common HTTP status codes:
- `400` - Bad Request (validation errors)
- `401` - Unauthorized (authentication required)
- `403` - Forbidden (insufficient permissions)
- `404` - Not Found (resource doesn't exist)
- `409` - Conflict (business rule violation)
- `429` - Too Many Requests (rate limited)
- `500` - Internal Server Error

## Rate Limiting

API requests are subject to rate limiting:
- **General endpoints**: 100 requests per minute per IP
- **Authentication endpoints**: 10 requests per minute per IP
- **Batch processing**: 5 requests per minute per user

Rate limit headers are included in responses:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```
        "#,
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
        // ===================================================================
        // AUTH - Authentication & Authorization
        // ===================================================================
        crate::magic_link_handlers::request_magic_link,
        crate::magic_link_handlers::verify_magic_link,
        crate::magic_link_handlers::get_current_user,
        crate::magic_link_handlers::logout_endpoint,
        crate::authentication_handlers::validate_invite_code_endpoint,
        crate::authentication_handlers::generate_invite_code_endpoint,

        // ===================================================================
        // USERS - User Management & Profiles
        // ===================================================================
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

        // ===================================================================
        // COMPANIES - Company Management & Profiles
        // ===================================================================
        crate::merchant_handlers::register_merchant_endpoint,
        crate::merchant_handlers::get_merchant_profile_endpoint,
        crate::merchant_handlers::update_merchant_profile_endpoint,
        crate::merchant_handlers::register_investor_endpoint,
        crate::merchant_handlers::get_investor_profile_endpoint,
        crate::merchant_handlers::update_investor_profile_endpoint,
        crate::turnover_handlers::create_turnover_endpoint,
        crate::turnover_handlers::get_turnover_endpoint,
        crate::turnover_handlers::update_turnover_endpoint,
        crate::turnover_handlers::get_profile_completion_endpoint,
        crate::pool_handlers::list_pools,
        crate::pool_handlers::get_pool_details,
        crate::pool_handlers::create_pool,
        crate::pool_handlers::update_pool_status,
        crate::pool_handlers::get_pool_metrics_overview,
        crate::pool_handlers::get_investor_pool_metrics_overview,
        crate::pool_borough_handlers::set_borough_limit,
        crate::pool_borough_handlers::get_borough_limits,
        crate::pool_borough_handlers::get_borough_summary,
        crate::pool_borough_handlers::validate_borough_limit,
        crate::pool_borough_handlers::delete_borough_limit,
        crate::credit_line_handlers::check_credit_eligibility,
        crate::credit_line_handlers::submit_credit_line_request,
        crate::credit_line_handlers::list_credit_line_requests,
        crate::credit_line_handlers::browse_available_pools,
        crate::credit_line_handlers::list_active_credit_lines_with_pool_details,

        // ===================================================================
        // DEALS - Deal Lifecycle & Operations
        // ===================================================================
        crate::deal_handlers::create_deal_endpoint,
        crate::deal_handlers::list_deals_endpoint,
        crate::deal_handlers::get_deals_by_user_id_endpoint,
        crate::deal_handlers::get_deal_details_endpoint,
        crate::deal_handlers::get_deal_stats_endpoint,
        crate::deal_handlers::update_deal_endpoint,
        crate::deal_handlers::delete_deal_endpoint,
        // Deal Marketplace Operations
        crate::deal_handlers::browse_deals_for_sale_endpoint,
        crate::deal_handlers::list_deal_for_sale_endpoint,
        crate::deal_handlers::remove_deal_from_sale_endpoint,
        crate::deal_handlers::deal_marketplace_stats_endpoint,
        // Deal Transfer Operations
        crate::deal_handlers::transfer_deal_ownership_endpoint,
        crate::deal_handlers::get_deal_transfer_history_endpoint,
        crate::deal_handlers::get_user_deal_transfer_history_endpoint,
        crate::deal_handlers::validate_deal_transfer_endpoint,

        // ===================================================================
        // STATE - Blockchain State Management
        // ===================================================================
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
        crate::contract_handlers::get_contract_config,
        crate::contract_handlers::get_transaction_updates,

        // ===================================================================
        // ADMIN - Administrative Operations
        // ===================================================================
        crate::admin_handlers::list_pending_kyc_endpoint,
        crate::admin_handlers::get_kyc_statistics_endpoint,
        crate::admin_handlers::bulk_kyc_approval_endpoint,
        crate::admin_handlers::get_platform_metrics_endpoint,
        crate::withdrawal_handlers::approve_withdrawal_request,
        crate::withdrawal_handlers::reject_withdrawal_request,
        crate::withdrawal_handlers::get_admin_withdrawal_dashboard,
        crate::withdrawal_handlers::manual_execute_withdrawal,
        crate::withdrawal_handlers::get_pending_withdrawals_endpoint,
        crate::document_handlers::approve_document,
        crate::document_handlers::reject_document,
        crate::document_handlers::verify_document,
        crate::document_handlers::get_audit_logs,

        // ===================================================================
        // ANALYTICS - Data Analysis & Reporting
        // ===================================================================
        crate::turnover_handlers::get_turnover_analytics_endpoint,
        crate::transaction_history_handlers::get_transaction_history,
        crate::transaction_history_handlers::get_pool_transaction_history,
        crate::ai_analysis_handlers::analyze_document_diff,
        crate::ai_analysis_handlers::trigger_analysis,
        crate::ai_analysis_handlers::get_negotiation_insights,
        crate::ai_analysis_handlers::get_analysis_history,
        crate::ai_analysis_handlers::process_chat_message,
        crate::ai_analysis_handlers::get_chat_history,
        crate::ai_analysis_handlers::get_analytics_dashboard,
        crate::document_handlers::upload_document,
        crate::document_handlers::download_document,
        crate::document_handlers::get_document,
        crate::document_handlers::list_documents,
        crate::document_handlers::delete_document,

        // ===================================================================
        // PAYMENTS - Financial Transactions & Payments
        // ===================================================================
        crate::deal_handlers::record_deal_payment_endpoint,
        crate::deal_handlers::get_payment_details_endpoint,
        crate::deal_handlers::get_deal_payment_history_endpoint,
        crate::withdrawal_handlers::initiate_withdrawal_endpoint,
        crate::withdrawal_handlers::get_withdrawal_status_endpoint,
        crate::withdrawal_handlers::get_user_available_balance_endpoint,
        crate::withdrawal_handlers::get_user_withdrawal_history_endpoint,
        crate::withdrawal_handlers::cancel_withdrawal_endpoint,
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

        // ===================================================================
        // SYSTEM - Health & Information
        // ===================================================================
        crate::health_handlers::health_check,
        crate::health_handlers::api_info,
    ),
    tags(
        // ===============================================================
        // PRIMARY CATEGORIES (8 main groups as requested)
        // ===============================================================
        (name = "Auth", description = "🔐 Authentication & Authorization - User authentication, sessions, and access control"),
        (name = "Users", description = "👤 User Management - User profiles, KYC verification, and account management"),
        (name = "Companies", description = "🏢 Company Management - Merchant/Investor profiles, pools, credit lines, and company operations"),
        (name = "Deals", description = "📊 Deal Operations - Deal lifecycle, marketplace, transfers, and ownership management"),
        (name = "State", description = "⚡ Blockchain State - ZK proof batching, contract interactions, and state management"),
        (name = "Admin", description = "🔧 Administrative Operations - Platform administration, approvals, and system management"),
        (name = "Analytics", description = "📈 Data Analysis & AI - Reporting, insights, document analysis, and AI-powered features"),
        (name = "Payments", description = "💰 Financial Transactions - Payments, withdrawals, deposits, and financial tracking"),

        // ===============================================================
        // SUPPORTING CATEGORIES (detailed sub-categories for clarity)
        // ===============================================================
        (name = "Health", description = "🩺 System Health - Health checks and API information"),
        (name = "KYC", description = "✅ Know Your Customer - Identity verification and compliance"),
        (name = "Documents", description = "📄 Document Management - Upload, verification, and audit trails"),
        (name = "Liquidity Pools", description = "🏦 Pool Management - Liquidity pool creation, funding, and metrics"),
        (name = "Credit Lines", description = "💳 Credit Operations - Credit eligibility, requests, and management"),
        (name = "AI Analysis", description = "🤖 AI Document Analysis - Automated document processing and insights"),
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

        // ===============================================================
        // ERROR RESPONSE DOCUMENTATION
        // ===============================================================
        // The following error response types provide comprehensive error
        // handling across all API endpoints:
        //
        // • ErrorResponse - Standard error format for most endpoints
        // • CreditLineErrorResponse - Credit line specific errors
        // • BoroughErrorResponse - Borough limit validation errors
        // • AiApiError - AI analysis and document processing errors
        // • MagicLinkErrorResponse - Authentication flow errors
        //
        // All error responses follow the standardized format:
        // {
        //   "error": "error_code",
        //   "message": "Human readable message",
        //   "details": { "field": "field_name", "code": "SPECIFIC_ERROR" }
        // }
        //
        // HTTP status codes used throughout the API:
        // 400 - Bad Request (validation errors)
        // 401 - Unauthorized (authentication required)
        // 403 - Forbidden (insufficient permissions)
        // 404 - Not Found (resource doesn't exist)
        // 409 - Conflict (business rule violation)
        // 429 - Too Many Requests (rate limited)
        // 500 - Internal Server Error
        // ===============================================================
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
