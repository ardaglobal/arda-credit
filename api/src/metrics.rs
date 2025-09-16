//! Prometheus Metrics Integration for Arda Credit API
//!
//! This module provides comprehensive metrics collection for both hot and cold paths:
//!
//! Hot Path Metrics:
//! - HTTP request/response metrics
//! - Database operation metrics  
//! - System resource usage (CPU/Memory)
//!
//! Cold Path Metrics:
//! - Smart contract API calls
//! - Sindri proving operations
//! - Batch processing metrics
//! - Gas cost tracking

use axum::{extract::MatchedPath, http::Request, middleware::Next, response::Response};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Prometheus metrics handle for the application
pub struct MetricsHandle {
    pub prometheus_handle: PrometheusHandle,
}

impl MetricsHandle {
    /// Initialize Prometheus metrics exporter
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let builder = PrometheusBuilder::new()
            .add_global_label("service", "arda-credit-api")
            .add_global_label("version", env!("CARGO_PKG_VERSION"));

        let recorder = builder.build_recorder();
        let prometheus_handle = recorder.handle();
        metrics::set_global_recorder(recorder)?;

        // Register all metrics upfront
        Self::register_metrics();

        info!("📊 Prometheus metrics initialized");
        Ok(Self { prometheus_handle })
    }

    /// Register all application metrics
    fn register_metrics() {
        // Hot Path Metrics - initialize them by calling them once
        counter!("http_requests_total").increment(0);
        histogram!("http_request_duration_seconds").record(0.0);
        counter!("http_responses_total").increment(0);
        
        counter!("db_operations_total").increment(0);
        histogram!("db_operation_duration_seconds").record(0.0);
        counter!("db_connections_active").increment(0);
        
        gauge!("system_cpu_usage_percent").set(0.0);
        gauge!("system_memory_usage_bytes").set(0.0);
        gauge!("system_memory_available_bytes").set(0.0);

        // Cold Path Metrics
        counter!("smart_contract_calls_total").increment(0);
        histogram!("smart_contract_call_duration_seconds").record(0.0);
        counter!("smart_contract_errors_total").increment(0);
        
        counter!("sindri_api_calls_total").increment(0);
        histogram!("sindri_proving_duration_seconds").record(0.0);
        counter!("sindri_proof_cost_wei").increment(0);
        gauge!("sindri_proof_cycles").set(0.0);
        
        counter!("batch_transactions_total").increment(0);
        histogram!("batch_processing_duration_seconds").record(0.0);
        gauge!("batch_size_transactions").set(0.0);
        counter!("batch_gas_cost_wei").increment(0);
        
        // Code-level tracing metrics
        histogram!("function_execution_duration_seconds").record(0.0);
        counter!("business_logic_operations_total").increment(0);
        counter!("error_occurrences_total").increment(0);

        info!("✅ All Prometheus metrics registered");
    }

    /// Get the current metrics as a string (for /metrics endpoint)
    pub fn render(&self) -> String {
        self.prometheus_handle.render()
    }
}

/// Middleware for HTTP request metrics
pub async fn metrics_middleware(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| matched_path.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Increment request counter
    counter!("http_requests_total", "method" => method.clone(), "path" => path.clone()).increment(1);

    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    // Record metrics
    histogram!("http_request_duration_seconds", "method" => method.clone(), "path" => path.clone()).record(duration);
    counter!("http_responses_total", "method" => method, "path" => path, "status" => status).increment(1);

    response
}

/// Hot Path Metrics - Database Operations
pub struct DatabaseMetrics;

impl DatabaseMetrics {
    /// Record a database operation
    pub fn record_operation(operation: &str, duration: std::time::Duration, success: bool) {
        let duration_secs = duration.as_secs_f64();
        let status = if success { "success" } else { "error" };
        
        counter!("db_operations_total", "operation" => operation.to_string(), "status" => status.to_string()).increment(1);
        histogram!("db_operation_duration_seconds", "operation" => operation.to_string()).record(duration_secs);
        
        debug!("📊 DB operation recorded: {} ({}s, {})", operation, duration_secs, status);
    }

    /// Update active connections gauge
    pub fn update_active_connections(count: u64) {
        gauge!("db_connections_active").set(count as f64);
    }
}

/// Hot Path Metrics - System Resources
pub struct SystemMetrics;

impl SystemMetrics {
    /// Update CPU usage
    pub fn update_cpu_usage(percentage: f64) {
        gauge!("system_cpu_usage_percent").set(percentage);
    }

    /// Update memory usage
    pub fn update_memory_usage(used_bytes: u64, available_bytes: u64) {
        gauge!("system_memory_usage_bytes").set(used_bytes as f64);
        gauge!("system_memory_available_bytes").set(available_bytes as f64);
    }
}

/// Cold Path Metrics - Smart Contract Operations
pub struct SmartContractMetrics;

impl SmartContractMetrics {
    /// Record a smart contract API call
    pub fn record_call(contract: &str, method: &str, duration: std::time::Duration, success: bool) {
        let duration_secs = duration.as_secs_f64();
        let status = if success { "success" } else { "error" };
        
        counter!("smart_contract_calls_total", 
            "contract" => contract.to_string(), 
            "method" => method.to_string(), 
            "status" => status.to_string()
        ).increment(1);
        
        histogram!("smart_contract_call_duration_seconds", 
            "contract" => contract.to_string(), 
            "method" => method.to_string()
        ).record(duration_secs);

        if !success {
            counter!("smart_contract_errors_total", 
                "contract" => contract.to_string(), 
                "method" => method.to_string()
            ).increment(1);
        }
        
        info!("📊 Smart contract call: {}.{} ({}s, {})", contract, method, duration_secs, status);
    }
}

/// Cold Path Metrics - Sindri Operations
pub struct SindriMetrics;

impl SindriMetrics {
    /// Record a Sindri API call
    pub fn record_api_call(operation: &str, duration: std::time::Duration, success: bool) {
        let duration_secs = duration.as_secs_f64();
        let status = if success { "success" } else { "error" };
        
        counter!("sindri_api_calls_total", "operation" => operation.to_string(), "status" => status.to_string()).increment(1);
        histogram!("sindri_proving_duration_seconds", "operation" => operation.to_string()).record(duration_secs);
        
        info!("📊 Sindri API call: {} ({}s, {})", operation, duration_secs, status);
    }

    /// Record proof generation metrics
    pub fn record_proof_generation(
        proving_time: std::time::Duration,
        cost_wei: u64,
        cycles: u64,
        transactions_count: usize,
    ) {
        let proving_time_secs = proving_time.as_secs_f64();
        
        histogram!("sindri_proving_duration_seconds", "operation" => "generate_proof".to_string()).record(proving_time_secs);
        counter!("sindri_proof_cost_wei").increment(cost_wei);
        gauge!("sindri_proof_cycles").set(cycles as f64);
        
        info!("📊 Sindri proof generated: {}s, {} wei, {} cycles, {} txns", 
              proving_time_secs, cost_wei, cycles, transactions_count);
    }
}

/// Cold Path Metrics - Batch Processing
pub struct BatchMetrics;

impl BatchMetrics {
    /// Record batch processing
    pub fn record_batch_processing(
        batch_id: &str,
        transaction_count: usize,
        processing_duration: std::time::Duration,
        gas_cost_wei: u64,
        success: bool,
    ) {
        let duration_secs = processing_duration.as_secs_f64();
        let status = if success { "success" } else { "error" };
        
        counter!("batch_transactions_total", "status" => status.to_string()).increment(transaction_count as u64);
        histogram!("batch_processing_duration_seconds").record(duration_secs);
        gauge!("batch_size_transactions").set(transaction_count as f64);
        counter!("batch_gas_cost_wei").increment(gas_cost_wei);
        
        info!("📊 Batch processed: {} ({} txns, {}s, {} wei, {})", 
              batch_id, transaction_count, duration_secs, gas_cost_wei, status);
    }

    /// Record transactions per batch
    pub fn record_transactions_per_batch(count: usize) {
        gauge!("batch_size_transactions").set(count as f64);
    }

    /// Record gas cost per batch
    pub fn record_gas_cost_per_batch(cost_wei: u64) {
        counter!("batch_gas_cost_wei").increment(cost_wei);
    }
}

/// Code-level tracing metrics
pub struct CodeMetrics;

impl CodeMetrics {
    /// Record function execution time
    pub fn record_function_execution(function_name: &str, duration: std::time::Duration) {
        let duration_secs = duration.as_secs_f64();
        histogram!("function_execution_duration_seconds", "function" => function_name.to_string()).record(duration_secs);
        
        debug!("📊 Function execution: {} ({}s)", function_name, duration_secs);
    }

    /// Record business logic operation
    pub fn record_business_operation(operation: &str, success: bool) {
        let status = if success { "success" } else { "error" };
        counter!("business_logic_operations_total", "operation" => operation.to_string(), "status" => status.to_string()).increment(1);
        
        debug!("📊 Business operation: {} ({})", operation, status);
    }

    /// Record error occurrence
    pub fn record_error(error_type: &str, context: &str) {
        counter!("error_occurrences_total", "type" => error_type.to_string(), "context" => context.to_string()).increment(1);
        
        warn!("📊 Error recorded: {} in {}", error_type, context);
    }
}

/// Macro for timing function execution
#[macro_export]
macro_rules! time_function {
    ($func_name:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed();
        $crate::metrics::CodeMetrics::record_function_execution($func_name, duration);
        result
    }};
}

/// Macro for recording business operations
#[macro_export]
macro_rules! record_business_op {
    ($operation:expr, $code:block) => {{
        let result = $code;
        let success = result.is_ok();
        $crate::metrics::CodeMetrics::record_business_operation($operation, success);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_database_metrics() {
        DatabaseMetrics::record_operation("select", Duration::from_millis(100), true);
        DatabaseMetrics::update_active_connections(5);
    }

    #[test]
    fn test_system_metrics() {
        SystemMetrics::update_cpu_usage(75.5);
        SystemMetrics::update_memory_usage(1024 * 1024 * 512, 1024 * 1024 * 256);
    }

    #[test]
    fn test_smart_contract_metrics() {
        SmartContractMetrics::record_call(
            "ArdaCredit",
            "submitBatch",
            Duration::from_secs(2),
            true,
        );
    }

    #[test]
    fn test_sindri_metrics() {
        SindriMetrics::record_proof_generation(
            Duration::from_secs(30),
            1000000,
            50000,
            25,
        );
    }

    #[test]
    fn test_batch_metrics() {
        BatchMetrics::record_batch_processing(
            "batch_123",
            50,
            Duration::from_secs(5),
            2000000,
            true,
        );
    }
}