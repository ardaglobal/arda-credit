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

// Core modules for batch processing functionality
pub mod ads_service;
pub mod background_processor;
pub mod merkle_tree;
pub mod merkle_tree_32;
pub mod vapp_integration;

pub mod ccr;
pub mod db;
pub mod error;

// Re-export batch processing types and functions
pub use db::{
    // Invite code functions
    check_invite_code,
    // Batch functions
    create_batch,
    // CCR Domain functions
    create_identity,
    create_identity_with_password,
    // Investor profile functions
    create_investor_profile,
    // Liquidity pool functions
    create_liquidity_pool,
    create_loan,
    // Merchant functions
    create_merchant,
    create_merchant_document,
    create_merchant_profile,
    create_or_update_turnover,
    create_payment,
    create_state_snapshot,
    // USDC Deposit functions
    create_usdc_deposit,
    // Withdrawal functions
    create_withdrawal,
    create_withdrawal_request,
    delete_identity,

    find_matching_liquidity_pools,
    generate_invite_code,
    get_all_batches,
    // Pool query functions
    get_all_liquidity_pools,
    get_balance,
    get_batch_by_id,
    get_contract_submission_data,
    // State functions
    get_current_counter_value,
    get_current_state,
    get_deposit_by_tx_hash,
    get_deposits_by_user_address,

    get_identity_by_email,
    get_identity_by_id,
    get_identity_by_reset_token,
    get_investor_dashboard_metrics,
    get_investor_pool_summaries,
    get_investor_profile_by_user_id,
    get_investor_profile_with_identity,
    get_invite_code_info,
    get_kyc_statistics,
    get_liquidity_pool_by_id,
    get_liquidity_pools_by_investor,
    get_loan_by_id,
    get_market_overview_metrics,
    // Merchant query functions
    get_merchant_by_id,
    get_merchant_by_user_id,
    get_merchant_documents,
    get_merchant_profile_by_merchant_id,
    get_merchant_profile_completion,
    get_merchant_turnover,
    get_merchant_turnover_analytics,
    get_merchant_with_profile_by_user_id,
    get_payments_by_loan,
    get_pending_transactions,

    get_pending_withdrawal_requests,
    get_pool_summary,
    get_pool_transactions,
    get_pool_utilization_history,
    get_potential_stuck_batches,
    get_proven_unposted_batches,
    get_user_balance_by_address,
    get_user_balance_by_id,
    get_user_deposit_balance,
    get_user_id_by_wallet_address,
    get_user_withdrawal_balance,
    get_withdrawal_by_tx_hash,
    get_withdrawal_request_by_id,
    get_withdrawal_requests_by_user_address,
    get_withdrawal_requests_by_user_id,
    get_withdrawals_by_user_address,
    // Database connection
    init_db,
    init_db_with_url,

    list_identities,
    list_loans,
    list_pending_kyc_applications,
    mark_batch_posted_to_contract,
    mark_batch_posted_to_contract_with_tx_hash,

    register_or_reregister_user,
    // Authentication functions
    reset_password_with_token,
    set_email_verification_token,
    set_password_reset_token,
    // ADS/Merkle functions
    store_ads_state_commit,
    submit_kyc_data,
    // Transaction functions
    submit_transaction,
    update_balance,
    update_batch_proof,
    update_deposit_status,
    update_deposit_status_tx,
    // Merchant update functions
    update_document_status,
    update_identity_profile,
    update_investor_profile,
    update_kyc_status,
    update_last_login,
    update_liquidity_pool_status,
    update_loan,
    update_merchant_profile,
    update_turnover,
    update_user_balance_from_deposit,
    update_user_balance_from_deposit_tx,
    update_user_balance_from_withdrawal,
    update_user_balance_from_withdrawal_tx,
    update_user_password,
    update_withdrawal_request_confirmations,
    update_withdrawal_request_state,
    update_withdrawal_request_submitted,
    update_withdrawal_request_validation,
    update_withdrawal_status,
    update_withdrawal_status_tx,
    use_invite_code,
    validate_pool_parameters,
    verify_email_with_token,
    // Types
    AdsStateCommit,
    Balance,
    BusinessType,
    ContractPrivateData,
    ContractPublicData,
    ContractSubmissionData,
    CounterState,
    // Merchant turnover and document types
    CreateDocumentRequest,
    // Investor profile types
    CreateInvestorProfileRequest,
    CreateLiquidityPoolRequest,
    CreateMerchantProfileRequest,
    CreateMerchantRequest,
    CreateTurnoverRequest,
    // CCR Domain types
    Identity,
    IncomingTransaction,
    InvestorDashboardMetrics,
    InvestorProfile,
    InviteCodeInfo,
    KycInvestorType,
    KycStatistics,
    LiquidityPool,
    LiquidityPoolAudit,
    Loan,
    LoanTransactionDb,
    MarketOverviewMetrics,
    // Merchant types
    Merchant,
    MerchantDocument,
    MerchantProfile,
    MerchantProfileCompletion,
    MerchantTurnoverAnalytics,
    MerchantTurnoverDaily,
    Payment,
    PoolMatchingCriteria,
    // Pool query types
    PoolSummary,
    PoolTransaction,
    PoolUtilizationData,
    ProofBatch,

    RegisterResult,
    StateSnapshot,
    UpdateInvestorProfileRequest,
    UpdateMerchantProfileRequest,
    UpdateTurnoverRequest,
    UsdcDeposit,
    UsdcWithdrawal,
    UserDepositBalance,
    UserWithdrawalBalance,
    WithdrawalRequest,
    WithdrawalRequestHistory,
};

// Re-export essential error types
pub use error::{DbError, DbResult};

// Re-export ADS and related types
pub use ads_service::{
    AdsConfig, AdsError, AdsMetrics, AdsServiceFactory, AuditEvent, AuditEventType, AuditTrail,
    AuthenticatedDataStructure, BatchInclusionInfo, CcrMetadata, ComplianceStatus,
    IndexedMerkleTreeADS, LoanInclusionProof, LoanInclusionRequest, LoanLifecycleSummary,
    LoanMetadata, LoanOperation, LoanOperationProof, MembershipProof, NonMembershipProof,
    StateCommitment, StateTransition, WitnessData,
};

pub use background_processor::{BackgroundProcessor, ProcessorBuilder, ProcessorConfig};
pub use merkle_tree::{
    AlgorithmInsertionResult, IndexedMerkleTree, InsertionMetrics, InsertionProof, InsertionResult,
    LowNullifier, MerkleNode, MerkleNodeDb, MerkleProof, MerkleTreeDb, Nullifier, NullifierDb,
    TreeState, TreeStateDb, TreeStats,
};
pub use merkle_tree_32::{BatchUpdate, MerkleProof32, MerkleTree32, Tree32Stats, TreeMetrics};
pub use vapp_integration::{
    ComplianceError, ComplianceResult, Environment, ProofError, ProofType, SettlementError,
    SettlementResult, VAppAdsIntegration, VAppBatchResponse, VAppConfig, VAppError,
    VAppInsertionResponse, VAppProofResponse, ZkProof,
};

// Re-export CCR types
pub use ccr::{CcrOperation, MappingContext, MappingError, TransactionMapper};

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;

// Temporarily disabled - depends on removed modules
// #[cfg(test)]
// mod proof_verification_tests;
