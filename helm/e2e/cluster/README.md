# Kind Cluster Configuration

This directory contains Kind cluster configuration files for different environments and scenarios.

## Files

### `kind-config.yaml`
The primary cluster configuration for local development:
- **Nodes**: 1 control-plane + 1 worker node
- **Port Mappings**: Direct access to services via localhost
- **Networking**: Custom subnets to avoid Docker Desktop conflicts
- **Container Runtime**: Optimized containerd configuration

## Port Mappings

| Service | Container Port | Host Port | Description |
|---------|---------------|-----------|-------------|
| Frontend App | 30000 | 3000 | Arda Credit web interface |
| API Server | 30001 | 8080 | REST API and GraphQL |
| PostgreSQL | 30002 | 5432 | Database access for debugging |
| Ingress HTTP | 30080 | 80 | HTTP ingress (future use) |
| Ingress HTTPS | 30443 | 443 | HTTPS ingress (future use) |

## Cluster Architecture

```
┌─────────────────────────┐
│    arda-credit-e2e      │
│      Kind Cluster       │
├─────────────────────────┤
│  Control Plane Node     │
│  ├─ API Server          │
│  ├─ etcd                │
│  ├─ Controller Manager  │
│  ├─ Scheduler           │
│  └─ Port Mappings       │
├─────────────────────────┤
│  Worker Node            │
│  ├─ kubelet             │
│  ├─ kube-proxy          │
│  ├─ Container Runtime   │
│  └─ Application Pods    │
└─────────────────────────┘
```

## Network Configuration

- **Pod Subnet**: `10.244.0.0/16`
- **Service Subnet**: `10.96.0.0/16`
- **API Server**: `127.0.0.1:6443`
- **Container Registry**: Local registry support via `localhost:5001`

## Extending Configuration

### Adding New Port Mappings
```yaml
extraPortMappings:
- containerPort: 30003  # NodePort service port
  hostPort: 9090        # Host access port
  protocol: TCP
```

### Multi-Environment Configs
For different testing scenarios, create additional config files:
- `kind-config-minimal.yaml` - Single node for resource-constrained environments
- `kind-config-ha.yaml` - Multi control-plane for HA testing
- `kind-config-ingress.yaml` - With ingress controller testing