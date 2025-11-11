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

**For tracing and monitoring:**
- Set `ENABLE_OPENTELEMETRY=true` in your `.env` file
- Metrics collected via OpenTelemetry Collector
- PostgreSQL metrics via prometheus-postgres-exporter
- Debug output to logs for local development

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

The server will start on http://0.0.0.0:8080

### 4. Verify Setup

Test the health endpoint:
```bash
curl http://0.0.0.0:8080/api/v1/health
```

View traces and metrics in logs (if OpenTelemetry is enabled)

## Development Workflow

### Daily Development
1. `make up` (starts database and services)
2. `make server` (runs API server with debug logging)
3. Make your changes
4. Test with `curl` or your frontend
5. View traces and metrics in logs if needed

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
| Arda Credit API | 8080 | Main application | http://0.0.0.0:8080 |
| PostgreSQL | 5432 | Database | 0.0.0.0:5432 |

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
- Look for OpenTelemetry initialization messages in server logs
- Check that traces and metrics are output to debug logs
- Verify PostgreSQL metrics exporter is accessible (if using Kubernetes)

### Port Conflicts
If you get port binding errors, check what's using the ports:
```bash
# Check what's using port 5432 (PostgreSQL)
lsof -i :5432
```

## Production Differences

- **Development**: PostgreSQL in Docker, API server via `make server`, metrics to debug logs
- **Production**: Everything deployed to Kubernetes with full monitoring

This setup gives you:
- ✅ Fast iteration (no container rebuilds for API changes)
- ✅ Minimal local infrastructure (just PostgreSQL)
- ✅ Debug logging for local development
- ✅ Production-ready monitoring in Kubernetes
