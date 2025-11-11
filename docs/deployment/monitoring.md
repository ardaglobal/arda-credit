# OpenTelemetry Setup Guide

This guide explains how to set up OpenTelemetry tracing and monitoring for the Arda Credit API. For security setup and production deployment, see [`docs/deployment/security-production.md`](../deployment/security-production.md).

## Overview

The monitoring stack collects metrics and traces from the Arda Credit API and PostgreSQL database using OpenTelemetry.

### Setup

1. **Start the background services:**
   ```bash
   docker-compose up -d
   ```
   This starts PostgreSQL.

2. **Set environment variables in your `.env` file:**
   ```bash
   ENABLE_OPENTELEMETRY=true
   OTEL_SERVICE_NAME=arda-credit-api
   ```

3. **Run the Arda Credit API server:**
   ```bash
   make server
   ```

4. **View metrics:**
   - Metrics are collected via OpenTelemetry Collector
   - PostgreSQL metrics via prometheus-postgres-exporter
   - Traces and metrics output to debug exporter (logs)

5. **Test the setup:**
   ```bash
   curl http://0.0.0.0:8080/api/v1/health
   ```

## Architecture

### Monitoring Architecture

```
┌─────────────────┐    ┌──────────────────────┐
│   Arda Credit   │───▶│  OpenTelemetry       │
│      API        │    │  Collector           │
│ (make server)   │    │  (Debug Exporter)    │
└─────────────────┘    └──────────────────────┘
         │
         ▼
┌──────────────────┐    ┌──────────────────────┐
│    PostgreSQL    │───▶│  Postgres Exporter   │
│  (Docker:5432)   │    │  (Prometheus)        │
└──────────────────┘    └──────────────────────┘
```

**Components:**
- **OpenTelemetry Collector** - Collects traces and metrics
- **Postgres Exporter** - Exports PostgreSQL metrics
- **Debug Exporter** - Outputs metrics to logs for local development

## Services and Ports

### Local Development

| Service | Port | Purpose |
|---------|------|---------|
| Arda Credit API | 8080 | Main application |
| PostgreSQL | 5432 | Database |
| Postgres Exporter | 9187 | PostgreSQL metrics |
| OTEL Collector | 4317 | OTLP receiver |

### URLs

- **Application**: http://0.0.0.0:8080
- **Database**: 0.0.0.0:5432
- **Postgres Metrics**: http://localhost:9187/metrics (via port-forward)

## Configuration Files

### `docker-compose.yml`
Minimal background services for development:
- PostgreSQL database with persistent storage
- Health checks for database connectivity

### `.env`
Environment configuration:
- `ENABLE_OPENTELEMETRY=true` - Enable tracing
- `OTEL_SERVICE_NAME=arda-credit-api` - Service name

### Helm Chart
For Kubernetes deployment, the OpenTelemetry Collector is configured in:
- `helm/charts/arda-credit/values.yaml` - Main collector configuration
- `helm/e2e/values/values-local.yaml` - Local testing configuration

## Troubleshooting

### Issue: Server hangs on shutdown
**Solution**: The `shutdown_telemetry()` function includes a 5-second timeout to prevent hanging.

### Issue: No metrics visible
**Possible causes**:
1. `ENABLE_OPENTELEMETRY=false` - Set to `true`
2. Application not generating traces - Make requests to trigger spans
3. Server not running - Run `make server`
4. Check collector logs for errors

### Debugging Steps

1. **Check application logs**:
   ```bash
   # Look for OpenTelemetry initialization messages
   make server
   # Should see: "✅ OpenTelemetry tracer initialized"
   ```

2. **Verify configuration**:
   ```bash
   # Check if tracing is enabled
   curl http://0.0.0.0:8080/api/v1/health
   ```

3. **Test trace generation**:
   ```bash
   # Make several requests to generate spans
   for i in {1..5}; do curl http://0.0.0.0:8080/api/v1/health; done
   ```

4. **Check PostgreSQL metrics**:
   ```bash
   # Port-forward to access metrics
   kubectl port-forward svc/arda-credit-prometheus-postgres-exporter 9187:9187 -n arda-credit
   # View metrics
   curl http://localhost:9187/metrics
   ```

## Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_OPENTELEMETRY` | `false` | Enable/disable tracing |
| `OTEL_SERVICE_NAME` | `arda-credit-api` | Service name in traces |

## Disabling Telemetry

To disable OpenTelemetry completely:

```bash
ENABLE_OPENTELEMETRY=false
```

All other OpenTelemetry configuration will be ignored, and the application will use a no-op tracer.

## Performance Considerations

- OpenTelemetry adds minimal overhead when properly configured
- Batching is enabled to reduce network calls
- Memory limits are set to prevent resource exhaustion
- Debug exporter is suitable for local development only

For questions or issues, refer to the main project documentation or create an issue in the repository.
