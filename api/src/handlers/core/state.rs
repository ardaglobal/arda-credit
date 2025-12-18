//! State management handlers for the Arda Credit API
//!
//! This module contains all state-related API endpoints including:
//! - State transaction queries
//! - Batch processing and management
//! - Proof generation and verification
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

extern crate hex;

use crate::config::types::ApiState;

// Use Axum's built-in response types for Axum 0.8 compatibility
use arda_credit_db::{
    get_all_batches, get_batch_by_id, get_contract_submission_data, get_current_state,
    store_ads_state_commit, update_batch_proof,
};

// ============================================================================
// STATE TRANSACTION TYPES
// ============================================================================

/// Query parameters for state transactions listing
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct StateTransactionsQuery {
    /// Maximum number of transactions to return (default: 50, max: 500)
    #[param(example = 50)]
    pub limit: Option<i32>,

    /// Number of transactions to skip for pagination (default: 0)
    #[param(example = 0)]
    pub offset: Option<i32>,

    /// Filter by pending status (true = unbatched, false = batched, omit = all)
    #[param(example = true)]
    pub pending: Option<bool>,

    /// Filter by batch ID (transactions included in this batch)
    #[param(example = 42)]
    pub batch_id: Option<i32>,

    /// Filter by transaction types (comma-separated, e.g., "deal_payment,collateral_update")
    #[param(example = "deal_origination,deal_payment")]
    pub transaction_type: Option<String>,

    /// Filter by user ID (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Option<uuid::Uuid>,

    /// Filter by deal ID (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub deal_id: Option<uuid::Uuid>,

    /// Filter by originator company ID (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub originator_company_id: Option<uuid::Uuid>,

    /// Filter by investor company ID (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440003")]
    pub investor_company_id: Option<uuid::Uuid>,

    /// Filter by company ID - matches originator OR investor (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440004")]
    pub company_id: Option<uuid::Uuid>,

    /// Filter by collateral ID (UUID)
    #[param(example = "550e8400-e29b-41d4-a716-446655440005")]
    pub collateral_id: Option<uuid::Uuid>,

    /// Sort order: "asc" or "desc" (default: "desc")
    #[param(example = "desc")]
    pub order: Option<String>,

    /// Include decoded amount and metadata from transaction_data (default: false)
    #[param(example = false)]
    pub include_decoded: Option<bool>,
}

/// State transaction item in API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateTransactionItem {
    pub id: String,
    pub transaction_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub included_in_batch_id: Option<i32>,
    pub pending: bool,
    pub user_id: Option<String>,
    pub deal_id: Option<String>,
    pub originator_company_id: Option<String>,
    pub investor_company_id: Option<String>,
    pub collateral_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// State transactions list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateTransactionsResponse {
    pub transactions: Vec<StateTransactionItem>,
    pub total_count: usize,
    pub pagination: StateTransactionsPagination,
}

/// Pagination info for state transactions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StateTransactionsPagination {
    pub limit: i32,
    pub offset: i32,
    pub has_more: bool,
}

/// Response for batch listing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchListResponse {
    pub batches: Vec<BatchInfo>,
    pub total_count: usize,
}

/// Batch info for API responses (includes embedded contract data)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchInfo {
    pub id: i32,
    pub previous_state_root: Option<String>, // 0x-prefixed hex (32 bytes)
    pub final_state_root: Option<String>,    // 0x-prefixed hex (32 bytes)
    pub transaction_count: usize,
    pub proof_status: String,
    pub sindri_proof_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub proven_at: Option<chrono::DateTime<chrono::Utc>>,
    pub posted_to_contract: bool,
    pub posted_to_contract_at: Option<chrono::DateTime<chrono::Utc>>,
    pub contract_tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_prev_merkle_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_new_merkle_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_ready: Option<bool>,
}

/// Query parameters for batch listing
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchListQuery {
    pub limit: Option<i32>,
}

// ============================================================================
// STATE TRANSACTION ENDPOINTS
// ============================================================================

/// List state transactions with flexible filtering
#[utoipa::path(
    get,
    path = "/api/v1/state/transactions",
    params(StateTransactionsQuery),
    responses(
        (status = 200, description = "State transactions retrieved successfully", body = StateTransactionsResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_state_transactions_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<StateTransactionsQuery>,
) -> Result<Json<StateTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Listing state transactions with filters: pending={:?}, type={:?}, user_id={:?}, deal_id={:?}",
        query.pending, query.transaction_type, query.user_id, query.deal_id);

    // Validate and set defaults
    let limit = query.limit.unwrap_or(50).min(500).max(1);
    let offset = query.offset.unwrap_or(0).max(0);
    let include_decoded = query.include_decoded.unwrap_or(false);
    let order = query.order.as_deref().unwrap_or("desc");

    // Parse transaction types if provided
    let transaction_types: Option<Vec<String>> = query
        .transaction_type
        .as_ref()
        .map(|types| types.split(',').map(|s| s.trim().to_string()).collect());

    // Build filter params for DB layer
    let filter_params = arda_credit_db::StateTransactionFilters {
        pending: query.pending,
        batch_id: query.batch_id,
        transaction_types,
        user_id: query.user_id,
        deal_id: query.deal_id,
        originator_company_id: query.originator_company_id,
        investor_company_id: query.investor_company_id,
        company_id: query.company_id,
        collateral_id: query.collateral_id,
        limit: limit as i64,
        offset: offset as i64,
        order_desc: order == "desc",
    };

    // Query database
    match arda_credit_db::list_state_transactions(&state.pool, filter_params).await {
        Ok(state_txs) => {
            let total_count = state_txs.len();

            // Convert to response format
            let mut items = Vec::with_capacity(state_txs.len());
            for tx in state_txs {
                let pending = tx.included_in_batch_id.is_none();

                // Decode transaction_data if requested
                let (amount, metadata) = if include_decoded {
                    decode_transaction_data(&tx.transaction_data)
                } else {
                    (None, None)
                };

                items.push(StateTransactionItem {
                    id: tx.id.to_string(),
                    transaction_type: tx.transaction_type,
                    created_at: tx.created_at,
                    updated_at: tx.updated_at,
                    included_in_batch_id: tx.included_in_batch_id,
                    pending,
                    user_id: tx.user_id.map(|u| u.to_string()),
                    deal_id: tx.deal_id.map(|d| d.to_string()),
                    originator_company_id: tx.originator_company_id.map(|c| c.to_string()),
                    investor_company_id: tx.investor_company_id.map(|c| c.to_string()),
                    collateral_id: tx.collateral_id.map(|c| c.to_string()),
                    amount,
                    metadata,
                });
            }

            let has_more = total_count >= limit as usize;

            let response = StateTransactionsResponse {
                transactions: items,
                total_count,
                pagination: StateTransactionsPagination {
                    limit,
                    offset,
                    has_more,
                },
            };

            info!("✅ API: Retrieved {} state transactions", total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to list state transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list state transactions: {}", e),
            ))
        }
    }
}

/// Decode transaction_data bytes to extract amount and metadata
fn decode_transaction_data(transaction_data: &[u8]) -> (Option<String>, Option<serde_json::Value>) {
    use arda_credit_lib::transactions::DealTransaction;
    use rust_decimal::Decimal;

    // Deserialize DealTransaction from bincode
    match bincode::serde::decode_from_slice::<DealTransaction, _>(
        transaction_data,
        bincode::config::standard(),
    ) {
        Ok((deal_tx, _)) => {
            // Convert amount from u64 (cents) to Decimal USDC
            let amount = Decimal::from(deal_tx.amount) / Decimal::from(1_000_000);
            let amount_str = format!("{:.6}", amount);

            // Extract metadata from MetadataArray
            let metadata = if deal_tx.metadata.as_bytes().iter().any(|&b| b != 0) {
                let metadata_bytes = deal_tx.metadata.as_bytes();
                if let Ok(metadata_str) = std::str::from_utf8(metadata_bytes) {
                    if metadata_str.trim_start().starts_with('{') {
                        serde_json::from_str(metadata_str).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            (Some(amount_str), metadata)
        }
        Err(e) => {
            warn!("Failed to decode transaction_data: {}", e);
            (None, None)
        }
    }
}

// ============================================================================
// BATCH MANAGEMENT ENDPOINTS
// ============================================================================

/// Get all batches
#[utoipa::path(
    get,
    path = "/api/v1/state/batches",
    responses(
        (status = 200, description = "Batches retrieved successfully", body = BatchListResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "State"
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
                    previous_state_root: b
                        .previous_state_root
                        .as_ref()
                        .map(|r| format!("0x{}", hex::encode(r))),
                    final_state_root: b
                        .final_state_root
                        .as_ref()
                        .map(|r| format!("0x{}", hex::encode(r))),
                    transaction_count: b.state_transaction_ids.len(),
                    proof_status: b.proof_status,
                    sindri_proof_id: b.sindri_proof_id,
                    created_at: b.created_at,
                    proven_at: b.proven_at,
                    posted_to_contract: b.posted_to_contract,
                    posted_to_contract_at: b.posted_to_contract_at,
                    contract_tx_hash: b.contract_tx_hash,
                    contract_prev_merkle_root: None,
                    contract_new_merkle_root: None,
                    contract_ready: None,
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
    tag = "State"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_batch_details_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Response {
    info!("🔍 API: Getting batch details for ID: {}", batch_id);

    match get_batch_by_id(&state.pool, batch_id).await {
        Ok(batch) => {
            // Try to fetch contract submission data (may not exist for all batches)
            let (contract_prev_root, contract_new_root, contract_ready) =
                match get_contract_submission_data(&state.pool, batch_id).await {
                    Ok(contract_data) => (
                        Some(contract_data.public.prev_merkle_root),
                        Some(contract_data.public.new_merkle_root),
                        Some(true),
                    ),
                    Err(_) => (None, None, Some(false)),
                };

            let response = BatchInfo {
                id: batch.id,
                previous_state_root: batch
                    .previous_state_root
                    .as_ref()
                    .map(|r| format!("0x{}", hex::encode(r))),
                final_state_root: batch
                    .final_state_root
                    .as_ref()
                    .map(|r| format!("0x{}", hex::encode(r))),
                transaction_count: batch.state_transaction_ids.len(),
                proof_status: batch.proof_status.clone(),
                sindri_proof_id: batch.sindri_proof_id,
                created_at: batch.created_at,
                proven_at: batch.proven_at,
                posted_to_contract: batch.posted_to_contract,
                posted_to_contract_at: batch.posted_to_contract_at,
                contract_tx_hash: batch.contract_tx_hash,
                contract_prev_merkle_root: contract_prev_root,
                contract_new_merkle_root: contract_new_root,
                contract_ready,
            };

            info!(
                "✅ API: Batch details retrieved: id={}, status={}, transactions={}, contract_ready={:?}",
                response.id, batch.proof_status, response.transaction_count, contract_ready
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
