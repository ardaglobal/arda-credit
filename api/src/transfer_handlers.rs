//! Transfer API handlers for loan trading
//!
//! This module contains all loan transfer-related API endpoints including:
//! - Execute loan ownership transfers with payment settlement
//! - View transfer history for loans and users
//! - Cancel pending transfers
//! - Transfer validation and eligibility checking

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use metrics;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::trading_auth::extract_user_id_from_headers;
use crate::types::ApiState;
use arda_credit_db::{
    cancel_transfer, execute_loan_transfer, get_loan_transfer_history, get_user_transfer_history,
    validate_transfer_eligibility, ExecuteTransferRequest, LoanTransferHistory,
    TransferValidationRequest,
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to transfer loan ownership
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferRequest {
    pub new_lender_id: String,          // UUID of the buyer
    pub purchase_price: String,         // Decimal as string (e.g., "50000.00")
    pub transfer_notes: Option<String>, // Optional transfer notes
}

/// Response after successful loan transfer
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferResponse {
    pub transfer_id: String,                   // UUID of the transfer record
    pub loan_id: String,                       // UUID of the transferred loan
    pub previous_lender_id: String,            // UUID of the seller
    pub new_lender_id: String,                 // UUID of the buyer
    pub purchase_price: String,                // Amount paid for the loan
    pub transfer_date: DateTime<Utc>,          // When transfer occurred
    pub updated_loan: TransferredLoanDetails,  // Updated loan details
    pub payment_settlement: PaymentSettlement, // Payment details
}

/// Updated loan details after transfer
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransferredLoanDetails {
    pub loan_id: String,
    pub new_owner_id: String, // new lender_id
    pub loan_status: String,  // "active"
    pub last_updated: DateTime<Utc>,
}

/// Payment settlement details
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentSettlement {
    pub buyer_transaction_id: String,  // Financial transaction for buyer
    pub seller_transaction_id: String, // Financial transaction for seller
    pub settled_amount: String,        // Amount settled
    pub settlement_timestamp: DateTime<Utc>,
}

/// Transfer history item
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransferHistoryItem {
    pub transfer_id: String,
    pub loan_id: String,
    pub from_lender_id: String,
    pub to_lender_id: String,
    pub from_lender_email: String,
    pub to_lender_email: String,
    pub purchase_price: String,
    pub transfer_status: String, // pending, completed, failed, cancelled
    pub transfer_date: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Response for loan transfer history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoanTransferHistoryResponse {
    pub loan_id: String,
    pub transfers: Vec<TransferHistoryItem>,
    pub total_transfers: i32,
}

/// Response for user transfer history
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserTransferHistoryResponse {
    pub user_id: String,
    pub transfers: Vec<TransferHistoryItem>,
    pub total_count: i64,
    pub has_more: bool,
}

/// Query parameters for transfer history
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TransferHistoryQuery {
    pub limit: Option<i64>,  // Pagination limit (default: 50, max: 100)
    pub offset: Option<i64>, // Pagination offset (default: 0)
}

/// Response for transfer cancellation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CancelTransferResponse {
    pub transfer_id: String,
    pub cancelled_at: DateTime<Utc>,
    pub status: String, // "cancelled"
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Parse decimal from string with validation
fn parse_decimal(s: &str, field_name: &str) -> Result<Decimal, (StatusCode, String)> {
    s.parse::<Decimal>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid {} format: {}", field_name, s),
        )
    })
}

/// Convert database transfer history to API response format
fn transfer_history_to_api(history: LoanTransferHistory) -> TransferHistoryItem {
    TransferHistoryItem {
        transfer_id: history.transfer_id.to_string(),
        loan_id: history.loan_id.to_string(),
        from_lender_id: history.from_lender_id.to_string(),
        to_lender_id: history.to_lender_id.to_string(),
        from_lender_email: history.from_lender_email,
        to_lender_email: history.to_lender_email,
        purchase_price: history.purchase_price.to_string(),
        transfer_status: history.transfer_status,
        transfer_date: DateTime::<Utc>::from_naive_utc_and_offset(history.transfer_date, Utc),
        completed_at: history
            .completed_at
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
        notes: history.notes,
    }
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Transfer loan ownership with payment settlement
#[utoipa::path(
    post,
    path = "/api/v1/loan/{loan_id}/transfer",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan to transfer")
    ),
    request_body = LoanTransferRequest,
    responses(
        (status = 200, description = "Loan transferred successfully", body = LoanTransferResponse),
        (status = 400, description = "Invalid request parameters or insufficient funds"),
        (status = 401, description = "Unauthorized - user not authenticated"),
        (status = 403, description = "Forbidden - user not KYC approved"),
        (status = 404, description = "Loan not found or not for sale"),
        (status = 409, description = "Transfer conflict (already in progress, etc.)"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn transfer_loan_ownership_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    headers: HeaderMap,
    Json(request): Json<LoanTransferRequest>,
) -> Result<Json<LoanTransferResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Record metrics
    metrics::counter!("loan_transfer_requests_total").increment(1);

    info!(
        "🔄 TRANSFER: Processing loan transfer - loan={}, buyer={}, price={}",
        loan_id_str, request.new_lender_id, request.purchase_price
    );

    // Parse and validate input
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    let buyer_id = Uuid::parse_str(&request.new_lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid buyer ID format".to_string(),
        )
    })?;

    let purchase_price = parse_decimal(&request.purchase_price, "purchase_price")?;

    // Validate purchase price is positive
    if purchase_price <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            "Purchase price must be positive".to_string(),
        ));
    }

    // Authenticate user - must be the buyer making the transfer
    let authenticated_user_id = extract_user_id_from_headers(&headers)?;

    // Verify the authenticated user is the buyer specified in the request
    if authenticated_user_id != buyer_id {
        return Err((
            StatusCode::FORBIDDEN,
            "You can only execute transfers where you are the buyer".to_string(),
        ));
    }

    // Create transfer request
    let transfer_request = ExecuteTransferRequest {
        loan_id,
        buyer_id,
        purchase_price,
        notes: request.transfer_notes,
    };

    // Execute transfer using your database function
    let transfer_result = execute_loan_transfer(&state.pool, &transfer_request)
        .await
        .map_err(|e| {
            error!("❌ Failed to execute loan transfer: {}", e);

            // Map specific database errors to HTTP status codes
            match e {
                sqlx::Error::Database(ref db_err) => {
                    let code = db_err
                        .code()
                        .map(|c| c.as_ref().to_string())
                        .unwrap_or_else(|| "".to_string());
                    match code.as_str() {
                        "23514" => {
                            metrics::counter!("transfer_errors_total", "type" => "validation")
                                .increment(1);
                            if db_err.message().contains("validation failed") {
                                (StatusCode::BAD_REQUEST, db_err.message().to_string())
                            } else if db_err.message().contains("insufficient") {
                                (
                                    StatusCode::BAD_REQUEST,
                                    "Insufficient balance for transfer".to_string(),
                                )
                            } else {
                                (
                                    StatusCode::CONFLICT,
                                    "Transfer validation failed".to_string(),
                                )
                            }
                        }
                        "23505" => {
                            metrics::counter!("transfer_errors_total", "type" => "conflict")
                                .increment(1);
                            (
                                StatusCode::CONFLICT,
                                "Transfer already in progress".to_string(),
                            )
                        }
                        _ => {
                            metrics::counter!("transfer_errors_total", "type" => "database")
                                .increment(1);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Database error during transfer".to_string(),
                            )
                        }
                    }
                }
                _ => {
                    metrics::counter!("transfer_errors_total", "type" => "unknown").increment(1);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to execute transfer".to_string(),
                    )
                }
            }
        })?;

    let response = LoanTransferResponse {
        transfer_id: transfer_result.transfer_id.to_string(),
        loan_id: transfer_result.loan_id.to_string(),
        previous_lender_id: transfer_result.from_lender_id.to_string(),
        new_lender_id: transfer_result.to_lender_id.to_string(),
        purchase_price: transfer_result.purchase_price.to_string(),
        transfer_date: DateTime::<Utc>::from_naive_utc_and_offset(
            transfer_result.transfer_date,
            Utc,
        ),
        updated_loan: TransferredLoanDetails {
            loan_id: transfer_result.loan_id.to_string(),
            new_owner_id: transfer_result.to_lender_id.to_string(),
            loan_status: "active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        payment_settlement: PaymentSettlement {
            buyer_transaction_id: transfer_result.settlement.buyer_transaction_id.to_string(),
            seller_transaction_id: transfer_result.settlement.seller_transaction_id.to_string(),
            settled_amount: transfer_result.settlement.settled_amount.to_string(),
            settlement_timestamp: DateTime::<Utc>::from_naive_utc_and_offset(
                transfer_result.settlement.settlement_timestamp,
                Utc,
            ),
        },
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("loan_transfer_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("loan_transfers_completed_total").increment(1);
    metrics::histogram!("loan_transfer_amount_usdc")
        .record(transfer_result.purchase_price.to_f64().unwrap_or(0.0));

    info!(
        "✅ TRANSFER: Loan {} transferred successfully from {} to {} for ${} ({}ms)",
        loan_id,
        transfer_result.from_lender_id,
        transfer_result.to_lender_id,
        transfer_result.purchase_price,
        duration.as_millis()
    );

    Ok(Json(response))
}

/// Get loan transfer history
#[utoipa::path(
    get,
    path = "/api/v1/loan/{loan_id}/transfers",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan")
    ),
    responses(
        (status = 200, description = "Transfer history retrieved successfully", body = LoanTransferHistoryResponse),
        (status = 400, description = "Invalid loan ID format"),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_loan_transfer_history_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    headers: HeaderMap,
) -> Result<Json<LoanTransferHistoryResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    info!(
        "📋 TRANSFER: Getting transfer history for loan {}",
        loan_id_str
    );

    // Parse loan ID
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Authenticate user
    let _authenticated_user_id = extract_user_id_from_headers(&headers)?;

    // Get transfer history using your database function
    let history = get_loan_transfer_history(&state.pool, loan_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get loan transfer history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve transfer history".to_string(),
            )
        })?;

    let transfers: Vec<TransferHistoryItem> =
        history.into_iter().map(transfer_history_to_api).collect();

    let response = LoanTransferHistoryResponse {
        loan_id: loan_id.to_string(),
        total_transfers: transfers.len() as i32,
        transfers,
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("transfer_history_query_duration_seconds").record(duration.as_secs_f64());
    metrics::counter!("transfer_history_queries_total").increment(1);
    metrics::histogram!("transfer_history_results_count").record(response.total_transfers as f64);

    info!(
        "✅ TRANSFER: Retrieved {} transfers for loan {} ({}ms)",
        response.total_transfers,
        loan_id,
        duration.as_millis()
    );

    Ok(Json(response))
}

/// Get user transfer history (loans bought or sold)
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}/transfers",
    params(
        ("user_id" = String, Path, description = "UUID of the user"),
        TransferHistoryQuery
    ),
    responses(
        (status = 200, description = "User transfer history retrieved successfully", body = UserTransferHistoryResponse),
        (status = 400, description = "Invalid user ID format or query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_transfer_history_endpoint(
    State(state): State<ApiState>,
    Path(user_id_str): Path<String>,
    headers: HeaderMap,
    Query(query): Query<TransferHistoryQuery>,
) -> Result<Json<UserTransferHistoryResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    info!(
        "👤 TRANSFER: Getting transfer history for user {}",
        user_id_str
    );

    // Parse user ID
    let user_id = Uuid::parse_str(&user_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid user ID format".to_string(),
        )
    })?;

    // Authenticate user - can only view their own transfer history
    let authenticated_user_id = extract_user_id_from_headers(&headers)?;

    // Verify the authenticated user is requesting their own history
    if authenticated_user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "You can only view your own transfer history".to_string(),
        ));
    }

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // Get user transfer history using your database function
    let history = get_user_transfer_history(&state.pool, user_id, Some(limit), Some(offset))
        .await
        .map_err(|e| {
            error!("❌ Failed to get user transfer history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve user transfer history".to_string(),
            )
        })?;

    let transfers: Vec<TransferHistoryItem> =
        history.into_iter().map(transfer_history_to_api).collect();

    // Calculate pagination
    let has_more = transfers.len() as i64 == limit;

    let response = UserTransferHistoryResponse {
        user_id: user_id.to_string(),
        total_count: transfers.len() as i64,
        has_more,
        transfers,
    };

    // Record metrics
    let duration = start_time.elapsed();
    metrics::histogram!("user_transfer_history_query_duration_seconds")
        .record(duration.as_secs_f64());

    info!(
        "✅ TRANSFER: Retrieved {} transfers for user {} ({}ms)",
        response.transfers.len(),
        user_id,
        duration.as_millis()
    );

    Ok(Json(response))
}

/// Validate transfer eligibility (utility endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/loan/{loan_id}/transfer/validate",
    params(
        ("loan_id" = String, Path, description = "UUID of the loan")
    ),
    request_body = LoanTransferRequest,
    responses(
        (status = 200, description = "Transfer validation result", body = serde_json::Value),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transfer"
)]
#[instrument(skip(state), level = "info")]
pub async fn validate_transfer_endpoint(
    State(state): State<ApiState>,
    Path(loan_id_str): Path<String>,
    headers: HeaderMap,
    Json(request): Json<LoanTransferRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("🔍 TRANSFER: Validating transfer for loan {}", loan_id_str);

    // Record validation request metric
    metrics::counter!("transfer_validation_requests_total").increment(1);

    // Parse inputs
    let loan_id = Uuid::parse_str(&loan_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    let buyer_id = Uuid::parse_str(&request.new_lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid buyer ID format".to_string(),
        )
    })?;

    let purchase_price = parse_decimal(&request.purchase_price, "purchase_price")?;

    // Authenticate user - must be the buyer validating the transfer
    let authenticated_user_id = extract_user_id_from_headers(&headers)?;

    // Verify the authenticated user is the buyer specified in the request
    if authenticated_user_id != buyer_id {
        return Err((
            StatusCode::FORBIDDEN,
            "You can only validate transfers where you are the buyer".to_string(),
        ));
    }

    // Validate transfer using your database function
    let validation = validate_transfer_eligibility(
        &state.pool,
        &TransferValidationRequest {
            loan_id,
            buyer_id,
            purchase_price,
        },
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to validate transfer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to validate transfer".to_string(),
        )
    })?;

    // Return validation result as JSON
    let response = serde_json::json!({
        "is_valid": validation.is_valid,
        "loan_exists": validation.loan_exists,
        "loan_for_sale": validation.loan_for_sale,
        "buyer_has_funds": validation.buyer_has_funds,
        "buyer_is_not_owner": validation.buyer_is_not_owner,
        "buyer_kyc_approved": validation.buyer_kyc_approved,
        "price_matches": validation.price_matches,
        "available_balance": validation.available_balance.map(|b| b.to_string()),
        "validation_errors": validation.validation_errors,
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}
