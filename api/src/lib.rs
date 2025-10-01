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

// Organized module structure
pub mod api;
pub mod client;
pub mod config;
pub mod handlers;
pub mod middleware;
pub mod services;
pub mod utils;

// Re-export commonly used items from organized modules
// Note: Be careful with glob imports to avoid name conflicts
pub use middleware::*;

// Re-export config items (types::ErrorResponse is not re-exported to avoid conflicts with handlers)
pub use config::{
    email_config::*, email_templates::*, magic_link_types::*, paths::*, telemetry::*,
};

// Re-export specific types from config::types (excluding ErrorResponse to avoid ambiguity)
pub use config::types::{ApiConfig, ApiState, UserType};

// Selectively re-export API items (ApiConfig and ApiState already exported above)
pub use api::{create_router, ApiServer, ApiServerBuilder, ApiServerConfig};
pub use services::{
    AuthError, AuthService, AvailableBalance, BalanceVerificationResult,
    BalanceVerificationService, BatchCreationResult, BatchProcessorHandle, Claims, CompanyService,
    DocumentService, EmailService, KycValidationResult, KycValidationService, MagicLinkService,
    SessionService, UnifiedBatchService, WithdrawalTransactionService,
};

// Re-export withdrawal types
pub use services::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalServiceError,
    WithdrawalStatusResponse,
};

// Re-export company types
pub use services::{
    BeneficialOwnerInfo, CompanyRegistrationRequest, CompanyResponseDto, CompanyServiceError,
    KybSubmissionRequest, TeamInvitationRequest, TeamMemberPermissions, TeamMemberResponseDto,
};

// Re-export AI types
pub use services::{
    AiAnalysisService, AiChatMessage, AiChatRequest, AiChatResponse, AiNotificationService,
    ChangeAnalysis, ChangeCategory, ChangeType, ChatPrompt, DeliveryMethod, DocumentAnalysisPrompt,
    DocumentDiffRequest, DocumentDiffResponse, ImpactLevel, LlmProvider, LlmRequest, LlmResponse,
    LlmService, NegotiationInsight, NotificationPriority, NotificationRecord, RiskFactor,
    UserNotificationPreferences, WebhookConfig,
};

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
    TransactionInfo,
    TransactionStatusResponse,
    UserBalanceResponse,
    UserTransactionsResponse,
};

// Re-export handler modules for backward compatibility
pub use handlers::*;

// Note: New handler modules are available but not yet replacing rest.rs functionality

// Re-export database types that the API uses
pub use arda_credit_db::{
    // Credit line operations
    create_batch,
    create_deal,
    get_all_batches,
    get_batch_by_id,
    get_contract_submission_data,
    get_current_state,
    get_deal_by_id,
    // Database functions
    init_db,
    list_deals,
    store_ads_state_commit,
    submit_transaction,
    update_batch_proof,
    update_deal,
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
    ProofBatch,
};
