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
use utoipa::{IntoParams, ToSchema};

use crate::paths;
use crate::types::ApiState;
use arda_credit_db::{
    create_loan, get_loan_by_id, list_loans, update_loan,
};

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
