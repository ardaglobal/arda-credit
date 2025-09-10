# Prometheus Metrics Setup for Arda Credit API

This document outlines the comprehensive Prometheus metrics integration that has been implemented for the Arda Credit API system to monitor both hot and cold path operations as specified in Linear issue ARDGBL-318.

## Overview

The Prometheus SDK has been integrated into the Rust codebase to provide detailed observability for:

### Hot Path Metrics (High Frequency)
- **HTTP Requests**: Request count, duration, in-flight requests by method/endpoint/status
- **Database Operations**: Query count, duration, connection pool status, errors by operation/table
- **Code Level Traces**: ADS operations, merkle tree operations, batch processing stages
- **System Resources**: CPU usage, memory usage, file descriptors (for infrastructure collection)

### Cold Path Metrics (Lower Frequency, High Value)
- **Smart Contract Interactions**: Call count, duration, gas usage, transaction costs by contract/method
- **Batch Processing**: Batch size distribution, processing duration, transactions per batch
- **Sindri API**: Proof generation requests, API call duration, proof status, cost tracking
- **ZK Proof Generation**: Proof size, public values size, generation duration, cycles used

## Architecture

### Core Components

1. **`api/src/metrics.rs`** - Main metrics collector with all metric definitions
2. **`api/src/middleware/metrics.rs`** - HTTP request instrumentation middleware  
3. **`api/src/db_metrics.rs`** - Database operation instrumentation wrapper
4. **`api/src/server.rs`** - Metrics endpoint (`/metrics`) and initialization

### Metrics Categories

#### HTTP Request Metrics
```rust
// Request count by method, endpoint, status code
arda_credit_api_http_requests_total{method="GET", endpoint="/api/v2/batches", status_code="200"}

// Request duration histograms
arda_credit_api_http_request_duration_seconds{method="POST", endpoint="/api/v2/transactions"}

// In-flight requests gauge
arda_credit_api_http_requests_in_flight
```

#### Database Metrics
```rust
// Operation count by operation type, table, status
arda_credit_db_operations_total{operation="SELECT", table="batches", status="success"}

// Operation duration
arda_credit_db_operation_duration_seconds{operation="INSERT", table="transactions"}

// Connection pool status
arda_credit_db_connections_active
arda_credit_db_connections_idle
```

#### Smart Contract Metrics
```rust
// Contract call metrics
arda_credit_ethereum_smart_contract_calls_total{contract="ILoan", method="updateState", status="success", chain_id="1"}

// Gas usage tracking
arda_credit_ethereum_smart_contract_gas_used_total{contract="ILoan", method="updateState", chain_id="1"}

// Transaction costs in ETH
arda_credit_ethereum_smart_contract_tx_cost_eth{contract="ILoan", method="updateState", chain_id="1"}
```

#### Batch Processing Metrics
```rust
// Batch processing stages
arda_credit_batch_processing_duration_seconds{stage="complete", trigger_source="api"}

// Batch size distribution
arda_credit_batch_size_transactions

// Transactions per batch
arda_credit_batch_transactions_per_batch
```

#### Sindri API Metrics
```rust
// API call metrics
arda_credit_sindri_api_calls_total{endpoint="prove_circuit", method="POST", status_code="200"}

// Proof generation duration
arda_credit_sindri_proof_generation_duration_seconds{circuit="arda-credit", proof_system="groth16"}

// Cost tracking
arda_credit_sindri_cost_per_batch_usd{circuit="arda-credit", proof_system="groth16"}

// Proof cycles
arda_credit_sindri_proof_cycles_total{circuit="arda-credit", proof_system="groth16"}
```

#### ADS (Authenticated Data Structure) Metrics
```rust
// Merkle tree operations
arda_credit_ads_merkle_tree_operations_total{operation="process_loan_batch", tree_type="indexed_merkle_tree"}

// ADS operation duration
arda_credit_ads_operations_duration_seconds{operation="process_loan_batch", tree_type="indexed_merkle_tree"}
```

## Implementation Details

### Automatic Instrumentation

#### HTTP Requests
All HTTP requests are automatically instrumented via middleware:
```rust
// In server.rs
let router = router.layer(axum::middleware::from_fn(
    crate::middleware::metrics_middleware,
));
```

#### Database Operations
Database operations can be instrumented using the provided wrapper:
```rust
use crate::instrument_db;

// Instrument a database query
let result = instrument_db!("SELECT", "users", {
    sqlx::query!("SELECT * FROM users WHERE id = ?", user_id)
        .fetch_one(&pool)
        .await
}).await?;
```

#### Smart Contract Calls
Smart contract interactions are instrumented in the ethereum-client:
```rust
// Record successful contract call
record_smart_contract_call(
    "ILoan", 
    "updateState", 
    &chain_id, 
    "success", 
    duration_seconds,
    Some(gas_used),
    Some(gas_price_gwei),
    Some(tx_cost_eth)
).await;
```

### Metrics Endpoint

The `/metrics` endpoint is automatically exposed and provides Prometheus-compatible metrics:

```
GET /metrics
Content-Type: text/plain; version=0.0.4; charset=utf-8

# HELP arda_credit_api_http_requests_total Total number of HTTP requests
# TYPE arda_credit_api_http_requests_total counter
arda_credit_api_http_requests_total{method="GET",endpoint="/health",status_code="200"} 42

# HELP arda_credit_batch_processing_duration_seconds Total batch processing duration in seconds
# TYPE arda_credit_batch_processing_duration_seconds histogram
arda_credit_batch_processing_duration_seconds_bucket{stage="complete",trigger_source="api",le="1"} 0
arda_credit_batch_processing_duration_seconds_bucket{stage="complete",trigger_source="api",le="5"} 2
arda_credit_batch_processing_duration_seconds_bucket{stage="complete",trigger_source="api",le="+Inf"} 5
arda_credit_batch_processing_duration_seconds_sum{stage="complete",trigger_source="api"} 12.5
arda_credit_batch_processing_duration_seconds_count{stage="complete",trigger_source="api"} 5
```

## Configuration

### Environment Variables

```bash
# Enable Prometheus metrics (default: true)
PROMETHEUS_METRICS_ENABLED=true

# Metrics collection interval for system metrics (default: 30s)
SYSTEM_METRICS_INTERVAL=30
```

### Prometheus Configuration

Add the following to your `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'arda-credit-api'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 10s
```

## Grafana Dashboard

### Key Panels to Create

1. **HTTP Request Rate & Latency**
   - Query: `rate(arda_credit_api_http_requests_total[5m])`
   - Query: `histogram_quantile(0.95, rate(arda_credit_api_http_request_duration_seconds_bucket[5m]))`

2. **Database Performance**
   - Query: `rate(arda_credit_db_operations_total[5m])`
   - Query: `histogram_quantile(0.95, rate(arda_credit_db_operation_duration_seconds_bucket[5m]))`

3. **Batch Processing Throughput**
   - Query: `rate(arda_credit_app_batches_created_total[5m])`
   - Query: `rate(arda_credit_app_transactions_processed_total[5m])`

4. **Smart Contract Costs**
   - Query: `rate(arda_credit_ethereum_smart_contract_gas_used_total[5m])`
   - Query: `arda_credit_ethereum_smart_contract_tx_cost_eth`

5. **Sindri API Performance**
   - Query: `rate(arda_credit_sindri_api_calls_total[5m])`
   - Query: `arda_credit_sindri_cost_per_batch_usd`
   - Query: `histogram_quantile(0.95, rate(arda_credit_sindri_proof_generation_duration_seconds_bucket[1h]))`

## Alerting Rules

### Critical Alerts

```yaml
groups:
  - name: arda-credit-critical
    rules:
      - alert: HighErrorRate
        expr: rate(arda_credit_api_http_requests_total{status_code=~"5.."}[5m]) > 0.1
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          
      - alert: DatabaseConnectionPoolExhausted
        expr: arda_credit_db_connections_active / (arda_credit_db_connections_active + arda_credit_db_connections_idle) > 0.9
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Database connection pool nearly exhausted"
          
      - alert: SmartContractCallFailures
        expr: rate(arda_credit_ethereum_smart_contract_calls_total{status="error"}[5m]) > 0.05
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Smart contract call failure rate elevated"
          
      - alert: SindriProofGenerationStuck
        expr: increase(arda_credit_sindri_proof_status_total{status="pending"}[30m]) > 5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Sindri proof generation appears to be stuck"
```

### Performance Alerts

```yaml
  - name: arda-credit-performance
    rules:
      - alert: SlowDatabaseQueries
        expr: histogram_quantile(0.95, rate(arda_credit_db_operation_duration_seconds_bucket[5m])) > 1.0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Database queries are running slowly"
          
      - alert: HighBatchProcessingTime
        expr: histogram_quantile(0.95, rate(arda_credit_batch_processing_duration_seconds_bucket[5m])) > 300
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Batch processing taking longer than expected"
          
      - alert: HighSindriCosts
        expr: arda_credit_sindri_cost_per_batch_usd > 10.0
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Sindri costs per batch are high"
```

## Usage Examples

### Recording Custom Metrics

```rust
use crate::metrics::{record_batch_processing, record_sindri_proof_generation};

// Record batch processing metrics
record_batch_processing(
    "complete",           // stage
    "api",               // trigger_source
    duration_seconds,    // duration
    batch_size          // number of transactions
).await;

// Record Sindri proof generation
record_sindri_proof_generation(
    "arda-credit",      // circuit
    "groth16",          // proof_system
    "ready",            // status
    Some(1800.0),       // duration_seconds
    Some(5.50),         // cost_usd
    Some(1_000_000),    // cycles
    Some(2048)          // proof_size_bytes
).await;
```

### Querying Metrics

```rust
use crate::metrics::get_metrics;

// Get current metrics collector
if let Some(metrics) = get_metrics().await {
    // Access specific metrics
    let request_count = metrics.http_requests_total
        .with_label_values(&["GET", "/health", "200"])
        .get();
    
    println!("Health check requests: {}", request_count);
}
```

## Troubleshooting

### Common Issues

1. **Metrics not appearing**: Ensure `init_metrics()` is called during server startup
2. **High cardinality warnings**: Review label values to avoid unbounded label sets
3. **Memory usage**: Monitor metrics collector memory usage with high-traffic systems

### Debug Endpoints

- `GET /metrics` - View all metrics in Prometheus format
- `GET /health` - Basic health check (also recorded in metrics)

### Log Messages

Look for these log messages during startup:
```
✅ Prometheus metrics collector initialized
✅ API server initialized successfully with authentication
```

## Performance Considerations

- Metrics collection adds minimal overhead (~1-2% CPU)
- Label cardinality should be kept reasonable (< 10k unique combinations per metric)
- System metrics are collected every 30 seconds by default
- Database instrumentation can be disabled for high-frequency operations if needed

## Security

- The `/metrics` endpoint is accessible without authentication for Prometheus scraping
- Sensitive data is not included in metric labels
- Consider network-level restrictions for the metrics endpoint in production

## Future Enhancements

1. **Distributed Tracing**: Integration with Jaeger/Zipkin for request tracing
2. **Custom Dashboards**: Pre-built Grafana dashboards for common use cases
3. **SLO/SLI Tracking**: Service Level Objective monitoring
4. **Cost Attribution**: More granular cost tracking per user/operation
5. **Predictive Alerting**: ML-based anomaly detection for metrics

This comprehensive metrics setup provides full observability into both the high-frequency operations (hot path) and the critical but less frequent operations (cold path) of the Arda Credit system, enabling effective monitoring, alerting, and performance optimization.