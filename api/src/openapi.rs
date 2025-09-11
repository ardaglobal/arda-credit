//! OpenAPI/Swagger documentation for the Arda Credit API
//!
//! This module contains all OpenAPI schema definitions and endpoint documentation
//! for the REST API. This is the single source of truth for API documentation.
//!
//! The schemas and endpoints are organized by functional area for maintainability.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};

// Re-export types from their new locations
pub use crate::rest::UserType;
pub use crate::authentication_handlers::{
    LoginRequest, LoginResponse, LogoutResponse, GenerateInviteCodeRequest,
    GenerateInviteCodeResponse, ValidateInviteCodeResponse,
};
pub use crate::user_handlers::{
    RegisterUserRequest, RegisterUserResponse, UserDetailsResponse, UpdateUserRequest,
    ListIdentitiesQuery, ListIdentitiesResponse, SubmitKycRequest, SubmitKycResponse,
    UpdateKycStatusRequest, UserBalanceResponse, UserTransactionsResponse,
};
pub use crate::merchant_handlers::{
    RegisterMerchantRequest, RegisterMerchantResponse, RegisterInvestorRequest,
    RegisterInvestorResponse,
};
pub use crate::state_handlers::{
    CreateBatchRequest, CreateBatchResponse, BatchListResponse, BatchInfo,
    SubmitTransactionRequest, SubmitTransactionResponse, PendingTransactionsResponse,
    TransactionInfo, CurrentStateResponse, UpdateBatchProofRequest,
    ContractSubmissionData, BatchProcessorStatsResponse, TriggerBatchResponse,
};
pub use crate::loan_handlers::{
    CreateLoanRequest, LoanDetailsResponse, ListLoansQuery, ListLoansResponse,
    LoanStatsResponse, UpdateLoanRequest, UpdateLoanResponse, DeleteLoanResponse,
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
        RegisterUserRequest,
        RegisterUserResponse,
        UserDetailsResponse,
        UpdateUserRequest,
        LoginRequest,
        LoginResponse,
        LogoutResponse,
        RegisterInvestorRequest,
        RegisterMerchantRequest,
        UserType,
        PaginationParams
    ))
)]
pub struct ApiDoc;
