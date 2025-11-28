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

//! # Arda Credit Database Layer
//!
//! This crate provides database operations for the Arda Credit system,
//! including deal management, marketplace operations, and ownership transfers.
//!
//! ## Core Operations
//! - Deal creation, modification, and querying
//! - Deal marketplace listings and browsing
//! - Deal ownership transfers between investors
//! - User management and KYC operations
//! - USDC deposits and withdrawals
//! - Batch processing and ZK proof generation
//!
//! ## Recent Changes
//! Converted from loan-based terminology to deal-based terminology:
//! - Loan → Deal
//! - Borrower → Originator
//! - Lender → Investor
//! - All function names updated accordingly

// Core modules organized by domain
pub mod ads;
pub mod business;
pub mod processing;
pub mod taxonomy;

// Core infrastructure modules
pub mod db;
pub mod error;
pub mod telemetry;

// Document extraction mappers (reference implementation)
// Note: This module is provided as a reference for building your ingestion pipeline.
// It cannot be compiled directly due to cross-crate dependencies.
// See db/src/document_mappers.rs for implementation examples.
// pub mod document_mappers;

// Re-export marketplace operations
pub use business::marketplace::{
    browse_marketplace_deals, create_deal_listing, get_deal_listing_by_deal_id,
    remove_deal_listing, update_deal_listing_status, validate_deal_for_transfer, DealListing,
    MarketplaceDeal,
};

// Re-export transfer operations
pub use business::transfers::{
    cancel_transfer, execute_deal_transfer, get_deal_transfer_history, get_user_transfer_history,
    validate_transfer_eligibility, DealTransferHistory, ExecuteTransferRequest,
    TransferValidationRequest,
};

// Re-export metrics operations
pub use business::metrics::{
    batch_upsert_market_value_metrics, batch_upsert_pie_chart_metrics, clear_all_metrics,
    delete_old_market_value_metrics, delete_pie_chart_metrics, get_market_value_metrics,
    get_pie_chart_metrics, upsert_market_value_metric, upsert_pie_chart_metric,
    CreateMarketValueMetricRequest, CreatePieChartMetricRequest, MetricsMarketValueDaily,
    MetricsPieChartData,
};

// Re-export taxonomy operations
pub use taxonomy::credit_agreements::{
    create_credit_agreement, create_credit_agreements_batch, delete_credit_agreement,
    get_credit_agreement_by_document_id, get_credit_agreement_by_id, list_credit_agreements,
    update_credit_agreement, CreateCreditAgreementRequest, CreditAgreement,
    UpdateCreditAgreementRequest,
};

pub use taxonomy::fee_letters::{
    create_fee_letter, create_fee_letters_batch, delete_fee_letter, get_fee_letter_by_document_id,
    get_fee_letter_by_id, list_fee_letters, update_fee_letter, CreateFeeLetterRequest, FeeLetter,
    UpdateFeeLetterRequest,
};

pub use taxonomy::legal_opinions::{
    create_legal_opinion, create_legal_opinions_batch, delete_legal_opinion,
    get_legal_opinion_by_document_id, get_legal_opinion_by_id, list_legal_opinions,
    update_legal_opinion, CreateLegalOpinionRequest, LegalOpinion, UpdateLegalOpinionRequest,
};

pub use taxonomy::security_agreements::{
    create_security_agreement, create_security_agreements_batch, delete_security_agreement,
    get_security_agreement_by_document_id, get_security_agreement_by_id, list_security_agreements,
    update_security_agreement, CreateSecurityAgreementRequest, SecurityAgreement,
    UpdateSecurityAgreementRequest,
};

pub use taxonomy::ucc_financing_statements::{
    create_ucc_financing_statement, create_ucc_financing_statements_batch,
    delete_ucc_financing_statement, get_ucc_financing_statement_by_document_id,
    get_ucc_financing_statement_by_id, list_ucc_financing_statements,
    update_ucc_financing_statement, CreateUccFinancingStatementRequest, UccFinancingStatement,
    UpdateUccFinancingStatementRequest,
};

pub use taxonomy::account_control_agreements::{
    create_account_control_agreement, create_account_control_agreements_batch,
    delete_account_control_agreement, get_account_control_agreement_by_document_id,
    get_account_control_agreement_by_id, list_account_control_agreements,
    update_account_control_agreement, AccountControlAgreement,
    CreateAccountControlAgreementRequest, UpdateAccountControlAgreementRequest,
};

pub use taxonomy::guaranties::{
    create_guaranties_batch, create_guaranty, delete_guaranty, get_guaranty_by_document_id,
    get_guaranty_by_id, list_guaranties, update_guaranty, CreateGuarantyRequest, Guaranty,
    UpdateGuarantyRequest,
};

pub use taxonomy::assignments::{
    create_assignment_assumption_agreement, create_assignment_assumption_agreements_batch,
    delete_assignment_assumption_agreement, get_assignment_assumption_agreement_by_document_id,
    get_assignment_assumption_agreement_by_id, list_assignment_assumption_agreements,
    update_assignment_assumption_agreement, AssignmentAssumptionAgreement,
    CreateAssignmentAssumptionAgreementRequest, UpdateAssignmentAssumptionAgreementRequest,
};

pub use taxonomy::participations::{
    create_participation_agreement, create_participation_agreements_batch,
    delete_participation_agreement, get_participation_agreement_by_document_id,
    get_participation_agreement_by_id, list_participation_agreements,
    update_participation_agreement, CreateParticipationAgreementRequest, ParticipationAgreement,
    UpdateParticipationAgreementRequest,
};

pub use taxonomy::ssi_records::{
    create_ssi_record, create_ssi_records_batch, delete_ssi_record, get_ssi_record_by_document_id,
    get_ssi_record_by_id, list_ssi_records, update_ssi_record, CreateSsiRecordRequest, SsiRecord,
    UpdateSsiRecordRequest,
};

pub use taxonomy::secondary_trades::{
    create_secondary_trade, create_secondary_trades_batch, delete_secondary_trade,
    get_secondary_trade_by_id, get_secondary_trade_by_trade_id, list_secondary_trades,
    update_secondary_trade, CreateSecondaryTradeRequest, SecondaryTrade,
    UpdateSecondaryTradeRequest,
};

pub use taxonomy::collateral::{
    check_for_duplicates, create_collateral, create_collateral_batch, delete_collateral,
    get_collateral_by_custom_id, get_collateral_by_deal_id, get_collateral_by_id,
    get_collateral_matches, get_collateral_with_duplicates, get_match_threshold, list_collateral,
    process_duplicate_check, update_collateral, Collateral, CollateralCheck,
    CollateralCheckQueueEntry, CollateralMatch, CollateralMatchDetail, CollateralWithDuplicates,
    CollateralWithMatches, CreateCollateralRequest, UpdateCollateralRequest,
};

// Re-export batch processing types and functions
pub use db::{
    approve_withdrawal_request,
    // Invite code functions
    check_invite_code,
    // Loan tape functions
    count_loan_tape_records,
    // Financial transaction functions
    create_account_financial_transaction,
    // Batch functions
    create_batch,
    create_deal,
    create_deal_payment,
    // CCR Domain functions
    create_identity,
    create_identity_with_password,
    // Investor profile functions
    create_investor_profile,
    create_loan_tape_record,
    create_state_snapshot,
    // USDC Deposit functions
    create_usdc_deposit,
    // Withdrawal functions
    create_withdrawal,
    create_withdrawal_request,
    delete_deal,
    delete_identity,
    generate_invite_code,
    get_all_batches,
    get_balance,
    get_batch_by_id,
    get_contract_submission_data,
    // State functions
    get_current_counter_value,
    get_current_state,
    get_deal_by_id,
    get_deals_by_company_id,
    get_deals_by_user_id,
    get_deposit_by_tx_hash,
    get_deposits_by_user_address,
    get_draft_deals_by_company_id,
    get_identity_by_email,
    get_identity_by_id,
    get_identity_by_reset_token,
    get_investor_profile_by_user_id,
    get_investor_profile_with_identity,
    get_invite_code_info,
    get_kyc_statistics,
    get_loan_tape_record_by_id,
    get_loan_tape_records,
    get_payments_by_deal,
    get_pending_transactions,
    get_pending_withdrawal_requests,
    get_potential_stuck_batches,
    get_proven_unposted_batches,
    get_user_account_financial_transactions,
    get_user_available_balance,
    get_user_balance_by_address,
    get_user_balance_by_id,
    get_user_deposit_balance,
    get_user_financial_summary,
    get_user_financial_transactions,
    get_user_id_by_wallet_address,
    get_user_withdrawal_balance,
    get_validated_pending_withdrawals,
    get_withdrawal_by_tx_hash,
    get_withdrawal_request_by_id,
    get_withdrawal_requests_by_user_address,
    get_withdrawal_requests_by_user_id,
    get_withdrawals_by_user_address,
    // Admin withdrawal functions
    get_withdrawals_pending_admin_approval,
    // Database connection
    init_db,
    init_db_with_url,
    list_deals,
    list_identities,
    list_pending_kyc_applications,
    mark_batch_posted_to_contract,
    mark_batch_posted_to_contract_with_tx_hash,
    mark_withdrawal_submitted,
    register_or_reregister_user,
    reject_withdrawal_request,
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
    update_deal,
    update_deposit_status,
    update_deposit_status_tx,
    update_identity_profile,
    update_investor_profile,
    update_kyc_status,
    update_last_login,
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
    verify_email_with_token,
    // Financial transaction types
    AccountFinancialTransaction,
    // Types
    AdsStateCommit,
    Balance,
    ContractPrivateData,
    ContractPublicData,
    ContractSubmissionData,
    CounterState,
    CreateAccountFinancialTransactionRequest,
    // Investor profile types
    CreateInvestorProfileRequest,
    Deal,
    DealNegotiationStatus,
    DealPayment,
    DealPaymentStatus,
    DealTransactionDb,
    DealsWithCount,
    IncomingTransaction,
    InvestorProfile,
    InviteCodeInfo,
    KycInvestorType,
    KycStatistics,
    LoanTapeRecord,
    ProofBatch,
    RegisterResult,
    StateSnapshot,
    UpdateInvestorProfileRequest,
    UsdcDeposit,
    UsdcWithdrawal,
    // CCR Domain types
    User,
    UserDepositBalance,
    UserFinancialSummary,
    UserFinancialTransaction,
    UserWithdrawalBalance,
    WithdrawalRequest,
    WithdrawalRequestHistory,
};

// Re-export essential error types
pub use error::{DbError, DbResult};

// Re-export ADS and related types
pub use ads::ads_service::{
    AdsConfig, AdsError, AdsMetrics, AdsServiceFactory, AuditEvent, AuditEventType, AuditTrail,
    AuthenticatedDataStructure, BatchInclusionInfo, ComplianceStatus, IndexedMerkleTreeADS,
    LoanInclusionProof, LoanInclusionRequest, LoanLifecycleSummary, LoanMetadata, LoanOperation,
    LoanOperationProof, MembershipProof, NonMembershipProof, StateCommitment, StateTransition,
    WitnessData,
};

pub use ads::merkle_tree::{
    AlgorithmInsertionResult, IndexedMerkleTree, InsertionMetrics, InsertionProof, InsertionResult,
    LowNullifier, MerkleNode, MerkleNodeDb, MerkleProof, MerkleTreeDb, Nullifier, NullifierDb,
    TreeState, TreeStateDb, TreeStats,
};
pub use ads::merkle_tree_32::{BatchUpdate, MerkleProof32, MerkleTree32, Tree32Stats, TreeMetrics};
pub use ads::vapp_integration::{
    ComplianceError, ComplianceResult, Environment, ProofError, ProofType, SettlementError,
    SettlementResult, VAppAdsIntegration, VAppBatchResponse, VAppConfig, VAppError,
    VAppInsertionResponse, VAppProofResponse, ZkProof,
};
pub use processing::background_processor::{
    BackgroundProcessor, ProcessorBuilder, ProcessorConfig,
};

#[cfg(any(test, feature = "test-utils", debug_assertions))]
pub mod test_utils;

// Test modules moved to db/tests/ directory
// See: tests/tests.rs, tests/error_tests.rs, tests/ads_service_tests.rs, etc.
