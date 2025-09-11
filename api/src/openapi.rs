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
};
pub use crate::authentication_handlers::{
    GenerateInviteCodeRequest, GenerateInviteCodeResponse, LoginRequest, LoginResponse,
    LogoutResponse, ValidateInviteCodeResponse,
};
pub use crate::credit_line_handlers::{
    AvailablePoolResponse, CreditLineErrorResponse, CreditLineRequestListResponse,
    CreditLineRequestQuery, CreditLineRequestResponse, EligibilityCheckDetail,
    EligibilityCheckResponse, PoolBrowsingQuery, SubmitCreditLineRequestBody,
};
pub use crate::financial_transaction_handlers::{
    AccountFinancialTransactionResponse, CreateAccountFinancialTransactionRequest,
    CreateCreditLineFinancialTransactionRequest, CreatePoolFinancialTransactionRequest,
    CreditLineFinancialTransactionResponse, FinancialSummaryResponse,
    FinancialTransactionQuery, PaginatedFinancialTransactionsResponse,
    PoolFinancialTransactionResponse, UnifiedFinancialTransactionResponse,
};
pub use crate::loan_handlers::{
    CreateLoanRequest, DeleteLoanResponse, ListLoansQuery, ListLoansResponse, LoanDetailsResponse,
    LoanPaymentHistoryResponse, LoanStatsResponse, PaymentDetailsResponse, PaymentResponse,
    RecordPaymentRequest, UpdateLoanRequest, UpdateLoanResponse,
};
pub use crate::merchant_handlers::{
    RegisterInvestorRequest, RegisterInvestorResponse, RegisterMerchantRequest,
    RegisterMerchantResponse, MerchantProfileQuery, MerchantProfileResponse,
    InvestorProfileQuery, InvestorProfileResponse, UpdateMerchantProfileRequest,
    UpdateInvestorProfileRequest,
};
pub use crate::pool_api::{
    BenchmarkData, CreatePoolRequest, CreatePoolResponse, ErrorResponse, InvestorDashboardResponse,
    InvestorMetrics, InvestorPoolSummariesResponse, InvestorPoolSummary,
    MarketOverviewResponse, PageInfo, PlatformMetrics, PoolDetailsResponse,
    PoolListQuery, PoolListResponse, PoolMatchRequest, PoolPerformanceData,
    PoolPerformanceMetrics, PoolPerformanceResponse, PoolSummaryInfo,
    PoolTransactionInfo, PoolTransactionsResponse, PoolUtilizationResponse,
    RecentActivity, StandardPoolObject, TransactionHistoryQuery,
    UpdatePoolStatusRequest, UtilizationDataPoint, UtilizationHistoryQuery,
};
pub use crate::rest::UserType;
pub use crate::state_handlers::{
    BatchInfo, BatchListResponse, BatchProcessorStatsResponse, ContractSubmissionData,
    CreateBatchRequest, CreateBatchResponse, CurrentStateResponse, PendingTransactionsResponse,
    SubmitTransactionRequest, SubmitTransactionResponse, TransactionInfo, TriggerBatchResponse,
    UpdateBatchProofRequest,
};
pub use crate::turnover_handlers::{
    ProfileCompletionResponse, TurnoverAnalyticsResponse, TurnoverCreateRequest, TurnoverResponse,
    TurnoverUpdateRequest,
};
pub use crate::user_handlers::{
    ListIdentitiesQuery, ListIdentitiesResponse, RegisterUserRequest, RegisterUserResponse,
    SubmitKycRequest, SubmitKycResponse, UpdateKycStatusRequest, UpdateUserRequest,
    UserBalanceResponse, UserDetailsResponse, UserTransactionsResponse,
};
pub use crate::withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalStatusResponse,
};
pub use crate::balance_service::AvailableBalance;

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
        description = "REST API for the Arda Credit platform providing credit and investment services"
    ),
    servers(
        (url = "http://localhost:8080", description = "Development server"),
        (url = "https://api.arda.credit", description = "Production server")
    ),
    tags(
        (name = "Authentication", description = "User authentication endpoints"),
        (name = "User Management", description = "User profile and account management"),
        (name = "Investor", description = "Investor-specific operations"),
        (name = "Merchant", description = "Merchant profile and operations"),
        (name = "KYC", description = "Know Your Customer verification"),
        (name = "State Management", description = "Blockchain state operations"),
        (name = "Batch Processing", description = "Transaction batch management"),
        (name = "Transactions", description = "Transaction operations"),
        (name = "Pools", description = "Liquidity pool management"),
        (name = "Loans", description = "Loan lifecycle management"),
        (name = "Withdrawals", description = "Withdrawal operations"),
        (name = "Credit Lines", description = "Credit line requests and management"),
        (name = "Financial", description = "Financial transaction tracking"),
        (name = "Analytics", description = "Platform analytics and reporting"),
        (name = "Admin", description = "Administrative operations")
    ),
    components(schemas(
        // Authentication types
        RegisterUserRequest,
        RegisterUserResponse,
        UserDetailsResponse,
        UpdateUserRequest,
        LoginRequest,
        LoginResponse,
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

        // Additional merchant types
        InvestorProfileQuery,
        InvestorProfileResponse,
        UpdateMerchantProfileRequest,
        UpdateInvestorProfileRequest
    ))
)]
pub struct ApiDoc;
