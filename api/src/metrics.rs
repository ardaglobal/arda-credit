//! Prometheus metrics module for Arda Credit API
//!
//! This module provides comprehensive monitoring for both hot and cold paths:
//! 
//! Hot paths:
//! - Requests coming/going out
//! - DB read/writes
//! - Code level traces
//! - CPU/Memory usage (exposed for infra collection)
//! 
//! Cold paths:
//! - API Requests to smart contract
//! - Transactions per batch
//! - Gas cost per batch
//! - Proving times on Sindri
//! - Details about proof composition
//! - Cost per batch for Sindri
//! - API requests to Sindri

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec, Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Global metrics registry
static METRICS_REGISTRY: once_cell::sync::Lazy<Arc<RwLock<Option<MetricsCollector>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Main metrics collector for the Arda Credit API
#[derive(Clone)]
pub struct MetricsCollector {
    pub registry: Registry,
    
    // Hot Path Metrics - High frequency operations
    pub http_requests_total: IntCounterVec,
    pub http_request_duration: HistogramVec,
    pub http_requests_in_flight: IntGauge,
    
    // Database metrics
    pub db_operations_total: IntCounterVec,
    pub db_operation_duration: HistogramVec,
    pub db_connections_active: IntGauge,
    pub db_connections_idle: IntGauge,
    pub db_query_errors_total: IntCounterVec,
    
    // Application-level metrics
    pub transactions_processed_total: IntCounter,
    pub batches_created_total: IntCounter,
    pub merkle_tree_operations: IntCounterVec,
    pub ads_operations_duration: HistogramVec,
    
    // System metrics (for infra collection)
    pub memory_usage_bytes: IntGauge,
    pub cpu_usage_percent: Gauge,
    pub open_file_descriptors: IntGauge,
    
    // Cold Path Metrics - Lower frequency, high-value operations
    // Smart Contract interactions
    pub smart_contract_calls_total: IntCounterVec,
    pub smart_contract_call_duration: HistogramVec,
    pub smart_contract_gas_used: IntCounterVec,
    pub smart_contract_errors_total: IntCounterVec,
    
    // Batch processing metrics
    pub batch_size_histogram: Histogram,
    pub batch_processing_duration: HistogramVec,
    pub transactions_per_batch: Histogram,
    pub batch_proof_generation_duration: HistogramVec,
    
    // Sindri API metrics
    pub sindri_api_calls_total: IntCounterVec,
    pub sindri_api_call_duration: HistogramVec,
    pub sindri_proof_generation_duration: HistogramVec,
    pub sindri_proof_status: IntCounterVec,
    pub sindri_cost_per_batch: GaugeVec,
    pub sindri_proof_cycles: IntCounterVec,
    pub sindri_api_errors_total: IntCounterVec,
    
    // Proving system metrics
    pub proof_generation_requests: IntCounterVec,
    pub proof_verification_requests: IntCounterVec,
    pub proof_size_bytes: HistogramVec,
    pub proof_public_values_size: Histogram,
}

impl MetricsCollector {
    /// Initialize the metrics collector with all required metrics
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        // Hot Path Metrics
        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests")
                .namespace("arda_credit")
                .subsystem("api"),
            &["method", "endpoint", "status_code"],
        )?;
        
        let http_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("api")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["method", "endpoint"],
        )?;
        
        let http_requests_in_flight = IntGauge::new(
            Opts::new("http_requests_in_flight", "Number of HTTP requests currently being processed")
                .namespace("arda_credit")
                .subsystem("api"),
        )?;
        
        // Database metrics
        let db_operations_total = IntCounterVec::new(
            Opts::new("db_operations_total", "Total number of database operations")
                .namespace("arda_credit")
                .subsystem("db"),
            &["operation", "table", "status"],
        )?;
        
        let db_operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "db_operation_duration_seconds",
                "Database operation duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("db")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["operation", "table"],
        )?;
        
        let db_connections_active = IntGauge::new(
            Opts::new("db_connections_active", "Number of active database connections")
                .namespace("arda_credit")
                .subsystem("db"),
        )?;
        
        let db_connections_idle = IntGauge::new(
            Opts::new("db_connections_idle", "Number of idle database connections")
                .namespace("arda_credit")
                .subsystem("db"),
        )?;
        
        let db_query_errors_total = IntCounterVec::new(
            Opts::new("db_query_errors_total", "Total number of database query errors")
                .namespace("arda_credit")
                .subsystem("db"),
            &["error_type", "table"],
        )?;
        
        // Application-level metrics
        let transactions_processed_total = IntCounter::new(
            Opts::new("transactions_processed_total", "Total number of transactions processed")
                .namespace("arda_credit")
                .subsystem("app"),
        )?;
        
        let batches_created_total = IntCounter::new(
            Opts::new("batches_created_total", "Total number of batches created")
                .namespace("arda_credit")
                .subsystem("app"),
        )?;
        
        let merkle_tree_operations = IntCounterVec::new(
            Opts::new("merkle_tree_operations_total", "Total number of merkle tree operations")
                .namespace("arda_credit")
                .subsystem("ads"),
            &["operation", "tree_type"],
        )?;
        
        let ads_operations_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "ads_operations_duration_seconds",
                "ADS operations duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("ads")
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0]),
            &["operation", "tree_type"],
        )?;
        
        // System metrics
        let memory_usage_bytes = IntGauge::new(
            Opts::new("memory_usage_bytes", "Current memory usage in bytes")
                .namespace("arda_credit")
                .subsystem("system"),
        )?;
        
        let cpu_usage_percent = Gauge::new(
            Opts::new("cpu_usage_percent", "Current CPU usage percentage")
                .namespace("arda_credit")
                .subsystem("system"),
        )?;
        
        let open_file_descriptors = IntGauge::new(
            Opts::new("open_file_descriptors", "Number of open file descriptors")
                .namespace("arda_credit")
                .subsystem("system"),
        )?;
        
        // Cold Path Metrics - Smart Contract
        let smart_contract_calls_total = IntCounterVec::new(
            Opts::new("smart_contract_calls_total", "Total number of smart contract calls")
                .namespace("arda_credit")
                .subsystem("blockchain"),
            &["contract", "method", "status", "chain_id"],
        )?;
        
        let smart_contract_call_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "smart_contract_call_duration_seconds",
                "Smart contract call duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("blockchain")
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]),
            &["contract", "method"],
        )?;
        
        let smart_contract_gas_used = IntCounterVec::new(
            Opts::new("smart_contract_gas_used_total", "Total gas used in smart contract calls")
                .namespace("arda_credit")
                .subsystem("blockchain"),
            &["contract", "method", "chain_id"],
        )?;
        
        let smart_contract_errors_total = IntCounterVec::new(
            Opts::new("smart_contract_errors_total", "Total smart contract call errors")
                .namespace("arda_credit")
                .subsystem("blockchain"),
            &["contract", "method", "error_type", "chain_id"],
        )?;
        
        // Batch processing metrics
        let batch_size_histogram = Histogram::new(
            prometheus::HistogramOpts::new(
                "batch_size_transactions",
                "Number of transactions per batch",
            )
            .namespace("arda_credit")
            .subsystem("batch")
            .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
        )?;
        
        let batch_processing_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "batch_processing_duration_seconds",
                "Total batch processing duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("batch")
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0]),
            &["stage", "trigger_source"],
        )?;
        
        let transactions_per_batch = Histogram::new(
            prometheus::HistogramOpts::new(
                "transactions_per_batch",
                "Distribution of transactions per batch",
            )
            .namespace("arda_credit")
            .subsystem("batch")
            .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
        )?;
        
        let batch_proof_generation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "batch_proof_generation_duration_seconds",
                "Batch proof generation duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("batch")
            .buckets(vec![10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0]),
            &["proof_system", "circuit"],
        )?;
        
        // Sindri API metrics
        let sindri_api_calls_total = IntCounterVec::new(
            Opts::new("sindri_api_calls_total", "Total number of Sindri API calls")
                .namespace("arda_credit")
                .subsystem("sindri"),
            &["endpoint", "method", "status_code"],
        )?;
        
        let sindri_api_call_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "sindri_api_call_duration_seconds",
                "Sindri API call duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("sindri")
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
            &["endpoint", "method"],
        )?;
        
        let sindri_proof_generation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "sindri_proof_generation_duration_seconds",
                "Sindri proof generation duration in seconds",
            )
            .namespace("arda_credit")
            .subsystem("sindri")
            .buckets(vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 14400.0, 28800.0]),
            &["circuit", "proof_system"],
        )?;
        
        let sindri_proof_status = IntCounterVec::new(
            Opts::new("sindri_proof_status_total", "Sindri proof status counts")
                .namespace("arda_credit")
                .subsystem("sindri"),
            &["status", "circuit"],
        )?;
        
        let sindri_cost_per_batch = GaugeVec::new(
            Opts::new("sindri_cost_per_batch_usd", "Cost per batch for Sindri in USD")
                .namespace("arda_credit")
                .subsystem("sindri"),
            &["circuit", "proof_system"],
        )?;
        
        let sindri_proof_cycles = IntCounterVec::new(
            Opts::new("sindri_proof_cycles_total", "Total cycles used in Sindri proofs")
                .namespace("arda_credit")
                .subsystem("sindri"),
            &["circuit", "proof_system"],
        )?;
        
        let sindri_api_errors_total = IntCounterVec::new(
            Opts::new("sindri_api_errors_total", "Total Sindri API errors")
                .namespace("arda_credit")
                .subsystem("sindri"),
            &["endpoint", "error_type"],
        )?;
        
        // Proving system metrics
        let proof_generation_requests = IntCounterVec::new(
            Opts::new("proof_generation_requests_total", "Total proof generation requests")
                .namespace("arda_credit")
                .subsystem("proving"),
            &["proof_system", "circuit", "status"],
        )?;
        
        let proof_verification_requests = IntCounterVec::new(
            Opts::new("proof_verification_requests_total", "Total proof verification requests")
                .namespace("arda_credit")
                .subsystem("proving"),
            &["proof_system", "status"],
        )?;
        
        let proof_size_bytes = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "proof_size_bytes",
                "Size of generated proofs in bytes",
            )
            .namespace("arda_credit")
            .subsystem("proving")
            .buckets(vec![1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0, 4194304.0]),
            &["proof_system", "circuit"],
        )?;
        
        let proof_public_values_size = Histogram::new(
            prometheus::HistogramOpts::new(
                "proof_public_values_size_bytes",
                "Size of proof public values in bytes",
            )
            .namespace("arda_credit")
            .subsystem("proving")
            .buckets(vec![32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0]),
        )?;
        
        // Register all metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;
        registry.register(Box::new(db_operations_total.clone()))?;
        registry.register(Box::new(db_operation_duration.clone()))?;
        registry.register(Box::new(db_connections_active.clone()))?;
        registry.register(Box::new(db_connections_idle.clone()))?;
        registry.register(Box::new(db_query_errors_total.clone()))?;
        registry.register(Box::new(transactions_processed_total.clone()))?;
        registry.register(Box::new(batches_created_total.clone()))?;
        registry.register(Box::new(merkle_tree_operations.clone()))?;
        registry.register(Box::new(ads_operations_duration.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;
        registry.register(Box::new(open_file_descriptors.clone()))?;
        registry.register(Box::new(smart_contract_calls_total.clone()))?;
        registry.register(Box::new(smart_contract_call_duration.clone()))?;
        registry.register(Box::new(smart_contract_gas_used.clone()))?;
        registry.register(Box::new(smart_contract_errors_total.clone()))?;
        registry.register(Box::new(batch_size_histogram.clone()))?;
        registry.register(Box::new(batch_processing_duration.clone()))?;
        registry.register(Box::new(transactions_per_batch.clone()))?;
        registry.register(Box::new(batch_proof_generation_duration.clone()))?;
        registry.register(Box::new(sindri_api_calls_total.clone()))?;
        registry.register(Box::new(sindri_api_call_duration.clone()))?;
        registry.register(Box::new(sindri_proof_generation_duration.clone()))?;
        registry.register(Box::new(sindri_proof_status.clone()))?;
        registry.register(Box::new(sindri_cost_per_batch.clone()))?;
        registry.register(Box::new(sindri_proof_cycles.clone()))?;
        registry.register(Box::new(sindri_api_errors_total.clone()))?;
        registry.register(Box::new(proof_generation_requests.clone()))?;
        registry.register(Box::new(proof_verification_requests.clone()))?;
        registry.register(Box::new(proof_size_bytes.clone()))?;
        registry.register(Box::new(proof_public_values_size.clone()))?;
        
        Ok(MetricsCollector {
            registry,
            http_requests_total,
            http_request_duration,
            http_requests_in_flight,
            db_operations_total,
            db_operation_duration,
            db_connections_active,
            db_connections_idle,
            db_query_errors_total,
            transactions_processed_total,
            batches_created_total,
            merkle_tree_operations,
            ads_operations_duration,
            memory_usage_bytes,
            cpu_usage_percent,
            open_file_descriptors,
            smart_contract_calls_total,
            smart_contract_call_duration,
            smart_contract_gas_used,
            smart_contract_errors_total,
            batch_size_histogram,
            batch_processing_duration,
            transactions_per_batch,
            batch_proof_generation_duration,
            sindri_api_calls_total,
            sindri_api_call_duration,
            sindri_proof_generation_duration,
            sindri_proof_status,
            sindri_cost_per_batch,
            sindri_proof_cycles,
            sindri_api_errors_total,
            proof_generation_requests,
            proof_verification_requests,
            proof_size_bytes,
            proof_public_values_size,
        })
    }
    
    /// Get metrics in Prometheus text format
    pub fn get_metrics_text(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families)
    }
}

/// Initialize global metrics collector
pub async fn init_metrics() -> Result<(), prometheus::Error> {
    let collector = MetricsCollector::new()?;
    let mut registry_guard = METRICS_REGISTRY.write().await;
    *registry_guard = Some(collector);
    info!("✅ Prometheus metrics collector initialized");
    Ok(())
}

/// Get reference to global metrics collector
pub async fn get_metrics() -> Option<MetricsCollector> {
    let registry_guard = METRICS_REGISTRY.read().await;
    registry_guard.clone()
}

/// Record HTTP request metrics
pub async fn record_http_request(
    method: &str,
    endpoint: &str,
    status_code: u16,
    duration_seconds: f64,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .http_requests_total
            .with_label_values(&[method, endpoint, &status_code.to_string()])
            .inc();
        
        metrics
            .http_request_duration
            .with_label_values(&[method, endpoint])
            .observe(duration_seconds);
    }
}

/// Record database operation metrics
pub async fn record_db_operation(
    operation: &str,
    table: &str,
    status: &str,
    duration_seconds: f64,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .db_operations_total
            .with_label_values(&[operation, table, status])
            .inc();
        
        metrics
            .db_operation_duration
            .with_label_values(&[operation, table])
            .observe(duration_seconds);
    }
}

/// Record smart contract call metrics
pub async fn record_smart_contract_call(
    contract: &str,
    method: &str,
    chain_id: &str,
    status: &str,
    duration_seconds: f64,
    gas_used: Option<u64>,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .smart_contract_calls_total
            .with_label_values(&[contract, method, status, chain_id])
            .inc();
        
        metrics
            .smart_contract_call_duration
            .with_label_values(&[contract, method])
            .observe(duration_seconds);
        
        if let Some(gas) = gas_used {
            metrics
                .smart_contract_gas_used
                .with_label_values(&[contract, method, chain_id])
                .inc_by(gas);
        }
    }
}

/// Record batch processing metrics
pub async fn record_batch_processing(
    stage: &str,
    trigger_source: &str,
    duration_seconds: f64,
    batch_size: usize,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .batch_processing_duration
            .with_label_values(&[stage, trigger_source])
            .observe(duration_seconds);
        
        metrics.batch_size_histogram.observe(batch_size as f64);
        metrics.transactions_per_batch.observe(batch_size as f64);
        
        if stage == "complete" {
            metrics.batches_created_total.inc();
            metrics.transactions_processed_total.inc_by(batch_size as u64);
        }
    }
}

/// Record Sindri API call metrics
pub async fn record_sindri_api_call(
    endpoint: &str,
    method: &str,
    status_code: u16,
    duration_seconds: f64,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .sindri_api_calls_total
            .with_label_values(&[endpoint, method, &status_code.to_string()])
            .inc();
        
        metrics
            .sindri_api_call_duration
            .with_label_values(&[endpoint, method])
            .observe(duration_seconds);
    }
}

/// Record Sindri proof generation metrics
pub async fn record_sindri_proof_generation(
    circuit: &str,
    proof_system: &str,
    status: &str,
    duration_seconds: Option<f64>,
    cost_usd: Option<f64>,
    cycles: Option<u64>,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .sindri_proof_status
            .with_label_values(&[status, circuit])
            .inc();
        
        if let Some(duration) = duration_seconds {
            metrics
                .sindri_proof_generation_duration
                .with_label_values(&[circuit, proof_system])
                .observe(duration);
        }
        
        if let Some(cost) = cost_usd {
            metrics
                .sindri_cost_per_batch
                .with_label_values(&[circuit, proof_system])
                .set(cost);
        }
        
        if let Some(cycle_count) = cycles {
            metrics
                .sindri_proof_cycles
                .with_label_values(&[circuit, proof_system])
                .inc_by(cycle_count);
        }
    }
}

/// Record ADS operation metrics
pub async fn record_ads_operation(
    operation: &str,
    tree_type: &str,
    duration_seconds: f64,
) {
    if let Some(metrics) = get_metrics().await {
        metrics
            .merkle_tree_operations
            .with_label_values(&[operation, tree_type])
            .inc();
        
        metrics
            .ads_operations_duration
            .with_label_values(&[operation, tree_type])
            .observe(duration_seconds);
    }
}

/// Update system metrics (called periodically by a background task)
pub async fn update_system_metrics() {
    if let Some(metrics) = get_metrics().await {
        // Update memory usage
        if let Ok(memory) = get_memory_usage() {
            metrics.memory_usage_bytes.set(memory as i64);
        }
        
        // Update CPU usage
        if let Ok(cpu) = get_cpu_usage() {
            metrics.cpu_usage_percent.set(cpu);
        }
        
        // Update file descriptors
        if let Ok(fds) = get_open_file_descriptors() {
            metrics.open_file_descriptors.set(fds as i64);
        }
    }
}

/// Get current memory usage in bytes (Linux-specific)
fn get_memory_usage() -> Result<u64, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let contents = std::fs::read_to_string("/proc/self/status")?;
        for line in contents.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        warn!("Memory usage monitoring not implemented for this platform");
    }
    
    Ok(0)
}

/// Get current CPU usage percentage (simplified implementation)
fn get_cpu_usage() -> Result<f64, Box<dyn std::error::Error>> {
    // This is a simplified implementation
    // In production, you might want to use a more sophisticated CPU monitoring library
    #[cfg(target_os = "linux")]
    {
        let contents = std::fs::read_to_string("/proc/loadavg")?;
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if !parts.is_empty() {
            let load: f64 = parts[0].parse()?;
            // Convert load average to rough percentage (assuming single core)
            return Ok((load * 100.0).min(100.0));
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        warn!("CPU usage monitoring not implemented for this platform");
    }
    
    Ok(0.0)
}

/// Get number of open file descriptors (Linux-specific)
fn get_open_file_descriptors() -> Result<usize, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let fd_dir = std::fs::read_dir("/proc/self/fd")?;
        return Ok(fd_dir.count());
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        warn!("File descriptor monitoring not implemented for this platform");
    }
    
    Ok(0)
}

/// Start background system metrics collection task
pub fn start_system_metrics_task() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            update_system_metrics().await;
        }
    })
}