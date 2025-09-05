# Helm Values Configuration

This directory contains Helm values files for different testing environments and scenarios.

## Files

### `values-local.yaml`
Primary values file for local Kind development:
- **Images**: Uses locally built images (`localhost/arda-credit:local-dev`)
- **Resources**: Minimal CPU/memory allocation for development
- **Services**: NodePort configuration for direct access
- **Storage**: Ephemeral storage (no persistence)
- **Security**: Permissive settings for ease of development

## Configuration Sections

### Global Settings
- **Image Pull Secrets**: Disabled (local images)
- **Node Selectors**: Unset (allow any node)
- **Namespace**: Uses default namespace

### PostgreSQL Configuration
```yaml
postgresql:
  enabled: true
  auth:
    database: "arda_credit_db_local"
    username: "postgres"
    password: "local-dev-password"
  primary:
    persistence:
      enabled: false  # Ephemeral storage
    service:
      type: NodePort
      nodePorts:
        postgresql: 30002
```

### Server Configuration
```yaml
server:
  image:
    registry: "localhost"
    repository: "arda-credit"
    tag: "local-dev"
    pullPolicy: Never  # Use local images only
  service:
    type: NodePort
    nodePort: 30001
  resources:
    requests:
      cpu: 100m
      memory: 256Mi
    limits:
      cpu: 500m
      memory: 512Mi
```

### App Configuration
```yaml
app:
  image:
    registry: "localhost"
    repository: "arda-credit-app"
    tag: "local-dev"
    pullPolicy: Never
  service:
    type: NodePort
    nodePort: 30000
```

### Secrets (Development Values)
```yaml
secrets:
  app:
    sindriApiKey: "local-test-key"
    ethereumRpcUrl: "http://host.docker.internal:8545"
    ethereumContractAddress: "0x1234..."
    ethereumWalletPrivateKey: "0xac09..."  # Test key
```

## Environment Variants

### Creating New Environment Values

For different testing scenarios, create additional values files:

#### `values-ci.yaml` - CI/CD Testing
```yaml
# Optimized for GitHub Actions
global:
  imagePullSecrets:
    - name: ghcr-secret

server:
  image:
    registry: "ghcr.io"
    repository: "ardaglobal/arda-credit"
    tag: "ci-test"
  
postgresql:
  auth:
    password: "ci-test-password"
```

#### `values-performance.yaml` - Load Testing
```yaml
# Higher resource allocation
server:
  replicaCount: 3
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 2000m
      memory: 2Gi
  
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10
```

#### `values-debug.yaml` - Debug Mode
```yaml
# Enhanced logging and debugging
server:
  config:
    rustLog: "trace"  # Verbose logging
  
  # Debug tools sidecar
  extraContainers:
    - name: debug-tools
      image: nicolaka/netshoot
      command: ["/bin/sleep", "infinity"]
```

## Usage Examples

### Deploy with specific values
```bash
helm upgrade --install arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml \
  --wait
```

### Override specific values
```bash
helm upgrade --install arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml \
  --set server.image.tag=latest \
  --set postgresql.auth.password=new-password
```

### Multiple values files
```bash
helm upgrade --install arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml \
  --values values-debug.yaml
```

## Value Precedence

Helm values are merged in this order (last wins):
1. Chart default values (`charts/arda-credit/values.yaml`)
2. Values file (`--values values-local.yaml`)
3. Command line sets (`--set key=value`)

## Validation

### Validate values before deployment
```bash
# Dry run with values
helm upgrade --install arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml \
  --dry-run --debug

# Template rendering
helm template arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml
```

### Check generated manifests
```bash
# Render and save manifests
helm template arda-credit \
  ../charts/arda-credit \
  --values values-local.yaml \
  --output-dir ./rendered-manifests/
```