//! Metrics database operations
//!
//! This module provides database operations for cached metrics data:
//! - Market value time series data for cumulative growth charts
//! - Pie chart distribution data for industry, region, and asset class breakdowns
//! - Background job operations for daily metric recalculation

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::DbResult;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Market value daily record from database for time series charts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct MetricsMarketValueDaily {
    pub id: Uuid,
    pub date_value: NaiveDate,
    pub growth_type: String,     // market_growth | arda_growth
    pub filter_type: String,     // all | originator | investor
    pub entity_id: Option<Uuid>, // company_id for originator/investor filters
    pub cumulative_value: Decimal,
    pub deal_count: i32,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pie chart distribution data record from database
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct MetricsPieChartData {
    pub id: Uuid,
    pub chart_type: String,      // industry | region | asset_class
    pub filter_type: String,     // all | originator | investor
    pub entity_id: Option<Uuid>, // company_id for originator/investor filters
    pub time_range: String,      // all_time | last_year
    pub category: String,        // actual value like "Technology", "Real Estate", etc.
    pub value: Decimal,          // sum of principal amounts for this category
    pub percentage: Decimal,     // calculated percentage
    pub deal_count: i32,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request structure for creating market value metrics
#[derive(Debug, Clone)]
pub struct CreateMarketValueMetricRequest {
    pub date_value: NaiveDate,
    pub growth_type: String,
    pub filter_type: String,
    pub entity_id: Option<Uuid>,
    pub cumulative_value: Decimal,
    pub deal_count: i32,
}

/// Request structure for creating pie chart metrics
#[derive(Debug, Clone)]
pub struct CreatePieChartMetricRequest {
    pub chart_type: String,
    pub filter_type: String,
    pub entity_id: Option<Uuid>,
    pub time_range: String,
    pub category: String,
    pub value: Decimal,
    pub percentage: Decimal,
    pub deal_count: i32,
}

// ============================================================================
// MARKET VALUE METRICS OPERATIONS
// ============================================================================

/// Insert or update a market value daily metric
pub async fn upsert_market_value_metric(
    pool: &PgPool,
    request: CreateMarketValueMetricRequest,
) -> DbResult<MetricsMarketValueDaily> {
    let now = Utc::now();

    let record = sqlx::query_as!(
        MetricsMarketValueDaily,
        r#"
        INSERT INTO metrics_market_value_daily 
        (date_value, growth_type, filter_type, entity_id, cumulative_value, deal_count, calculated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (date_value, growth_type, filter_type, entity_id) 
        DO UPDATE SET 
            cumulative_value = EXCLUDED.cumulative_value,
            deal_count = EXCLUDED.deal_count,
            calculated_at = EXCLUDED.calculated_at,
            updated_at = now()
        RETURNING *
        "#,
        request.date_value,
        request.growth_type,
        request.filter_type,
        request.entity_id,
        request.cumulative_value,
        request.deal_count,
        now
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get market value metrics for a specific filter and date range
pub async fn get_market_value_metrics(
    pool: &PgPool,
    growth_type: &str,
    filter_type: &str,
    entity_id: Option<Uuid>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> DbResult<Vec<MetricsMarketValueDaily>> {
    let mut query = String::from(
        "SELECT * FROM metrics_market_value_daily 
         WHERE growth_type = $1 AND filter_type = $2",
    );

    let mut param_count = 2;

    // Handle entity_id filtering
    if filter_type == "all" {
        query.push_str(" AND entity_id IS NULL");
    } else {
        param_count += 1;
        query.push_str(&format!(" AND entity_id = ${}", param_count));
    }

    // Add date range filtering if provided
    if start_date.is_some() {
        param_count += 1;
        query.push_str(&format!(" AND date_value >= ${}", param_count));
    }
    if end_date.is_some() {
        param_count += 1;
        query.push_str(&format!(" AND date_value <= ${}", param_count));
    }

    query.push_str(" ORDER BY date_value ASC");

    let mut query_builder = sqlx::query_as::<Postgres, MetricsMarketValueDaily>(&query)
        .bind(growth_type)
        .bind(filter_type);

    if filter_type != "all" {
        query_builder = query_builder.bind(entity_id);
    }

    if let Some(start) = start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = end_date {
        query_builder = query_builder.bind(end);
    }

    let records = query_builder.fetch_all(pool).await?;
    Ok(records)
}

/// Delete market value metrics older than the specified date
pub async fn delete_old_market_value_metrics(
    pool: &PgPool,
    before_date: NaiveDate,
) -> DbResult<u64> {
    let result = sqlx::query!(
        "DELETE FROM metrics_market_value_daily WHERE date_value < $1",
        before_date
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

// ============================================================================
// PIE CHART METRICS OPERATIONS
// ============================================================================

/// Insert or update a pie chart metric
pub async fn upsert_pie_chart_metric(
    pool: &PgPool,
    request: CreatePieChartMetricRequest,
) -> DbResult<MetricsPieChartData> {
    let now = Utc::now();

    let record = sqlx::query_as!(
        MetricsPieChartData,
        r#"
        INSERT INTO metrics_pie_chart_data 
        (chart_type, filter_type, entity_id, time_range, category, value, percentage, deal_count, calculated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (chart_type, filter_type, entity_id, time_range, category) 
        DO UPDATE SET 
            value = EXCLUDED.value,
            percentage = EXCLUDED.percentage,
            deal_count = EXCLUDED.deal_count,
            calculated_at = EXCLUDED.calculated_at,
            updated_at = now()
        RETURNING *
        "#,
        request.chart_type,
        request.filter_type,
        request.entity_id,
        request.time_range,
        request.category,
        request.value,
        request.percentage,
        request.deal_count,
        now
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get pie chart metrics for a specific chart type and filter
pub async fn get_pie_chart_metrics(
    pool: &PgPool,
    chart_type: &str,
    filter_type: &str,
    entity_id: Option<Uuid>,
    time_range: &str,
) -> DbResult<Vec<MetricsPieChartData>> {
    let records = if filter_type == "all" {
        sqlx::query_as!(
            MetricsPieChartData,
            "SELECT * FROM metrics_pie_chart_data 
             WHERE chart_type = $1 AND filter_type = $2 AND entity_id IS NULL AND time_range = $3
             ORDER BY value DESC",
            chart_type,
            filter_type,
            time_range
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            MetricsPieChartData,
            "SELECT * FROM metrics_pie_chart_data 
             WHERE chart_type = $1 AND filter_type = $2 AND entity_id = $3 AND time_range = $4
             ORDER BY value DESC",
            chart_type,
            filter_type,
            entity_id,
            time_range
        )
        .fetch_all(pool)
        .await?
    };

    Ok(records)
}

/// Delete pie chart metrics for a specific filter combination
pub async fn delete_pie_chart_metrics(
    pool: &PgPool,
    chart_type: Option<&str>,
    filter_type: Option<&str>,
    entity_id: Option<Uuid>,
    time_range: Option<&str>,
) -> DbResult<u64> {
    // Use a simpler approach with conditional query building to avoid trait object issues
    match (chart_type, filter_type, entity_id, time_range) {
        (Some(ct), Some(ft), Some(eid), Some(tr)) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE chart_type = $1 AND filter_type = $2 AND entity_id = $3 AND time_range = $4",
                ct, ft, eid, tr
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (Some(ct), Some(ft), None, Some(tr)) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE chart_type = $1 AND filter_type = $2 AND entity_id IS NULL AND time_range = $3",
                ct, ft, tr
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (Some(ct), Some(ft), Some(eid), None) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE chart_type = $1 AND filter_type = $2 AND entity_id = $3",
                ct, ft, eid
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (Some(ct), Some(ft), None, None) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE chart_type = $1 AND filter_type = $2 AND entity_id IS NULL",
                ct, ft
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (Some(ct), None, None, None) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE chart_type = $1",
                ct
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (None, Some(ft), None, None) => {
            let result = sqlx::query!(
                "DELETE FROM metrics_pie_chart_data WHERE filter_type = $1 AND entity_id IS NULL",
                ft
            )
            .execute(pool)
            .await?;
            Ok(result.rows_affected())
        }
        (None, None, None, None) => {
            // Delete all pie chart metrics
            let result = sqlx::query!("DELETE FROM metrics_pie_chart_data")
                .execute(pool)
                .await?;
            Ok(result.rows_affected())
        }
        _ => {
            // For other combinations, use a simple delete all (this function is mainly used for cleanup)
            let result = sqlx::query!("DELETE FROM metrics_pie_chart_data")
                .execute(pool)
                .await?;
            Ok(result.rows_affected())
        }
    }
}

// ============================================================================
// BATCH OPERATIONS FOR BACKGROUND PROCESSING
// ============================================================================

/// Clear all metrics data (used for full refresh)
pub async fn clear_all_metrics(tx: &mut Transaction<'_, Postgres>) -> DbResult<(u64, u64)> {
    let market_value_deleted = sqlx::query!("DELETE FROM metrics_market_value_daily")
        .execute(&mut **tx)
        .await?
        .rows_affected();

    let pie_chart_deleted = sqlx::query!("DELETE FROM metrics_pie_chart_data")
        .execute(&mut **tx)
        .await?
        .rows_affected();

    Ok((market_value_deleted, pie_chart_deleted))
}

/// Insert multiple market value metrics in a batch
pub async fn batch_upsert_market_value_metrics(
    tx: &mut Transaction<'_, Postgres>,
    metrics: Vec<CreateMarketValueMetricRequest>,
) -> DbResult<usize> {
    let mut inserted_count = 0;

    for metric in metrics {
        sqlx::query!(
            r#"
            INSERT INTO metrics_market_value_daily 
            (date_value, growth_type, filter_type, entity_id, cumulative_value, deal_count, calculated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now())
            ON CONFLICT (date_value, growth_type, filter_type, entity_id) 
            DO UPDATE SET 
                cumulative_value = EXCLUDED.cumulative_value,
                deal_count = EXCLUDED.deal_count,
                calculated_at = EXCLUDED.calculated_at,
                updated_at = now()
            "#,
            metric.date_value,
            metric.growth_type,
            metric.filter_type,
            metric.entity_id,
            metric.cumulative_value,
            metric.deal_count
        )
        .execute(&mut **tx)
        .await?;

        inserted_count += 1;
    }

    Ok(inserted_count)
}

/// Insert multiple pie chart metrics in a batch
pub async fn batch_upsert_pie_chart_metrics(
    tx: &mut Transaction<'_, Postgres>,
    metrics: Vec<CreatePieChartMetricRequest>,
) -> DbResult<usize> {
    let mut inserted_count = 0;

    for metric in metrics {
        sqlx::query!(
            r#"
            INSERT INTO metrics_pie_chart_data 
            (chart_type, filter_type, entity_id, time_range, category, value, percentage, deal_count, calculated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
            ON CONFLICT (chart_type, filter_type, entity_id, time_range, category) 
            DO UPDATE SET 
                value = EXCLUDED.value,
                percentage = EXCLUDED.percentage,
                deal_count = EXCLUDED.deal_count,
                calculated_at = EXCLUDED.calculated_at,
                updated_at = now()
            "#,
            metric.chart_type,
            metric.filter_type,
            metric.entity_id,
            metric.time_range,
            metric.category,
            metric.value,
            metric.percentage,
            metric.deal_count
        )
        .execute(&mut **tx)
        .await?;

        inserted_count += 1;
    }

    Ok(inserted_count)
}
