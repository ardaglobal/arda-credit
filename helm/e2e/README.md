# Arda Credit Kind E2E Testing

This directory contains everything needed to set up a local Kind (Kubernetes in Docker) cluster for end-to-end testing and development of the Arda Credit Helm chart.

## Overview

The e2e environment provides:
- **Local Kubernetes cluster** using Kind with realistic multi-node setup
- **Local image builds** loaded directly into the cluster (no registry needed)
- **Direct port access** to services for debugging (NodePort configuration)
- **Complete Arda Credit stack** deployed via Helm with PostgreSQL
- **Developer-friendly tooling** integrated with existing Makefile

## Quick Start

```bash
# Setup the entire environment (cluster + build + deploy)
make kind-setup

# Check status
make kind-status

# View logs for debugging
make kind-logs

# Access the application
open http://localhost:3000    # Frontend
open http://localhost:8080    # API Server

# Teardown when done
make kind-teardown
```

## Files

### Core Configuration
- **`kind-config.yaml`** - Kind cluster configuration with port mappings
- **`values-local.yaml`** - Helm values optimized for local development
- **`setup-cluster.sh`** - Complete environment setup script
- **`teardown-cluster.sh`** - Clean teardown script
- **`test-deployment.sh`** - Deployment validation tests

### Integration
- **Makefile targets** - `kind-*` commands integrated with existing workflow
- **Local image builds** - Uses localhost registry, no external dependencies
- **PostgreSQL integration** - Ephemeral storage, exposed for debugging

## Architecture

```
┌─────────────────┐    ┌─────────────────┐
│   Host System   │    │   Kind Cluster  │
│                 │    │                 │
│ localhost:3000  ├────┤ App (NodePort)  │
│ localhost:8080  ├────┤ Server (NodePort│
│ localhost:5432  ├────┤ PostgreSQL      │
│                 │    │                 │
│ Docker Images   │    │                 │
│ ├ arda-credit   ├────┤ Loaded Images   │
│ └ arda-credit-app    │                 │
└─────────────────┘    └─────────────────┘
```

## Development Workflow

### Initial Setup
```bash
# One-time setup (creates cluster and deploys)
make kind-setup
```

### Code Development Cycle
```bash
# Make code changes...

# Rebuild and restart (fast iteration)
make kind-rebuild

# Or just restart services (if no code changes)
make kind-restart

# View logs for debugging
make kind-logs
```

### Testing and Validation
```bash
# Run deployment tests
./helm/e2e/test-deployment.sh

# Manual testing
curl http://localhost:8080/api/v1/health
kubectl get pods --context=kind-arda-credit-e2e
```

### Cleanup
```bash
# Remove everything
make kind-teardown
```

## Available Commands

| Command | Description |
|---------|-------------|
| `make kind-setup` | Create cluster, build images, deploy Helm chart |
| `make kind-teardown` | Remove cluster and cleanup resources |
| `make kind-status` | Show cluster status and running pods |
| `make kind-logs` | Display logs from all services |
| `make kind-restart` | Restart deployments (keep cluster/data) |
| `make kind-rebuild` | Rebuild images, load into cluster, restart |

## Service Access

### Direct Access (NodePort)
- **Frontend**: http://localhost:3000
- **API Server**: http://localhost:8080  
- **PostgreSQL**: localhost:5432

### Kubernetes Access
```bash
# Set context
kubectl config use-context kind-arda-credit-e2e

# View resources
kubectl get pods
kubectl get services
kubectl describe deployment arda-credit-server

# Access logs
kubectl logs -f deployment/arda-credit-server
kubectl logs -f deployment/arda-credit-app
```

### Database Access
```bash
# Direct connection
psql -h localhost -p 5432 -U postgres -d arda_credit_db_local

# Via kubectl port-forward (if NodePort not working)
kubectl port-forward svc/arda-credit-postgresql 5432:5432
```

## Configuration Details

### Kind Cluster
- **Nodes**: 1 control-plane + 1 worker
- **Networking**: Custom subnets to avoid Docker Desktop conflicts
- **Port Mappings**: Direct access to services via localhost
- **MetalLB**: LoadBalancer simulation for realistic testing

### Helm Values (values-local.yaml)
- **Images**: Uses `localhost/arda-credit:local-dev` (no registry)
- **Resources**: Minimal CPU/memory for local development  
- **Storage**: Ephemeral (no persistence) for fast cleanup
- **Services**: NodePort type for direct access
- **Security**: Permissive settings for development ease

### Local Images
- **Build**: Standard Dockerfile with local SP1 base image
- **Tag**: `localhost/arda-credit:local-dev`
- **Loading**: Direct load into Kind cluster (no push/pull)
- **Caching**: Docker build cache for faster rebuilds

## Troubleshooting

### Common Issues

**Cluster won't start**
```bash
# Check Docker is running
docker ps

# Check available resources
docker system df
docker system prune  # If low on space
```

**Images not loading**  
```bash
# Rebuild and reload images
make kind-rebuild

# Check images in cluster
docker exec -it arda-credit-e2e-control-plane crictl images
```

**Services not accessible**
```bash
# Check port mappings
kubectl get services

# Check pod status
kubectl get pods -o wide

# Check logs
make kind-logs
```

**Database connection issues**
```bash
# Check PostgreSQL pod
kubectl logs -l app.kubernetes.io/name=postgresql

# Test connection
telnet localhost 5432
```

### Debug Commands

```bash
# Cluster information
kubectl cluster-info --context=kind-arda-credit-e2e

# Node details
kubectl describe nodes

# Pod details
kubectl describe pod <pod-name>

# Service endpoints
kubectl get endpoints

# Events
kubectl get events --sort-by=.metadata.creationTimestamp
```

### Clean Slate Recovery

```bash
# Complete cleanup and restart
make kind-teardown
docker system prune -f
make kind-setup
```

## Integration with CI/CD

This setup can be easily integrated into GitHub Actions or other CI systems:

```yaml
# Example GitHub Actions integration
- name: Setup Kind e2e environment
  run: make kind-setup

- name: Run deployment tests  
  run: ./helm/e2e/test-deployment.sh

- name: Run additional tests
  run: |
    # Add your e2e tests here
    curl -f http://localhost:8080/api/v1/health

- name: Cleanup
  run: make kind-teardown
```

## Performance Considerations

- **Startup time**: ~2-3 minutes for full setup
- **Resource usage**: ~2GB RAM, 4 CPU cores recommended
- **Rebuild time**: ~30 seconds for code changes
- **Storage**: Ephemeral, no persistent data between runs

## Extending the Setup

### Adding New Services
1. Add service to `values-local.yaml`
2. Update port mappings in `kind-config.yaml`
3. Add health checks to `test-deployment.sh`

### Custom Test Scenarios
1. Create new test scripts in `helm/e2e/`
2. Add Makefile targets as needed
3. Use existing cluster for test execution

### Environment Variants
1. Copy `values-local.yaml` to `values-<variant>.yaml`
2. Modify setup script to accept values file parameter
3. Create variant-specific Makefile targets