//! Document operations monitoring and observability
//!
//! This module provides comprehensive metrics, logging, and monitoring
//! for document storage operations with security and performance tracking.

use metrics::{counter, gauge, histogram, register_counter, register_gauge, register_histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Document operation metrics collector
#[derive(Debug, Clone)]
pub struct DocumentMetrics {
    operation_counters: Arc<RwLock<HashMap<String, u64>>>,
    error_counters: Arc<RwLock<HashMap<String, u64>>>,
    performance_metrics: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    security_events: Arc<RwLock<Vec<SecurityEvent>>>,
}

impl DocumentMetrics {
    /// Create a new document metrics collector
    pub fn new() -> Self {
        // Register Prometheus metrics
        register_counter!("document_operations_total", "Total document operations");
        register_counter!("document_errors_total", "Total document operation errors");
        register_histogram!("document_operation_duration_seconds", "Document operation duration");
        register_gauge!("document_active_uploads", "Currently active document uploads");
        register_gauge!("document_storage_size_bytes", "Total document storage size");
        register_counter!("document_security_events_total", "Document security events");

        Self {
            operation_counters: Arc::new(RwLock::new(HashMap::new())),
            error_counters: Arc::new(RwLock::new(HashMap::new())),
            performance_metrics: Arc::new(RwLock::new(HashMap::new())),
            security_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a document operation
    pub async fn record_operation(
        &self,
        operation: &str,
        company_id: Uuid,
        user_id: Uuid,
        duration: Duration,
        success: bool,
    ) {
        let operation_key = format!("{}_{}", operation, if success { "success" } else { "failure" });

        // Update internal counters
        {
            let mut counters = self.operation_counters.write().await;
            *counters.entry(operation_key.clone()).or_insert(0) += 1;
        }

        // Update performance metrics
        {
            let mut perf_metrics = self.performance_metrics.write().await;
            perf_metrics
                .entry(operation.to_string())
                .or_insert_with(Vec::new)
                .push(duration);

            // Keep only last 1000 measurements
            let measurements = perf_metrics.get_mut(operation).unwrap();
            if measurements.len() > 1000 {
                measurements.drain(0..measurements.len() - 1000);
            }
        }

        // Update Prometheus metrics
        counter!("document_operations_total", 1, "operation" => operation, "success" => success.to_string());
        histogram!("document_operation_duration_seconds", duration.as_secs_f64(), "operation" => operation);

        if !success {
            counter!("document_errors_total", 1, "operation" => operation);
        }

        info!(
            operation = operation,
            company_id = %company_id,
            user_id = %user_id,
            duration_ms = duration.as_millis(),
            success = success,
            "Document operation recorded"
        );
    }

    /// Record a security event
    pub async fn record_security_event(
        &self,
        event_type: SecurityEventType,
        company_id: Uuid,
        user_id: Option<Uuid>,
        document_id: Option<Uuid>,
        details: Option<String>,
        severity: SecuritySeverity,
    ) {
        let event = SecurityEvent {
            event_type,
            company_id,
            user_id,
            document_id,
            details,
            severity,
            timestamp: chrono::Utc::now(),
        };

        // Store security event
        {
            let mut events = self.security_events.write().await;
            events.push(event.clone());

            // Keep only last 10000 events
            if events.len() > 10000 {
                events.drain(0..events.len() - 10000);
            }
        }

        // Update Prometheus metrics
        counter!(
            "document_security_events_total", 1,
            "event_type" => event_type.to_string(),
            "severity" => severity.to_string()
        );

        // Log security event
        match severity {
            SecuritySeverity::Low => info!(
                event_type = %event_type,
                company_id = %company_id,
                user_id = ?user_id,
                document_id = ?document_id,
                details = ?details,
                "Security event recorded"
            ),
            SecuritySeverity::Medium => warn!(
                event_type = %event_type,
                company_id = %company_id,
                user_id = ?user_id,
                document_id = ?document_id,
                details = ?details,
                "Security event recorded"
            ),
            SecuritySeverity::High | SecuritySeverity::Critical => error!(
                event_type = %event_type,
                company_id = %company_id,
                user_id = ?user_id,
                document_id = ?document_id,
                details = ?details,
                "Security event recorded"
            ),
        }
    }

    /// Record file size metrics
    pub async fn record_file_size(&self, operation: &str, size_bytes: u64) {
        histogram!(
            "document_file_size_bytes", size_bytes as f64,
            "operation" => operation
        );

        info!(
            operation = operation,
            size_bytes = size_bytes,
            size_mb = size_bytes / (1024 * 1024),
            "File size recorded"
        );
    }

    /// Update active uploads gauge
    pub fn update_active_uploads(&self, count: i64) {
        gauge!("document_active_uploads", count as f64);
    }

    /// Update total storage size
    pub fn update_storage_size(&self, size_bytes: u64) {
        gauge!("document_storage_size_bytes", size_bytes as f64);
    }

    /// Get operation statistics
    pub async fn get_operation_stats(&self) -> HashMap<String, u64> {
        self.operation_counters.read().await.clone()
    }

    /// Get error statistics
    pub async fn get_error_stats(&self) -> HashMap<String, u64> {
        self.error_counters.read().await.clone()
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> HashMap<String, PerformanceStats> {
        let perf_metrics = self.performance_metrics.read().await;
        let mut stats = HashMap::new();

        for (operation, durations) in perf_metrics.iter() {
            if !durations.is_empty() {
                let mut sorted_durations = durations.clone();
                sorted_durations.sort();

                let total_ms: u64 = durations.iter().map(|d| d.as_millis() as u64).sum();
                let count = durations.len() as u64;
                let avg_ms = total_ms / count;

                let p50_ms = sorted_durations[count as usize * 50 / 100].as_millis() as u64;
                let p95_ms = sorted_durations[count as usize * 95 / 100].as_millis() as u64;
                let p99_ms = sorted_durations[count as usize * 99 / 100].as_millis() as u64;

                stats.insert(
                    operation.clone(),
                    PerformanceStats {
                        count,
                        avg_ms,
                        p50_ms,
                        p95_ms,
                        p99_ms,
                        min_ms: sorted_durations.first().unwrap().as_millis() as u64,
                        max_ms: sorted_durations.last().unwrap().as_millis() as u64,
                    },
                );
            }
        }

        stats
    }

    /// Get recent security events
    pub async fn get_recent_security_events(&self, limit: usize) -> Vec<SecurityEvent> {
        let events = self.security_events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get security events by severity
    pub async fn get_security_events_by_severity(
        &self,
        severity: SecuritySeverity,
        limit: usize,
    ) -> Vec<SecurityEvent> {
        let events = self.security_events.read().await;
        events
            .iter()
            .rev()
            .filter(|e| e.severity == severity)
            .take(limit)
            .cloned()
            .collect()
    }
}

impl Default for DocumentMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance statistics for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub count: u64,
    pub avg_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
}

/// Security event types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityEventType {
    UnauthorizedAccess,
    InvalidFileType,
    FileSizeExceeded,
    IntegrityVerificationFailed,
    SuspiciousActivity,
    RateLimitExceeded,
    InvalidAuthentication,
    PermissionDenied,
    DataExfiltrationAttempt,
    MaliciousFileDetected,
}

impl std::fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnauthorizedAccess => write!(f, "unauthorized_access"),
            Self::InvalidFileType => write!(f, "invalid_file_type"),
            Self::FileSizeExceeded => write!(f, "file_size_exceeded"),
            Self::IntegrityVerificationFailed => write!(f, "integrity_verification_failed"),
            Self::SuspiciousActivity => write!(f, "suspicious_activity"),
            Self::RateLimitExceeded => write!(f, "rate_limit_exceeded"),
            Self::InvalidAuthentication => write!(f, "invalid_authentication"),
            Self::PermissionDenied => write!(f, "permission_denied"),
            Self::DataExfiltrationAttempt => write!(f, "data_exfiltration_attempt"),
            Self::MaliciousFileDetected => write!(f, "malicious_file_detected"),
        }
    }
}

/// Security event severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for SecuritySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Security event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub company_id: Uuid,
    pub user_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub details: Option<String>,
    pub severity: SecuritySeverity,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Document operation timer for measuring performance
pub struct DocumentOperationTimer {
    operation: String,
    company_id: Uuid,
    user_id: Uuid,
    start_time: Instant,
    metrics: Arc<DocumentMetrics>,
}

impl DocumentOperationTimer {
    /// Start a new operation timer
    pub fn new(
        operation: String,
        company_id: Uuid,
        user_id: Uuid,
        metrics: Arc<DocumentMetrics>,
    ) -> Self {
        Self {
            operation,
            company_id,
            user_id,
            start_time: Instant::now(),
            metrics,
        }
    }

    /// Complete the operation and record metrics
    pub async fn complete(self, success: bool) {
        let duration = self.start_time.elapsed();
        self.metrics
            .record_operation(&self.operation, self.company_id, self.user_id, duration, success)
            .await;
    }

    /// Complete with error and record metrics
    pub async fn complete_with_error(self, error: &str) {
        let duration = self.start_time.elapsed();
        self.metrics
            .record_operation(&self.operation, self.company_id, self.user_id, duration, false)
            .await;

        error!(
            operation = %self.operation,
            company_id = %self.company_id,
            user_id = %self.user_id,
            duration_ms = duration.as_millis(),
            error = error,
            "Document operation failed"
        );
    }
}

/// Document health check metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentHealthMetrics {
    pub total_documents: u64,
    pub active_documents: u64,
    pub pending_approvals: u64,
    pub failed_uploads: u64,
    pub storage_size_bytes: u64,
    pub avg_upload_time_ms: u64,
    pub error_rate_percent: f64,
    pub security_events_last_24h: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Document metrics dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetricsDashboard {
    pub health_metrics: DocumentHealthMetrics,
    pub operation_stats: HashMap<String, u64>,
    pub performance_stats: HashMap<String, PerformanceStats>,
    pub recent_security_events: Vec<SecurityEvent>,
    pub top_companies_by_usage: Vec<CompanyUsageStats>,
    pub file_type_distribution: HashMap<String, u64>,
}

/// Company usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyUsageStats {
    pub company_id: Uuid,
    pub company_name: String,
    pub document_count: u64,
    pub storage_size_bytes: u64,
    pub upload_count_last_30d: u64,
    pub download_count_last_30d: u64,
}

/// Metrics middleware for automatic operation tracking
pub async fn document_metrics_middleware<F, Fut, T>(
    operation: &str,
    company_id: Uuid,
    user_id: Uuid,
    metrics: Arc<DocumentMetrics>,
    operation_fn: F,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
{
    let timer = DocumentOperationTimer::new(operation.to_string(), company_id, user_id, metrics);

    match operation_fn().await {
        Ok(result) => {
            timer.complete(true).await;
            Ok(result)
        }
        Err(error) => {
            timer.complete_with_error(&error.to_string()).await;
            Err(error)
        }
    }
}

/// Alert thresholds for document operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAlertThresholds {
    pub error_rate_percent: f64,
    pub avg_response_time_ms: u64,
    pub security_events_per_hour: u64,
    pub storage_size_gb: u64,
    pub failed_uploads_per_hour: u64,
}

impl Default for DocumentAlertThresholds {
    fn default() -> Self {
        Self {
            error_rate_percent: 5.0,     // 5% error rate
            avg_response_time_ms: 5000,  // 5 seconds
            security_events_per_hour: 10, // 10 security events per hour
            storage_size_gb: 1000,       // 1TB storage
            failed_uploads_per_hour: 50, // 50 failed uploads per hour
        }
    }
}

/// Document alert manager
pub struct DocumentAlertManager {
    thresholds: DocumentAlertThresholds,
    metrics: Arc<DocumentMetrics>,
}

impl DocumentAlertManager {
    /// Create a new alert manager
    pub fn new(thresholds: DocumentAlertThresholds, metrics: Arc<DocumentMetrics>) -> Self {
        Self { thresholds, metrics }
    }

    /// Check for alert conditions
    pub async fn check_alerts(&self) -> Vec<DocumentAlert> {
        let mut alerts = Vec::new();

        // Check error rate
        let operation_stats = self.metrics.get_operation_stats().await;
        let total_ops: u64 = operation_stats.values().sum();
        let failed_ops: u64 = operation_stats
            .iter()
            .filter(|(k, _)| k.contains("failure"))
            .map(|(_, v)| *v)
            .sum();

        if total_ops > 0 {
            let error_rate = (failed_ops as f64 / total_ops as f64) * 100.0;
            if error_rate > self.thresholds.error_rate_percent {
                alerts.push(DocumentAlert {
                    alert_type: DocumentAlertType::HighErrorRate,
                    severity: if error_rate > 20.0 {
                        SecuritySeverity::Critical
                    } else if error_rate > 10.0 {
                        SecuritySeverity::High
                    } else {
                        SecuritySeverity::Medium
                    },
                    message: format!("Document error rate is {:.2}%", error_rate),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // Check performance
        let perf_stats = self.metrics.get_performance_stats().await;
        for (operation, stats) in perf_stats {
            if stats.avg_ms > self.thresholds.avg_response_time_ms {
                alerts.push(DocumentAlert {
                    alert_type: DocumentAlertType::SlowPerformance,
                    severity: if stats.avg_ms > self.thresholds.avg_response_time_ms * 2 {
                        SecuritySeverity::High
                    } else {
                        SecuritySeverity::Medium
                    },
                    message: format!(
                        "Operation {} average response time is {}ms",
                        operation, stats.avg_ms
                    ),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // Check security events
        let recent_events = self.metrics.get_recent_security_events(100).await;
        let high_severity_events = recent_events
            .iter()
            .filter(|e| {
                matches!(e.severity, SecuritySeverity::High | SecuritySeverity::Critical)
                    && e.timestamp > chrono::Utc::now() - chrono::Duration::hours(1)
            })
            .count();

        if high_severity_events > self.thresholds.security_events_per_hour as usize {
            alerts.push(DocumentAlert {
                alert_type: DocumentAlertType::SecurityThreat,
                severity: SecuritySeverity::Critical,
                message: format!(
                    "High number of security events in last hour: {}",
                    high_severity_events
                ),
                timestamp: chrono::Utc::now(),
            });
        }

        alerts
    }
}

/// Document alert types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentAlertType {
    HighErrorRate,
    SlowPerformance,
    SecurityThreat,
    StorageQuotaExceeded,
    UnusualActivity,
}

/// Document alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAlert {
    pub alert_type: DocumentAlertType,
    pub severity: SecuritySeverity,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_document_metrics_creation() {
        let metrics = DocumentMetrics::new();
        let stats = metrics.get_operation_stats().await;
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_operation_recording() {
        let metrics = DocumentMetrics::new();
        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        metrics
            .record_operation("upload", company_id, user_id, Duration::from_millis(100), true)
            .await;

        let stats = metrics.get_operation_stats().await;
        assert_eq!(stats.get("upload_success"), Some(&1));
    }

    #[tokio::test]
    async fn test_security_event_recording() {
        let metrics = DocumentMetrics::new();
        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        metrics
            .record_security_event(
                SecurityEventType::UnauthorizedAccess,
                company_id,
                Some(user_id),
                None,
                Some("Test event".to_string()),
                SecuritySeverity::High,
            )
            .await;

        let events = metrics.get_recent_security_events(10).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, SecurityEventType::UnauthorizedAccess);
        assert_eq!(events[0].severity, SecuritySeverity::High);
    }

    #[tokio::test]
    async fn test_performance_stats() {
        let metrics = DocumentMetrics::new();
        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Record multiple operations
        for i in 1..=10 {
            metrics
                .record_operation(
                    "test_op",
                    company_id,
                    user_id,
                    Duration::from_millis(i * 10),
                    true,
                )
                .await;
        }

        let perf_stats = metrics.get_performance_stats().await;
        let test_op_stats = perf_stats.get("test_op").unwrap();

        assert_eq!(test_op_stats.count, 10);
        assert_eq!(test_op_stats.min_ms, 10);
        assert_eq!(test_op_stats.max_ms, 100);
        assert_eq!(test_op_stats.avg_ms, 55); // (10+20+...+100)/10 = 55
    }

    #[tokio::test]
    async fn test_operation_timer() {
        let metrics = Arc::new(DocumentMetrics::new());
        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let timer = DocumentOperationTimer::new(
            "test_timer".to_string(),
            company_id,
            user_id,
            metrics.clone(),
        );

        // Simulate some work
        sleep(Duration::from_millis(10)).await;

        timer.complete(true).await;

        let stats = metrics.get_operation_stats().await;
        assert_eq!(stats.get("test_timer_success"), Some(&1));
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let metrics = Arc::new(DocumentMetrics::new());
        let thresholds = DocumentAlertThresholds {
            error_rate_percent: 10.0,
            ..Default::default()
        };
        let alert_manager = DocumentAlertManager::new(thresholds, metrics.clone());

        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Record some failed operations to trigger alert
        for _ in 0..5 {
            metrics
                .record_operation("test", company_id, user_id, Duration::from_millis(100), false)
                .await;
        }

        // Record some successful operations
        for _ in 0..5 {
            metrics
                .record_operation("test", company_id, user_id, Duration::from_millis(100), true)
                .await;
        }

        let alerts = alert_manager.check_alerts().await;
        assert!(!alerts.is_empty());

        let error_rate_alert = alerts
            .iter()
            .find(|a| a.alert_type == DocumentAlertType::HighErrorRate);
        assert!(error_rate_alert.is_some());
    }

    #[tokio::test]
    async fn test_metrics_middleware() {
        let metrics = Arc::new(DocumentMetrics::new());
        let company_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let result = document_metrics_middleware(
            "test_middleware",
            company_id,
            user_id,
            metrics.clone(),
            || async { Ok::<String, Box<dyn std::error::Error + Send + Sync>>("success".to_string()) },
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let stats = metrics.get_operation_stats().await;
        assert_eq!(stats.get("test_middleware_success"), Some(&1));
    }
}