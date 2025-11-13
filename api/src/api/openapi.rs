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

    // Finance types
    AccountFinancialTransactionResponse,
    // Admin & User types
    AdminUserIdentity,
    AdminUserListResponse,
    AdminWithdrawalDashboard,
    ApproveWithdrawalRequest,
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
    FinancialSummaryResponse,
    FinancialTransactionQuery,
    // Auth types
    GenerateInviteCodeRequest,
    GenerateInviteCodeResponse,
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
    RegisterUserRequest,
    RegisterUserResponse,
    RejectWithdrawalRequest,
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
    TriggerBatchResponse,
    UnifiedFinancialTransactionResponse,
    UpdateBatchProofRequest,

    UpdateDealRequest,
    UpdateDealResponse,
    UpdateKycStatusRequest,
    UpdateUserRequest,
    UserDetailsResponse,
    UserTransactionsResponse,
    ValidateDealTransferRequest,
    ValidateInviteCodeResponse,
};

pub use crate::config::magic_link_types::{
    MagicLinkErrorResponse, MagicLinkRequest, MagicLinkResponse, UserInfo, VerifyMagicLinkRequest,
    VerifyMagicLinkResponse,
};
pub use crate::config::types::UserType;
pub use crate::handlers::companies::{
    InvestorProfileQuery, InvestorProfileResponse, RegisterInvestorRequest,
    RegisterInvestorResponse, UpdateInvestorProfileRequest,
};
pub use crate::handlers::users::balance::{
    AddressBalance, AddressBalanceResponse, UserBalanceResponse,
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

        // Loan Tape endpoints
        crate::handlers::loan_tape_handlers::create_loan_tape_handler,
        crate::handlers::loan_tape_handlers::create_bulk_loan_tape_handler,
        crate::handlers::loan_tape_handlers::list_loan_tape_handler,
        crate::handlers::loan_tape_handlers::get_loan_tape_handler,

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

        // Faucet endpoints (local/staging only)
        crate::handlers::faucet::get_faucet_info,
        crate::handlers::faucet::check_faucet_status,
        crate::handlers::faucet::request_faucet_funds,

        // Admin withdrawal endpoints
        crate::handlers::finance::withdrawals::approve_withdrawal_request,
        crate::handlers::finance::withdrawals::reject_withdrawal_request,
        crate::handlers::finance::withdrawals::get_admin_withdrawal_dashboard,
        crate::handlers::finance::withdrawals::manual_execute_withdrawal,

        // Financial Transaction endpoints
        crate::handlers::finance::transactions::record_account_deposit,
        crate::handlers::finance::transactions::record_account_withdrawal,
        crate::handlers::finance::transactions::get_account_financial_transactions,
        crate::handlers::finance::transactions::get_user_financial_transactions,
        crate::handlers::finance::transactions::get_user_financial_summary,

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

        // Company Membership endpoints (with profile_type support)
        crate::handlers::companies::memberships::invite_member,
        crate::handlers::companies::memberships::list_members,
        crate::handlers::companies::memberships::get_membership,
        crate::handlers::companies::memberships::get_user_memberships,
        crate::handlers::companies::memberships::update_membership,
        crate::handlers::companies::memberships::remove_member,

        // Company Wallet endpoints (permission-based access control)
        crate::handlers::companies::wallets::create_wallet_address,
        crate::handlers::companies::wallets::list_wallet_addresses,
        crate::handlers::companies::wallets::get_user_wallet_addresses,
        crate::handlers::companies::wallets::get_wallet_address,
        crate::handlers::companies::wallets::update_wallet_address,
        crate::handlers::companies::wallets::grant_wallet_permission,
        crate::handlers::companies::wallets::list_wallet_permissions,
        crate::handlers::companies::wallets::revoke_wallet_permission,

        // Contract configuration endpoints
        crate::handlers::core::contract::get_contract_config,
        crate::handlers::core::contract::get_transaction_updates,

        // Taxonomy endpoints - Credit Agreements
        crate::handlers::taxonomy::credit_agreements::list_credit_agreements_handler,
        crate::handlers::taxonomy::credit_agreements::get_credit_agreement_handler,
        crate::handlers::taxonomy::credit_agreements::create_credit_agreement_handler,
        crate::handlers::taxonomy::credit_agreements::create_credit_agreements_batch_handler,
        crate::handlers::taxonomy::credit_agreements::update_credit_agreement_handler,
        crate::handlers::taxonomy::credit_agreements::delete_credit_agreement_handler,

        // Taxonomy endpoints - Fee Letters
        crate::handlers::taxonomy::fee_letters::list_fee_letters_handler,
        crate::handlers::taxonomy::fee_letters::get_fee_letter_handler,
        crate::handlers::taxonomy::fee_letters::create_fee_letter_handler,
        crate::handlers::taxonomy::fee_letters::create_fee_letters_batch_handler,
        crate::handlers::taxonomy::fee_letters::update_fee_letter_handler,
        crate::handlers::taxonomy::fee_letters::delete_fee_letter_handler,

        // Taxonomy endpoints - Legal Opinions
        crate::handlers::taxonomy::legal_opinions::list_legal_opinions_handler,
        crate::handlers::taxonomy::legal_opinions::get_legal_opinion_handler,
        crate::handlers::taxonomy::legal_opinions::create_legal_opinion_handler,
        crate::handlers::taxonomy::legal_opinions::create_legal_opinions_batch_handler,
        crate::handlers::taxonomy::legal_opinions::update_legal_opinion_handler,
        crate::handlers::taxonomy::legal_opinions::delete_legal_opinion_handler,

        // Taxonomy endpoints - Security Agreements
        crate::handlers::taxonomy::security_agreements::list_security_agreements_handler,
        crate::handlers::taxonomy::security_agreements::get_security_agreement_handler,
        crate::handlers::taxonomy::security_agreements::create_security_agreement_handler,
        crate::handlers::taxonomy::security_agreements::create_security_agreements_batch_handler,
        crate::handlers::taxonomy::security_agreements::update_security_agreement_handler,
        crate::handlers::taxonomy::security_agreements::delete_security_agreement_handler,

        // Taxonomy endpoints - UCC Financing Statements
        crate::handlers::taxonomy::ucc_financing_statements::list_ucc_financing_statements_handler,
        crate::handlers::taxonomy::ucc_financing_statements::get_ucc_financing_statement_handler,
        crate::handlers::taxonomy::ucc_financing_statements::create_ucc_financing_statement_handler,
        crate::handlers::taxonomy::ucc_financing_statements::create_ucc_financing_statements_batch_handler,
        crate::handlers::taxonomy::ucc_financing_statements::update_ucc_financing_statement_handler,
        crate::handlers::taxonomy::ucc_financing_statements::delete_ucc_financing_statement_handler,

        // Taxonomy endpoints - Account Control Agreements
        crate::handlers::taxonomy::account_control_agreements::list_account_control_agreements_handler,
        crate::handlers::taxonomy::account_control_agreements::get_account_control_agreement_handler,
        crate::handlers::taxonomy::account_control_agreements::create_account_control_agreement_handler,
        crate::handlers::taxonomy::account_control_agreements::create_account_control_agreements_batch_handler,
        crate::handlers::taxonomy::account_control_agreements::update_account_control_agreement_handler,
        crate::handlers::taxonomy::account_control_agreements::delete_account_control_agreement_handler,

        // Taxonomy endpoints - Guaranties
        crate::handlers::taxonomy::guaranties::list_guaranties_handler,
        crate::handlers::taxonomy::guaranties::get_guaranty_handler,
        crate::handlers::taxonomy::guaranties::create_guaranty_handler,
        crate::handlers::taxonomy::guaranties::create_guaranties_batch_handler,
        crate::handlers::taxonomy::guaranties::update_guaranty_handler,
        crate::handlers::taxonomy::guaranties::delete_guaranty_handler,

        // Taxonomy endpoints - Assignment & Assumption Agreements
        crate::handlers::taxonomy::assignments::list_assignment_assumption_agreements_handler,
        crate::handlers::taxonomy::assignments::get_assignment_assumption_agreement_handler,
        crate::handlers::taxonomy::assignments::create_assignment_assumption_agreement_handler,
        crate::handlers::taxonomy::assignments::create_assignment_assumption_agreements_batch_handler,
        crate::handlers::taxonomy::assignments::update_assignment_assumption_agreement_handler,
        crate::handlers::taxonomy::assignments::delete_assignment_assumption_agreement_handler,

        // Taxonomy endpoints - Participation Agreements
        crate::handlers::taxonomy::participations::list_participation_agreements_handler,
        crate::handlers::taxonomy::participations::get_participation_agreement_handler,
        crate::handlers::taxonomy::participations::create_participation_agreement_handler,
        crate::handlers::taxonomy::participations::create_participation_agreements_batch_handler,
        crate::handlers::taxonomy::participations::update_participation_agreement_handler,
        crate::handlers::taxonomy::participations::delete_participation_agreement_handler,

        // Taxonomy endpoints - SSI Records
        crate::handlers::taxonomy::ssi_records::list_ssi_records_handler,
        crate::handlers::taxonomy::ssi_records::get_ssi_record_handler,
        crate::handlers::taxonomy::ssi_records::create_ssi_record_handler,
        crate::handlers::taxonomy::ssi_records::create_ssi_records_batch_handler,
        crate::handlers::taxonomy::ssi_records::update_ssi_record_handler,
        crate::handlers::taxonomy::ssi_records::delete_ssi_record_handler,

        // Taxonomy endpoints - Secondary Trades
        crate::handlers::taxonomy::secondary_trades::list_secondary_trades_handler,
        crate::handlers::taxonomy::secondary_trades::get_secondary_trade_handler,
        crate::handlers::taxonomy::secondary_trades::create_secondary_trade_handler,
        crate::handlers::taxonomy::secondary_trades::create_secondary_trades_batch_handler,
        crate::handlers::taxonomy::secondary_trades::update_secondary_trade_handler,
        crate::handlers::taxonomy::secondary_trades::delete_secondary_trade_handler,
    ),
    tags(
        // Primary API Groups (9 main groups + Dev)
        (name = "Auth", description = "Authentication, authorization, and invite code management"),
        (name = "Users", description = "User account management, KYC verification, balances, and transaction history"),
        (name = "Companies", description = "Originator and investor company profiles, documents, turnover tracking, and AI analysis"),
        (name = "Company Memberships", description = "Company team member management with originator/investor profile type support. Users can have multiple profiles (originator and investor) in the same company."),
        (name = "Company Wallets", description = "Company wallet address management with permission-based access control. Wallet addresses belong to companies with granular user permissions. Only admin users can authorize transactions."),
        (name = "Deals", description = "Deal lifecycle management, marketplace operations, ownership transfers, and payment processing"),
        (name = "State", description = "Blockchain state management, batch processing, ZK proof generation, transaction history, and contract integration"),
        (name = "Admin", description = "Administrative operations including KYC approval, withdrawal management, and platform oversight"),
        (name = "Payments", description = "Financial transactions, withdrawal processing, and payment tracking across accounts, pools, and credit lines"),
        (name = "Taxonomy - Credit Agreements", description = "Credit agreement documents with full document attributes, facility structure, pricing, and covenants"),
        (name = "Taxonomy - Fee Letters", description = "Fee letter documents with confidential fee structures and schedules"),
        (name = "Taxonomy - Legal Opinions", description = "Legal opinion documents covering enforceability, authority, and perfection"),
        (name = "Taxonomy - Security Agreements", description = "Security agreement documents with collateral and perfection details"),
        (name = "Taxonomy - UCC Financing Statements", description = "UCC financing statement filings for perfecting security interests"),
        (name = "Taxonomy - Account Control Agreements", description = "Deposit and securities account control agreements (DACA/ACA/SACA) for perfecting security interests"),
        (name = "Taxonomy - Guaranties", description = "Guaranty agreements where third parties guarantee borrower obligations"),
        (name = "Taxonomy - Assignment & Assumption Agreements", description = "Assignment and assumption agreements for transferring loan interests between lenders"),
        (name = "Taxonomy - Participation Agreements", description = "Participation agreements for economic interest transfers where grantor remains lender of record"),
        (name = "Taxonomy - SSI Records", description = "Standard Settlement Instructions for deal settlements, supporting both traditional banking and digital asset custody"),
        (name = "Taxonomy - Secondary Trades", description = "Secondary market trade documents with trade execution, pricing, settlement, and transfer details"),
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

        // Company Membership types (with profile_type support)
        crate::dto::company_dto::InviteMemberRequest,
        crate::dto::company_dto::MembershipResponse,
        crate::dto::company_dto::MembershipWithUserResponse,
        crate::dto::company_dto::UpdateMembershipRequest,
        crate::dto::company_dto::InvitationResponse,
        crate::handlers::companies::memberships::ListMembersQuery,

        // Company Wallet types (permission-based access control)
        crate::dto::wallet_dto::CreateWalletAddressRequest,
        crate::dto::wallet_dto::UpdateWalletAddressRequest,
        crate::dto::wallet_dto::WalletAddressResponse,
        crate::dto::wallet_dto::UserWalletAddressResponse,
        crate::dto::wallet_dto::GrantWalletPermissionRequest,
        crate::dto::wallet_dto::WalletPermissionResponse,
        crate::handlers::companies::wallets::ListWalletsQuery,

        // Admin types
        AdminUserIdentity,
        AdminUserListResponse,
        BulkKycApprovalRequest,
        BulkKycApprovalResponse,
        BulkOperationResponse,
        KycStatisticsResponse,
        ListPendingKycQuery,
        ListPendingKycResponse,

        // Financial transaction types
        FinancialTransactionQuery,
        CreateAccountFinancialTransactionRequest,
        AccountFinancialTransactionResponse,
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

        // Loan Tape types
        crate::dto::loan_tape_dto::CreateLoanTapeRequest,
        crate::dto::loan_tape_dto::BulkCreateLoanTapeRequest,
        crate::dto::loan_tape_dto::LoanTapeRecord,
        crate::dto::loan_tape_dto::LoanTapeListResponse,
        crate::handlers::loan_tape_handlers::ListLoanTapeQuery,
        crate::handlers::loan_tape_handlers::LoanTapeResponse,
        crate::handlers::loan_tape_handlers::BulkLoanTapeResponse,
        crate::handlers::loan_tape_handlers::ErrorResponse,

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

        // Magic Link Authentication schemas (NEW)
        MagicLinkRequest,
        MagicLinkResponse,
        VerifyMagicLinkRequest,
        VerifyMagicLinkResponse,
        UserInfo,
        MagicLinkErrorResponse,

        // Taxonomy schemas - Credit Agreements
        crate::handlers::taxonomy::credit_agreements::CreditAgreementResponse,
        crate::handlers::taxonomy::credit_agreements::CreateCreditAgreementApiRequest,
        crate::handlers::taxonomy::credit_agreements::UpdateCreditAgreementApiRequest,
        crate::handlers::taxonomy::credit_agreements::CreditAgreementListParams,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::credit_agreements::CreditAgreementResponse>,
        crate::handlers::taxonomy::common::DeleteResponse,
        crate::handlers::taxonomy::common::BatchOperationResponse,
        crate::handlers::taxonomy::common::BatchError,

        // Taxonomy schemas - Fee Letters
        crate::handlers::taxonomy::fee_letters::FeeLetterResponse,
        crate::handlers::taxonomy::fee_letters::CreateFeeLetterApiRequest,
        crate::handlers::taxonomy::fee_letters::UpdateFeeLetterApiRequest,
        crate::handlers::taxonomy::fee_letters::FeeLetterListParams,
        crate::handlers::taxonomy::fee_letters::BatchCreateFeeLettersRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::fee_letters::FeeLetterResponse>,

        // Taxonomy schemas - Legal Opinions
        crate::handlers::taxonomy::legal_opinions::LegalOpinionResponse,
        crate::handlers::taxonomy::legal_opinions::CreateLegalOpinionApiRequest,
        crate::handlers::taxonomy::legal_opinions::UpdateLegalOpinionApiRequest,
        crate::handlers::taxonomy::legal_opinions::LegalOpinionListParams,
        crate::handlers::taxonomy::legal_opinions::BatchCreateLegalOpinionsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::legal_opinions::LegalOpinionResponse>,

        // Taxonomy schemas - Security Agreements
        crate::handlers::taxonomy::security_agreements::SecurityAgreementResponse,
        crate::handlers::taxonomy::security_agreements::CreateSecurityAgreementApiRequest,
        crate::handlers::taxonomy::security_agreements::UpdateSecurityAgreementApiRequest,
        crate::handlers::taxonomy::security_agreements::SecurityAgreementListParams,
        crate::handlers::taxonomy::security_agreements::BatchCreateSecurityAgreementsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::security_agreements::SecurityAgreementResponse>,

        // Taxonomy schemas - UCC Financing Statements
        crate::handlers::taxonomy::ucc_financing_statements::UccFinancingStatementResponse,
        crate::handlers::taxonomy::ucc_financing_statements::CreateUccFinancingStatementApiRequest,
        crate::handlers::taxonomy::ucc_financing_statements::UpdateUccFinancingStatementApiRequest,
        crate::handlers::taxonomy::ucc_financing_statements::UccFinancingStatementListParams,
        crate::handlers::taxonomy::ucc_financing_statements::BatchCreateUccFinancingStatementsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::ucc_financing_statements::UccFinancingStatementResponse>,

        // Taxonomy schemas - Account Control Agreements
        crate::handlers::taxonomy::account_control_agreements::AccountControlAgreementResponse,
        crate::handlers::taxonomy::account_control_agreements::CreateAccountControlAgreementApiRequest,
        crate::handlers::taxonomy::account_control_agreements::UpdateAccountControlAgreementApiRequest,
        crate::handlers::taxonomy::account_control_agreements::AccountControlAgreementListParams,
        crate::handlers::taxonomy::account_control_agreements::BatchCreateAccountControlAgreementsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::account_control_agreements::AccountControlAgreementResponse>,

        // Taxonomy schemas - Guaranties
        crate::handlers::taxonomy::guaranties::GuarantyResponse,
        crate::handlers::taxonomy::guaranties::CreateGuarantyApiRequest,
        crate::handlers::taxonomy::guaranties::UpdateGuarantyApiRequest,
        crate::handlers::taxonomy::guaranties::GuarantyListParams,
        crate::handlers::taxonomy::guaranties::BatchCreateGuarantiesRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::guaranties::GuarantyResponse>,

        // Taxonomy schemas - Assignment & Assumption Agreements
        crate::handlers::taxonomy::assignments::AssignmentAssumptionAgreementResponse,
        crate::handlers::taxonomy::assignments::CreateAssignmentAssumptionAgreementApiRequest,
        crate::handlers::taxonomy::assignments::UpdateAssignmentAssumptionAgreementApiRequest,
        crate::handlers::taxonomy::assignments::AssignmentAssumptionAgreementListParams,
        crate::handlers::taxonomy::assignments::BatchCreateAssignmentAssumptionAgreementsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::assignments::AssignmentAssumptionAgreementResponse>,


        // Taxonomy schemas - Participation Agreements
        crate::handlers::taxonomy::participations::ParticipationAgreementResponse,
        crate::handlers::taxonomy::participations::CreateParticipationAgreementApiRequest,
        crate::handlers::taxonomy::participations::UpdateParticipationAgreementApiRequest,
        crate::handlers::taxonomy::participations::ParticipationAgreementListParams,
        crate::handlers::taxonomy::participations::BatchCreateParticipationAgreementsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::participations::ParticipationAgreementResponse>,
        // Taxonomy schemas - SSI Records
        crate::handlers::taxonomy::ssi_records::SsiRecordResponse,
        crate::handlers::taxonomy::ssi_records::CreateSsiRecordApiRequest,
        crate::handlers::taxonomy::ssi_records::UpdateSsiRecordApiRequest,
        crate::handlers::taxonomy::ssi_records::SsiRecordListParams,
        crate::handlers::taxonomy::ssi_records::BatchCreateSsiRecordsRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::ssi_records::SsiRecordResponse>,

        // Taxonomy schemas - Secondary Trades
        crate::handlers::taxonomy::secondary_trades::SecondaryTradeResponse,
        crate::handlers::taxonomy::secondary_trades::CreateSecondaryTradeApiRequest,
        crate::handlers::taxonomy::secondary_trades::UpdateSecondaryTradeApiRequest,
        crate::handlers::taxonomy::secondary_trades::SecondaryTradeListParams,
        crate::handlers::taxonomy::secondary_trades::BatchCreateSecondaryTradesRequest,
        crate::handlers::taxonomy::common::PaginatedResponse<crate::handlers::taxonomy::secondary_trades::SecondaryTradeResponse>,

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
