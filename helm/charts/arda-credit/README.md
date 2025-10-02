# Arda Credit Helm Chart

This production-ready Helm chart deploys the Arda Credit application stack on Kubernetes with enterprise-grade features including autoscaling, high availability, and security best practices.

## Features

✅ **High Availability**: Pod Disruption Budgets, anti-affinity rules, health checks  
✅ **Auto-scaling**: Horizontal Pod Autoscaler with CPU/memory metrics  
✅ **Security**: Service accounts, RBAC, proper secret management  
✅ **Configuration Management**: ConfigMaps for non-sensitive config  
✅ **PostgreSQL Integration**: Bundled Bitnami PostgreSQL or external database  
✅ **Production Ready**: Resource limits, security contexts, graceful shutdowns  

## Prerequisites

- Kubernetes 1.20+
- Helm 3.8+
- Metrics Server (required for autoscaling features)
- RBAC enabled cluster

## Quick Start (Bundled PostgreSQL)

The chart includes PostgreSQL as a dependency for easy deployment:

```bash
# Add Bitnami Legacy repository (for PostgreSQL dependency)
helm repo add bitnamilegacy https://charts.bitnami.com/bitnamilegacy
helm repo update

# Update dependencies
helm dependency update ./helm/charts/arda-credit

# Deploy with bundled PostgreSQL (namespace auto-created)
helm install arda-credit ./helm/charts/arda-credit \
  --set postgresql.auth.password=your-secure-password \
  --set ingress.hosts[0].host=your-domain.com \
  --namespace arda-credit

# Create GHCR authentication secret (for private container registry)
# Use organization service account with PAT having 'read:packages' scope
kubectl create secret docker-registry ghcr-secret \
  --docker-server=ghcr.io \
  --docker-username=arda-bot \
  --docker-password=$GITHUB_ORG_PAT \
  --namespace arda-credit

# Create application secrets
kubectl create secret generic arda-credit-secrets \
  --from-literal=sindri-api-key=your-sindri-key \
  --from-literal=ethereum-rpc-url=your-eth-rpc \
  --from-literal=ethereum-contract-address=0x... \
  --from-literal=ethereum-wallet-private-key=0x... \
  --from-literal=ethereum-deployer-address=0x... \
  --namespace arda-credit

# Deploy with autoscaling enabled
helm install arda-credit ./helm/charts/arda-credit \
  --set postgresql.auth.password=your-secure-password \
  --set server.autoscaling.enabled=true \
  --set app.autoscaling.enabled=true \
  --set ingress.hosts[0].host=your-domain.com \
  --namespace arda-credit
```

## External PostgreSQL Setup (Optional)

To use an external PostgreSQL instance:

```bash
# Deploy with external PostgreSQL
helm install arda-credit ./helm/charts/arda-credit \
  --set postgresql.enabled=false \
  --set postgresql.external.host=your-postgres-host \
  --set postgresql.external.existingSecret=your-postgres-secret \
  --set ingress.hosts[0].host=your-domain.com \
  --namespace arda-credit
```

## Configuration

Key configuration options in `values.yaml`:

| Parameter | Description | Default |
|-----------|-------------|---------|
| **General** |
| `namespaceCreate` | Auto-create namespace | `true` |
| **Server Configuration** |
| `server.replicaCount` | Number of server replicas | `2` |
| `server.config.rustLog` | Rust logging level | `"info"` |
| `server.config.serverPort` | Server port | `"8080"` |
| **App Configuration** |
| `app.replicaCount` | Number of app replicas | `1` |
| `app.image.repository` | Frontend app image name | `"arda-credit-app"` |
| **Autoscaling** |
| `server.autoscaling.enabled` | Enable server autoscaling | `false` |
| `server.autoscaling.minReplicas` | Min server replicas | `2` |
| `server.autoscaling.maxReplicas` | Max server replicas | `10` |
| `app.autoscaling.enabled` | Enable app autoscaling | `false` |
| **High Availability** |
| `server.podDisruptionBudget.enabled` | Enable server PDB | `true` |
| `server.podAntiAffinity.enabled` | Enable pod anti-affinity | `true` |
| `server.podAntiAffinity.type` | Anti-affinity type (soft/hard) | `"soft"` |
| **Security** |
| `serviceAccount.create` | Create service account | `true` |
| `rbac.create` | Create RBAC resources | `true` |
| **Database** |
| `postgresql.enabled` | Use bundled PostgreSQL | `true` |
| `postgresql.auth.password` | PostgreSQL password | `"change-me-in-production"` |
| **Ingress** |
| `ingress.enabled` | Enable ingress | `true` |
| `ingress.className` | Ingress class (nginx/alb) | `"nginx"` |
| `ingress.hosts[0].host` | Ingress hostname | `arda-credit.example.com` |

## EKS Deployment

For AWS EKS deployments with ALB and ACM:

```bash
# 1. Install AWS Load Balancer Controller (required for ALB)
helm repo add eks https://aws.github.io/eks-charts
helm install aws-load-balancer-controller eks/aws-load-balancer-controller \
  -n kube-system \
  --set clusterName=your-cluster-name \
  --set serviceAccount.create=false \
  --set serviceAccount.name=aws-load-balancer-controller \
  --set region=us-east-1 \
  --set vpcId=vpc-xxxxxxxx

# 2. Deploy with ALB configuration  
helm install arda-credit ./helm/charts/arda-credit \
  --set namespaceCreate=false \
  --set ingress.className=alb \
  --set-string 'ingress.annotations.alb\.ingress\.kubernetes\.io/scheme=internet-facing' \
  --set-string 'ingress.annotations.alb\.ingress\.kubernetes\.io/target-type=ip' \
  --set-string 'ingress.annotations.alb\.ingress\.kubernetes\.io/listen-ports=\[{\"HTTP\": 80}, {\"HTTPS\": 443}\]' \
  --set-string 'ingress.annotations.alb\.ingress\.kubernetes\.io/ssl-redirect=443' \
  --set-string 'ingress.annotations.alb\.ingress\.kubernetes\.io/certificate-arn=arn:aws:acm:region:account:certificate/cert-id' \
  --set-string 'ingress.annotations.external-dns\.alpha\.kubernetes\.io/hostname=app.yourdomain.com' \
  --set ingress.hosts[0].host=app.yourdomain.com \
  --set postgresql.primary.persistence.storageClass=gp3 \
  --namespace arda-credit \
  --create-namespace
```

**EKS Prerequisites:**
- AWS Load Balancer Controller installed with proper IAM role (IRSA)  
- Subnets tagged for ALB controller: `kubernetes.io/role/elb=1` (public subnets)
- External-DNS (optional, for automatic Route53 management)
- ACM certificate for your domain

## Production Deployment

For production environments, enable additional features:

```bash
# Production deployment with all features enabled
helm install arda-credit ./helm/charts/arda-credit \
  --set postgresql.auth.password=secure-random-password \
  --set server.autoscaling.enabled=true \
  --set app.autoscaling.enabled=true \
  --set server.podDisruptionBudget.enabled=true \
  --set server.podAntiAffinity.type=hard \
  --set ingress.hosts[0].host=arda-credit.production.com \
  --namespace arda-credit-prod \
  --create-namespace
```

## Required Secrets

The chart requires two types of secrets to be created manually:

### 1. GHCR Authentication Secret
For pulling private container images from GitHub Container Registry:

```bash
# Create GitHub organization service account (e.g., 'arda-bot')
# Generate PAT with 'read:packages' scope
# Store PAT in environment variable: export GITHUB_ORG_PAT=ghp_xxx...

kubectl create secret docker-registry ghcr-secret \
  --docker-server=ghcr.io \
  --docker-username=arda-bot \
  --docker-password=$GITHUB_ORG_PAT \
  --namespace arda-credit
```

### 2. Application Secrets
For Sindri API and Ethereum configuration:

```bash
kubectl create secret generic arda-credit-secrets \
  --from-literal=sindri-api-key=$SINDRI_API_KEY \
  --from-literal=ethereum-rpc-url=$ETHEREUM_RPC_URL \
  --from-literal=ethereum-contract-address=$ETHEREUM_CONTRACT_ADDRESS \
  --from-literal=ethereum-wallet-private-key=$ETHEREUM_WALLET_PRIVATE_KEY \
  --from-literal=ethereum-deployer-address=$ETHEREUM_DEPLOYER_ADDRESS \
  --namespace arda-credit
```

## Configuration Management

The chart separates configuration into:

- **ConfigMap** (`arda-credit-config`): Non-sensitive config like `RUST_LOG`, `SERVER_PORT`, database connection details
- **Secrets** (`arda-credit-secrets`): Sensitive data like API keys and private keys
- **Image Pull Secret** (`ghcr-secret`): Container registry authentication

Update configuration without redeploying:
```bash
# Update ConfigMap
kubectl patch configmap arda-credit-config -p '{"data":{"RUST_LOG":"debug"}}'

# Restart pods to pick up changes
kubectl rollout restart deployment/arda-credit-server
```

## Security Features

### RBAC
Service accounts with minimal required permissions are automatically created.

### Secret Management
Keep sensitive data separate from configuration:
```bash
kubectl create secret generic arda-credit-secrets \
  --from-literal=sindri-api-key=$SINDRI_API_KEY \
  --from-literal=ethereum-wallet-private-key=$ETH_PRIVATE_KEY
```

## Troubleshooting

### API Connectivity Issues

If you're seeing "Failed to fetch" errors when the frontend tries to call API endpoints:

1. **Check ingress routing**:
   ```bash
   kubectl get ingress -n arda-credit
   kubectl describe ingress arda-credit -n arda-credit
   ```

2. **Verify API routing**:
   ```bash
   # Test API endpoint directly
   curl -H "Host: your-domain.com" http://ingress-controller-ip/api/v1/health

   # Check if requests are reaching the server
   kubectl logs -f deployment/arda-credit-server -n arda-credit
   ```

3. **Frontend API calls**: Ensure your frontend makes relative API calls like `/api/v1/health` instead of hardcoded localhost URLs.

4. **CORS issues**: The nginx proxy in the app deployment handles CORS headers. Check app logs for CORS-related errors.

### General Troubleshooting

### Check pod status
```bash
kubectl get pods -n arda-credit
kubectl describe pod <pod-name> -n arda-credit
```

### View logs
```bash
kubectl logs -f deployment/arda-credit-server -n arda-credit
kubectl logs -f deployment/arda-credit-app -n arda-credit
```

### Check autoscaling
```bash
kubectl get hpa -n arda-credit
kubectl describe hpa arda-credit-server -n arda-credit
```

### Test service connectivity
```bash
# Test server service
kubectl run test-pod --image=busybox --rm -it --restart=Never -- sh
# Inside pod:
wget -O- http://arda-credit-server:8080/api/v1/health

# Test app service
wget -O- http://arda-credit-app:80
```

## Testing the API Routing

After deployment, test the API routing setup:

```bash
# 1. Check all resources are created
kubectl get all -n arda-credit

# 2. Test API endpoint through ingress
curl -H "Host: your-domain.com" https://your-ingress-ip/api/v1/health

# 3. Test frontend through ingress
curl -H "Host: your-domain.com" https://your-ingress-ip/

# 4. Check nginx proxy configuration in app pod
kubectl exec -it deployment/arda-credit-app -n arda-credit -- cat /etc/nginx/conf.d/default.conf
```

## Upgrading

```bash
helm upgrade arda-credit ./helm/charts/arda-credit --namespace arda-credit
```

## Uninstalling

```bash
helm uninstall arda-credit --namespace arda-credit
```