//! Database metrics instrumentation wrapper
//!
//! This module provides automatic instrumentation for database operations
//! by wrapping common database functions with metrics collection.

use crate::metrics::record_db_operation;
use std::time::Instant;
use tracing::{debug, error};

/// Instrument a database operation with metrics
pub async fn instrument_db_operation<T, E, F, Fut>(
    operation: &str,
    table: &str,
    db_operation: F,
) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let start_time = Instant::now();
    debug!("🗄️  Starting DB operation: {} on {}", operation, table);
    
    let result = db_operation().await;
    
    let duration = start_time.elapsed().as_secs_f64();
    let status = if result.is_ok() { "success" } else { "error" };
    
    // Record the metrics
    record_db_operation(operation, table, status, duration).await;
    
    match &result {
        Ok(_) => debug!(
            "✅ DB operation completed: {} on {} ({:.3}s)",
            operation, table, duration
        ),
        Err(e) => error!(
            "❌ DB operation failed: {} on {} ({:.3}s): {}",
            operation, table, duration, e
        ),
    }
    
    result
}

/// Convenience macro for instrumenting database operations
#[macro_export]
macro_rules! instrument_db {
    ($operation:expr, $table:expr, $db_call:expr) => {
        crate::db_metrics::instrument_db_operation($operation, $table, || async { $db_call }).await
    };
}

// Re-export the macro for easier use
pub use instrument_db;