# Development Setup Guide

This guide explains how to set up your local development environment for the Arda Credit API.

## Quick Start

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

Create a `.env` file in the project root with:

```bash
# Database
DATABASE_URL=postgresql://postgres:password@localhost:5432/arda_credit_db

# OpenTelemetry with SigNoz Cloud
ENABLE_OPENTELEMETRY=true
OTEL_SERVICE_NAME=arda-credit-api
OTEL_EXPORTER_OTLP_ENDPOINT=https://ingest.{region}.signoz.cloud:443
SIGNOZ_INGESTION_KEY=your-signoz-ingestion-key

# Other configuration...
JWT_SECRET=your-super-secure-jwt-secret-key-at-least-32-characters-long
```

### 3. Run the API Server

```bash
cd api
cargo run --bin server
```

The server will start on http://localhost:8080

### 4. Verify Setup

Test the health endpoint:
```bash
curl http://localhost:8080/api/v2/health
```

View traces in your SigNoz Cloud dashboard (if OpenTelemetry is enabled)

## Development Workflow

### Daily Development
1. `docker-compose up -d` (if not already running)
2. `cd api && cargo run --bin server`
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
cd db
cargo run --bin migrate
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

- **Development**: PostgreSQL in Docker, API server via `cargo run`
- **Production**: Everything deployed to Kubernetes
- **Tracing**: SigNoz Cloud for both development and production (consistent setup)

This setup gives you:
- ✅ Fast iteration (no container rebuilds for API changes)
- ✅ Minimal local infrastructure (just PostgreSQL)
- ✅ Consistent tracing across all environments
- ✅ Production-like observability without complexity
