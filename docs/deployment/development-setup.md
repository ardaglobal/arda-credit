# Development Setup Guide

This guide explains how to set up your local development environment for the Arda Credit API. For production deployment and security setup, see [`docs/deployment/security-production.md`](../deployment/security-production.md). For monitoring configuration, see [`docs/deployment/monitoring.md`](../deployment/monitoring.md).

## Overview

### 1. Start Background Services

```bash
# Start PostgreSQL database
docker-compose up -d
```

This will start:
- **PostgreSQL** on port 5432

**For tracing, we use SigNoz Cloud:**
- Set `SIGNOZ_INGESTION_KEY` in your `.env` file
- Use SigNoz Cloud endpoint in `OTEL_EXPORTER_OTLP_ENDPOINT`
- View traces in your SigNoz Cloud dashboard
- Same setup works for both local development and production

### 2. Configure Environment

Copy and configure the environment file:
```bash
make env    # Copies .env.example to .env
# Edit .env to add your SINDRI_API_KEY and other required variables
```

**Configuration details:** See `.env.example` for complete configuration options and required environment variables.

### 3. Run the API Server

```bash
make server    # Run API server with debug logging
```

The server will start on http://localhost:8080

### 4. Verify Setup

Test the health endpoint:
```bash
curl http://localhost:8080/api/v1/health
```

View traces in your SigNoz Cloud dashboard (if OpenTelemetry is enabled)

## Development Workflow

### Daily Development
1. `make up` (starts database and services)
2. `make server` (runs API server with debug logging)
3. Make your changes
4. Test with `curl` or your frontend
5. View traces in SigNoz Cloud dashboard if needed

### Stopping Services
```bash
# Stop just the background services (keeps data)
docker-compose stop

# Stop and remove containers (keeps data)
docker-compose down

# Stop and remove everything including data
docker-compose down -v
```

### Database Management

The PostgreSQL database runs in Docker with persistent storage.

**Access the database:**
```bash
# Using psql in the container
docker exec -it arda_credit_postgres psql -U postgres -d arda_credit_db

# Or connect from your host (if you have psql installed)
psql -h localhost -p 5432 -U postgres -d arda_credit_db
```

**Run migrations:**
```bash
make initDB    # Runs migrations and generates SQLx cache
```

## Services Overview

| Service | Port | Purpose | URL |
|---------|------|---------|-----|
| Arda Credit API | 8080 | Main application | http://localhost:8080 |
| PostgreSQL | 5432 | Database | localhost:5432 |
| SigNoz Cloud | 443 | Trace visualization | Your SigNoz Cloud dashboard |

## Troubleshooting

### Database Connection Issues
```bash
# Check if PostgreSQL is running
docker-compose ps postgres

# Check database logs
docker logs arda_credit_postgres

# Restart database
docker-compose restart postgres
```

### OpenTelemetry Issues
- Check that `ENABLE_OPENTELEMETRY=true` in your `.env` file
- Verify `SIGNOZ_INGESTION_KEY` is set correctly
- Ensure `OTEL_EXPORTER_OTLP_ENDPOINT` points to your SigNoz Cloud region
- Check your SigNoz Cloud dashboard for traces
- Look for OpenTelemetry initialization messages in server logs

### Port Conflicts
If you get port binding errors, check what's using the ports:
```bash
# Check what's using port 5432 (PostgreSQL)
lsof -i :5432
```

## Production Differences

- **Development**: PostgreSQL in Docker, API server via `make server`
- **Production**: Everything deployed to Kubernetes
- **Tracing**: SigNoz Cloud for both development and production (consistent setup)

This setup gives you:
- ✅ Fast iteration (no container rebuilds for API changes)
- ✅ Minimal local infrastructure (just PostgreSQL)
- ✅ Consistent tracing across all environments
- ✅ Production-like observability without complexity
