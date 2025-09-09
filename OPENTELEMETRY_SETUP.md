# OpenTelemetry Setup Guide

This guide explains how to set up OpenTelemetry tracing for the Arda Credit API using SigNoz Cloud for both local development and production environments.

## Quick Start

### Unified SigNoz Cloud Setup

1. **Start the background services:**
   ```bash
   docker-compose up -d
   ```
   This starts PostgreSQL only.

2. **Set environment variables in your `.env` file:**
   ```bash
   ENABLE_OPENTELEMETRY=true
   OTEL_SERVICE_NAME=arda-credit-api
   OTEL_EXPORTER_OTLP_ENDPOINT=https://ingest.{region}.signoz.cloud:443
   SIGNOZ_INGESTION_KEY=your-signoz-ingestion-key
   ```

3. **Run the Arda Credit API server:**
   ```bash
   cd api && cargo run --bin server
   ```

4. **View traces in SigNoz Cloud:**
   - Open your SigNoz Cloud dashboard
   - Navigate to the traces section
   - Filter by service name "arda-credit-api"
   - View your application traces

5. **Test the setup:**
   ```bash
   curl http://localhost:8080/api/v2/health
   ```

## Architecture

### Simplified Architecture (Both Development & Production)

```
┌─────────────────┐    ┌──────────────────┐
│   Arda Credit   │───▶│   SigNoz Cloud   │
│      API        │    │   (Port 443)     │
│ (cargo run)     │    │                  │
└─────────────────┘    └──────────────────┘
         │
         ▼
┌──────────────────┐
│    PostgreSQL    │
│  (Docker:5432)   │
│ (Dev only)       │
└──────────────────┘
```

**Benefits:**
- ✅ **Consistent setup** - Same tracing for dev and prod
- ✅ **Minimal complexity** - No local collector/Jaeger to manage  
- ✅ **Team visibility** - Everyone sees traces in the same place
- ✅ **Persistent data** - Traces don't disappear on restart

## Services and Ports

### Local Development

| Service | Port | Purpose |
|---------|------|---------|
| Arda Credit API | 8080 | Main application |
| PostgreSQL | 5432 | Database |
| SigNoz Cloud | 443 | Trace visualization |

### URLs

- **Application**: http://localhost:8080
- **SigNoz Dashboard**: Your SigNoz Cloud URL
- **Database**: localhost:5432

## Configuration Files

### `docker-compose.yml`
Minimal background services for development:
- PostgreSQL database with persistent storage
- Health checks for database connectivity

### `.env`
Environment configuration:
- `ENABLE_OPENTELEMETRY=true` - Enable tracing
- `OTEL_SERVICE_NAME=arda-credit-api` - Service name in SigNoz
- `OTEL_EXPORTER_OTLP_ENDPOINT` - SigNoz Cloud endpoint
- `SIGNOZ_INGESTION_KEY` - Your SigNoz Cloud API key

## Troubleshooting

### Issue: Server hangs on shutdown
**Solution**: The updated `shutdown_telemetry()` function now includes a 5-second timeout to prevent hanging.

### Issue: No traces visible in SigNoz Cloud
**Possible causes**:
1. `ENABLE_OPENTELEMETRY=false` - Set to `true`
2. Missing `SIGNOZ_INGESTION_KEY` - Set your SigNoz ingestion key
3. Wrong endpoint format - Should be `https://ingest.{region}.signoz.cloud:443`
4. Invalid ingestion key - Check your SigNoz Cloud settings
5. Network connectivity issues - Check internet connection
6. Application not generating traces - Make requests to trigger spans
7. Server not running - Run `cd api && cargo run --bin server`

### Debugging Steps

1. **Check application logs**:
   ```bash
   # Look for OpenTelemetry initialization messages
   cd api && cargo run --bin server
   # Should see: "✅ OpenTelemetry tracer initialized for SigNoz Cloud"
   ```

2. **Verify configuration**:
   ```bash
   # Check if tracing is enabled
   curl http://localhost:8080/api/v2/health
   # Look for traces in your SigNoz Cloud dashboard
   ```

3. **Test trace generation**:
   ```bash
   # Make several requests to generate spans
   for i in {1..5}; do curl http://localhost:8080/api/v2/health; done
   ```

## Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_OPENTELEMETRY` | `false` | Enable/disable tracing |
| `OTEL_SERVICE_NAME` | `arda-credit-api` | Service name in traces |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP endpoint |
| `SIGNOZ_INGESTION_KEY` | (empty) | SigNoz Cloud ingestion key |

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
- Sampling can be configured in the collector for high-traffic scenarios

For questions or issues, refer to the main project documentation or create an issue in the repository.
