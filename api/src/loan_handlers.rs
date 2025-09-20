//! Loan management handlers for the Arda Credit API
//!
//! This module contains all loan-related API endpoints including:
//! - Loan creation and management
//! - Loan payment processing
//! - Loan statistics and reporting
//! - Loan lifecycle tracking
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use metrics;
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::types::ApiState;
use arda_credit_db::{create_loan, get_loan_by_id, list_loans, update_loan};
use uuid::Uuid;

// ============================================================================
// LOAN MANAGEMENT TYPES (MOVED FROM rest.rs)
// ============================================================================

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
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
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

/// Update loan request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLoanRequest {
    pub principal_amount: Option<String>, // Amount in cents as string
    pub outstanding_balance: Option<String>, // Outstanding balance in cents as string
    pub interest_rate_bps: Option<u16>,   // Interest rate in basis points
    pub loan_status: Option<String>, // "originated", "active", "paid_off", "closed", "defaulted"
    pub maturity_date: Option<Option<String>>, // Optional maturity date (ISO 8601), Option<Option<T>> to handle NULL
}

/// Update loan response
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateLoanResponse {
    pub loan_id: String,
    pub updated_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub loan_details: LoanDetailsResponse,
}

/// Delete loan response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteLoanResponse {
    pub loan_id: String,
    pub deleted: bool,
    pub deleted_at: DateTime<Utc>,
    pub final_status: String,
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

// ============================================================================
// LOAN MANAGEMENT ENDPOINTS
// ============================================================================

/// Create a new loan
#[utoipa::path(
    post,
    path = "/api/v1/loan",
    request_body = CreateLoanRequest,
    responses(
        (status = 200, description = "Loan created successfully", body = LoanDetailsResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn create_loan_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateLoanRequest>,
) -> Result<Json<LoanDetailsResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    
    // Record business logic operation
    metrics::counter!("business_logic_operations_total", "operation" => "create_loan").increment(1);
    
    info!(
        "💰 API: Creating loan: borrower={}, lender={}, amount={}",
        request.borrower_id, request.lender_id, request.principal_amount
    );

    // Parse UUIDs
    let borrower_id = uuid::Uuid::parse_str(&request.borrower_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid borrower ID format".to_string(),
        )
    })?;
    let lender_id = uuid::Uuid::parse_str(&request.lender_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid lender ID format".to_string(),
        )
    })?;

    // Parse principal amount
    let principal_amount = request
        .principal_amount
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?;

    // Parse optional maturity date
    let maturity_date = if let Some(term_days) = request.term_days {
        Some(chrono::Utc::now() + chrono::Duration::days(term_days as i64))
    } else {
        None
    };

    // Create loan in database
    let loan = create_loan(
        &state.pool,
        borrower_id,
        lender_id,
        principal_amount,
        request.interest_rate_bps as i32,
        maturity_date,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create loan: {}", e);
        metrics::counter!("business_logic_errors_total", "operation" => "create_loan").increment(1);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create loan".to_string(),
        )
    })?;

    let response = LoanDetailsResponse {
        loan_id: loan.id.to_string(),
        borrower_id: loan.borrower_id.to_string(),
        lender_id: loan.lender_id.to_string(),
        principal_amount: loan.principal_amount.to_string(),
        outstanding_balance: loan.outstanding_balance.to_string(),
        interest_rate_bps: loan.interest_rate_bps as u16,
        loan_status: loan.loan_status,
        originated_at: loan.originated_at,
        last_updated: Some(loan.updated_at),
    };

    // Record function execution time
    let duration = start_time.elapsed();
    metrics::histogram!("function_execution_duration_seconds", "function" => "create_loan")
        .record(duration.as_secs_f64());
    
    info!("✅ API: Loan created successfully: {}", response.loan_id);
    Ok(Json(response))
}

/// List loans
#[utoipa::path(
    get,
    path = "/api/v1/loans",
    responses(
        (status = 200, description = "Loans listed successfully", body = ListLoansResponse),
        (status = 500, description = "Internal server error")
    ),
    params(ListLoansQuery),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn list_loans_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<ListLoansQuery>,
) -> Result<Json<ListLoansResponse>, (StatusCode, String)> {
    info!("📋 API: Listing loans with filters");

    // Parse optional UUIDs
    let borrower_id = if let Some(id_str) = &params.borrower_id {
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid borrower ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    let lender_id = if let Some(id_str) = &params.lender_id {
        Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid lender ID format".to_string(),
            )
        })?)
    } else {
        None
    };

    // Get loans from database
    let loans = list_loans(
        &state.pool,
        params.limit.map(|l| l as i64),
        params.offset.map(|o| o as i64),
        params.status.as_deref(),
        borrower_id,
        lender_id,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to list loans: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list loans".to_string(),
        )
    })?;

    // Convert to response format
    let loan_responses: Vec<LoanDetailsResponse> = loans
        .into_iter()
        .map(|loan| LoanDetailsResponse {
            loan_id: loan.id.to_string(),
            borrower_id: loan.borrower_id.to_string(),
            lender_id: loan.lender_id.to_string(),
            principal_amount: loan.principal_amount.to_string(),
            outstanding_balance: loan.outstanding_balance.to_string(),
            interest_rate_bps: loan.interest_rate_bps as u16,
            loan_status: loan.loan_status,
            originated_at: loan.originated_at,
            last_updated: Some(loan.updated_at),
        })
        .collect();

    let total_count = loan_responses.len();
    let limit = params.limit.unwrap_or(100) as usize;
    let has_more = total_count >= limit;

    let response = ListLoansResponse {
        loans: loan_responses,
        total_count,
        has_more,
    };

    info!("✅ API: Listed {} loans", response.total_count);
    Ok(Json(response))
}

/// Get loan details by ID
#[utoipa::path(
    get,
    path = "/api/v1/loan/{loan_id}",
    responses(
        (status = 200, description = "Loan details retrieved successfully", body = LoanDetailsResponse),
        (status = 404, description = "Loan not found"),
        (status = 400, description = "Invalid loan ID format"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("loan_id" = String, Path, description = "Loan ID")
    ),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_loan_details_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<String>,
) -> Result<Json<LoanDetailsResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting loan details: loan_id={}", loan_id);

    // Parse loan ID
    let loan_uuid = Uuid::parse_str(&loan_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Get loan from database
    let loan = get_loan_by_id(&state.pool, loan_uuid).await.map_err(|e| {
        error!("❌ Failed to get loan: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get loan".to_string(),
        )
    })?;

    match loan {
        Some(loan) => {
            let response = LoanDetailsResponse {
                loan_id: loan.id.to_string(),
                borrower_id: loan.borrower_id.to_string(),
                lender_id: loan.lender_id.to_string(),
                principal_amount: loan.principal_amount.to_string(),
                outstanding_balance: loan.outstanding_balance.to_string(),
                interest_rate_bps: loan.interest_rate_bps as u16,
                loan_status: loan.loan_status,
                originated_at: loan.originated_at,
                last_updated: Some(loan.updated_at),
            };

            info!("✅ API: Loan details retrieved");
            Ok(Json(response))
        }
        None => {
            warn!("❌ API: Loan not found: {}", loan_id);
            Err((StatusCode::NOT_FOUND, "Loan not found".to_string()))
        }
    }
}

/// Get loan statistics
#[utoipa::path(
    get,
    path = "/api/v1/loans/stats",
    responses(
        (status = 200, description = "Loan statistics retrieved successfully", body = LoanStatsResponse),
        (status = 500, description = "Internal server error")
    ),
    params(ListLoansQuery),
    tag = "Loans"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_loan_stats_endpoint(
    State(_state): State<ApiState>,
    Query(_params): Query<ListLoansQuery>, // Reuse query params for filtering
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

/// Update loan
#[utoipa::path(
    put,
    path = "/api/v1/loan/{loan_id}",
    request_body = UpdateLoanRequest,
    responses(
        (status = 200, description = "Loan updated successfully", body = UpdateLoanResponse),
        (status = 400, description = "Invalid request data"),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("loan_id" = String, Path, description = "Loan ID")
    ),
    tag = "Loans"
)]
#[instrument(skip(state), level = "info")]
pub async fn update_loan_endpoint(
    State(state): State<ApiState>,
    Path(loan_id): Path<String>,
    Json(request): Json<UpdateLoanRequest>,
) -> Result<Json<UpdateLoanResponse>, (StatusCode, String)> {
    info!("📝 API: Updating loan: {}", loan_id);

    // Parse loan ID
    let loan_uuid = Uuid::parse_str(&loan_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // Convert request fields to database types
    let principal_amount = if let Some(amount_str) = &request.principal_amount {
        Some(amount_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid principal amount".to_string(),
            )
        })?)
    } else {
        None
    };

    let outstanding_balance = if let Some(balance_str) = &request.outstanding_balance {
        Some(balance_str.parse::<rust_decimal::Decimal>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid outstanding balance".to_string(),
            )
        })?)
    } else {
        None
    };

    let interest_rate_bps = request.interest_rate_bps.map(|rate| rate as i32);

    let maturity_date = if let Some(date_opt) = &request.maturity_date {
        Some(match date_opt {
            Some(date_str) => Some(
                chrono::DateTime::parse_from_rfc3339(date_str)
                    .map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            "Invalid maturity date format".to_string(),
                        )
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        })
    } else {
        None
    };

    // Update the loan in the database
    let updated_loan = update_loan(
        &state.pool,
        loan_uuid,
        principal_amount,
        outstanding_balance,
        interest_rate_bps,
        request.loan_status.as_deref(),
        maturity_date,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to update loan: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update loan".to_string(),
        )
    })?;

    match updated_loan {
        Some(loan) => {
            // Track which fields were updated
            let mut updated_fields = Vec::new();
            if request.principal_amount.is_some() {
                updated_fields.push("principal_amount".to_string());
            }
            if request.outstanding_balance.is_some() {
                updated_fields.push("outstanding_balance".to_string());
            }
            if request.interest_rate_bps.is_some() {
                updated_fields.push("interest_rate_bps".to_string());
            }
            if request.loan_status.is_some() {
                updated_fields.push("loan_status".to_string());
            }
            if request.maturity_date.is_some() {
                updated_fields.push("maturity_date".to_string());
            }

            // Convert to response format
            let loan_details = LoanDetailsResponse {
                loan_id: loan.id.to_string(),
                borrower_id: loan.borrower_id.to_string(),
                lender_id: loan.lender_id.to_string(),
                principal_amount: loan.principal_amount.to_string(),
                outstanding_balance: loan.outstanding_balance.to_string(),
                interest_rate_bps: loan.interest_rate_bps as u16,
                loan_status: loan.loan_status,
                originated_at: loan.originated_at,
                last_updated: Some(loan.updated_at),
            };

            let response = UpdateLoanResponse {
                loan_id: loan.id.to_string(),
                updated_fields,
                updated_at: loan.updated_at,
                loan_details,
            };

            info!("✅ API: Loan updated successfully: {}", loan_id);
            Ok(Json(response))
        }
        None => {
            warn!("❌ API: Loan not found: {}", loan_id);
            Err((StatusCode::NOT_FOUND, "Loan not found".to_string()))
        }
    }
}

/// Delete/close loan
#[utoipa::path(
    delete,
    path = "/api/v1/loan/{loan_id}",
    responses(
        (status = 200, description = "Loan closed successfully", body = DeleteLoanResponse),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("loan_id" = String, Path, description = "Loan ID")
    ),
    tag = "Loans"
)]
#[instrument(skip(_state), level = "info")]
pub async fn delete_loan_endpoint(
    State(_state): State<ApiState>,
    Path(loan_id): Path<String>,
) -> Result<Json<DeleteLoanResponse>, (StatusCode, String)> {
    info!("🗑️ API: Closing loan: loan_id={}", loan_id);

    let _loan_uuid = Uuid::parse_str(&loan_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid loan ID format".to_string(),
        )
    })?;

    // TODO: Implement loan closure with database integration
    // For now, simulate successful closure
    let _updated_rows = 1; // Simulate one row updated

    let response = DeleteLoanResponse {
        loan_id,
        deleted: true,
        deleted_at: Utc::now(),
        final_status: "closed".to_string(),
    };

    info!("✅ API: Loan closed successfully");
    Ok(Json(response))
}

/// Record a loan payment
#[utoipa::path(
    post,
    path = "/api/v1/payments",
    request_body = RecordPaymentRequest,
    responses(
        (status = 200, description = "Payment recorded successfully", body = PaymentResponse),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn record_payment_endpoint(
    State(_state): State<ApiState>,
    Json(request): Json<RecordPaymentRequest>,
) -> Result<Json<PaymentResponse>, (StatusCode, String)> {
    info!(
        "💳 API: Recording payment: loan_id={}, amount={}, type={}",
        request.loan_id, request.amount, request.payment_type
    );

    // TODO: Implement payment recording with database integration
    // TODO: When implementing payment recording, also decrease borough allocation:
    // if let Some((pool_id, merchant_borough)) = get_loan_pool_and_borough(&request.loan_id).await {
    //     let payment_amount = Decimal::from_str(&request.amount).unwrap_or(Decimal::ZERO);
    //     match arda_credit_db::update_borough_allocation(
    //         &state.pool,
    //         pool_id,
    //         &merchant_borough,
    //         -payment_amount,  // Negative for payment (reduces allocation)
    //     ).await {
    //         Ok(()) => info!("Reduced borough allocation by payment amount"),
    //         Err(e) => error!("Failed to update borough allocation for payment: {}", e),
    //     }
    // }

    let transaction_id = Uuid::new_v4().to_string();

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
#[utoipa::path(
    get,
    path = "/api/v1/payment/{transaction_id}",
    responses(
        (status = 200, description = "Payment details retrieved successfully", body = PaymentDetailsResponse),
        (status = 404, description = "Payment not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("transaction_id" = String, Path, description = "Transaction ID")
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_payment_details_endpoint(
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
        loan_id: Uuid::new_v4().to_string(),
        amount: "25000".to_string(),
        payment_type: "Principal".to_string(),
        transaction_status: "Processed".to_string(),
        batch_id: Some(Uuid::new_v4().to_string()),
        created_at: Utc::now(),
        processed_at: Some(Utc::now()),
    };

    info!("✅ API: Payment details retrieved");
    Ok(Json(response))
}

/// Get loan payment history
#[utoipa::path(
    get,
    path = "/api/v1/loan/{loan_id}/payments",
    responses(
        (status = 200, description = "Payment history retrieved successfully", body = LoanPaymentHistoryResponse),
        (status = 404, description = "Loan not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("loan_id" = String, Path, description = "Loan ID"),
        ListLoansQuery
    ),
    tag = "Payments"
)]
#[instrument(skip(_state), level = "info")]
pub async fn get_loan_payment_history_endpoint(
    State(_state): State<ApiState>,
    Path(loan_id): Path<String>,
    Query(_params): Query<ListLoansQuery>, // Reuse query params for limit/offset
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
