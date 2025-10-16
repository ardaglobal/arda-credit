//! Business logic services module
//!
//! This module contains all business logic services including:
//! - AI services (analysis, notifications, LLM integration)
//! - Authentication and authorization
//! - Balance and withdrawal services
//! - Batch processing
//! - Company and document services
//! - Email services
//! - KYC services
//! - Magic link authentication
//! - Session management
//! - Trading authentication

pub mod ai_analysis_service;
pub mod ai_notification_service;
pub mod auth;
pub mod balance_service;
pub mod batch_processor;
pub mod company_service;
pub mod deployment_config;
pub mod document_ingestion;
pub mod document_ingestion_orchestrator;
pub mod document_service;
pub mod document_storage;
pub mod email_service;
pub mod kyc_service;
pub mod llm_integration;
pub mod magic_link_service;
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
pub use company_service::{
    BeneficialOwnerInfo, CompanyRegistrationRequest, CompanyResponseDto, CompanyService,
    CompanyServiceError, KybSubmissionRequest, TeamInvitationRequest, TeamMemberPermissions,
    TeamMemberResponseDto,
};
pub use document_service::DocumentService;
pub use document_storage::{
    DocumentStorageConfig, DocumentStorageService, StorageBackend, UploadedFile,
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

pub use ai_analysis_service::{
    AiAnalysisService, AiChatMessage, AiChatRequest, AiChatResponse, ChangeAnalysis,
    ChangeCategory, ChangeType, DocumentDiffRequest, DocumentDiffResponse, ImpactLevel,
    NegotiationInsight, RiskFactor,
};

pub use ai_notification_service::{
    AiNotificationService, DeliveryMethod, NotificationPriority, NotificationRecord,
    UserNotificationPreferences, WebhookConfig,
};

pub use llm_integration::{
    ChatPrompt, DocumentAnalysisPrompt, LlmProvider, LlmRequest, LlmResponse, LlmService,
};

// Re-export LLM extraction types from document_ingestion module
pub use document_ingestion::{
    ChunkMetadata, CollateralType, CurrencyCode, DealCollateral, DealCovenants, DealFinancialTerms,
    DealParties, DealType, DocumentMetadata, EmbeddingService, ExtractedDealData, InterestRateType,
    LlmExtractionService, PaymentFrequency, QdrantService, SearchResult,
};

// Re-export document ingestion orchestrator
pub use document_ingestion_orchestrator::{
    DocumentIngestionOrchestrator, DocumentIngestionRequest, DocumentIngestionResult,
};
