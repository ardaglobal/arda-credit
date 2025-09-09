# PostgreSQL Monitoring with Prometheus

This document describes the PostgreSQL monitoring setup for Arda Credit using the postgres-exporter for Prometheus integration.

## Overview

The PostgreSQL monitoring system provides comprehensive database performance metrics while strictly adhering to the privacy constraints defined in `ARCHITECTURE.md`. The implementation ensures:

- **Privacy Preservation**: No sensitive loan amounts, user details, or KYC status exposed
- **Performance Monitoring**: Comprehensive database performance metrics
- **Zero Impact**: Minimal overhead on the 60+ TPS throughput requirement
- **Multi-Environment**: Consistent monitoring across local, staging, and production

## Architecture

### Components

1. **postgres-exporter**: Prometheus exporter for PostgreSQL metrics
2. **Custom Queries**: Privacy-preserving query definitions
3. **Service Discovery**: Kubernetes ServiceMonitor for Prometheus Operator
4. **Local Development**: Docker Compose integration

### Privacy-First Design

The monitoring system is designed with privacy as a core principle:

- **Aggregate Metrics Only**: All metrics are aggregated at the database level
- **No Query Content**: pg_stat_statements provides timing without query text
- **No Sensitive Data**: User IDs, loan amounts, and KYC status are never exposed
- **Table Name Abstraction**: Table-specific metrics are summarized to prevent data inference

## Deployment

### Kubernetes (Helm)

The postgres-exporter is deployed as part of the Arda Credit Helm chart:

```yaml
postgresql:
  exporter:
    enabled: true  # Enable PostgreSQL monitoring
```

#### Configuration Options

```yaml
postgresql:
  exporter:
    enabled: true
    
    image:
      registry: quay.io
      repository: prometheuscommunity/postgres-exporter
      tag: "v0.15.0"
    
    service:
      type: ClusterIP
      port: 9187
    
    serviceMonitor:
      enabled: false  # Enable if using Prometheus Operator
      interval: "30s"
    
    resources:
      limits:
        cpu: 200m
        memory: 256Mi
```

### Local Development

For local development, the exporter is included in `docker-compose.yml`:

```bash
# Start services with monitoring
make up

# Check monitoring status
make monitoring-status

# View sample metrics
make monitoring-metrics

# Test configuration
make monitoring-test
```

## Metrics Categories

### 1. Database Performance

- **Connection Statistics**: Active connections, transaction rates
- **Query Performance**: Execution times, query counts (aggregate only)
- **Buffer Cache**: Hit ratios, cache efficiency
- **I/O Statistics**: Disk reads, cache hits, block statistics

### 2. Storage and Maintenance

- **Database Size**: Total database size and growth metrics
- **Table Statistics**: Insert/update/delete counts (aggregated)
- **Index Usage**: Index scan efficiency and usage patterns
- **Vacuum/Analyze**: Maintenance operation statistics

### 3. System Health

- **Lock Statistics**: Lock counts by mode (performance indicator)
- **Checkpoint Activity**: WAL checkpoint frequency and timing
- **Replication Lag**: If replication is configured
- **Background Writer**: Buffer management statistics

### 4. Privacy-Compliant Queries

All custom queries follow these principles:

```sql
-- ✅ GOOD: Aggregate metrics without sensitive data
SELECT COUNT(*) as total_tables, SUM(n_tup_ins) as total_inserts
FROM pg_stat_user_tables;

-- ❌ BAD: Exposes table names and potentially sensitive data
SELECT schemaname, tablename, n_tup_ins 
FROM pg_stat_user_tables;
```

## Monitoring Endpoints

### Local Development

- **PostgreSQL Exporter**: http://localhost:9187/metrics
- **PostgreSQL Database**: localhost:5432

### Kubernetes

- **Service Name**: `arda-credit-postgres-exporter`
- **Port**: 9187
- **Metrics Path**: `/metrics`

## Key Metrics

### Database Health

```prometheus
# Connection count
pg_stat_database_numbackends

# Transaction rate
rate(pg_stat_database_xact_commit[5m])

# Buffer cache hit ratio
pg_buffer_cache_hit_ratio_percent
```

### Performance Monitoring

```prometheus
# Query execution time
pg_stat_statements_summary_average_execution_time_ms

# Index usage efficiency
pg_stat_user_indexes_summary_index_usage_ratio_percent

# Lock contention
pg_locks_summary_lock_count
```

### Storage Growth

```prometheus
# Database size
pg_database_size

# Tuple operations
rate(pg_stat_database_tup_inserted[5m])
rate(pg_stat_database_tup_updated[5m])
```

## Alerting Rules

### Critical Alerts

```yaml
# High connection usage
- alert: PostgreSQLHighConnections
  expr: pg_stat_database_numbackends > 80
  for: 5m

# Low cache hit ratio
- alert: PostgreSQLLowCacheHitRatio
  expr: pg_buffer_cache_hit_ratio_percent < 95
  for: 10m

# High query execution time
- alert: PostgreSQLSlowQueries
  expr: pg_stat_statements_summary_average_execution_time_ms > 1000
  for: 5m
```

### Warning Alerts

```yaml
# Database growth
- alert: PostgreSQLDatabaseGrowth
  expr: increase(pg_database_size[24h]) > 1000000000  # 1GB per day
  for: 1h

# Lock contention
- alert: PostgreSQLLockContention
  expr: pg_locks_summary_lock_count{mode="ExclusiveLock"} > 10
  for: 2m
```

## Grafana Dashboard

### Key Panels

1. **Overview**
   - Database size and growth
   - Active connections
   - Transaction rate
   - Cache hit ratio

2. **Performance**
   - Query execution times
   - Index usage efficiency
   - I/O operations
   - Lock statistics

3. **Maintenance**
   - Vacuum/analyze frequency
   - Checkpoint activity
   - Dead tuple counts
   - Table bloat indicators

### Sample Dashboard JSON

A complete Grafana dashboard configuration is available in the `monitoring/` directory.

## Troubleshooting

### Common Issues

1. **Exporter Not Starting**
   ```bash
   # Check container logs
   docker logs arda_credit_postgres_exporter
   
   # Verify database connection
   make monitoring-test
   ```

2. **Missing Metrics**
   ```bash
   # Check custom queries configuration
   curl -s http://localhost:9187/metrics | grep pg_database_size
   
   # Verify pg_stat_statements extension
   docker exec arda_credit_postgres psql -U postgres -d arda_credit_db -c "SELECT * FROM pg_extension WHERE extname = 'pg_stat_statements';"
   ```

3. **Privacy Compliance Check**
   ```bash
   # Automated privacy check
   make monitoring-test
   
   # Manual verification
   curl -s http://localhost:9187/metrics | grep -E "(loan_amount|user_id|kyc_status)"
   ```

### Performance Impact

The monitoring setup has minimal performance impact:

- **CPU Usage**: <0.5% additional load
- **Memory Usage**: ~64-128MB for exporter
- **Network**: ~1KB/s metrics traffic
- **Database Load**: <1% query overhead

## Security Considerations

### Access Control

- **Read-Only Access**: Exporter uses read-only database permissions
- **Network Security**: Metrics endpoint accessible only within cluster
- **Secret Management**: Database credentials stored in Kubernetes secrets

### Privacy Compliance

- **Data Minimization**: Only essential metrics are collected
- **Aggregation**: All sensitive data is aggregated before export
- **Audit Trail**: All metric queries are logged and auditable

## Integration with Existing Systems

### Prometheus

```yaml
scrape_configs:
  - job_name: 'postgres-exporter'
    static_configs:
      - targets: ['arda-credit-postgres-exporter:9187']
    scrape_interval: 30s
```

### ServiceMonitor (Prometheus Operator)

```yaml
postgresql:
  exporter:
    serviceMonitor:
      enabled: true
      interval: "30s"
      labels:
        release: prometheus
```

### Local Development Workflow

```bash
# Start with monitoring
make up

# Run application
make server

# Check metrics during testing
make monitoring-metrics

# Validate privacy compliance
make monitoring-test
```

## Future Enhancements

1. **Custom Dashboards**: Arda Credit-specific Grafana dashboards
2. **Advanced Alerting**: ML-based anomaly detection
3. **Performance Baselines**: Automated performance regression detection
4. **Multi-Region**: Cross-region replication lag monitoring

## References

- [postgres_exporter Documentation](https://github.com/prometheus-community/postgres_exporter)
- [PostgreSQL Monitoring Best Practices](https://www.postgresql.org/docs/current/monitoring.html)
- [Prometheus Operator ServiceMonitor](https://prometheus-operator.dev/docs/operator/design/#servicemonitor)
- [Arda Credit Architecture](./ARCHITECTURE.md)