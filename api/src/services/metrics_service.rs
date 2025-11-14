//! Metrics calculation service
//!
//! This service provides business logic for:
//! - Calculating cumulative market value metrics from deal data
//! - Computing pie chart distribution percentages
//! - Orchestrating metric refresh operations
//! - Converting raw deal data into cached metric structures

use std::collections::HashMap;

use arda_credit_db::{
    batch_upsert_market_value_metrics, batch_upsert_pie_chart_metrics, clear_all_metrics,
    get_market_value_metrics, get_pie_chart_metrics, list_deals, CreateMarketValueMetricRequest,
    CreatePieChartMetricRequest, DbError, Deal,
};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Metrics calculation service
#[derive(Debug, Clone)]
pub struct MetricsService {
    pool: PgPool,
}

/// Market value query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketValueQuery {
    pub growth_type: String,     // market_growth | arda_growth
    pub filter_by: String,       // all | originator | investor
    pub entity_id: Option<Uuid>, // company_id when filter_by is originator or investor
    pub time_range: String,      // all_time | last_year
}

/// Pie chart query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieChartQuery {
    pub chart_type: String,      // industry | region | asset_class
    pub filter_by: String,       // all | originator | investor
    pub entity_id: Option<Uuid>, // company_id when filter_by is originator or investor
    pub time_range: String,      // all_time | last_year
}

/// Market value response data point
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MarketValueDataPoint {
    pub date: NaiveDate,
    pub cumulative_value: String,
    pub deal_count: i32,
}

/// Pie chart response data point
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PieChartDataPoint {
    pub category: String,
    pub value: String,
    pub percentage: String,
    pub deal_count: i32,
}

/// Refresh operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResult {
    pub success: bool,
    pub market_value_points: usize,
    pub pie_chart_points: usize,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("Calculation error: {0}")]
    Calculation(String),
}

type MetricsResult<T> = Result<T, MetricsError>;

impl MetricsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ========================================================================
    // PUBLIC API METHODS
    // ========================================================================

    /// Get market value time series data
    #[instrument(skip(self), level = "info")]
    pub async fn get_market_value_data(
        &self,
        query: MarketValueQuery,
    ) -> MetricsResult<Vec<MarketValueDataPoint>> {
        self.validate_market_value_query(&query)?;

        let start_date = if query.time_range == "last_year" {
            Some(self.get_one_year_ago_date())
        } else {
            None
        };

        let metrics = get_market_value_metrics(
            &self.pool,
            &query.growth_type,
            &query.filter_by,
            query.entity_id,
            start_date,
            None, // end_date
        )
        .await?;

        let data_points = metrics
            .into_iter()
            .map(|m| MarketValueDataPoint {
                date: m.date_value,
                cumulative_value: m.cumulative_value.to_string(),
                deal_count: m.deal_count,
            })
            .collect();

        Ok(data_points)
    }

    /// Get pie chart distribution data
    #[instrument(skip(self), level = "info")]
    pub async fn get_pie_chart_data(
        &self,
        query: PieChartQuery,
    ) -> MetricsResult<Vec<PieChartDataPoint>> {
        self.validate_pie_chart_query(&query)?;

        let metrics = get_pie_chart_metrics(
            &self.pool,
            &query.chart_type,
            &query.filter_by,
            query.entity_id,
            &query.time_range,
        )
        .await?;

        let data_points = metrics
            .into_iter()
            .map(|m| PieChartDataPoint {
                category: m.category,
                value: m.value.to_string(),
                percentage: m.percentage.to_string(),
                deal_count: m.deal_count,
            })
            .collect();

        Ok(data_points)
    }

    /// Refresh all metrics data (full recalculation)
    #[instrument(skip(self), level = "info")]
    pub async fn refresh_all_metrics(&self) -> MetricsResult<RefreshResult> {
        info!("Starting full metrics refresh");

        let mut tx = self.pool.begin().await?;

        // Clear existing metrics
        match clear_all_metrics(&mut tx).await {
            Ok((market_deleted, pie_deleted)) => {
                info!(
                    "Cleared existing metrics: {} market value, {} pie chart",
                    market_deleted, pie_deleted
                );
            }
            Err(e) => {
                error!("Failed to clear existing metrics: {}", e);
                tx.rollback().await?;
                return Ok(RefreshResult {
                    success: false,
                    market_value_points: 0,
                    pie_chart_points: 0,
                    error: Some(format!("Failed to clear metrics: {}", e)),
                });
            }
        }

        // Calculate new metrics
        let market_value_result = self.calculate_all_market_value_metrics(&mut tx).await;
        let pie_chart_result = self.calculate_all_pie_chart_metrics(&mut tx).await;

        match (market_value_result, pie_chart_result) {
            (Ok(market_count), Ok(pie_count)) => {
                tx.commit().await?;
                info!(
                    "Successfully refreshed metrics: {} market value points, {} pie chart points",
                    market_count, pie_count
                );
                Ok(RefreshResult {
                    success: true,
                    market_value_points: market_count,
                    pie_chart_points: pie_count,
                    error: None,
                })
            }
            (Err(market_err), Ok(_)) => {
                error!("Market value calculation failed: {}", market_err);
                tx.rollback().await?;
                Ok(RefreshResult {
                    success: false,
                    market_value_points: 0,
                    pie_chart_points: 0,
                    error: Some(format!("Market value calculation failed: {}", market_err)),
                })
            }
            (Ok(_), Err(pie_err)) => {
                error!("Pie chart calculation failed: {}", pie_err);
                tx.rollback().await?;
                Ok(RefreshResult {
                    success: false,
                    market_value_points: 0,
                    pie_chart_points: 0,
                    error: Some(format!("Pie chart calculation failed: {}", pie_err)),
                })
            }
            (Err(market_err), Err(pie_err)) => {
                error!(
                    "Both calculations failed - Market: {}, Pie: {}",
                    market_err, pie_err
                );
                tx.rollback().await?;
                Ok(RefreshResult {
                    success: false,
                    market_value_points: 0,
                    pie_chart_points: 0,
                    error: Some(format!(
                        "Both calculations failed - Market: {}, Pie: {}",
                        market_err, pie_err
                    )),
                })
            }
        }
    }

    // ========================================================================
    // METRIC CALCULATION METHODS
    // ========================================================================

    /// Calculate all market value metrics and insert them
    async fn calculate_all_market_value_metrics(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> MetricsResult<usize> {
        let mut total_inserted = 0;

        // Get ALL deals for calculation (set high limit to avoid pagination issues)
        let deals_result =
            list_deals(&self.pool, Some(1_000_000), None, None, None, None, None).await?;
        let deals = &deals_result.deals;

        // Calculate for different filter combinations
        let filter_combinations = vec![
            ("all", None),
            // We could add specific company filters here, but for now we'll focus on 'all'
            // In the future, we might want to calculate for all companies that have deals
        ];

        for (filter_type, entity_id) in filter_combinations {
            // Market growth (by originated_at)
            let market_growth_metrics = self.calculate_market_value_time_series(
                deals,
                "market_growth",
                filter_type,
                entity_id,
            )?;
            total_inserted += batch_upsert_market_value_metrics(tx, market_growth_metrics).await?;

            // Arda growth (by created_at)
            let arda_growth_metrics = self.calculate_market_value_time_series(
                deals,
                "arda_growth",
                filter_type,
                entity_id,
            )?;
            total_inserted += batch_upsert_market_value_metrics(tx, arda_growth_metrics).await?;
        }

        Ok(total_inserted)
    }

    /// Calculate all pie chart metrics and insert them
    async fn calculate_all_pie_chart_metrics(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> MetricsResult<usize> {
        let mut total_inserted = 0;

        // Get ALL deals for calculation (set high limit to avoid pagination issues)
        let deals_result =
            list_deals(&self.pool, Some(1_000_000), None, None, None, None, None).await?;
        let deals = &deals_result.deals;

        let chart_types = vec!["industry", "region", "asset_class"];
        let time_ranges = vec!["all_time", "last_year"];
        let filter_combinations = vec![("all", None)];

        for chart_type in chart_types {
            for time_range in &time_ranges {
                for (filter_type, entity_id) in &filter_combinations {
                    let filtered_deals = self.filter_deals_for_time_range(deals, time_range);
                    let pie_metrics = self.calculate_pie_chart_distribution(
                        &filtered_deals,
                        chart_type,
                        filter_type,
                        *entity_id,
                        time_range,
                    )?;
                    total_inserted += batch_upsert_pie_chart_metrics(tx, pie_metrics).await?;
                }
            }
        }

        Ok(total_inserted)
    }

    /// Calculate market value time series for given parameters
    fn calculate_market_value_time_series(
        &self,
        deals: &[Deal],
        growth_type: &str,
        filter_type: &str,
        entity_id: Option<Uuid>,
    ) -> MetricsResult<Vec<CreateMarketValueMetricRequest>> {
        // Filter deals based on filter_type and entity_id
        let filtered_deals = self.filter_deals_by_entity(deals, filter_type, entity_id)?;

        // Group deals by date (either originated_at or created_at)
        let mut daily_totals: HashMap<NaiveDate, (Decimal, i32)> = HashMap::new();

        for deal in &filtered_deals {
            let date = match growth_type {
                "market_growth" => deal.originated_at.date_naive(),
                "arda_growth" => deal.created_at.date_naive(),
                _ => {
                    return Err(MetricsError::InvalidParameter(
                        "Invalid growth_type".to_string(),
                    ))
                }
            };

            let entry = daily_totals.entry(date).or_insert((Decimal::ZERO, 0));
            entry.0 += deal.principal_amount;
            entry.1 += 1;
        }

        // Convert to cumulative time series
        let mut sorted_dates: Vec<_> = daily_totals.keys().cloned().collect();
        sorted_dates.sort();

        let mut cumulative_value = Decimal::ZERO;
        let mut cumulative_count = 0;
        let mut metrics = Vec::new();

        for date in sorted_dates {
            let (daily_value, daily_count) = daily_totals[&date];
            cumulative_value += daily_value;
            cumulative_count += daily_count;

            metrics.push(CreateMarketValueMetricRequest {
                date_value: date,
                growth_type: growth_type.to_string(),
                filter_type: filter_type.to_string(),
                entity_id,
                cumulative_value,
                deal_count: cumulative_count,
            });
        }

        Ok(metrics)
    }

    /// Calculate pie chart distribution for given parameters
    fn calculate_pie_chart_distribution(
        &self,
        deals: &[Deal],
        chart_type: &str,
        filter_type: &str,
        entity_id: Option<Uuid>,
        time_range: &str,
    ) -> MetricsResult<Vec<CreatePieChartMetricRequest>> {
        let filtered_deals = self.filter_deals_by_entity(deals, filter_type, entity_id)?;

        // Group deals by category
        let mut category_totals: HashMap<String, (Decimal, i32)> = HashMap::new();
        let mut total_value = Decimal::ZERO;

        for deal in &filtered_deals {
            let category = match chart_type {
                "industry" => deal.industry.as_deref().unwrap_or("Unknown").to_string(),
                "region" => deal.region.as_deref().unwrap_or("Unknown").to_string(),
                "asset_class" => deal.asset_class.as_deref().unwrap_or("Unknown").to_string(),
                _ => {
                    return Err(MetricsError::InvalidParameter(
                        "Invalid chart_type".to_string(),
                    ))
                }
            };

            let entry = category_totals
                .entry(category)
                .or_insert((Decimal::ZERO, 0));
            entry.0 += deal.principal_amount;
            entry.1 += 1;
            total_value += deal.principal_amount;
        }

        // Calculate percentages and create metrics
        let mut metrics = Vec::new();
        for (category, (value, count)) in category_totals {
            let percentage = if total_value > Decimal::ZERO {
                (value / total_value * Decimal::from(100)).round_dp(2)
            } else {
                Decimal::ZERO
            };

            metrics.push(CreatePieChartMetricRequest {
                chart_type: chart_type.to_string(),
                filter_type: filter_type.to_string(),
                entity_id,
                time_range: time_range.to_string(),
                category,
                value,
                percentage,
                deal_count: count,
            });
        }

        Ok(metrics)
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    fn filter_deals_by_entity(
        &self,
        deals: &[Deal],
        filter_type: &str,
        entity_id: Option<Uuid>,
    ) -> MetricsResult<Vec<Deal>> {
        match filter_type {
            "all" => Ok(deals.to_vec()),
            "originator" => {
                let company_id = entity_id.ok_or_else(|| {
                    MetricsError::InvalidParameter(
                        "entity_id required for originator filter".to_string(),
                    )
                })?;
                Ok(deals
                    .iter()
                    .filter(|d| d.originator_id == company_id)
                    .cloned()
                    .collect())
            }
            "investor" => {
                let company_id = entity_id.ok_or_else(|| {
                    MetricsError::InvalidParameter(
                        "entity_id required for investor filter".to_string(),
                    )
                })?;
                Ok(deals
                    .iter()
                    .filter(|d| d.investor_id == Some(company_id))
                    .cloned()
                    .collect())
            }
            _ => Err(MetricsError::InvalidParameter(
                "Invalid filter_type".to_string(),
            )),
        }
    }

    fn filter_deals_for_time_range(&self, deals: &[Deal], time_range: &str) -> Vec<Deal> {
        match time_range {
            "all_time" => deals.to_vec(),
            "last_year" => {
                let cutoff = self
                    .get_one_year_ago_date()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_local_timezone(Utc)
                    .unwrap();
                deals
                    .iter()
                    .filter(|d| d.originated_at >= cutoff)
                    .cloned()
                    .collect()
            }
            _ => deals.to_vec(),
        }
    }

    fn get_one_year_ago_date(&self) -> NaiveDate {
        let now = Utc::now();
        let one_year_ago = now - chrono::Duration::days(365);
        one_year_ago.date_naive()
    }

    fn validate_market_value_query(&self, query: &MarketValueQuery) -> MetricsResult<()> {
        if !matches!(query.growth_type.as_str(), "market_growth" | "arda_growth") {
            return Err(MetricsError::InvalidParameter(
                "growth_type must be 'market_growth' or 'arda_growth'".to_string(),
            ));
        }

        if !matches!(query.filter_by.as_str(), "all" | "originator" | "investor") {
            return Err(MetricsError::InvalidParameter(
                "filter_by must be 'all', 'originator', or 'investor'".to_string(),
            ));
        }

        if !matches!(query.time_range.as_str(), "all_time" | "last_year") {
            return Err(MetricsError::InvalidParameter(
                "time_range must be 'all_time' or 'last_year'".to_string(),
            ));
        }

        if matches!(query.filter_by.as_str(), "originator" | "investor")
            && query.entity_id.is_none()
        {
            return Err(MetricsError::InvalidParameter(
                "entity_id is required when filter_by is 'originator' or 'investor'".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_pie_chart_query(&self, query: &PieChartQuery) -> MetricsResult<()> {
        if !matches!(
            query.chart_type.as_str(),
            "industry" | "region" | "asset_class"
        ) {
            return Err(MetricsError::InvalidParameter(
                "chart_type must be 'industry', 'region', or 'asset_class'".to_string(),
            ));
        }

        if !matches!(query.filter_by.as_str(), "all" | "originator" | "investor") {
            return Err(MetricsError::InvalidParameter(
                "filter_by must be 'all', 'originator', or 'investor'".to_string(),
            ));
        }

        if !matches!(query.time_range.as_str(), "all_time" | "last_year") {
            return Err(MetricsError::InvalidParameter(
                "time_range must be 'all_time' or 'last_year'".to_string(),
            ));
        }

        if matches!(query.filter_by.as_str(), "originator" | "investor")
            && query.entity_id.is_none()
        {
            return Err(MetricsError::InvalidParameter(
                "entity_id is required when filter_by is 'originator' or 'investor'".to_string(),
            ));
        }

        Ok(())
    }
}
