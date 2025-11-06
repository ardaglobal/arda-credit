# Arda Credit Kind E2E Testing

This directory contains everything needed to set up a local Kind (Kubernetes in Docker) cluster for end-to-end testing and development of the Arda Credit Helm chart.

## Overview

The e2e environment provides:
- **Local Kubernetes cluster** using Kind with realistic multi-node setup
- **Local container registry** for seamless image distribution to cluster nodes
- **Hybrid image deployment** - local server builds + production frontend from GHCR
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
open http://0.0.0.0:8080    # API Server

# Teardown when done or after code changes to setup cluster again
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
- **Local container registry** - Automatic setup on port 5001, connected to Kind network
- **Hybrid image strategy** - Local server builds + production frontend from GHCR
- **PostgreSQL integration** - Ephemeral storage, exposed for debugging

## Architecture

```
┌─────────────────┐    ┌─────────────────────┐    ┌─────────────────┐
│   Host System   │    │  Local Registry     │    │   Kind Cluster  │
│                 │    │  (localhost:5001)   │    │                 │
│ localhost:3000  ├────┼─────────────────────┼────┤ App (NodePort)  │
│ 0.0.0.0:8080    ├────┼─────────────────────┼────┤ Server          │
│ 0.0.0.0:5432    ├────┼─────────────────────┼────┤ PostgreSQL      │
│                 │    │                     │    │                 │
│ Docker Build:   │    │ Registry Storage:   │    │ Image Sources:  │
│ • Build image   ├────┤ • arda-credit:      ├────┤ • kind-registry │
│ • Push to reg   │    │   local-dev         │    │ • ghcr.io/...   │
│                 │    │                     │    │                 │
└─────────────────┘    └─────────────────────┘    └─────────────────┘
                                │
                                │ Connected to Kind network
                                │ (eliminates snapshotter issue)
                                ▼
                       Auto-configured as kind-registry:5000
```

## Testing and Validation
```bash
# Run deployment tests
./helm/e2e/test-deployment.sh

# Manual testing
curl http://0.0.0.0:8080/api/v1/health
kubectl get pods --context=kind-arda-credit-e2e
```

## Available Commands

| Command | Description |
|---------|-------------|
| `make kind-setup` | Create cluster, build images, deploy Helm chart |
| `make kind-teardown` | Remove cluster and cleanup resources |
| `make kind-status` | Show cluster status and running pods |
| `make kind-logs` | Display logs from all services |

## Service Access

### Direct Access (NodePort)
- **Frontend**: http://localhost:3000
- **API Server**: http://0.0.0.0:8080  
- **PostgreSQL**: 0.0.0.0:5432

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
- **Server Image**: `kind-registry:5000/arda-credit:local-dev` (local registry)
- **Resources**: Minimal CPU/memory for local development  
- **Storage**: Ephemeral (no persistence) for fast cleanup
- **Services**: NodePort type for direct access
- **Security**: Permissive settings for development ease

### Image Management
- **Local Registry**: Automatic `registry:2` container on port 5001 
- **Server Build**: Local Dockerfile → pushed to local registry
- **App Deployment**: Production frontend from GHCR (separate repository)
- **Network Integration**: Registry connected to Kind network as `kind-registry`
- **Caching**: Docker build cache + registry storage for faster rebuilds

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
# Check local registry is running
docker ps | grep registry

# Check images in registry
curl http://localhost:5001/v2/_catalog

# Check images in cluster nodes
docker exec -it arda-credit-e2e-control-plane crictl images | grep arda-credit
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
make kind-setup
```

### Registry Issues

```bash
# Registry not accessible
docker ps | grep registry  # Check if running
docker logs kind-registry   # Check registry logs

# Network connectivity 
docker network inspect kind | grep registry  # Should show registry connected

# Manual registry restart
docker stop kind-registry && docker rm kind-registry
make kind-setup  # Will recreate registry automatically
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
    curl -f http://0.0.0.0:8080/api/v1/health

- name: Cleanup
  run: make kind-teardown
```

## Performance Considerations

- **Startup time**: ~2-3 minutes for full setup (includes registry setup)
- **Resource usage**: ~2GB RAM, 4 CPU cores recommended  
- **Registry overhead**: ~50MB RAM for local registry container
- **Rebuild time**: ~30 seconds for code changes (push to registry)
- **Storage**: Ephemeral cluster + registry storage for image caching
- **Network**: Local registry eliminates external image pulls for server

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