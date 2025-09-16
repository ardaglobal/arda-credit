#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::single_match_else)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::unused_self)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::zero_sized_map_values)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::suboptimal_flops)]

pub mod admin_handlers;
pub mod auth;
pub mod authentication_handlers;
pub mod balance_service;
pub mod batch_processor;
pub mod client;
pub mod credit_line_handlers;
pub mod financial_transaction_handlers;
pub mod health_handlers;
pub mod kyc_service;
pub mod loan_handlers;
pub mod merchant_handlers;
pub mod metrics;
pub mod middleware;
pub mod openapi;
pub mod paths;
pub mod pool_api;
pub mod pool_borough_handlers;
pub mod pool_handlers;
pub mod router;
pub mod server;
pub mod state_handlers;
pub mod telemetry;
pub mod transaction_history_handlers;
pub mod turnover_handlers;
pub mod types;
pub mod unified_batch_service;
pub mod user_handlers;
pub mod withdrawal_handlers;
pub mod withdrawal_service;

#[cfg(test)]
pub mod openapi_test;

// Temporarily disabled for minimal PoC:
// pub mod graphql;       // 883 lines - GraphQL API for old nullifier system (depends on disabled modules)
// pub mod integration;   // 480 lines - Complex deployment/scaling configs (depends on disabled modules)

// Re-export main API types for convenience
pub use client::{
    ApiClientError,
    ArithmeticApiClient, // Keep old name for compatibility
    BatchApiClient,
    BatchInfo,
    BatchListResponse,
    ContractSubmissionData,
    CreateBatchRequest,
    CreateBatchResponse,
    CurrentStateResponse,
    DepositTransactionInfo,
    HealthResponse,
    PendingTransactionsResponse,
    SubmitTransactionRequest,
    SubmitTransactionResponse,
    TransactionInfo,
    TransactionStatusResponse,
    UserBalanceResponse,
    UserTransactionsResponse,
};

pub use health_handlers::EndpointInfo;
pub use router::{create_router, ApiConfig, ApiState};

// Re-export new handler modules
pub use admin_handlers::*;
pub use merchant_handlers::*;
pub use state_handlers::*;
pub use user_handlers::*;
pub use withdrawal_handlers::*;

pub use server::{ApiServer, ApiServerBuilder, ApiServerConfig};

pub use unified_batch_service::{BatchCreationResult, UnifiedBatchService};

pub use auth::{AuthError, AuthService, Claims};

pub use kyc_service::{KycValidationResult, KycValidationService};

pub use balance_service::{
    AvailableBalance, BalanceVerificationResult, BalanceVerificationService,
};

pub use withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalServiceError,
    WithdrawalStatusResponse, WithdrawalTransactionService,
};

pub use pool_api::{
    CreatePoolRequest, CreatePoolResponse, ErrorResponse, InvestorDashboardResponse,
    InvestorMetrics, InvestorPoolSummariesResponse, InvestorPoolSummary, MarketOverviewResponse,
    PageInfo, PlatformMetrics, PoolDetailsResponse, PoolListQuery, PoolListResponse,
    PoolMatchRequest, PoolPerformanceResponse, PoolSummaryInfo, PoolTransactionsResponse,
    PoolUtilizationResponse, StandardPoolObject, TransactionHistoryQuery, UpdatePoolStatusRequest,
    UtilizationDataPoint, UtilizationHistoryQuery,
};

pub use pool_handlers::{
    get_investor_dashboard, get_investor_pool_summaries as handler_get_investor_pool_summaries,
    get_market_overview, get_pool_details, get_pool_performance,
    get_pool_transactions as handler_get_pool_transactions, get_pool_utilization, list_pools,
};

pub use credit_line_handlers::{
    browse_available_pools, check_credit_eligibility, list_credit_line_requests,
    submit_credit_line_request, AvailablePoolResponse, CreditLineErrorResponse,
    CreditLineRequestListResponse, CreditLineRequestQuery, CreditLineRequestResponse,
    EligibilityCheckResponse, PoolBrowsingQuery, SubmitCreditLineRequestBody,
};

pub use telemetry::{init_tracer, init_tracing, shutdown_telemetry};

// Note: New handler modules are available but not yet replacing rest.rs functionality

// Re-export database types that the API uses
pub use arda_credit_db::{
    // Credit line operations
    approve_credit_line_request,
    close_credit_line,
    create_batch,
    create_credit_line,
    create_credit_line_request,
    create_loan,
    get_all_batches,
    get_all_liquidity_pools,
    get_batch_by_id,
    get_contract_submission_data,
    get_credit_line_by_id,
    get_credit_line_request_by_id,
    get_credit_line_requests_by_merchant,
    get_current_state,
    get_investor_dashboard_metrics,
    get_investor_pool_summaries as db_get_investor_pool_summaries,
    get_liquidity_pool_by_id,
    get_loan_by_id,
    get_market_overview_metrics,
    get_merchant_credit_line_stats,
    get_pending_transactions,
    get_pool_summary,
    get_pool_transactions as db_get_pool_transactions,
    get_pool_utilization_history,
    // Database functions
    init_db,
    list_active_credit_lines,
    list_credit_lines_by_merchant,
    list_loans,
    store_ads_state_commit,
    submit_transaction,
    update_batch_proof,
    update_credit_line_request_evaluation,
    update_credit_line_request_status,
    update_credit_line_utilization,
    update_loan,

    AdsStateCommit,
    ContractPrivateData,

    ContractPublicData,
    ContractSubmissionData as DbContractSubmissionData,
    CounterState,
    CreateCreditLineRequestData,
    CreditLine as DbCreditLine,
    CreditLineRequest as DbCreditLineRequest,
    CreditLineStatus as DbCreditLineStatus,
    // Essential error handling
    DbError,
    DbResult,
    // New batch processing types
    IncomingTransaction,
    InvestorDashboardMetrics,
    MarketOverviewMetrics,
    MerchantCreditLineStats,

    PoolSummary,
    PoolTransaction,
    PoolUtilizationData,
    ProofBatch,
};
