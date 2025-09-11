use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use arda_credit_db::{
    InvestorDashboardMetrics, LiquidityPool, MarketOverviewMetrics, PoolSummary, PoolTransaction,
    PoolUtilizationData,
};

// ============================================================================
// POOL QUERY PARAMETERS
// ============================================================================

#[derive(Debug, Deserialize, IntoParams)]
pub struct PoolListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub investor_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default = "default_order_by")]
    pub order_by: String,
}

fn default_limit() -> i64 {
    50
}

fn default_order_by() -> String {
    "created_at DESC".to_string()
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UtilizationHistoryQuery {
    #[serde(default = "default_days")]
    pub days: i32,
    #[serde(default = "default_interval")]
    pub interval: String,
}

fn default_days() -> i32 {
    30
}

fn default_interval() -> String {
    "1 day".to_string()
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TransactionHistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub operation_type: Option<String>,
}

// ============================================================================
// STANDARD POOL OBJECT (used across all endpoints)
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct StandardPoolObject {
    pub pool_id: String,
    pub name: String,
    pub investor_id: String,
    pub total_deposits: String,
    pub available_liquidity: String,
    pub utilized_liquidity: String,
    pub utilization_rate: Option<String>,
    pub max_ltv_ratio: String,
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LiquidityPool> for StandardPoolObject {
    fn from(pool: LiquidityPool) -> Self {
        let utilization_rate = if pool.total_deposits > Decimal::ZERO {
            Some((pool.utilized_liquidity / pool.total_deposits).to_string())
        } else {
            None
        };

        Self {
            pool_id: pool.id.to_string(),
            name: pool.name,
            investor_id: pool.investor_id.to_string(),
            total_deposits: pool.total_deposits.to_string(),
            available_liquidity: pool.available_liquidity.to_string(),
            utilized_liquidity: pool.utilized_liquidity.to_string(),
            utilization_rate,
            max_ltv_ratio: pool.max_ltv_ratio.to_string(),
            max_interest_rate: pool.max_interest_rate.to_string(),
            repayment_period_days: pool.repayment_period_days,
            status: pool.status,
            created_at: pool.created_at,
            updated_at: pool.updated_at,
        }
    }
}

// ============================================================================
// POOL ENDPOINTS RESPONSES
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolListResponse {
    pub pools: Vec<StandardPoolObject>,
    pub total_count: i64,
    pub page_info: PageInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PageInfo {
    pub limit: i64,
    pub offset: i64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolDetailsResponse {
    pub pool_id: String,
    pub name: String,
    pub investor_id: String,
    /// Total deposits as decimal string
    pub total_deposits: String,
    /// Available liquidity as decimal string
    pub available_liquidity: String,
    /// Utilized liquidity as decimal string
    pub utilized_liquidity: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolSummaryInfo {
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: String,
    pub average_loan_amount: Option<String>,
    pub default_rate: String,
    pub total_interest_earned: String,
    pub total_principal_repaid: String,
}

impl From<PoolSummary> for PoolSummaryInfo {
    fn from(summary: PoolSummary) -> Self {
        Self {
            total_loans: summary.total_loans,
            active_loans: summary.active_loans,
            total_loan_volume: summary.total_loan_volume.to_string(),
            average_loan_amount: summary.average_loan_amount.map(|a| a.to_string()),
            default_rate: summary.default_rate.to_string(),
            total_interest_earned: summary.total_interest_earned.to_string(),
            total_principal_repaid: summary.total_principal_repaid.to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolUtilizationResponse {
    pub pool_id: String,
    pub current_utilization_rate: String,
    pub historical_data: Vec<UtilizationDataPoint>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UtilizationDataPoint {
    pub timestamp: DateTime<Utc>,
    pub utilization_rate: String,
    pub total_deposits: String,
    pub utilized_liquidity: String,
    pub available_liquidity: String,
}

impl From<PoolUtilizationData> for UtilizationDataPoint {
    fn from(data: PoolUtilizationData) -> Self {
        Self {
            timestamp: data.timestamp,
            utilization_rate: data
                .utilization_rate
                .map_or("0".to_string(), |u| u.to_string()),
            total_deposits: data.total_deposits.to_string(),
            utilized_liquidity: data.utilized_liquidity.to_string(),
            available_liquidity: data.available_liquidity.to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolTransactionsResponse {
    pub pool_id: String,
    pub transactions: Vec<PoolTransactionInfo>,
    pub total_count: i64,
    pub page_info: PageInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolTransactionInfo {
    pub transaction_id: String,
    pub operation_type: String,
    pub amount: String,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<String>,
}

impl From<PoolTransaction> for PoolTransactionInfo {
    fn from(transaction: PoolTransaction) -> Self {
        Self {
            transaction_id: transaction.transaction_id.to_string(),
            operation_type: transaction.operation_type,
            amount: transaction.amount.to_string(),
            user_id: transaction.user_id.to_string(),
            timestamp: transaction.timestamp,
            metadata: transaction.metadata,
        }
    }
}

// ============================================================================
// INVESTOR ENDPOINTS RESPONSES
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct InvestorDashboardResponse {
    pub metrics: InvestorMetrics,
    pub recent_activity: Vec<RecentActivity>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvestorMetrics {
    pub total_pools: i64,
    pub active_pools: i64,
    pub total_liquidity_provided: String,
    pub total_utilized_liquidity: String,
    pub average_utilization_rate: String,
    pub total_interest_earned: String,
    pub total_loans_originated: i64,
    pub total_loan_volume: String,
}

impl From<InvestorDashboardMetrics> for InvestorMetrics {
    fn from(metrics: InvestorDashboardMetrics) -> Self {
        Self {
            total_pools: metrics.total_pools,
            active_pools: metrics.active_pools,
            total_liquidity_provided: metrics.total_liquidity_provided.to_string(),
            total_utilized_liquidity: metrics.total_utilized_liquidity.to_string(),
            average_utilization_rate: metrics.average_utilization_rate.to_string(),
            total_interest_earned: metrics.total_interest_earned.to_string(),
            total_loans_originated: metrics.total_loans_originated,
            total_loan_volume: metrics.total_loan_volume.to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecentActivity {
    pub activity_type: String,
    pub pool_id: String,
    pub pool_name: String,
    pub amount: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvestorPoolSummariesResponse {
    pub summaries: Vec<InvestorPoolSummary>,
    pub total_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvestorPoolSummary {
    pub pool: StandardPoolObject,
    pub performance: PoolPerformanceMetrics,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolPerformanceMetrics {
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: String,
    pub total_interest_earned: String,
    pub default_rate: String,
    pub roi: String,
}

// ============================================================================
// ANALYTICS ENDPOINTS RESPONSES
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct MarketOverviewResponse {
    pub platform_metrics: PlatformMetrics,
    pub market_trends: MarketTrends,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlatformMetrics {
    pub total_pools: i64,
    pub active_pools: i64,
    pub total_liquidity: String,
    pub total_utilized_liquidity: String,
    pub platform_utilization_rate: String,
    pub total_loans: i64,
    pub active_loans: i64,
    pub total_loan_volume: String,
    pub average_interest_rate: String,
    pub platform_default_rate: String,
}

impl From<MarketOverviewMetrics> for PlatformMetrics {
    fn from(metrics: MarketOverviewMetrics) -> Self {
        Self {
            total_pools: metrics.total_pools,
            active_pools: metrics.active_pools,
            total_liquidity: metrics.total_liquidity.to_string(),
            total_utilized_liquidity: metrics.total_utilized_liquidity.to_string(),
            platform_utilization_rate: metrics.platform_utilization_rate.to_string(),
            total_loans: metrics.total_loans,
            active_loans: metrics.active_loans,
            total_loan_volume: metrics.total_loan_volume.to_string(),
            average_interest_rate: metrics.average_interest_rate.to_string(),
            platform_default_rate: metrics.platform_default_rate.to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MarketTrends {
    pub liquidity_growth_rate: String,
    pub loan_volume_growth_rate: String,
    pub utilization_trend: String,
    pub average_pool_size: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolPerformanceResponse {
    pub pool_performance: Vec<PoolPerformanceData>,
    pub benchmark_data: BenchmarkData,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolPerformanceData {
    pub pool_id: String,
    pub pool_name: String,
    pub total_return: String,
    pub annualized_return: String,
    pub risk_score: String,
    pub utilization_rate: String,
    pub loan_count: i64,
    pub default_rate: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BenchmarkData {
    pub average_return: String,
    pub median_utilization: String,
    pub average_risk_score: String,
    pub platform_default_rate: String,
}

// ============================================================================
// ERROR RESPONSES
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(error: &str, message: &str) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn with_details(error: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: Some(details),
        }
    }
}

// ============================================================================
// POOL MANAGEMENT REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to create a new liquidity pool
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolRequest {
    pub name: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    /// Investor ID (required for pool creation)
    pub investor_id: Option<String>,
}

/// Response from pool creation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolResponse {
    pub pool_id: String,
    pub investor_id: String,
    pub name: String,
    /// Maximum loan-to-value ratio as a decimal string (e.g., "0.8000" for 80%)
    pub max_ltv_ratio: String,
    /// Maximum interest rate as a decimal string (e.g., "0.1200" for 12%)
    pub max_interest_rate: String,
    pub repayment_period_days: i32,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request to update pool status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatePoolStatusRequest {
    pub status: String, // ACTIVE, PAUSED, CLOSED
}

/// Pool matching criteria for auto-deployment
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PoolMatchRequest {
    /// Required liquidity amount as decimal string
    pub required_liquidity: String,
    /// Maximum LTV ratio as decimal string
    pub max_ltv: String,
    /// Maximum interest rate as decimal string
    pub max_rate: String,
    pub min_repayment_days: i32,
}
