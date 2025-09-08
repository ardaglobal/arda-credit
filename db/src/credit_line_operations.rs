//! Credit line database operations
//!
//! This module provides database operations for credit line management including:
//! - Credit line request CRUD operations
//! - Credit line evaluation and approval processing
//! - Active credit line management

use chrono::NaiveDateTime;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::DbResult;

/// Credit line request data structure
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditLineRequest {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub requested_amount: Decimal,
    pub requested_rate_apr: Option<Decimal>,
    pub requested_tenor_days: Option<i32>,
    pub collateral_basis: Option<Decimal>,
    pub ltv_ratio: Option<Decimal>, // In basis points
    pub status: Option<String>,
    pub matched_pool_id: Option<Uuid>,
    pub approved_amount: Option<Decimal>,
    pub approved_rate_apr: Option<Decimal>,
    pub evaluation_notes: Option<serde_json::Value>,
    pub rejection_reason: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

/// Active credit line data structure
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditLine {
    pub id: Uuid,
    pub request_id: Uuid,
    pub merchant_id: Uuid,
    pub pool_id: Uuid,
    pub credit_limit: Option<Decimal>,
    pub available_credit: Option<Decimal>,
    pub utilized_amount: Option<Decimal>,
    pub rate_apr: Option<Decimal>, // In basis points
    pub repayment_window_days: Option<i32>,
    pub status: Option<String>,
    pub opened_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
    pub closed_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

/// Credit line request creation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCreditLineRequestData {
    pub merchant_id: Uuid,
    pub requested_amount: Decimal,
    pub requested_rate_apr: Option<Decimal>,
    pub requested_tenor_days: i32,
    pub request_description: Option<String>,
}

/// Credit line status enumeration for type safety
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreditLineStatus {
    Submitted,
    Evaluating,
    Approved,
    Rejected,
    Expired,
}

impl CreditLineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitted => "SUBMITTED",
            Self::Evaluating => "EVALUATING",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Expired => "EXPIRED",
        }
    }
}

/// Create a new credit line request
pub async fn create_credit_line_request(
    pool: &PgPool,
    data: CreateCreditLineRequestData,
) -> DbResult<CreditLineRequest> {
    let request = sqlx::query_as!(
        CreditLineRequest,
        r#"
        INSERT INTO credit_line_requests (
            merchant_id, 
            requested_amount, 
            requested_rate_apr, 
            requested_tenor_days,
            status
        )
        VALUES ($1, $2, $3, $4, 'SUBMITTED')
        RETURNING 
            id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
            collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
            approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
        "#,
        data.merchant_id,
        data.requested_amount,
        data.requested_rate_apr,
        data.requested_tenor_days
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Get credit line request by ID
pub async fn get_credit_line_request_by_id(
    pool: &PgPool,
    request_id: Uuid,
) -> DbResult<Option<CreditLineRequest>> {
    let request = sqlx::query_as!(
        CreditLineRequest,
        r#"
        SELECT 
            id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
            collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
            approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
        FROM credit_line_requests 
        WHERE id = $1
        "#,
        request_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(request)
}

/// Get credit line requests by merchant
pub async fn get_credit_line_requests_by_merchant(
    pool: &PgPool,
    merchant_id: Uuid,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> DbResult<Vec<CreditLineRequest>> {
    let requests = if let Some(status) = status_filter {
        sqlx::query_as!(
            CreditLineRequest,
            r#"
            SELECT 
                id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
                collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
                approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
            FROM credit_line_requests 
            WHERE merchant_id = $1 AND status = $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            merchant_id,
            status,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            CreditLineRequest,
            r#"
            SELECT 
                id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
                collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
                approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
            FROM credit_line_requests 
            WHERE merchant_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            merchant_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    };

    Ok(requests)
}

/// Update credit line request status
pub async fn update_credit_line_request_status(
    pool: &PgPool,
    request_id: Uuid,
    status: CreditLineStatus,
    rejection_reason: Option<String>,
) -> DbResult<CreditLineRequest> {
    let request = sqlx::query_as!(
        CreditLineRequest,
        r#"
        UPDATE credit_line_requests
        SET 
            status = $2,
            rejection_reason = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
            collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
            approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
        "#,
        request_id,
        status.as_str(),
        rejection_reason
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Update credit line request with evaluation results
pub async fn update_credit_line_request_evaluation(
    pool: &PgPool,
    request_id: Uuid,
    collateral_basis: Decimal,
    ltv_ratio: Decimal,
    matched_pool_id: Option<Uuid>,
    evaluation_notes: Option<serde_json::Value>,
) -> DbResult<CreditLineRequest> {
    let evaluation_json = evaluation_notes;

    let request = sqlx::query_as!(
        CreditLineRequest,
        r#"
        UPDATE credit_line_requests
        SET 
            collateral_basis = $2,
            ltv_ratio = $3,
            matched_pool_id = $4,
            evaluation_notes = $5,
            status = 'EVALUATING',
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
            collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
            approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
        "#,
        request_id,
        collateral_basis,
        ltv_ratio,
        matched_pool_id,
        evaluation_json
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Approve credit line request
pub async fn approve_credit_line_request(
    pool: &PgPool,
    request_id: Uuid,
    approved_amount: Decimal,
    approved_rate_apr: Decimal,
    matched_pool_id: Uuid,
) -> DbResult<CreditLineRequest> {
    let request = sqlx::query_as!(
        CreditLineRequest,
        r#"
        UPDATE credit_line_requests
        SET 
            status = 'APPROVED',
            approved_amount = $2,
            approved_rate_apr = $3,
            matched_pool_id = $4,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, merchant_id, requested_amount, requested_rate_apr, requested_tenor_days,
            collateral_basis, ltv_ratio, status, matched_pool_id, approved_amount,
            approved_rate_apr, evaluation_notes, rejection_reason, created_at, updated_at
        "#,
        request_id,
        approved_amount,
        approved_rate_apr,
        matched_pool_id
    )
    .fetch_one(pool)
    .await?;

    Ok(request)
}

/// Create active credit line from approved request
pub async fn create_credit_line(
    pool: &PgPool,
    request_id: Uuid,
    merchant_id: Uuid,
    pool_id: Uuid,
    credit_limit: Decimal,
    rate_apr: Decimal,
    repayment_window_days: i32,
) -> DbResult<CreditLine> {
    let credit_line = sqlx::query_as!(
        CreditLine,
        r#"
        INSERT INTO credit_lines (
            request_id,
            merchant_id,
            pool_id,
            credit_limit,
            available_credit,
            utilized_amount,
            rate_apr,
            repayment_window_days,
            status,
            opened_at,
            expires_at
        )
        VALUES ($1, $2, $3, $4, $4, 0, $5, $6, 'ACTIVE', NOW(), NOW() + ($7::TEXT || ' days')::INTERVAL)
        RETURNING 
            id, request_id, merchant_id, pool_id, credit_limit, available_credit,
            utilized_amount, rate_apr, repayment_window_days, status, opened_at,
            expires_at, closed_at, created_at, updated_at
        "#,
        request_id,
        merchant_id,
        pool_id,
        credit_limit,
        rate_apr,
        repayment_window_days,
        &repayment_window_days.to_string()
    )
    .fetch_one(pool)
    .await?;

    Ok(credit_line)
}

/// Get credit line by ID
pub async fn get_credit_line_by_id(
    pool: &PgPool,
    credit_line_id: Uuid,
) -> DbResult<Option<CreditLine>> {
    let credit_line = sqlx::query_as!(
        CreditLine,
        r#"
        SELECT 
            id, request_id, merchant_id, pool_id, credit_limit, available_credit,
            utilized_amount, rate_apr, repayment_window_days, status, opened_at,
            expires_at, closed_at, created_at, updated_at
        FROM credit_lines 
        WHERE id = $1
        "#,
        credit_line_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(credit_line)
}

/// List active credit lines for merchant
pub async fn list_credit_lines_by_merchant(
    pool: &PgPool,
    merchant_id: Uuid,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> DbResult<Vec<CreditLine>> {
    let credit_lines = if let Some(status) = status_filter {
        sqlx::query_as!(
            CreditLine,
            r#"
            SELECT 
                id, request_id, merchant_id, pool_id, credit_limit, available_credit,
                utilized_amount, rate_apr, repayment_window_days, status, opened_at,
                expires_at, closed_at, created_at, updated_at
            FROM credit_lines 
            WHERE merchant_id = $1 AND status = $2
            ORDER BY opened_at DESC
            LIMIT $3 OFFSET $4
            "#,
            merchant_id,
            status,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            CreditLine,
            r#"
            SELECT 
                id, request_id, merchant_id, pool_id, credit_limit, available_credit,
                utilized_amount, rate_apr, repayment_window_days, status, opened_at,
                expires_at, closed_at, created_at, updated_at
            FROM credit_lines 
            WHERE merchant_id = $1
            ORDER BY opened_at DESC
            LIMIT $2 OFFSET $3
            "#,
            merchant_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    };

    Ok(credit_lines)
}

/// List all active credit lines (for admin/monitoring)
pub async fn list_active_credit_lines(
    pool: &PgPool,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> DbResult<Vec<CreditLine>> {
    let credit_lines = if let Some(status) = status_filter {
        sqlx::query_as!(
            CreditLine,
            r#"
            SELECT 
                id, request_id, merchant_id, pool_id, credit_limit, available_credit,
                utilized_amount, rate_apr, repayment_window_days, status, opened_at,
                expires_at, closed_at, created_at, updated_at
            FROM credit_lines 
            WHERE status = $1
            ORDER BY opened_at DESC
            LIMIT $2 OFFSET $3
            "#,
            status,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            CreditLine,
            r#"
            SELECT 
                id, request_id, merchant_id, pool_id, credit_limit, available_credit,
                utilized_amount, rate_apr, repayment_window_days, status, opened_at,
                expires_at, closed_at, created_at, updated_at
            FROM credit_lines
            ORDER BY opened_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?
    };

    Ok(credit_lines)
}

/// Update credit line utilization (drawdown/repayment)
pub async fn update_credit_line_utilization(
    pool: &PgPool,
    credit_line_id: Uuid,
    utilized_amount: Decimal,
    available_credit: Decimal,
) -> DbResult<CreditLine> {
    let credit_line = sqlx::query_as!(
        CreditLine,
        r#"
        UPDATE credit_lines
        SET 
            utilized_amount = $2,
            available_credit = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, request_id, merchant_id, pool_id, credit_limit, available_credit,
            utilized_amount, rate_apr, repayment_window_days, status, opened_at,
            expires_at, closed_at, created_at, updated_at
        "#,
        credit_line_id,
        utilized_amount,
        available_credit
    )
    .fetch_one(pool)
    .await?;

    Ok(credit_line)
}

/// Close credit line
pub async fn close_credit_line(
    pool: &PgPool,
    credit_line_id: Uuid,
    _reason: Option<&str>,
) -> DbResult<CreditLine> {
    let credit_line = sqlx::query_as!(
        CreditLine,
        r#"
        UPDATE credit_lines
        SET 
            status = 'CLOSED',
            closed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING 
            id, request_id, merchant_id, pool_id, credit_limit, available_credit,
            utilized_amount, rate_apr, repayment_window_days, status, opened_at,
            expires_at, closed_at, created_at, updated_at
        "#,
        credit_line_id
    )
    .fetch_one(pool)
    .await?;

    Ok(credit_line)
}

/// Get credit line statistics for a merchant
pub async fn get_merchant_credit_line_stats(
    pool: &PgPool,
    merchant_id: Uuid,
) -> DbResult<MerchantCreditLineStats> {
    let stats = sqlx::query!(
        r#"
        SELECT 
            COUNT(CASE WHEN status = 'ACTIVE' THEN 1 END) as active_lines,
            COUNT(CASE WHEN status = 'CLOSED' THEN 1 END) as closed_lines,
            COALESCE(SUM(CASE WHEN status = 'ACTIVE' THEN credit_limit END), 0) as total_credit_limit,
            COALESCE(SUM(CASE WHEN status = 'ACTIVE' THEN utilized_amount END), 0) as total_utilized,
            COALESCE(SUM(CASE WHEN status = 'ACTIVE' THEN available_credit END), 0) as total_available
        FROM credit_lines
        WHERE merchant_id = $1
        "#,
        merchant_id
    )
    .fetch_one(pool)
    .await?;

    Ok(MerchantCreditLineStats {
        merchant_id,
        active_lines: stats.active_lines.unwrap_or(0) as i64,
        closed_lines: stats.closed_lines.unwrap_or(0) as i64,
        total_credit_limit: stats.total_credit_limit.unwrap_or(Decimal::ZERO),
        total_utilized: stats.total_utilized.unwrap_or(Decimal::ZERO),
        total_available: stats.total_available.unwrap_or(Decimal::ZERO),
        utilization_percentage: if stats.total_credit_limit.unwrap_or(Decimal::ZERO) > Decimal::ZERO
        {
            ((stats
                .total_utilized
                .unwrap_or(Decimal::ZERO)
                .to_f64()
                .unwrap_or(0.0)
                / stats
                    .total_credit_limit
                    .unwrap_or(Decimal::ONE)
                    .to_f64()
                    .unwrap_or(1.0))
                * 100.0) as i32
        } else {
            0
        },
    })
}

/// Merchant credit line statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantCreditLineStats {
    pub merchant_id: Uuid,
    pub active_lines: i64,
    pub closed_lines: i64,
    pub total_credit_limit: Decimal,
    pub total_utilized: Decimal,
    pub total_available: Decimal,
    pub utilization_percentage: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{create_test_merchant, setup_test_db};

    #[tokio::test]
    async fn test_credit_line_request_lifecycle() {
        let pool = setup_test_db().await;
        let merchant = create_test_merchant(&pool)
            .await
            .expect("Failed to create test merchant");

        // Create credit line request
        let request_data = CreateCreditLineRequestData {
            merchant_id: merchant.id,
            requested_amount: rust_decimal::Decimal::from(5000000), // $50,000
            requested_rate_apr: Some(rust_decimal::Decimal::from(1200)), // 12%
            requested_tenor_days: 90,
            request_description: Some("Test credit line request".to_string()),
        };

        let request = create_credit_line_request(&pool, request_data)
            .await
            .expect("Failed to create credit line request");

        assert_eq!(request.merchant_id, merchant.id);
        assert_eq!(
            request.requested_amount,
            rust_decimal::Decimal::from(5000000)
        );
        assert_eq!(request.status, Some("SUBMITTED".to_string()));

        // Update with evaluation
        let updated_request = update_credit_line_request_evaluation(
            &pool,
            request.id,
            rust_decimal::Decimal::from(10000000), // $100,000 collateral basis
            rust_decimal::Decimal::from(5000),     // 50% LTV
            None,
            Some(serde_json::json!({"eligibility_score": 85})),
        )
        .await
        .expect("Failed to update evaluation");

        assert_eq!(updated_request.status, Some("EVALUATING".to_string()));
        assert_eq!(
            updated_request.collateral_basis,
            Some(rust_decimal::Decimal::from(10000000))
        );
        assert_eq!(
            updated_request.ltv_ratio,
            Some(rust_decimal::Decimal::from(5000))
        );

        // Get request by ID
        let fetched_request = get_credit_line_request_by_id(&pool, request.id)
            .await
            .expect("Failed to get request")
            .expect("Request not found");

        assert_eq!(fetched_request.id, request.id);
        assert_eq!(fetched_request.status, Some("EVALUATING".to_string()));
    }
}
