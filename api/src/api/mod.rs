//! API layer module
//!
//! This module contains API layer components:
//! - Router configuration
//! - Server setup
//! - GraphQL schema (disabled - needs update)
//! - OpenAPI documentation
//! - Pool API
//! - Integration endpoints (disabled - needs update)

// Temporarily disabled - these modules use old APIs that need updating
// pub mod graphql;
// pub mod integration;

pub mod openapi;
pub mod pool_api;
pub mod router;
pub mod server;

// Re-export commonly used items
pub use openapi::*;
pub use pool_api::{
    CreatePoolRequest, CreatePoolResponse, InvestorDashboardResponse, InvestorMetrics,
    InvestorPoolSummariesResponse, InvestorPoolSummary, MarketOverviewResponse, PageInfo,
    PlatformMetrics, PoolDetailsResponse, PoolListQuery, PoolListResponse, PoolMatchRequest,
    PoolPerformanceResponse, PoolSummaryInfo, PoolTransactionsResponse, PoolUtilizationResponse,
    StandardPoolObject, TransactionHistoryQuery, UpdatePoolStatusRequest, UtilizationDataPoint,
    UtilizationHistoryQuery,
};
// Note: ErrorResponse from pool_api is NOT re-exported to avoid ambiguity with handlers
pub use router::{create_router, ApiConfig, ApiState};
pub use server::{ApiServer, ApiServerBuilder, ApiServerConfig};
