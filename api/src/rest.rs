use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use humantime::format_duration;
use serde::{Deserialize, Serialize};
use sqlx::{Error as SqlxError, PgPool};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid;

use crate::batch_processor::BatchProcessorHandle;
use crate::paths;
use arda_credit_db::{
    get_all_batches, get_batch_by_id, get_contract_submission_data, get_current_state,
    get_pending_transactions, store_ads_state_commit, submit_transaction, update_batch_proof,
    AuditEvent, IndexedMerkleTreeADS, LoanInclusionProof, LoanLifecycleSummary,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// API STATE
// ============================================================================

/// API state containing the database pool, configuration, and ADS service
#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub config: ApiConfig,
    pub batch_processor: Option<BatchProcessorHandle>,
    pub ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
}

/// Configuration for API server
#[derive(Debug, Clone, ToSchema)]
pub struct ApiConfig {
    pub server_name: String,
    pub version: String,
    pub max_batch_size: u32,
    pub enable_debug_endpoints: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            server_name: "Batch Processing API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 50,
            enable_debug_endpoints: false,
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE MODELS
// ============================================================================

/// Request to submit a single transaction
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitTransactionRequest {
    pub amount: i32,
}

/// Response from transaction submission
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitTransactionResponse {
    pub transaction_id: i32,
    pub amount: i32,
    pub status: String, // "pending"
    pub created_at: DateTime<Utc>,
}

/// Request to create a batch
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBatchRequest {
    pub batch_size: Option<i32>,
}

/// Response from batch creation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBatchResponse {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub created_at: DateTime<Utc>,
}

/// Response for pending transactions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingTransactionsResponse {
    pub transactions: Vec<TransactionInfo>,
    pub total_count: usize,
    pub total_amount: i32,
}

/// Transaction info for API responses
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    pub id: i32,
    pub amount: i32,
    pub created_at: DateTime<Utc>,
}

/// Response for batch listing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchListResponse {
    pub batches: Vec<BatchInfo>,
    pub total_count: usize,
}

/// Batch info for API responses
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchInfo {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub sindri_proof_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub proven_at: Option<DateTime<Utc>>,
}

/// Response for current state
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CurrentStateResponse {
    pub counter_value: i64,
    pub has_merkle_root: bool,
    pub last_batch_id: Option<i32>,
    pub last_proven_batch_id: Option<i32>,
}

/// Request to update batch with proof
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateBatchProofRequest {
    pub sindri_proof_id: String,
    pub status: String,              // "proven", "failed"
    pub merkle_root: Option<String>, // hex encoded
}

/// Query parameters for batch listing
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchListQuery {
    pub limit: Option<i32>,
}

/// Contract submission data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractSubmissionData {
    pub public: ContractPublicData,
    pub private: ContractPrivateData,
}

/// Contract public data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractPublicData {
    pub prev_merkle_root: String,
    pub new_merkle_root: String,
    pub zk_proof: String,
}

/// Contract private data
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ContractPrivateData {
    pub prev_counter_value: i64,
    pub new_counter_value: i64,
    pub transactions: Vec<i32>,
}

/// API information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiInfoResponse {
    pub server_name: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub description: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub database_connected: bool,
    pub api_version: String,
    pub git_tag: String,
    pub git_sha: String,
    pub git_sha_short: String,
    pub git_dirty: bool,
}

/// Batch processor stats response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchProcessorStatsResponse {
    pub total_batches_created: u64,
    pub total_transactions_processed: u64,
    pub timer_triggers: u64,
    pub count_triggers: u64,
    pub manual_triggers: u64,
    pub errors: u64,
    pub last_batch_time: Option<String>,
}

/// Manual batch trigger response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TriggerBatchResponse {
    pub triggered: bool,
    pub message: String,
}

// ============================================================================
// LOAN PROCESSING MODELS
// ============================================================================

/// Request to register a new user identity
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUserRequest {
    pub user_type: String, // "Investor" | "Lender"
    pub email: String,
    pub jurisdiction: Option<String>, // Optional, defaults to "US"
}

/// Response from user registration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterUserResponse {
    pub user_id: String, // UUID as string
    pub user_type: String,
    pub kyc_status: String,
    pub created_at: DateTime<Utc>,
}

/// User details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserDetailsResponse {
    pub user_id: String,
    pub user_type: String,
    pub kyc_status: String,
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// KYC submission request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitKycRequest {
    pub kyc_data: KycData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct KycData {
    pub document_type: String,
    pub document_hash: String,
    pub jurisdiction: Option<String>,
}

/// KYC submission response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SubmitKycResponse {
    pub user_id: String,
    pub kyc_status: String,
    pub submitted_at: DateTime<Utc>,
}

/// Update KYC status request (admin only)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateKycStatusRequest {
    pub status: String, // "Pending" | "Approved" | "Rejected"
    pub notes: Option<String>,
}

/// Update user profile request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub user_type: Option<String>, // "Investor" | "Lender"
    pub jurisdiction: Option<String>,
}

/// Update user response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserResponse {
    pub user_id: String,
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

/// Delete user response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteUserResponse {
    pub user_id: String,
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
}

/// List identities query parameters
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListIdentitiesQuery {
    pub user_type: Option<String>,
    pub kyc_status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List identities response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListIdentitiesResponse {
    pub identities: Vec<UserDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Create new loan request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanRequest {
    pub borrower_id: String,
    pub lender_id: String,
    pub principal_amount: String,            // Amount in cents as string
    pub interest_rate_bps: u16,              // Interest rate in basis points
    pub term_days: Option<u32>,              // Optional loan term
    pub metadata: Option<serde_json::Value>, // Optional additional data
}

/// Loan details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanDetailsResponse {
    pub loan_id: String,
    pub borrower_id: String,
    pub lender_id: String,
    pub principal_amount: String,
    pub outstanding_balance: String,
    pub interest_rate_bps: u16,
    pub loan_status: String,
    pub originated_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
}

/// List loans query parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListLoansQuery {
    pub status: Option<String>,
    pub borrower_id: Option<String>,
    pub lender_id: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// List loans response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListLoansResponse {
    pub loans: Vec<LoanDetailsResponse>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Loan statistics response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanStatsResponse {
    pub total_loans: u64,
    pub total_principal: String,   // Total principal amount as string
    pub total_outstanding: String, // Total outstanding balance as string
    pub average_interest_rate_bps: u32,
    pub status_breakdown: std::collections::HashMap<String, u64>,
}

/// Record loan payment request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecordPaymentRequest {
    pub loan_id: String,
    pub amount: String,                    // Amount in cents as string
    pub payment_type: String,              // "Principal" | "Interest" | "Mixed"
    pub payment_reference: Option<String>, // Optional external reference
    pub notes: Option<String>,
}

/// Payment response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub transaction_id: String,
    pub loan_id: String,
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String, // "Pending" | "Processed" | "Failed"
    pub created_at: DateTime<Utc>,
}

/// Payment details response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentDetailsResponse {
    pub transaction_id: String,
    pub loan_id: String,
    pub amount: String,
    pub payment_type: String,
    pub transaction_status: String,
    pub batch_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Loan payment history response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanPaymentHistoryResponse {
    pub loan_id: String,
    pub payments: Vec<PaymentDetailsResponse>,
    pub total_payments: usize,
    pub total_amount_paid: String,
}

/// Current state summary response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateSummaryResponse {
    pub current_state_root: String,
    pub transaction_count: u64,
    pub last_batch_id: Option<String>,
    pub last_updated: DateTime<Utc>,
    pub loan_summary: LoanStatsSummary,
    pub user_summary: UserStatsSummary,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanStatsSummary {
    pub total_loans: u64,
    pub total_principal: String,
    pub total_outstanding: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserStatsSummary {
    pub total_users: u64,
    pub kyc_approved_users: u64,
}

/// Transaction inclusion proof response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionProofResponse {
    pub transaction_id: String,
    pub transaction_type: String,
    pub proof: ProofData,
    pub verification_data: VerificationData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProofData {
    pub merkle_proof: String,
    pub batch_proof: String,
    pub state_root: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationData {
    pub batch_id: String,
    pub batch_timestamp: u64,
    pub transaction_index: u32,
}

/// Verify transaction inclusion request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifyTransactionRequest {
    pub transaction_id: String,
    pub proof_data: ProofData,
    pub expected_transaction_type: String,
}

/// Transaction verification response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionVerificationResponse {
    pub is_valid: bool,
    pub verification_result: VerificationResult,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationResult {
    pub merkle_proof_valid: bool,
    pub batch_proof_valid: bool,
    pub transaction_included: bool,
    pub transaction_type_matches: bool,
}

/// State transition history response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateHistoryResponse {
    pub state_transitions: Vec<StateTransition>,
    pub total_batches: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateTransition {
    pub batch_id: String,
    pub previous_state_root: String,
    pub new_state_root: String,
    pub transaction_count: u32,
    pub batch_timestamp: u64,
    pub proof_id: Option<String>,
    pub verification_status: String,
}

/// Batch creation request for loans
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanBatchRequest {
    pub max_transaction_count: Option<u32>,
    pub force_create: Option<bool>,
    pub include_transaction_types: Option<Vec<String>>,
}

/// Batch creation response for loans
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoanBatchResponse {
    pub batch_id: String,
    pub transaction_count: u32,
    pub batch_status: String,
    pub estimated_proof_time: String,
    pub created_at: DateTime<Utc>,
}

/// Pending transactions ready for batching
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingLoanTransactionsResponse {
    pub pending_transactions: Vec<PendingTransactionInfo>,
    pub total_pending: usize,
    pub total_amount: String,
    pub oldest_pending: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PendingTransactionInfo {
    pub transaction_id: String,
    pub transaction_type: String,
    pub user_id: String,
    pub amount: String,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// ROUTER SETUP
// ============================================================================

/// Create the main API router with all batch processing and loan endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints
        .route(paths::HEALTH, get(health_check))
        .route(paths::ROOT, get(api_info))
        .route(paths::API_HEALTH, get(health_check))
        .route(paths::API_INFO, get(api_info))
        // OpenAPI documentation routes - Swagger UI handles both the UI and JSON spec
        .merge(swagger_ui())
        // Transaction operations
        .route(paths::STATE_TRANSACTIONS, post(submit_transaction_endpoint))
        .route(
            paths::STATE_TRANSACTIONS_PENDING,
            get(get_pending_transactions_endpoint),
        )
        // Batch operations
        .route(paths::STATE_BATCHES, post(create_batch_endpoint))
        .route(paths::STATE_BATCHES, get(get_batches_endpoint))
        .route(paths::STATE_BATCH_BY_ID, get(get_batch_endpoint))
        .route(paths::STATE_BATCH_PROOF, post(update_batch_proof_endpoint))
        .route(paths::STATE_BATCHES_TRIGGER, post(trigger_batch_endpoint))
        .route(
            paths::STATE_BATCHES_STATS,
            get(get_batch_processor_stats_endpoint),
        )
        // State operations
        .route(paths::STATE_CURRENT, get(get_current_state_endpoint))
        .route(paths::STATE_CONTRACT, get(get_contract_data_endpoint))
        // ========================================================================
        // LOAN PROCESSING ENDPOINTS
        // ========================================================================
        // User Management
        .route(paths::USER, post(register_user_endpoint))
        .route(paths::USERS, get(list_identities_endpoint))
        .route(paths::USER_BY_ID, get(get_user_details_endpoint))
        .route(paths::USER_BY_ID, put(update_user_endpoint))
        .route(paths::USER_BY_ID, delete(delete_user_endpoint))
        .route(paths::USER_KYC, post(submit_kyc_endpoint))
        .route(paths::USER_KYC_STATUS, post(update_kyc_status_endpoint))
        // Loan Management
        .route(paths::LOANS, post(create_loan_endpoint))
        .route(paths::LOANS, get(list_loans_endpoint))
        .route(paths::LOAN_BY_ID, get(get_loan_details_endpoint))
        .route(paths::LOANS_STATS, get(get_loan_stats_endpoint))
        // Payment Processing
        .route(paths::STATE_PAYMENTS, post(record_payment_endpoint))
        .route(
            paths::STATE_PAYMENT_BY_ID,
            get(get_payment_details_endpoint),
        )
        .route(paths::LOAN_PAYMENTS, get(get_loan_payment_history_endpoint))
        // State Management & ZK Proofs
        .route(paths::STATE_SUMMARY, get(get_state_summary_endpoint))
        .route(paths::STATE_PROOF, get(get_transaction_proof_endpoint))
        .route(paths::STATE_VERIFY, post(verify_transaction_endpoint))
        .route(paths::STATE_HISTORY, get(get_state_history_endpoint))
        // Batch Operations for Loans
        .route(paths::STATE_BATCH_CREATE, post(create_loan_batch_endpoint))
        .route(paths::STATE_LOAN_BATCH_BY_ID, get(get_loan_batch_endpoint))
        .route(
            paths::STATE_BATCH_PENDING,
            get(get_pending_loan_transactions_endpoint),
        )
        // ========================================================================
        // LOAN OPERATION INCLUSION PROOF ENDPOINTS
        // ========================================================================
        .route(
            paths::STATE_PROOFS_LOAN_OPERATION,
            post(generate_loan_operation_proof_endpoint),
        )
        .route(
            paths::STATE_PROOFS_LOAN_PAYMENT,
            get(generate_loan_payment_proof_endpoint),
        )
        .route(paths::STATE_PROOFS_VERIFY, post(verify_loan_proof_endpoint))
        .route(
            paths::STATE_PROOFS_REPORT,
            post(generate_proof_report_endpoint),
        )
        .route(
            paths::STATE_PROOFS_BATCH_VERIFY,
            post(batch_verify_proofs_endpoint),
        )
        .route(paths::LOAN_AUDIT_TRAIL, get(get_loan_audit_trail_endpoint))
        .route(
            paths::LOAN_LIFECYCLE_SUMMARY,
            get(get_loan_lifecycle_summary_endpoint),
        )
        .with_state(state)
}

// ============================================================================
// ENDPOINT HANDLERS
// ============================================================================

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Health check passed", body = HealthResponse),
        (status = 500, description = "Health check failed")
    ),
    tag = "Health"
)]
#[instrument(skip(state), level = "info")]
async fn health_check(
    State(state): State<ApiState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    info!("🔍 API: Health check requested");

    // Test database connection
    let db_connected = match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => true,
        Err(e) => {
            error!("Database health check failed: {}", e);
            false
        }
    };

    let response = HealthResponse {
        status: if db_connected {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Utc::now(),
        database_connected: db_connected,
        api_version: state.config.version.clone(),
        git_tag: env!("GIT_TAG").to_string(),
        git_sha: env!("GIT_SHA").to_string(),
        git_sha_short: env!("GIT_SHA_SHORT").to_string(),
        git_dirty: env!("GIT_DIRTY").parse().unwrap_or(false),
    };

    if db_connected {
        info!("✅ API: Health check passed");
        Ok(Json(response))
    } else {
        error!("❌ API: Health check failed - database not connected");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available".to_string(),
        ))
    }
}

/// API information endpoint
#[instrument(skip(state), level = "info")]
/// Get API information
#[utoipa::path(
    get,
    path = "/api/v1/info",
    responses(
        (status = 200, description = "API information", body = ApiInfoResponse),
    ),
    tag = "Info"
)]
async fn api_info(State(state): State<ApiState>) -> Json<ApiInfoResponse> {
    info!("📋 API: API info requested");

    let endpoints = paths::get_all_endpoints();

    let response = ApiInfoResponse {
        server_name: state.config.server_name.clone(),
        version: state.config.version.clone(),
        timestamp: Utc::now(),
        endpoints,
    };

    info!("✅ API: API info returned");
    Json(response)
}

/// Submit a new transaction
#[utoipa::path(
    post,
    path = "/api/v1/state/transactions",
    request_body = SubmitTransactionRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully", body = SubmitTransactionResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transactions"
)]
#[instrument(skip(state), level = "info")]
async fn submit_transaction_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Result<Json<SubmitTransactionResponse>, (StatusCode, String)> {
    info!("💰 API: Submitting transaction: amount={}", request.amount);

    match submit_transaction(&state.pool, request.amount).await {
        Ok(transaction) => {
            let response = SubmitTransactionResponse {
                transaction_id: transaction.id,
                amount: transaction.amount,
                status: "pending".to_string(),
                created_at: transaction.created_at,
            };

            info!("✅ API: Transaction submitted: id={}", transaction.id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to submit transaction: {}", e),
            ))
        }
    }
}

/// Get pending transactions
#[utoipa::path(
    get,
    path = "/api/v1/state/transactions/pending",
    responses(
        (status = 200, description = "List of pending transactions", body = PendingTransactionsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transactions"
)]
#[instrument(skip(state), level = "info")]
async fn get_pending_transactions_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<PendingTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting pending transactions");

    match get_pending_transactions(&state.pool).await {
        Ok(transactions) => {
            let total_amount: i32 = transactions.iter().map(|t| t.amount).sum();
            let transaction_infos: Vec<TransactionInfo> = transactions
                .into_iter()
                .map(|t| TransactionInfo {
                    id: t.id,
                    amount: t.amount,
                    created_at: t.created_at,
                })
                .collect();

            let response = PendingTransactionsResponse {
                total_count: transaction_infos.len(),
                total_amount,
                transactions: transaction_infos,
            };

            info!(
                "✅ API: Found {} pending transactions",
                response.total_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get pending transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pending transactions: {}", e),
            ))
        }
    }
}

/// Create a new batch using unified ADS-integrated workflow
#[utoipa::path(
    post,
    path = "/api/v1/state/batches",
    request_body = CreateBatchRequest,
    responses(
        (status = 200, description = "Batch created successfully", body = CreateBatchResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn create_batch_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateBatchRequest>,
) -> Result<Json<CreateBatchResponse>, (StatusCode, String)> {
    // Validate and clamp batch_size to ensure it's in the valid range [1, max_batch_size]
    let requested_size = request
        .batch_size
        .unwrap_or(state.config.max_batch_size as i32);
    let batch_size = requested_size
        .max(1)
        .min(state.config.max_batch_size as i32);

    info!(
        "🔄 UNIFIED API: Creating batch with size: {} (using ADS integration)",
        batch_size
    );

    // Use unified batch service for consistent ADS integration
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool.clone(),
        state.ads_service.clone(),
        state.config.max_batch_size,
    );

    match unified_service
        .create_batch_with_ads(Some(batch_size), "api")
        .await
    {
        Ok(Some(result)) => {
            let response = CreateBatchResponse {
                batch_id: result.batch_id,
                previous_counter_value: result.previous_counter_value,
                final_counter_value: result.final_counter_value,
                transaction_count: result.transaction_count,
                proof_status: "pending".to_string(), // New batches start as pending
                created_at: chrono::Utc::now(),
            };

            info!(
                "✅ UNIFIED API: Batch created with ADS integration: id={}, transactions={}, nullifiers={}, merkle_root=0x{}",
                result.batch_id,
                result.transaction_count,
                result.nullifier_count,
                hex::encode(&result.merkle_root[..8])
            );

            // Trigger proof generation for the newly created batch
            if state.batch_processor.is_some() {
                info!(
                    "🚀 Triggering proof generation for batch {}",
                    result.batch_id
                );
                tokio::spawn({
                    let pool = state.pool.clone();
                    let batch_id = result.batch_id;
                    async move {
                        if let Err(e) = crate::batch_processor::BackgroundBatchProcessor::generate_proof_for_batch(&pool, batch_id).await {
                            error!("Failed to generate proof for unified batch {}: {}", batch_id, e);
                        }
                    }
                });
            } else {
                warn!("Batch processor not available - proof generation skipped");
            }

            Ok(Json(response))
        }
        Ok(None) => {
            info!("ℹ️ UNIFIED API: No transactions available to batch");
            Err((
                StatusCode::BAD_REQUEST,
                "No pending transactions available to batch".to_string(),
            ))
        }
        Err(e) => {
            error!("UNIFIED API: Failed to create batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create batch: {}", e),
            ))
        }
    }
}

/// Get all batches (paginated)
#[utoipa::path(
    get,
    path = "/api/v1/state/batches",
    params(
        ("limit" = Option<i32>, Query, description = "Limit the number of batches returned")
    ),
    responses(
        (status = 200, description = "List of batches", body = BatchListResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batches_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<BatchListQuery>,
) -> Result<Json<BatchListResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(20);
    info!("📋 API: Getting batches with limit: {}", limit);

    match get_all_batches(&state.pool, Some(limit)).await {
        Ok(batches) => {
            let batch_infos: Vec<BatchInfo> = batches
                .into_iter()
                .map(|b| BatchInfo {
                    id: b.id,
                    previous_counter_value: b.previous_counter_value,
                    final_counter_value: b.final_counter_value,
                    transaction_count: b.transaction_ids.len(),
                    proof_status: b.proof_status,
                    sindri_proof_id: b.sindri_proof_id,
                    created_at: b.created_at,
                    proven_at: b.proven_at,
                })
                .collect();

            let response = BatchListResponse {
                total_count: batch_infos.len(),
                batches: batch_infos,
            };

            info!("✅ API: Found {} batches", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get batches: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batches: {}", e),
            ))
        }
    }
}

/// Get specific batch by ID
#[utoipa::path(
    get,
    path = "/api/v1/state/batch/{batch_id}",
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    responses(
        (status = 200, description = "Batch details", body = BatchInfo),
        (status = 404, description = "Batch not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batch_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!("🔍 API: Getting batch: id={}", batch_id);

    match get_batch_by_id(&state.pool, batch_id).await {
        Ok(batch) => {
            let batch_info = BatchInfo {
                id: batch.id,
                previous_counter_value: batch.previous_counter_value,
                final_counter_value: batch.final_counter_value,
                transaction_count: batch.transaction_ids.len(),
                proof_status: batch.proof_status,
                sindri_proof_id: batch.sindri_proof_id,
                created_at: batch.created_at,
                proven_at: batch.proven_at,
            };

            info!("✅ API: Found batch: id={}", batch.id);
            Ok(Json(batch_info))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batch: {}", e),
            ))
        }
    }
}

/// Update batch with ZK proof information
///
/// Validates:
/// - Status must be one of: "pending", "proven", "failed"
/// - When status is "proven", merkle_root is required and must be exactly 32 bytes
/// - When status is not "proven", merkle_root is optional but if provided must be valid hex and 32 bytes
/// - Hex values can optionally start with "0x" prefix
#[utoipa::path(
    post,
    path = "/api/v1/state/batch/{batch_id}/proof",
    request_body = UpdateBatchProofRequest,
    responses(
        (status = 200, description = "Batch proof updated successfully", body = BatchInfo),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "Batch not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn update_batch_proof_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
    Json(request): Json<UpdateBatchProofRequest>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!(
        "🔐 API: Updating batch proof: id={}, proof={}",
        batch_id, request.sindri_proof_id
    );

    // Validate status against allowed values
    const ALLOWED_STATUSES: &[&str] = &["pending", "proven", "failed"];
    if !ALLOWED_STATUSES.contains(&request.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid status '{}'. Allowed values: {}",
                request.status,
                ALLOWED_STATUSES.join(", ")
            ),
        ));
    }

    // Validate merkle_root requirements based on status
    let validated_merkle_root = if request.status == "proven" {
        // For "proven" status, merkle_root is required
        match &request.merkle_root {
            Some(merkle_root_hex) => {
                // Decode hex string to bytes
                let merkle_root =
                    hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid merkle root hex: {}", e),
                        )
                    })?;

                // Validate that merkle_root is exactly 32 bytes
                if merkle_root.len() != 32 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Merkle root must be exactly 32 bytes, got {} bytes",
                            merkle_root.len()
                        ),
                    ));
                }

                Some(merkle_root)
            }
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Merkle root is required when status is 'proven'".to_string(),
                ));
            }
        }
    } else {
        // For other statuses, merkle_root is not required (but allowed)
        if let Some(merkle_root_hex) = &request.merkle_root {
            // If provided, still validate the format
            let merkle_root =
                hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid merkle root hex: {}", e),
                    )
                })?;

            // Validate that merkle_root is exactly 32 bytes
            if merkle_root.len() != 32 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Merkle root must be exactly 32 bytes, got {} bytes",
                        merkle_root.len()
                    ),
                ));
            }

            Some(merkle_root)
        } else {
            None
        }
    };

    // Update batch with proof
    match update_batch_proof(
        &state.pool,
        batch_id,
        &request.sindri_proof_id,
        &request.status,
    )
    .await
    {
        Ok(()) => {
            // Store Merkle root if validated and provided
            if let Some(merkle_root) = validated_merkle_root {
                if let Err(e) = store_ads_state_commit(&state.pool, batch_id, &merkle_root).await {
                    error!("Failed to store ADS state commit: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to store ADS state commit: {}", e),
                    ));
                }
            }

            // Return updated batch info
            match get_batch_by_id(&state.pool, batch_id).await {
                Ok(batch) => {
                    let batch_info = BatchInfo {
                        id: batch.id,
                        previous_counter_value: batch.previous_counter_value,
                        final_counter_value: batch.final_counter_value,
                        transaction_count: batch.transaction_ids.len(),
                        proof_status: batch.proof_status,
                        sindri_proof_id: batch.sindri_proof_id,
                        created_at: batch.created_at,
                        proven_at: batch.proven_at,
                    };

                    info!("✅ API: Batch proof updated: id={}", batch_id);
                    Ok(Json(batch_info))
                }
                Err(e) => {
                    error!("Failed to get updated batch: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to get updated batch: {}", e),
                    ))
                }
            }
        }
        Err(e) => {
            error!("Failed to update batch proof: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update batch proof: {}", e),
            ))
        }
    }
}

/// Get current counter state
#[utoipa::path(
    get,
    path = "/api/v1/state/current",
    responses(
        (status = 200, description = "Current state information", body = CurrentStateResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
async fn get_current_state_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<CurrentStateResponse>, (StatusCode, String)> {
    info!("🎯 API: Getting current state");

    match get_current_state(&state.pool).await {
        Ok(state_info) => {
            // Find last proven batch
            let last_proven_batch_id = if state_info.merkle_root.is_some() {
                state_info.last_batch_id
            } else {
                None
            };

            let response = CurrentStateResponse {
                counter_value: state_info.counter_value,
                has_merkle_root: state_info.merkle_root.is_some(),
                last_batch_id: state_info.last_batch_id,
                last_proven_batch_id,
            };

            info!(
                "✅ API: Current state: counter={}, has_root={}",
                response.counter_value, response.has_merkle_root
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get current state: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get current state: {}", e),
            ))
        }
    }
}

/// Get contract submission data (dry run)
#[utoipa::path(
    get,
    path = "/api/v1/state/{batch_id}/contract",
    responses(
        (status = 200, description = "Contract submission data", body = ContractSubmissionData),
        (status = 404, description = "Batch not found or not proven"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
async fn get_contract_data_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<ContractSubmissionData>, (StatusCode, String)> {
    info!("📄 API: Getting contract data for batch: id={}", batch_id);

    match get_contract_submission_data(&state.pool, batch_id).await {
        Ok(db_contract_data) => {
            info!("✅ API: Contract data prepared for batch: id={}", batch_id);

            // Convert from db type to API type
            let contract_data = ContractSubmissionData {
                public: ContractPublicData {
                    prev_merkle_root: db_contract_data.public.prev_merkle_root,
                    new_merkle_root: db_contract_data.public.new_merkle_root,
                    zk_proof: db_contract_data.public.zk_proof,
                },
                private: ContractPrivateData {
                    prev_counter_value: db_contract_data.private.prev_counter_value,
                    new_counter_value: db_contract_data.private.new_counter_value,
                    transactions: db_contract_data.private.transactions,
                },
            };

            Ok(Json(contract_data))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found or not proven: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found or not proven: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get contract data: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get contract data: {}", e),
            ))
        }
    }
}

/// Manually trigger batch processing
#[utoipa::path(
    post,
    path = "/api/v1/state/batches/trigger",
    responses(
        (status = 200, description = "Batch processing triggered successfully", body = TriggerBatchResponse),
        (status = 500, description = "Failed to trigger batch processing")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn trigger_batch_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<TriggerBatchResponse>, (StatusCode, String)> {
    info!("🔄 API: Manual batch trigger requested");

    if let Some(batch_processor) = &state.batch_processor {
        match batch_processor.trigger_batch() {
            Ok(()) => {
                info!("✅ API: Batch trigger sent successfully");
                Ok(Json(TriggerBatchResponse {
                    triggered: true,
                    message: "Batch processing triggered successfully".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to trigger batch processing: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to trigger batch processing: {}", e),
                ))
            }
        }
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}

/// Get batch processor statistics
#[utoipa::path(
    get,
    path = "/api/v1/state/batches/stats",
    responses(
        (status = 200, description = "Batch processor statistics", body = BatchProcessorStatsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
async fn get_batch_processor_stats_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<BatchProcessorStatsResponse>, (StatusCode, String)> {
    info!("📊 API: Batch processor stats requested");

    if let Some(batch_processor) = &state.batch_processor {
        let processor_stats = batch_processor.get_stats().await;

        let response = BatchProcessorStatsResponse {
            total_batches_created: processor_stats.total_batches_created,
            total_transactions_processed: processor_stats.total_transactions_processed,
            timer_triggers: processor_stats.timer_triggers,
            count_triggers: processor_stats.count_triggers,
            manual_triggers: processor_stats.manual_triggers,
            errors: processor_stats.errors,
            last_batch_time: processor_stats
                .last_batch_time
                .map(|t| format!("{} ago", format_duration(t.elapsed()))),
        };

        info!("✅ API: Batch processor stats returned");
        Ok(Json(response))
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}

// ============================================================================
// LOAN PROCESSING ENDPOINT HANDLERS
// ============================================================================

// ---- USER MANAGEMENT ENDPOINTS ----

/// Register a new user identity
#[utoipa::path(
    post,
    path = "/api/v1/user",
    request_body = RegisterUserRequest,
    responses(
        (status = 200, description = "User registered successfully", body = RegisterUserResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn register_user_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<RegisterUserRequest>,
) -> Result<Json<RegisterUserResponse>, (StatusCode, String)> {
    info!(
        "👤 API: Registering new user: type={}, email={}",
        request.user_type, request.email
    );

    // Basic user registration using direct SQL
    let user_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    let result = sqlx::query!(
        r#"
        INSERT INTO identities (id, user_type, email, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        "#,
        user_id,
        request.user_type,
        request.email,
        now
    )
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let response = RegisterUserResponse {
                user_id: user_id.to_string(),
                user_type: request.user_type,
                kyc_status: "not_submitted".to_string(),
                created_at: now,
            };

            info!("✅ API: User registered successfully: id={}", user_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to register user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to register user: {}", e),
            ))
        }
    }
}

/// List user identities
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(
        ListIdentitiesQuery
    ),
    responses(
        (status = 200, description = "List of user identities", body = ListIdentitiesResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn list_identities_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListIdentitiesQuery>,
) -> Result<Json<ListIdentitiesResponse>, (StatusCode, String)> {
    info!("📋 API: Listing identities with filters");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Basic identity listing using direct SQL
    let identities_result = sqlx::query!(
        r#"
        SELECT id, user_type, email, kyc_status, kyc_submitted_at, kyc_verified_at, created_at, updated_at
        FROM identities
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(&state.pool)
    .await;

    match identities_result {
        Ok(rows) => {
            let identities = rows
                .into_iter()
                .map(|row| UserDetailsResponse {
                    user_id: row.id.to_string(),
                    user_type: row.user_type,
                    kyc_status: row.kyc_status,
                    kyc_submitted_at: row.kyc_submitted_at,
                    kyc_verified_at: row.kyc_verified_at,
                    created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
                })
                .collect::<Vec<_>>();

            let total_count = identities.len() as u64;

            let response = ListIdentitiesResponse {
                identities,
                total_count: total_count as usize,
                has_more: total_count >= limit as u64,
            };

            info!("✅ API: Listed {} identities", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ API: Failed to list identities: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list identities: {}", e),
            ))
        }
    }
}

/// Get user details by ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}",
    responses(
        (status = 200, description = "User details retrieved successfully", body = UserDetailsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(_state), level = "info")]
async fn get_user_details_endpoint(
    State(_state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting user details: user_id={}", user_id);

    // TODO: Implement user lookup with database integration
    // For now, return mock user
    let response = UserDetailsResponse {
        user_id,
        user_type: "Investor".to_string(),
        kyc_status: "NotSubmitted".to_string(),
        kyc_submitted_at: None,
        kyc_verified_at: None,
        created_at: Utc::now(),
    };

    info!("✅ API: User details retrieved");
    Ok(Json(response))
}

/// Submit KYC information
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc",
    request_body = SubmitKycRequest,
    responses(
        (status = 200, description = "KYC information submitted successfully", body = SubmitKycResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(_state), level = "info")]
async fn submit_kyc_endpoint(
    State(_state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<SubmitKycRequest>,
) -> Result<Json<SubmitKycResponse>, (StatusCode, String)> {
    info!(
        "📄 API: KYC submission: user_id={}, doc_type={}",
        user_id, request.kyc_data.document_type
    );

    // TODO: Implement KYC submission with database integration
    let response = SubmitKycResponse {
        user_id,
        kyc_status: "Pending".to_string(),
        submitted_at: Utc::now(),
    };

    info!("✅ API: KYC submitted successfully");
    Ok(Json(response))
}

/// Update KYC status (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/user/{user_id}/kyc-status",
    request_body = UpdateKycStatusRequest,
    responses(
        (status = 200, description = "KYC status updated successfully", body = UserDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(_state), level = "info")]
async fn update_kyc_status_endpoint(
    State(_state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateKycStatusRequest>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    info!(
        "⚖️ API: Updating KYC status: user_id={}, new_status={}",
        user_id, request.status
    );

    // TODO: Implement KYC status update with database integration
    let response = UserDetailsResponse {
        user_id,
        user_type: "Investor".to_string(),
        kyc_status: request.status,
        kyc_submitted_at: Some(Utc::now()),
        kyc_verified_at: Some(Utc::now()),
        created_at: Utc::now(),
    };

    info!("✅ API: KYC status updated successfully");
    Ok(Json(response))
}

/// Update user profile
#[utoipa::path(
    put,
    path = "/api/v1/user/{user_id}",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated successfully", body = UpdateUserResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn update_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UpdateUserResponse>, (StatusCode, String)> {
    info!("🔄 API: Updating user profile: user_id={}", user_id);

    // Validate user exists first
    let user_exists = sqlx::query!(
        "SELECT id FROM identities WHERE id = $1::uuid",
        uuid::Uuid::parse_str(&user_id).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid user ID format".to_string(),
            )
        })?
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to check if user exists: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?;

    if user_exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
    }

    let mut updated_fields = Vec::new();
    let now = Utc::now();

    // Build dynamic update query based on provided fields
    let mut query = "UPDATE identities SET updated_at = $1".to_string();
    let mut param_count = 1;

    if request.email.is_some() {
        param_count += 1;
        query.push_str(&format!(", email = ${}", param_count));
        updated_fields.push("email".to_string());
    }

    if request.user_type.is_some() {
        param_count += 1;
        query.push_str(&format!(", user_type = ${}", param_count));
        updated_fields.push("user_type".to_string());
    }

    if request.jurisdiction.is_some() {
        param_count += 1;
        query.push_str(&format!(", jurisdiction = ${}", param_count));
        updated_fields.push("jurisdiction".to_string());
    }

    param_count += 1;
    query.push_str(&format!(" WHERE id = ${}", param_count));

    // Execute the update
    let user_uuid = uuid::Uuid::parse_str(&user_id).unwrap();
    let mut query_builder = sqlx::query(&query).bind(now);

    if let Some(email) = &request.email {
        query_builder = query_builder.bind(email);
    }
    if let Some(user_type) = &request.user_type {
        query_builder = query_builder.bind(user_type);
    }
    if let Some(jurisdiction) = &request.jurisdiction {
        query_builder = query_builder.bind(jurisdiction);
    }

    query_builder = query_builder.bind(user_uuid);

    query_builder.execute(&state.pool).await.map_err(|e| {
        error!("Failed to update user: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update user".to_string(),
        )
    })?;

    let response = UpdateUserResponse {
        user_id,
        updated_fields,
        updated_at: now,
    };

    info!("✅ API: User profile updated successfully");
    Ok(Json(response))
}

/// Delete user
#[utoipa::path(
    delete,
    path = "/api/v1/user/{user_id}",
    responses(
        (status = 200, description = "User deleted successfully", body = DeleteUserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    tag = "Users"
)]
#[instrument(skip(state), level = "info")]
async fn delete_user_endpoint(
    State(state): State<ApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<DeleteUserResponse>, (StatusCode, String)> {
    info!("🗑️ API: Deleting user: user_id={}", user_id);

    let user_uuid = uuid::Uuid::parse_str(&user_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Check if user exists and delete
    let deleted_rows = sqlx::query!("DELETE FROM identities WHERE id = $1", user_uuid)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!("Failed to delete user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete user".to_string(),
            )
        })?
        .rows_affected();

    if deleted_rows == 0 {
        return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
    }

    let response = DeleteUserResponse {
        user_id,
        deleted: true,
        deleted_at: Utc::now(),
    };

    info!("✅ API: User deleted successfully");
    Ok(Json(response))
}

// ---- LOAN MANAGEMENT ENDPOINTS ----

/// Create a new loan
#[utoipa::path(
    post,
    path = "/api/v1/loans",
    request_body = CreateLoanRequest,
    responses(
        (status = 200, description = "Loan created successfully", body = LoanDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Loans"
)]
#[instrument(skip(_state), level = "info")]
async fn create_loan_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<CreateLoanRequest>,
) -> Result<Json<LoanDetailsResponse>, (StatusCode, String)> {
    info!(
        "💰 API: Creating loan: borrower={}, lender={}, amount={}",
        request.borrower_id, request.lender_id, request.principal_amount
    );

    // TODO: Implement loan creation with database integration
    let loan_id = uuid::Uuid::new_v4().to_string();

    let response = LoanDetailsResponse {
        loan_id,
        borrower_id: request.borrower_id,
        lender_id: request.lender_id,
        principal_amount: request.principal_amount.clone(),
        outstanding_balance: request.principal_amount,
        interest_rate_bps: request.interest_rate_bps,
        loan_status: "Originated".to_string(),
        originated_at: Utc::now(),
        last_updated: None,
    };

    info!("✅ API: Loan created successfully");
    Ok(Json(response))
}

/// List loans
#[instrument(skip(_state), level = "info")]
async fn list_loans_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<ListLoansQuery>,
) -> Result<Json<ListLoansResponse>, (StatusCode, String)> {
    info!("📋 API: Listing loans with filters");

    // TODO: Implement loan listing with database integration
    let response = ListLoansResponse {
        loans: vec![],
        total_count: 0,
        has_more: false,
    };

    info!("✅ API: Listed {} loans", response.total_count);
    Ok(Json(response))
}

/// Get loan details by ID
#[instrument(skip(_state), level = "info")]
async fn get_loan_details_endpoint(
    State(_state): State<ApiState>,
    Path(loan_id): Path<String>,
) -> Result<Json<LoanDetailsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting loan details: loan_id={}", loan_id);

    // TODO: Implement loan lookup with database integration
    let response = LoanDetailsResponse {
        loan_id,
        borrower_id: uuid::Uuid::new_v4().to_string(),
        lender_id: uuid::Uuid::new_v4().to_string(),
        principal_amount: "100000".to_string(),
        outstanding_balance: "75000".to_string(),
        interest_rate_bps: 500,
        loan_status: "Active".to_string(),
        originated_at: Utc::now(),
        last_updated: Some(Utc::now()),
    };

    info!("✅ API: Loan details retrieved");
    Ok(Json(response))
}

/// Get loan statistics
#[instrument(skip(_state), level = "info")]
async fn get_loan_stats_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<ListLoansQuery>, // Reuse query params for filtering
) -> Result<Json<LoanStatsResponse>, (StatusCode, String)> {
    info!("📊 API: Getting loan statistics");

    // TODO: Implement loan statistics with database integration
    let mut status_breakdown = std::collections::HashMap::new();
    status_breakdown.insert("Originated".to_string(), 10);
    status_breakdown.insert("Active".to_string(), 120);
    status_breakdown.insert("PaidOff".to_string(), 15);
    status_breakdown.insert("Closed".to_string(), 5);

    let response = LoanStatsResponse {
        total_loans: 150,
        total_principal: "50000000".to_string(), // $500,000.00
        total_outstanding: "48500000".to_string(), // $485,000.00
        average_interest_rate_bps: 475,
        status_breakdown,
    };

    info!("✅ API: Loan statistics retrieved");
    Ok(Json(response))
}

// ---- PAYMENT PROCESSING ENDPOINTS ----

/// Record a loan payment
#[instrument(skip(_state), level = "info")]
async fn record_payment_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<RecordPaymentRequest>,
) -> Result<Json<PaymentResponse>, (StatusCode, String)> {
    info!(
        "💳 API: Recording payment: loan_id={}, amount={}, type={}",
        request.loan_id, request.amount, request.payment_type
    );

    // TODO: Implement payment recording with database integration
    let transaction_id = uuid::Uuid::new_v4().to_string();

    let response = PaymentResponse {
        transaction_id,
        loan_id: request.loan_id,
        amount: request.amount,
        payment_type: request.payment_type,
        transaction_status: "Pending".to_string(), // Will be batched for ZK proof
        created_at: Utc::now(),
    };

    info!("✅ API: Payment recorded successfully");
    Ok(Json(response))
}

/// Get payment details
#[instrument(skip(_state), level = "info")]
async fn get_payment_details_endpoint(
    State(_state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> Result<Json<PaymentDetailsResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Getting payment details: transaction_id={}",
        transaction_id
    );

    // TODO: Implement payment lookup with database integration
    let response = PaymentDetailsResponse {
        transaction_id,
        loan_id: uuid::Uuid::new_v4().to_string(),
        amount: "25000".to_string(),
        payment_type: "Principal".to_string(),
        transaction_status: "Processed".to_string(),
        batch_id: Some(uuid::Uuid::new_v4().to_string()),
        created_at: Utc::now(),
        processed_at: Some(Utc::now()),
    };

    info!("✅ API: Payment details retrieved");
    Ok(Json(response))
}

/// Get loan payment history
#[instrument(skip(_state), level = "info")]
async fn get_loan_payment_history_endpoint(
    State(_state): State<ApiState>,
    Path(loan_id): Path<String>,
    Query(params): Query<ListLoansQuery>, // Reuse query params for limit/offset
) -> Result<Json<LoanPaymentHistoryResponse>, (StatusCode, String)> {
    info!("📜 API: Getting payment history: loan_id={}", loan_id);

    // TODO: Implement payment history lookup with database integration
    let response = LoanPaymentHistoryResponse {
        loan_id,
        payments: vec![],
        total_payments: 0,
        total_amount_paid: "0".to_string(),
    };

    info!("✅ API: Payment history retrieved");
    Ok(Json(response))
}

// ---- STATE MANAGEMENT & ZK PROOF ENDPOINTS ----

/// Get current state summary
#[instrument(skip(_state), level = "info")]
async fn get_state_summary_endpoint(
    State(_state): State<ApiState>,
) -> Result<Json<StateSummaryResponse>, (StatusCode, String)> {
    info!("🎯 API: Getting current state summary");

    // TODO: Implement state summary with database integration
    let response = StateSummaryResponse {
        current_state_root: "0x789abc...".to_string(),
        transaction_count: 1250,
        last_batch_id: Some(uuid::Uuid::new_v4().to_string()),
        last_updated: Utc::now(),
        loan_summary: LoanStatsSummary {
            total_loans: 150,
            total_principal: "50000000".to_string(),
            total_outstanding: "48500000".to_string(),
        },
        user_summary: UserStatsSummary {
            total_users: 85,
            kyc_approved_users: 72,
        },
    };

    info!("✅ API: State summary retrieved");
    Ok(Json(response))
}

/// Generate transaction inclusion proof
#[instrument(skip(_state), level = "info")]
async fn get_transaction_proof_endpoint(
    State(_state): State<ApiState>,
    Path((transaction_type, transaction_id)): Path<(String, String)>,
) -> Result<Json<TransactionProofResponse>, (StatusCode, String)> {
    info!(
        "🔐 API: Generating inclusion proof: type={}, id={}",
        transaction_type, transaction_id
    );

    // TODO: Implement proof generation with database integration
    let response = TransactionProofResponse {
        transaction_id,
        transaction_type: transaction_type.clone(),
        proof: ProofData {
            merkle_proof: "0x123abc...".to_string(),
            batch_proof: "0x456def...".to_string(),
            state_root: "0x789abc...".to_string(),
        },
        verification_data: VerificationData {
            batch_id: uuid::Uuid::new_v4().to_string(),
            batch_timestamp: 1642681500,
            transaction_index: 42,
        },
    };

    info!("✅ API: Transaction proof generated");
    Ok(Json(response))
}

/// Verify transaction inclusion
#[instrument(skip(_state), level = "info")]
async fn verify_transaction_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<VerifyTransactionRequest>,
) -> Result<Json<TransactionVerificationResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Verifying transaction inclusion: id={}",
        request.transaction_id
    );

    // TODO: Implement transaction verification with database integration
    let response = TransactionVerificationResponse {
        is_valid: true,
        verification_result: VerificationResult {
            merkle_proof_valid: true,
            batch_proof_valid: true,
            transaction_included: true,
            transaction_type_matches: true,
        },
        verified_at: Utc::now(),
    };

    info!("✅ API: Transaction verified successfully");
    Ok(Json(response))
}

/// Get state transition history
#[instrument(skip(_state), level = "info")]
async fn get_state_history_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<BatchListQuery>, // Reuse query params for limit
) -> Result<Json<StateHistoryResponse>, (StatusCode, String)> {
    info!("📜 API: Getting state transition history");

    // TODO: Implement state history with database integration
    let response = StateHistoryResponse {
        state_transitions: vec![],
        total_batches: 0,
    };

    info!("✅ API: State history retrieved");
    Ok(Json(response))
}

// ---- BATCH OPERATIONS FOR LOANS ----

/// Create batch from pending loan transactions
#[instrument(skip(_state), level = "info")]
async fn create_loan_batch_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<CreateLoanBatchRequest>,
) -> Result<Json<CreateLoanBatchResponse>, (StatusCode, String)> {
    info!(
        "🔄 API: Creating loan batch: max_count={:?}, force={:?}",
        request.max_transaction_count, request.force_create
    );

    // TODO: Implement loan batch creation with database integration
    let batch_id = uuid::Uuid::new_v4().to_string();

    let response = CreateLoanBatchResponse {
        batch_id,
        transaction_count: 25,
        batch_status: "Created".to_string(),
        estimated_proof_time: "8m".to_string(),
        created_at: Utc::now(),
    };

    info!("✅ API: Loan batch created successfully");
    Ok(Json(response))
}

/// Get loan batch details
#[instrument(skip(state), level = "info")]
async fn get_loan_batch_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<String>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!("🔍 API: Getting loan batch details: batch_id={}", batch_id);

    // TODO: Implement loan batch lookup with database integration
    // For now, delegate to existing batch endpoint
    let batch_id_int: i32 = batch_id.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid batch ID format".to_string(),
        )
    })?;

    get_batch_endpoint(State(state), Path(batch_id_int)).await
}

/// Get pending loan transactions ready for batching
#[instrument(skip(_state), level = "info")]
async fn get_pending_loan_transactions_endpoint(
    State(_state): State<ApiState>,
    Query(params): Query<BatchListQuery>, // Reuse query params for limit
) -> Result<Json<PendingLoanTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting pending loan transactions");

    // TODO: Implement pending loan transactions lookup with database integration
    let response = PendingLoanTransactionsResponse {
        pending_transactions: vec![],
        total_pending: 0,
        total_amount: "0".to_string(),
        oldest_pending: None,
    };

    info!("✅ API: Pending loan transactions retrieved");
    Ok(Json(response))
}

// ============================================================================
// LOAN OPERATION INCLUSION PROOF ENDPOINT HANDLERS
// ============================================================================

/// Request body for generating loan operation inclusion proof
#[derive(Debug, Deserialize, ToSchema)]
struct GenerateLoanOperationProofRequest {
    user_id: uuid::Uuid,
    loan_id: Option<uuid::Uuid>,
    payment_id: Option<uuid::Uuid>,
    operation_type: String,
    batch_id: Option<i32>,
    user_role: String,
}

/// Response for loan operation inclusion proof generation
#[derive(Debug, Serialize, ToSchema)]
struct LoanOperationProofResponse {
    #[schema(value_type = Object)]
    proof: LoanInclusionProof,
    generated_at: DateTime<Utc>,
    verification_summary: serde_json::Value,
}

/// Generate loan operation inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn generate_loan_operation_proof_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<GenerateLoanOperationProofRequest>,
) -> Result<Json<LoanOperationProofResponse>, (StatusCode, String)> {
    info!(
        "🔍 API: Generating loan operation inclusion proof for user {}",
        request.user_id
    );

    // Use the unified batch service to generate proof
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool,
        state.ads_service,
        state.config.max_batch_size,
    );

    let proof = unified_service
        .generate_loan_inclusion_proof(
            request.user_id,
            request.loan_id,
            request.payment_id,
            request.operation_type,
            request.batch_id,
            request.user_role,
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Generate verification summary
    let verification_summary = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = LoanOperationProofResponse {
        proof,
        generated_at: Utc::now(),
        verification_summary,
    };

    info!("✅ API: Loan operation inclusion proof generated successfully");
    Ok(Json(response))
}

/// Generate loan payment inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn generate_loan_payment_proof_endpoint(
    State(state): State<ApiState>,
    Path((user_id, loan_id, payment_id, batch_id)): Path<(uuid::Uuid, uuid::Uuid, uuid::Uuid, i32)>,
) -> Result<Json<LoanOperationProofResponse>, (StatusCode, String)> {
    info!(
        "💰 API: Generating loan payment proof for payment {} in batch {}",
        payment_id, batch_id
    );

    // Use the unified batch service to generate proof
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let proof = unified_service
        .generate_loan_payment_proof(user_id, loan_id, payment_id, batch_id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Generate verification summary
    let verification_summary = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = LoanOperationProofResponse {
        proof,
        generated_at: Utc::now(),
        verification_summary,
    };

    info!("✅ API: Loan payment inclusion proof generated successfully");
    Ok(Json(response))
}

/// Verify loan inclusion proof endpoint
#[instrument(skip(state), level = "info")]
async fn verify_loan_proof_endpoint(
    State(state): State<ApiState>,
    Json(proof): Json<LoanInclusionProof>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🔍 API: Verifying loan inclusion proof");

    // Use the unified batch service to verify proof
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool,
        state.ads_service,
        state.config.max_batch_size,
    );

    let is_valid = unified_service
        .verify_loan_proof(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let report = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response = serde_json::json!({
        "valid": is_valid,
        "verified_at": Utc::now(),
        "proof_details": report
    });

    info!(
        "✅ API: Loan inclusion proof verification completed: valid={}",
        is_valid
    );
    Ok(Json(response))
}

/// Generate proof verification report endpoint
#[instrument(skip(state), level = "info")]
async fn generate_proof_report_endpoint(
    State(state): State<ApiState>,
    Json(proof): Json<LoanInclusionProof>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("📊 API: Generating proof verification report");

    // Use the unified batch service to generate report
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let report = unified_service
        .generate_proof_report(&proof)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    info!("✅ API: Proof verification report generated");
    Ok(Json(report))
}

/// Batch verify multiple loan proofs endpoint
#[instrument(skip(state), level = "info")]
async fn batch_verify_proofs_endpoint(
    State(state): State<ApiState>,
    Json(proofs): Json<Vec<LoanInclusionProof>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!(
        "🔍 API: Batch verifying {} loan inclusion proofs",
        proofs.len()
    );

    // Use the unified batch service to batch verify proofs
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let results = unified_service
        .batch_verify_proofs(&proofs)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let valid_count = results.iter().filter(|&&v| v).count();
    let response = serde_json::json!({
        "total_proofs": proofs.len(),
        "valid_proofs": valid_count,
        "invalid_proofs": proofs.len() - valid_count,
        "results": results,
        "verified_at": Utc::now()
    });

    info!(
        "✅ API: Batch verification completed: {}/{} proofs valid",
        valid_count,
        proofs.len()
    );
    Ok(Json(response))
}

/// Get loan audit trail endpoint
#[instrument(skip(state), level = "info")]
async fn get_loan_audit_trail_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<AuditEvent>>, (StatusCode, String)> {
    info!("📋 API: Getting loan audit trail for loan {}", loan_id);

    // Use the unified batch service to get audit trail
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let audit_trail = unified_service
        .get_loan_audit_trail(loan_id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    info!(
        "✅ API: Retrieved {} audit events for loan",
        audit_trail.len()
    );
    Ok(Json(audit_trail))
}

/// Get loan lifecycle summary endpoint
#[instrument(skip(state), level = "info")]
async fn get_loan_lifecycle_summary_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<uuid::Uuid>,
) -> Result<Json<LoanLifecycleSummary>, (StatusCode, String)> {
    info!(
        "📈 API: Getting loan lifecycle summary for loan {}",
        loan_id
    );

    // Use the unified batch service to get lifecycle summary
    let unified_service =
        crate::unified_batch_service::UnifiedBatchService::new(state.pool, state.ads_service, 100);

    let summary = unified_service
        .get_loan_lifecycle_summary(loan_id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    info!(
        "✅ API: Generated loan lifecycle summary: {} operations, {} payments",
        summary.total_operations, summary.total_payments
    );
    Ok(Json(summary))
}

// ============================================================================
// OPENAPI SPECIFICATION
// ============================================================================

/// OpenAPI specification for the Arda Credit API
#[derive(OpenApi)]
#[openapi(
    paths(
        // Core endpoints
        health_check,
        api_info,

        // Transaction endpoints
        submit_transaction_endpoint,
        get_pending_transactions_endpoint,

        // Batch endpoints
        create_batch_endpoint,
        get_batches_endpoint,
        get_batch_endpoint,
        update_batch_proof_endpoint,
        trigger_batch_endpoint,
        get_batch_processor_stats_endpoint,

        // State endpoints
        get_current_state_endpoint,
        get_contract_data_endpoint,

        // User Management endpoints
        register_user_endpoint,
        list_identities_endpoint,
        get_user_details_endpoint,
        update_user_endpoint,
        delete_user_endpoint,
        submit_kyc_endpoint,
        update_kyc_status_endpoint,

        // Loan Management endpoints
        create_loan_endpoint
    ),
    components(
        schemas(
            // Health & Core
            HealthResponse,
            ApiInfoResponse,
            EndpointInfo,
            ContractSubmissionData,
            ContractPublicData,
            ContractPrivateData,

            // Transaction Models
            SubmitTransactionRequest,
            SubmitTransactionResponse,
            TransactionInfo,
            PendingTransactionsResponse,

            // Batch Models
            CreateBatchRequest,
            CreateBatchResponse,
            BatchListQuery,
            BatchListResponse,
            BatchInfo,
            UpdateBatchProofRequest,
            CurrentStateResponse,
            BatchProcessorStatsResponse,
            TriggerBatchResponse,

            // User Management
            RegisterUserRequest,
            RegisterUserResponse,
            UserDetailsResponse,
            SubmitKycRequest,
            KycData,
            SubmitKycResponse,
            UpdateKycStatusRequest,
            UpdateUserRequest,
            UpdateUserResponse,
            DeleteUserResponse,
            ListIdentitiesQuery,
            ListIdentitiesResponse,

            // Loan Management
            CreateLoanRequest,
            LoanDetailsResponse,
            ListLoansQuery,
            ListLoansResponse,
            LoanStatsResponse,

            // Payment Processing
            RecordPaymentRequest,
            PaymentResponse,
            PaymentDetailsResponse,
            LoanPaymentHistoryResponse,

            // State Management
            StateSummaryResponse,
            LoanStatsSummary,
            UserStatsSummary,
            TransactionProofResponse,
            ProofData,
            VerificationData,
            VerifyTransactionRequest,
            TransactionVerificationResponse,
            VerificationResult,
            StateHistoryResponse,
            StateTransition,

            // Batch Operations
            CreateLoanBatchRequest,
            CreateLoanBatchResponse,
            PendingLoanTransactionsResponse,
            PendingTransactionInfo,

            // Proof Management
            GenerateLoanOperationProofRequest,
            LoanOperationProofResponse
        )
    ),
    tags(
        (name = "Health", description = "Health check and system status endpoints"),
        (name = "Transactions", description = "Transaction submission and management"),
        (name = "Batches", description = "Batch processing and proof management"),
        (name = "Users", description = "User registration and KYC management"),
        (name = "Loans", description = "Loan creation and management"),
        (name = "Payments", description = "Payment processing and history"),
        (name = "State", description = "System state and proof verification"),
        (name = "Proofs", description = "Zero-knowledge proof generation and verification")
    ),
    info(
        title = "Arda Credit API",
        version = "1.0.0",
        description = "Zero-knowledge proof system for batched transaction processing and loan management",
        contact(
            name = "Arda Global",
            url = "https://ardaglobal.com"
        )
    ),
    servers(
        (url = "/", description = "Default server")
    )
)]
pub struct ApiDoc;

/// Get the OpenAPI specification as JSON
pub fn get_openapi_spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Axum handler for serving the OpenAPI specification as JSON
pub async fn get_openapi_spec_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Create a Swagger UI router
fn swagger_ui() -> Router<ApiState> {
    SwaggerUi::new("/api-docs")
        .url("/api-docs/openapi.json", get_openapi_spec())
        .into()
}
