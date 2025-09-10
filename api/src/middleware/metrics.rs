//! Metrics middleware for automatic HTTP request instrumentation
//!
//! This middleware automatically records metrics for all HTTP requests including:
//! - Request count by method, endpoint, and status code
//! - Request duration histograms
//! - In-flight request gauge

use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{debug, warn};

use crate::metrics::{get_metrics, record_http_request};

/// Middleware function to instrument HTTP requests with Prometheus metrics
pub async fn metrics_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    let method = request.method().to_string();
    
    // Get the matched path for consistent endpoint labeling
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| matched_path.as_str())
        .unwrap_or_else(|| request.uri().path());
    
    // Normalize path for metrics (remove IDs and query parameters)
    let normalized_path = normalize_path(path);
    
    // Increment in-flight requests gauge
    if let Some(metrics) = get_metrics().await {
        metrics.http_requests_in_flight.inc();
    }
    
    debug!(
        "📊 HTTP request started: {} {} (normalized: {})",
        method, path, normalized_path
    );
    
    // Process the request
    let response = next.run(request).await;
    
    // Calculate duration and record metrics
    let duration = start_time.elapsed();
    let duration_seconds = duration.as_secs_f64();
    let status_code = response.status().as_u16();
    
    // Record the metrics
    record_http_request(&method, &normalized_path, status_code, duration_seconds).await;
    
    // Decrement in-flight requests gauge
    if let Some(metrics) = get_metrics().await {
        metrics.http_requests_in_flight.dec();
    }
    
    debug!(
        "📊 HTTP request completed: {} {} -> {} ({:.3}s)",
        method, normalized_path, status_code, duration_seconds
    );
    
    Ok(response)
}

/// Normalize URL paths for consistent metric labeling
/// 
/// This function removes variable parts like IDs, UUIDs, and query parameters
/// to create consistent metric labels while preserving the endpoint structure.
fn normalize_path(path: &str) -> String {
    // Remove query parameters
    let path = path.split('?').next().unwrap_or(path);
    
    // Split path into segments
    let segments: Vec<&str> = path.split('/').collect();
    let mut normalized_segments = Vec::new();
    
    for segment in segments {
        if segment.is_empty() {
            normalized_segments.push(segment);
            continue;
        }
        
        // Check if segment looks like an ID (UUID, number, or hex)
        if is_id_segment(segment) {
            normalized_segments.push("{id}");
        } else {
            normalized_segments.push(segment);
        }
    }
    
    let normalized = normalized_segments.join("/");
    
    // Ensure we don't have empty normalized paths
    if normalized.is_empty() || normalized == "/" {
        "/".to_string()
    } else {
        normalized
    }
}

/// Check if a path segment looks like an ID that should be normalized
fn is_id_segment(segment: &str) -> bool {
    // Check for UUID pattern (8-4-4-4-12 hex digits with dashes)
    if segment.len() == 36 && segment.chars().filter(|&c| c == '-').count() == 4 {
        return segment
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-');
    }
    
    // Check for pure numeric ID
    if segment.parse::<u64>().is_ok() {
        return true;
    }
    
    // Check for hex string (common for blockchain hashes)
    if segment.len() >= 8
        && segment.len() <= 66
        && (segment.starts_with("0x") || segment.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return true;
    }
    
    // Check for base64-like strings (common for encoded IDs)
    if segment.len() > 8
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    {
        return true;
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/api/v1/users"), "/api/v1/users");
        assert_eq!(normalize_path("/api/v1/users/123"), "/api/v1/users/{id}");
        assert_eq!(
            normalize_path("/api/v1/users/550e8400-e29b-41d4-a716-446655440000"),
            "/api/v1/users/{id}"
        );
        assert_eq!(
            normalize_path("/api/v1/transactions/0x1234567890abcdef"),
            "/api/v1/transactions/{id}"
        );
        assert_eq!(normalize_path("/api/v1/batches?limit=10"), "/api/v1/batches");
        assert_eq!(normalize_path("/health"), "/health");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
    }
    
    #[test]
    fn test_is_id_segment() {
        // UUIDs
        assert!(is_id_segment("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_id_segment("not-a-uuid"));
        
        // Numeric IDs
        assert!(is_id_segment("123"));
        assert!(is_id_segment("9876543210"));
        assert!(!is_id_segment("abc"));
        
        // Hex strings
        assert!(is_id_segment("0x1234567890abcdef"));
        assert!(is_id_segment("1234567890abcdef"));
        assert!(!is_id_segment("0xnotahex"));
        
        // Regular path segments
        assert!(!is_id_segment("users"));
        assert!(!is_id_segment("api"));
        assert!(!is_id_segment("v1"));
    }
}