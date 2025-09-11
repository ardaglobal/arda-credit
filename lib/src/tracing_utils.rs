//! Tracing utilities for ZK proof pipeline observability
//!
//! This module provides structured error tracing and metrics collection
//! for comprehensive observability across the Arda Credit ZK proof pipeline.

use serde_json::json;
use std::time::Instant;
use tracing::{error, warn, Span};

/// Error categories for structured logging
#[derive(Debug, Clone)]
pub enum ErrorCategory {
    SindriApi,
    ProofGeneration,
    ProofVerification,
    BatchProcessing,
    AdsOperations,
    SmartContract,
    Database,
    Configuration,
    Network,
    Timeout,
    ResourceExhaustion,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SindriApi => "sindri_api_error",
            Self::ProofGeneration => "proof_generation_error",
            Self::ProofVerification => "proof_verification_error",
            Self::BatchProcessing => "batch_processing_error",
            Self::AdsOperations => "ads_operations_error",
            Self::SmartContract => "smart_contract_error",
            Self::Database => "database_error",
            Self::Configuration => "configuration_error",
            Self::Network => "network_error",
            Self::Timeout => "timeout_error",
            Self::ResourceExhaustion => "resource_exhaustion_error",
        }
    }
}

/// Comprehensive error context for tracing
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub category: ErrorCategory,
    pub operation: String,
    pub stage: String,
    pub batch_id: Option<i32>,
    pub proof_id: Option<String>,
    pub transaction_count: Option<usize>,
    pub duration_ms: Option<u64>,
    pub retry_count: Option<u32>,
    pub is_recoverable: bool,
    pub additional_context: Option<serde_json::Value>,
}

/// Record a structured error with comprehensive context
pub fn record_error(error_context: ErrorContext, error_message: &str) {
    let span = Span::current();
    
    // Record error details in current span
    span.record("error.category", error_context.category.as_str());
    span.record("error.operation", &error_context.operation);
    span.record("error.stage", &error_context.stage);
    span.record("error.message", error_message);
    span.record("error.is_recoverable", error_context.is_recoverable);
    
    if let Some(batch_id) = error_context.batch_id {
        span.record("batch.id", batch_id);
    }
    
    if let Some(proof_id) = &error_context.proof_id {
        span.record("sindri.proof_id", proof_id);
    }
    
    if let Some(transaction_count) = error_context.transaction_count {
        span.record("batch.transaction_count", transaction_count);
    }
    
    if let Some(duration_ms) = error_context.duration_ms {
        span.record("error.duration_ms", duration_ms);
    }
    
    if let Some(retry_count) = error_context.retry_count {
        span.record("error.retry_count", retry_count);
    }
    
    if let Some(context) = &error_context.additional_context {
        span.record("error.additional_context", context.to_string());
    }
    
    // Log with appropriate level
    if error_context.is_recoverable {
        warn!("🚨 Recoverable error in {} ({}): {}", 
              error_context.operation, error_context.stage, error_message);
    } else {
        error!("❌ Critical error in {} ({}): {}", 
               error_context.operation, error_context.stage, error_message);
    }
}

/// Record error with timing information
pub fn record_timed_error(
    error_context: ErrorContext,
    error_message: &str,
    start_time: Instant,
) {
    let mut context = error_context;
    context.duration_ms = Some(start_time.elapsed().as_millis() as u64);
    record_error(context, error_message);
}

/// Create error context for Sindri API operations
pub fn sindri_error_context(
    operation: &str,
    stage: &str,
    proof_id: Option<String>,
    batch_id: Option<i32>,
    retry_count: Option<u32>,
) -> ErrorContext {
    ErrorContext {
        category: ErrorCategory::SindriApi,
        operation: operation.to_string(),
        stage: stage.to_string(),
        batch_id,
        proof_id,
        transaction_count: None,
        duration_ms: None,
        retry_count,
        is_recoverable: retry_count.map_or(false, |count| count < 3),
        additional_context: None,
    }
}

/// Create error context for batch processing operations
pub fn batch_processing_error_context(
    operation: &str,
    stage: &str,
    batch_id: Option<i32>,
    transaction_count: Option<usize>,
) -> ErrorContext {
    ErrorContext {
        category: ErrorCategory::BatchProcessing,
        operation: operation.to_string(),
        stage: stage.to_string(),
        batch_id,
        proof_id: None,
        transaction_count,
        duration_ms: None,
        retry_count: None,
        is_recoverable: true, // Most batch processing errors are recoverable
        additional_context: None,
    }
}

/// Create error context for ADS operations
pub fn ads_error_context(
    operation: &str,
    stage: &str,
    nullifier_count: Option<usize>,
) -> ErrorContext {
    ErrorContext {
        category: ErrorCategory::AdsOperations,
        operation: operation.to_string(),
        stage: stage.to_string(),
        batch_id: None,
        proof_id: None,
        transaction_count: nullifier_count,
        duration_ms: None,
        retry_count: None,
        is_recoverable: false, // ADS errors are typically critical
        additional_context: None,
    }
}

/// Create error context for smart contract operations
pub fn smart_contract_error_context(
    operation: &str,
    stage: &str,
    batch_id: Option<i32>,
    tx_hash: Option<String>,
) -> ErrorContext {
    let additional_context = tx_hash.map(|hash| {
        json!({
            "transaction_hash": hash
        })
    });
    
    ErrorContext {
        category: ErrorCategory::SmartContract,
        operation: operation.to_string(),
        stage: stage.to_string(),
        batch_id,
        proof_id: None,
        transaction_count: None,
        duration_ms: None,
        retry_count: None,
        is_recoverable: true, // Contract errors are often recoverable with retry
        additional_context,
    }
}

/// Performance metrics for operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub duration_ms: u64,
    pub throughput_ops_per_sec: Option<f64>,
    pub memory_usage_mb: Option<f64>,
    pub success: bool,
    pub error_category: Option<ErrorCategory>,
}

/// Record performance metrics
pub fn record_performance_metrics(metrics: PerformanceMetrics) {
    let span = Span::current();
    
    span.record("perf.operation", &metrics.operation);
    span.record("perf.duration_ms", metrics.duration_ms);
    span.record("perf.success", metrics.success);
    
    if let Some(throughput) = metrics.throughput_ops_per_sec {
        span.record("perf.throughput_ops_per_sec", throughput);
    }
    
    if let Some(memory_mb) = metrics.memory_usage_mb {
        span.record("perf.memory_usage_mb", memory_mb);
    }
    
    if let Some(error_category) = &metrics.error_category {
        span.record("perf.error_category", error_category.as_str());
    }
    
    if metrics.success {
        tracing::info!("📊 Performance: {} completed in {}ms", 
                      metrics.operation, metrics.duration_ms);
    } else {
        tracing::warn!("📊 Performance: {} failed after {}ms", 
                      metrics.operation, metrics.duration_ms);
    }
}

/// Cost tracking utilities
#[derive(Debug, Clone)]
pub struct CostMetrics {
    pub operation: String,
    pub estimated_cost_usd: f64,
    pub actual_cost_usd: Option<f64>,
    pub cost_category: String, // "sindri", "gas", "compute", etc.
    pub resource_usage: Option<serde_json::Value>,
}

/// Record cost metrics
pub fn record_cost_metrics(cost_metrics: CostMetrics) {
    let span = Span::current();
    
    span.record("cost.operation", &cost_metrics.operation);
    span.record("cost.estimated_usd", cost_metrics.estimated_cost_usd);
    span.record("cost.category", &cost_metrics.cost_category);
    
    if let Some(actual_cost) = cost_metrics.actual_cost_usd {
        span.record("cost.actual_usd", actual_cost);
        span.record("cost.variance_usd", actual_cost - cost_metrics.estimated_cost_usd);
    }
    
    if let Some(resource_usage) = &cost_metrics.resource_usage {
        span.record("cost.resource_usage", resource_usage.to_string());
    }
    
    tracing::info!("💰 Cost: {} - estimated: ${:.4}, category: {}", 
                  cost_metrics.operation, cost_metrics.estimated_cost_usd, cost_metrics.cost_category);
}

/// Retry tracking utilities
pub fn record_retry_attempt(
    operation: &str,
    attempt: u32,
    max_attempts: u32,
    backoff_ms: u64,
    last_error: Option<&str>,
) {
    let span = Span::current();
    
    span.record("retry.operation", operation);
    span.record("retry.attempt", attempt);
    span.record("retry.max_attempts", max_attempts);
    span.record("retry.backoff_ms", backoff_ms);
    
    if let Some(error) = last_error {
        span.record("retry.last_error", error);
    }
    
    if attempt < max_attempts {
        tracing::warn!("🔄 Retry: {} attempt {}/{} after {}ms backoff", 
                      operation, attempt, max_attempts, backoff_ms);
    } else {
        tracing::error!("❌ Retry: {} exhausted all {} attempts", 
                       operation, max_attempts);
    }
}

/// Record successful operation completion
pub fn record_success(operation: &str, duration_ms: u64, additional_info: Option<&str>) {
    let span = Span::current();
    
    span.record("success.operation", operation);
    span.record("success.duration_ms", duration_ms);
    span.record("success", true);
    
    if let Some(info) = additional_info {
        span.record("success.additional_info", info);
    }
    
    tracing::info!("✅ Success: {} completed in {}ms{}", 
                  operation, duration_ms, 
                  additional_info.map_or(String::new(), |info| format!(" - {}", info)));
}