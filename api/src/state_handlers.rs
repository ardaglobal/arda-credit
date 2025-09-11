//! State management handlers for the Arda Credit API
//!
//! This module contains all state-related API endpoints including:
//! - Batch processing and management
//! - State queries and history
//! - Proof generation and verification
//! - Transaction state management
//!
//! All routes are documented with OpenAPI and use consistent error handling.
//!
//! NOTE: These are placeholder implementations for compilation testing.
//! TODO: Implement proper database integration.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;

extern crate hex;

use crate::openapi::PaginationParams;
use crate::paths;
use crate::types::ApiState;

// Use Axum's built-in response types for Axum 0.8 compatibility
use arda_credit_db::{
    get_all_batches, get_batch_by_id, get_contract_submission_data, get_current_state,
    get_pending_transactions, store_ads_state_commit, submit_transaction, update_batch_proof,
    ProofBatch,
};

// ============================================================================
// STATE MANAGEMENT TYPES (MOVED FROM rest.rs)
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
    pub created_at: chrono::DateTime<chrono::Utc>,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub proven_at: Option<chrono::DateTime<chrono::Utc>>,
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
// STATE MANAGEMENT ENDPOINTS
// ============================================================================

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
pub async fn submit_transaction_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Response {
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
            Json(response).into_response()
        }
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to submit transaction: {}", e),
            )
                .into_response()
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
pub async fn get_pending_transactions_endpoint(State(state): State<ApiState>) -> Response {
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
            Json(response).into_response()
        }
        Err(e) => {
            error!("Failed to get pending transactions: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pending transactions: {}", e),
            )
                .into_response()
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
pub async fn create_batch_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateBatchRequest>,
) -> Response {
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

            Json(response).into_response()
        }
        Ok(None) => {
            info!("ℹ️ UNIFIED API: No transactions available to batch");
            (
                StatusCode::BAD_REQUEST,
                "No pending transactions available to batch".to_string(),
            )
                .into_response()
        }
        Err(e) => {
            error!("UNIFIED API: Failed to create batch: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create batch: {}", e),
            )
                .into_response()
        }
    }
}

/// Get all batches
#[utoipa::path(
    get,
    path = "/api/v1/state/batches",
    responses(
        (status = 200, description = "Batches retrieved successfully", body = BatchListResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Batches"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_batches_endpoint(
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
pub async fn get_current_state_endpoint(
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
pub async fn get_batch_endpoint(
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

/// Get batch details by ID
#[utoipa::path(
    get,
    path = "/api/v1/state/batches/{batch_id}",
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    responses(
        (status = 200, description = "Batch details retrieved successfully", body = BatchInfo),
        (status = 404, description = "Batch not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_batch_details_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Response {
    info!("🔍 API: Getting batch details for ID: {}", batch_id);

    match get_batch_by_id(&state.pool, batch_id).await {
        Ok(batch) => {
            let response = BatchInfo {
                id: batch.id,
                previous_counter_value: batch.previous_counter_value,
                final_counter_value: batch.final_counter_value,
                transaction_count: batch.transaction_ids.len(),
                proof_status: batch.proof_status,
                sindri_proof_id: batch.sindri_proof_id,
                created_at: batch.created_at,
                proven_at: batch.proven_at,
            };

            info!(
                "✅ API: Batch details retrieved: id={}, status={}, transactions={}",
                response.id, response.proof_status, response.transaction_count
            );
            Json(response).into_response()
        }
        Err(e) => {
            if let sqlx::Error::RowNotFound = e {
                warn!("❌ API: Batch not found: {}", batch_id);
                (StatusCode::NOT_FOUND, "Batch not found".to_string()).into_response()
            } else {
                error!("Failed to get batch details: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get batch details: {}", e),
                )
                    .into_response()
            }
        }
    }
}

/// Get contract submission data for a batch
#[utoipa::path(
    get,
    path = "/api/v1/state/batches/{batch_id}/contract-data",
    params(
        ("batch_id" = i32, Path, description = "Batch ID")
    ),
    responses(
        (status = 200, description = "Contract submission data retrieved successfully", body = ContractSubmissionData),
        (status = 404, description = "Batch not found or no contract data available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_contract_submission_data_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Response {
    info!(
        "📄 API: Getting contract submission data for batch: {}",
        batch_id
    );

    match get_contract_submission_data(&state.pool, batch_id).await {
        Ok(contract_data) => {
            info!(
                "✅ API: Contract submission data retrieved for batch: {}",
                batch_id
            );
            Json(contract_data).into_response()
        }
        Err(e) => {
            if let sqlx::Error::RowNotFound = e {
                warn!(
                    "❌ API: No contract submission data found for batch: {}",
                    batch_id
                );
                (
                    StatusCode::NOT_FOUND,
                    "No contract submission data found for this batch".to_string(),
                )
                    .into_response()
            } else {
                error!("Failed to get contract submission data: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get contract submission data: {}", e),
                )
                    .into_response()
            }
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
pub async fn update_batch_proof_endpoint(
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
pub async fn trigger_batch_endpoint(
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
        error!("Batch processor not available");
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Batch processor not available".to_string(),
        ))
    }
}
