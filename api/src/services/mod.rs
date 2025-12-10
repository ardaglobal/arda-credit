//! Business logic services module
//!
//! This module contains all business logic services including:
//! - Authentication and authorization
//! - Balance and withdrawal services
//! - Batch processing
//! - Company services
//! - Email services
//! - KYC services
//! - Magic link authentication
//! - Session management
//! - Trading authentication

pub mod auth;
pub mod balance_service;
pub mod batch_processor;
pub mod collateral_intel_client;
pub mod company_service;
pub mod deployment_config;
pub mod email_service;
pub mod kyc_service;
pub mod magic_link_service;
pub mod metrics_cache;
pub mod metrics_service;
pub mod session_service;
pub mod simple_withdrawal_executor;
pub mod team_invitation_service;
pub mod trading_auth;
pub mod unified_batch_service;
pub mod withdrawal_service;

// Re-export commonly used types (being specific to avoid conflicts)
pub use auth::{AuthError, AuthService, Claims};
pub use balance_service::{BalanceVerificationResult, BalanceVerificationService};
pub use batch_processor::BatchProcessorHandle;
pub use collateral_intel_client::{
    ClientError as CollateralIntelClientError, CollateralIntelClient, DetectionHistoryResponse,
    DetectionMatchDetail, DetectionRecord,
};
pub use company_service::{
    BeneficialOwnerInfo, CompanyRegistrationRequest, CompanyResponseDto, CompanyService,
    CompanyServiceError, KybSubmissionRequest, TeamInvitationRequest, TeamMemberPermissions,
    TeamMemberResponseDto,
};
pub use email_service::EmailService;
pub use kyc_service::{KycValidationResult, KycValidationService};
pub use magic_link_service::MagicLinkService;
pub use session_service::SessionService;
pub use team_invitation_service::{TeamInvitationError, TeamInvitationService};
pub use unified_batch_service::{BatchCreationResult, UnifiedBatchService};
pub use withdrawal_service::{
    InitiateWithdrawalRequest, InitiateWithdrawalResponse, WithdrawalServiceError,
    WithdrawalStatusResponse, WithdrawalTransactionService,
};

pub use deployment_config::{
    get_contract_address, get_deployment, is_deployment_available, refresh_deployment,
    ContractType, DeploymentConfig, DeploymentError, DeploymentInfo, DeploymentService,
    NetworkConfig,
};

pub use metrics_cache::{
    CacheStats, MarketValueCacheKey, MetricsCache, MetricsCacheConfig, PieChartCacheKey,
};

pub use metrics_service::{
    MarketValueDataPoint, MarketValueQuery, MetricsError, MetricsService, PieChartDataPoint,
    PieChartQuery, RefreshResult,
};
