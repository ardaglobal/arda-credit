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

pub mod auth;
pub mod balance_service;
pub mod batch_processor;
pub mod client;
pub mod kyc_service;
pub mod middleware;
pub mod paths;
pub mod pool_api;
pub mod pool_handlers;
pub mod rest;
pub mod server;
pub mod unified_batch_service;
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

pub use rest::{
    create_router, ApiConfig, ApiInfoResponse, ApiState, BatchListQuery,
    BatchListResponse as RestBatchListResponse, CreateBatchRequest as RestCreateBatchRequest,
    CreateBatchResponse as RestCreateBatchResponse,
    CurrentStateResponse as RestCurrentStateResponse, EndpointInfo,
    PendingTransactionsResponse as RestPendingTransactionsResponse,
    SubmitTransactionRequest as RestSubmitTransactionRequest,
    SubmitTransactionResponse as RestSubmitTransactionResponse, UpdateBatchProofRequest,
};

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
    ErrorResponse, InvestorDashboardResponse, InvestorMetrics, InvestorPoolSummariesResponse,
    InvestorPoolSummary, MarketOverviewResponse, PageInfo, PlatformMetrics, PoolDetailsResponse,
    PoolListQuery, PoolListResponse, PoolPerformanceResponse, PoolSummaryInfo,
    PoolTransactionsResponse, PoolUtilizationResponse, StandardPoolObject, TransactionHistoryQuery,
    UtilizationDataPoint, UtilizationHistoryQuery,
};

pub use pool_handlers::{
    get_investor_dashboard, get_investor_pool_summaries as handler_get_investor_pool_summaries,
    get_market_overview, get_pool_details, get_pool_performance,
    get_pool_transactions as handler_get_pool_transactions, get_pool_utilization, list_pools,
};

// Re-export database types that the API uses
pub use arda_credit_db::{
    create_batch,
    create_loan,
    get_all_batches,
    get_all_liquidity_pools,
    get_batch_by_id,
    get_contract_submission_data,
    get_current_state,
    get_investor_dashboard_metrics,
    get_investor_pool_summaries as db_get_investor_pool_summaries,
    get_liquidity_pool_by_id,
    get_loan_by_id,
    get_market_overview_metrics,
    get_pending_transactions,
    get_pool_summary,
    get_pool_transactions as db_get_pool_transactions,
    get_pool_utilization_history,
    // Database functions
    init_db,
    list_loans,
    store_ads_state_commit,
    submit_transaction,
    update_batch_proof,
    update_loan,

    AdsStateCommit,
    ContractPrivateData,

    ContractPublicData,
    ContractSubmissionData as DbContractSubmissionData,
    CounterState,
    // Essential error handling
    DbError,
    DbResult,
    // New batch processing types
    IncomingTransaction,
    InvestorDashboardMetrics,
    MarketOverviewMetrics,
    PoolSummary,
    PoolTransaction,
    PoolUtilizationData,
    ProofBatch,
};
