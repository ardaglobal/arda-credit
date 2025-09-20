# Arda Credit Observability Stack

This document describes the comprehensive observability setup for the Arda Credit platform, including Prometheus metrics collection, Grafana dashboards, and monitoring best practices.

## Overview

The observability stack provides comprehensive monitoring for both **hot path** (high-frequency) and **cold path** (blockchain/proving) operations:

### Hot Path Metrics (High Frequency)
- HTTP request/response metrics
- Database operations and performance
- System resource usage (CPU, memory)
- API endpoint performance

### Cold Path Metrics (Blockchain & Proving)
- Smart contract interactions and gas usage
- Sindri proof generation times and costs
- Batch processing metrics
- Transaction throughput

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Arda Credit   │───▶│   Prometheus    │───▶│    Grafana      │
│      API        │    │    (Metrics)    │    │  (Dashboards)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │
         ▼
┌─────────────────┐
│   Ethereum      │
│    Client       │
└─────────────────┘
         │
         ▼
┌─────────────────┐
│     Sindri      │
│   Integration   │
└─────────────────┘
```

## Setup Instructions

### 1. Start the Monitoring Stack

```bash
# Start Prometheus and Grafana
docker-compose up -d prometheus grafana

# Verify services are running
docker-compose ps
```

### 2. Access Dashboards

- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `admin`
- **Prometheus**: http://localhost:9090
- **API Metrics**: http://localhost:8080/metrics

### 3. Import Dashboards

The main dashboard is automatically provisioned at startup:
- **Arda Credit Overview**: Comprehensive view of all metrics

## Metrics Reference

### Hot Path Metrics

#### HTTP Metrics
```prometheus
# Request rate by endpoint and method
rate(http_requests_total[5m])

# Request duration percentiles
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))

# Response status codes
rate(http_responses_total[5m])
```

#### Database Metrics
```prometheus
# Database operation rate
rate(db_operations_total[5m])

# Database operation duration
histogram_quantile(0.95, rate(db_operation_duration_seconds_bucket[5m]))

# Active database connections
db_connections_active
```

#### System Metrics
```prometheus
# CPU usage percentage
system_cpu_usage_percent

# Memory usage
system_memory_usage_bytes
system_memory_available_bytes
```

### Cold Path Metrics

#### Smart Contract Metrics
```prometheus
# Smart contract call rate
rate(smart_contract_calls_total[5m])

# Transaction duration
histogram_quantile(0.95, rate(smart_contract_transaction_duration_seconds_bucket[5m]))

# Gas usage
histogram_quantile(0.95, rate(smart_contract_gas_used_bucket[5m]))
```

#### Sindri Metrics
```prometheus
# Proof generation rate
rate(sindri_proof_generation_total[5m])

# Proving duration
histogram_quantile(0.95, rate(sindri_proof_generation_duration_seconds_bucket[5m]))

# Transactions per batch
histogram_quantile(0.95, rate(sindri_proof_transactions_per_batch_bucket[5m]))

# API request rate
rate(sindri_api_requests_total[5m])
```

#### Batch Processing Metrics
```prometheus
# Batch processing duration
histogram_quantile(0.95, rate(batch_processing_duration_seconds_bucket[5m]))

# Batch processing success rate
rate(batch_processing_total{status="success"}[5m]) / rate(batch_processing_total[5m])

# Batch size distribution
histogram_quantile(0.95, rate(batch_size_bucket[5m]))
```

## Code Integration

### Adding New Metrics

#### 1. Counter Metrics
```rust
use metrics::counter;

// Increment a counter
counter!("my_operation_total", "operation" => "create").increment(1);
```

#### 2. Histogram Metrics
```rust
use metrics::histogram;

let start = std::time::Instant::now();
// ... perform operation ...
let duration = start.elapsed();
histogram!("my_operation_duration_seconds").record(duration.as_secs_f64());
```

#### 3. Gauge Metrics
```rust
use metrics::gauge;

// Set a gauge value
gauge!("active_connections").set(connection_count as f64);
```

### Instrumentation Examples

#### HTTP Endpoint Instrumentation
```rust
use metrics::{counter, histogram};
use std::time::Instant;

pub async fn my_handler() -> impl IntoResponse {
    let start = Instant::now();
    
    // Increment request counter
    counter!("http_requests_total", 
             "method" => "GET", 
             "endpoint" => "/api/my-endpoint").increment(1);
    
    // ... handle request ...
    
    // Record duration
    let duration = start.elapsed();
    histogram!("http_request_duration_seconds",
               "method" => "GET",
               "endpoint" => "/api/my-endpoint").record(duration.as_secs_f64());
    
    // Return response
}
```

#### Database Operation Instrumentation
```rust
use metrics::{counter, histogram};

pub async fn database_query() -> Result<Vec<Record>, Error> {
    let start = std::time::Instant::now();
    
    counter!("db_operations_total", "operation" => "select").increment(1);
    
    let result = sqlx::query!("SELECT * FROM table")
        .fetch_all(&pool)
        .await;
    
    let duration = start.elapsed();
    histogram!("db_operation_duration_seconds", "operation" => "select")
        .record(duration.as_secs_f64());
    
    match result {
        Ok(records) => Ok(records),
        Err(e) => {
            counter!("db_errors_total", "operation" => "select").increment(1);
            Err(e)
        }
    }
}
```

## Alerting Rules

### Critical Alerts

#### High Error Rate
```yaml
- alert: HighErrorRate
  expr: rate(http_responses_total{status=~"5.."}[5m]) / rate(http_responses_total[5m]) > 0.05
  for: 2m
  labels:
    severity: critical
  annotations:
    summary: "High error rate detected"
    description: "Error rate is {{ $value | humanizePercentage }} for the last 5 minutes"
```

#### High Response Time
```yaml
- alert: HighResponseTime
  expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 2
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "High response time detected"
    description: "95th percentile response time is {{ $value }}s"
```

#### Smart Contract Failures
```yaml
- alert: SmartContractFailures
  expr: rate(smart_contract_errors_total[5m]) > 0.1
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Smart contract failures detected"
    description: "Smart contract error rate: {{ $value }} errors/sec"
```

#### Sindri Proof Generation Issues
```yaml
- alert: SindriProvingDelays
  expr: histogram_quantile(0.95, rate(sindri_proof_generation_duration_seconds_bucket[5m])) > 300
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Sindri proof generation taking too long"
    description: "95th percentile proving time is {{ $value }}s"
```

## Performance Monitoring

### Key Performance Indicators (KPIs)

1. **API Performance**
   - Request rate: Target > 100 RPS
   - Response time: P95 < 500ms
   - Error rate: < 1%

2. **Database Performance**
   - Query duration: P95 < 100ms
   - Connection pool utilization: < 80%
   - Query error rate: < 0.1%

3. **Blockchain Operations**
   - Smart contract success rate: > 99%
   - Gas usage efficiency: Monitor trends
   - Transaction confirmation time: < 30s

4. **Proof Generation**
   - Sindri API success rate: > 99%
   - Proof generation time: P95 < 60s
   - Batch processing efficiency: > 95%

### Capacity Planning

Monitor these metrics for capacity planning:

```prometheus
# Request volume trends
increase(http_requests_total[1d])

# Resource utilization trends
avg_over_time(system_cpu_usage_percent[1d])
avg_over_time(system_memory_usage_bytes[1d])

# Database growth
increase(db_operations_total[1d])

# Blockchain activity growth
increase(smart_contract_calls_total[1d])
increase(sindri_proof_generation_total[1d])
```

## Troubleshooting

### Common Issues

#### 1. Metrics Not Appearing
- Check if metrics endpoint is accessible: `curl http://localhost:8080/metrics`
- Verify Prometheus is scraping: Check Prometheus targets page
- Ensure metrics are being recorded in code

#### 2. High Memory Usage
- Check for metric cardinality issues (too many label combinations)
- Review histogram bucket configurations
- Monitor Prometheus memory usage

#### 3. Dashboard Not Loading
- Verify Grafana datasource configuration
- Check Prometheus connectivity from Grafana
- Validate dashboard JSON syntax

### Debug Commands

```bash
# Check metrics endpoint
curl http://localhost:8080/metrics | grep -E "(http_requests|db_operations|smart_contract)"

# Verify Prometheus targets
curl http://localhost:9090/api/v1/targets

# Check Grafana datasource
curl -u admin:admin http://localhost:3000/api/datasources

# View container logs
docker-compose logs prometheus
docker-compose logs grafana
```

## Best Practices

### Metric Naming
- Use descriptive names: `http_request_duration_seconds` not `req_time`
- Include units in names: `_seconds`, `_bytes`, `_total`
- Use consistent prefixes: `http_`, `db_`, `smart_contract_`

### Label Usage
- Keep cardinality low (< 1000 unique combinations per metric)
- Use meaningful labels: `method`, `endpoint`, `status`
- Avoid high-cardinality labels: user IDs, timestamps

### Performance
- Use histograms for timing metrics
- Use counters for event counting
- Use gauges for current state values
- Batch metric updates when possible

### Security
- Avoid exposing sensitive data in metrics
- Use authentication for Grafana access
- Restrict Prometheus access to internal networks
- Regularly update monitoring stack components

## Maintenance

### Regular Tasks
- Review and update alerting rules monthly
- Clean up unused metrics and dashboards
- Monitor Prometheus storage usage
- Update Grafana dashboards based on new requirements

### Backup
- Export Grafana dashboards regularly
- Backup Prometheus data directory
- Document custom alerting rules
- Version control monitoring configurations

## Support

For questions or issues with the observability stack:
1. Check this documentation
2. Review Prometheus/Grafana logs
3. Consult the team's monitoring runbook
4. Contact the platform team

---

*Last updated: 2025-09-16*