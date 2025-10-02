# Contract Auto-Deployment Setup Guide

## Summary

This update adds automatic contract deployment to Anvil in Kubernetes for local/staging environments. Contracts are deployed via an init container before the API server starts, ensuring they're always available and up-to-date.

## What Was Changed

### New Files

1. **`Dockerfile.contract-deployer`**
   - Builds a specialized image with Foundry tools and contracts
   - Based on `ghcr.io/foundry-rs/foundry:latest`
   - Includes contract source, forge, cast, and jq

2. **`helm/charts/arda-credit/templates/contract-deployer-configmap.yaml`**
   - ConfigMap containing the deployment script
   - Waits for Anvil, deploys contracts, writes deployment JSON
   - Only created for local/staging environments

3. **`docs/development/contract-auto-deployment.md`**
   - Complete documentation for the auto-deployment feature
   - Usage guide, troubleshooting, architecture details

4. **`CONTRACT_AUTO_DEPLOY_SETUP.md`** (this file)
   - Quick start guide and change summary

### Modified Files

1. **`helm/charts/arda-credit/templates/server-deployment.yaml`**
   - Added init container that runs contract deployer
   - Added shared emptyDir volume for deployment artifacts
   - Added volume mount for deployment files
   - Changes only apply when `server.environment` is "local" or "staging"

2. **`helm/charts/arda-credit/values.yaml`**
   - Added `server.contractDeployer` configuration section
   - Defines the contract deployer image registry, repository, and tag

3. **`Makefile`**
   - Added `docker-build-contract-deployer` target
   - Added `docker-push-contract-deployer` target
   - Added `docker-build-all` and `docker-push-all` for building both images
   - Updated `docker-lint` to include new Dockerfile

## Quick Start

### 1. Build the Contract Deployer Image

```bash
# Build locally
make docker-build-contract-deployer

# Or build and push to registry
make docker-push-contract-deployer

# Or build everything at once
make docker-build-all
make docker-push-all
```

### 2. Deploy to Kubernetes

```bash
# Deploy with Helm (contracts will auto-deploy)
helm install arda-credit ./helm/charts/arda-credit \
  --set server.environment=staging \
  --namespace arda-credit \
  --create-namespace

# Or upgrade existing deployment
helm upgrade arda-credit ./helm/charts/arda-credit \
  --namespace arda-credit
```

### 3. Verify Deployment

```bash
# Check init container logs
kubectl logs -l app.kubernetes.io/component=server \
  -c contract-deployer \
  -n arda-credit

# Check deployment file was created
kubectl exec deployment/arda-credit-server -n arda-credit -- \
  cat /app/contracts/deployments/staging-deployment.json
```

## How It Works

### Flow

1. **Helm Chart Deployment**
   - Creates Anvil pod (local Ethereum devnet)
   - Creates API server pod with init container

2. **Init Container Runs**
   - Waits for Anvil to be ready (up to 60 seconds)
   - Checks if contracts already deployed (skip if yes)
   - Runs `forge script script/Deploy.s.sol` to deploy contracts
   - Parses deployment addresses from broadcast file
   - Creates deployment JSON in `/deployments/`

3. **Main Container Starts**
   - Reads deployment file from shared volume
   - Faucet handler and other code find contract addresses
   - API endpoints work with fresh contracts

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ API Server Pod                                              │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Init Container: contract-deployer                    │  │
│  │ - Waits for Anvil                                    │  │
│  │ - Deploys contracts                                  │  │
│  │ - Writes /deployments/staging-deployment.json       │  │
│  └──────────────────┬───────────────────────────────────┘  │
│                     │ (exits successfully)                 │
│                     ▼                                       │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Main Container: server                               │  │
│  │ - Reads /app/contracts/deployments/...json          │  │
│  │ - Faucet API uses contract addresses                │  │
│  │ - Serves requests                                    │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  Shared Volume: emptyDir                                   │
│  └─ /deployments/staging-deployment.json                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Anvil Pod                                                   │
│ - Local Ethereum devnet (chain ID 31337)                   │
│ - Accessible at: arda-credit-anvil:8545                     │
└─────────────────────────────────────────────────────────────┘
```

## Updating Contracts

When you modify contracts and want to deploy the changes:

### Option 1: Rebuild and Redeploy (Recommended)

```bash
# 1. Update contract source in contracts/src/
# 2. Rebuild contract deployer image
make docker-build-contract-deployer
make docker-push-contract-deployer

# 3. Update Helm deployment (triggers pod restart)
helm upgrade arda-credit ./helm/charts/arda-credit \
  --set server.contractDeployer.image.tag=latest \
  --reuse-values \
  -n arda-credit

# Or just restart the deployment
kubectl rollout restart deployment/arda-credit-server -n arda-credit
```

### Option 2: Reset Anvil State

```bash
# Delete Anvil pod (loses all blockchain state)
kubectl delete pod -l app.kubernetes.io/component=anvil -n arda-credit

# Restart API server (will redeploy contracts to fresh Anvil)
kubectl rollout restart deployment/arda-credit-server -n arda-credit
```

## Configuration

### Helm Values

```yaml
server:
  # Environment determines if auto-deployment runs
  environment: "staging"  # local, staging, sandbox, or production
  
  # Contract deployer image configuration
  contractDeployer:
    image:
      registry: ghcr.io/ardaglobal
      repository: arda-credit-contract-deployer
      tag: "latest"
      pullPolicy: Always
```

### Environments

| Environment | Auto-Deploy | Anvil | Notes |
|-------------|-------------|-------|-------|
| `local`     | ✅ Yes      | ✅ Yes | Full automation |
| `staging`   | ✅ Yes      | ✅ Yes | Full automation |
| `sandbox`   | ❌ No       | ❌ No  | Use Sepolia testnet |
| `production`| ❌ No       | ❌ No  | Use mainnet |

### Private Key

The init container uses `ETHEREUM_PRIVATE_KEY` from secrets to deploy contracts. By default, this is the Anvil default private key for local/staging:

```
0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

**⚠️ Never use this key in production!**

## Troubleshooting

### Init Container Fails to Start

```bash
# Check pod status
kubectl get pods -l app.kubernetes.io/component=server -n arda-credit

# Check init container logs
kubectl logs -l app.kubernetes.io/component=server \
  -c contract-deployer \
  -n arda-credit --tail=100
```

Common issues:
- **Anvil not ready**: Init waits 60 seconds, check Anvil pod is running
- **Image pull errors**: Ensure contract-deployer image is built and pushed
- **Permission errors**: Check security contexts in values.yaml

### Contracts Not Found by API

```bash
# 1. Verify deployment file exists
kubectl exec deployment/arda-credit-server -n arda-credit -- \
  ls -la /app/contracts/deployments/

# 2. Check file contents
kubectl exec deployment/arda-credit-server -n arda-credit -- \
  cat /app/contracts/deployments/staging-deployment.json

# 3. Check faucet API response
kubectl exec deployment/arda-credit-server -n arda-credit -- \
  curl http://localhost:8080/api/v1/faucet/info
```

### Need to Force Redeployment

```bash
# Delete the deployment file and restart
kubectl exec deployment/arda-credit-server -n arda-credit -- \
  rm /app/contracts/deployments/staging-deployment.json

kubectl delete pod -l app.kubernetes.io/component=server -n arda-credit
```

## CI/CD Integration

Add to your GitHub Actions or CI pipeline:

```yaml
# Build and push both images
- name: Build main image
  run: make docker-push DOCKER_TAG=${{ github.sha }}

- name: Build contract deployer image
  run: make docker-push-contract-deployer DOCKER_TAG=${{ github.sha }}

# Or build all at once
- name: Build all images
  run: make docker-push-all DOCKER_TAG=${{ github.sha }}
```

## Benefits

### Before (Manual Process)
1. Deploy contracts manually to Anvil
2. Copy contract addresses
3. Update environment variables
4. Restart server
5. Hope nobody restarted Anvil in the meantime

### After (Automatic Process)
1. Deploy to Kubernetes
2. ✨ Contracts automatically deployed
3. ✨ Addresses automatically discovered
4. ✨ Everything just works

### Key Improvements

✅ **Zero Manual Steps**: No more manual contract deployment  
✅ **Always Consistent**: Contracts match your code version  
✅ **Self-Healing**: Contracts redeploy if Anvil restarts  
✅ **Fast Iteration**: Update contracts, restart pod, done  
✅ **Development Parity**: Local dev, staging, and tests all work the same  

## Testing Locally

You can test the init container locally with Docker:

```bash
# Build the image
make docker-build-contract-deployer

# Run Anvil
docker run -d --name anvil -p 8545:8545 ghcr.io/foundry-rs/foundry:latest anvil --host 0.0.0.0

# Run the deployer
docker run --rm \
  --network host \
  -e ETHEREUM_RPC_URL=http://127.0.0.1:8545 \
  -e ETHEREUM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  -v $(pwd)/deployment-test:/deployments \
  ghcr.io/ardaglobal/arda-credit-contract-deployer:$(whoami)-dev \
  /bin/bash /contracts/deploy-script.sh

# Check results
cat deployment-test/staging-deployment.json
```

## Next Steps

1. **Build and push the contract deployer image**
   ```bash
   make docker-push-contract-deployer
   ```

2. **Deploy or upgrade your Helm release**
   ```bash
   helm upgrade --install arda-credit ./helm/charts/arda-credit \
     -n arda-credit --create-namespace
   ```

3. **Test the faucet endpoint**
   ```bash
   kubectl port-forward svc/arda-credit-server 8080:8080 -n arda-credit
   curl http://localhost:8080/api/v1/faucet/info
   ```

4. **Check the documentation** at `docs/development/contract-auto-deployment.md` for more details

## Questions?

- 📚 See full documentation: `docs/development/contract-auto-deployment.md`
- 🐛 Issues with deployment? Check troubleshooting section above
- 💡 Want to customize? Check the Helm values and ConfigMap

---

**Happy deploying! 🚀**

