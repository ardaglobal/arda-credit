# Automatic Contract Deployment in Kubernetes

## Overview

For local and staging environments, Arda Credit automatically deploys smart contracts to the Anvil devnet when the API server starts. This ensures contracts are always up-to-date without manual intervention.

## How It Works

### Architecture

1. **Anvil Service**: Runs as a separate pod providing a local Ethereum devnet
2. **Init Container**: Runs before the main API server starts
   - Waits for Anvil to be ready
   - Deploys all contracts (MockUSDC, ARDAFaucet, ARDA, SP1MockVerifier)
   - Writes deployment info to a shared volume
3. **API Server**: Reads deployment info from the shared volume

### Components

#### 1. Contract Deployer Image

A specialized Docker image (`arda-credit-contract-deployer`) that includes:
- Foundry tools (forge, cast, anvil)
- Contract source code
- Pre-compiled contract artifacts
- Deployment scripts

**Built from**: `Dockerfile.contract-deployer`

#### 2. Deployment Script

A bash script that:
- Waits for Anvil to be responsive
- Checks if contracts are already deployed
- Runs `forge script` to deploy contracts
- Parses deployment addresses
- Creates deployment JSON file in expected format

**Location**: Helm ConfigMap `contract-deployer-configmap.yaml`

#### 3. Kubernetes Resources

- **Init Container**: Runs deployment before server starts
- **Shared Volume**: EmptyDir volume for deployment artifacts
- **ConfigMap**: Contains deployment script

## Usage

### Building Images

```bash
# Build contract deployer image
make docker-build-contract-deployer

# Push to registry
make docker-push-contract-deployer

# Build all images (main + contract deployer)
make docker-build-all
make docker-push-all
```

### Deployment

The contract deployment happens automatically when deploying to Kubernetes with `server.environment` set to `local` or `staging`:

```bash
# Deploy with Helm
helm install arda-credit ./helm/charts/arda-credit \
  --set server.environment=staging \
  --namespace arda-credit

# Or upgrade existing deployment
helm upgrade arda-credit ./helm/charts/arda-credit \
  --set server.environment=staging \
  --namespace arda-credit
```

### Updating Contracts

When you update contracts and want to redeploy:

1. **Update contract source code** in `contracts/src/`

2. **Rebuild contract deployer image**:
   ```bash
   make docker-build-contract-deployer
   make docker-push-contract-deployer
   ```

3. **Restart the API server pod**:
   ```bash
   kubectl rollout restart deployment arda-credit-server -n arda-credit
   ```

   The init container will run again and redeploy contracts.

4. **Or restart Anvil** (if using persistent state):
   ```bash
   kubectl delete pod -l app.kubernetes.io/component=anvil -n arda-credit
   ```
   
   This resets the blockchain state and the next server restart will redeploy contracts.

## Configuration

### Helm Values

```yaml
server:
  environment: "staging"  # or "local"
  
  contractDeployer:
    image:
      registry: ghcr.io/ardaglobal
      repository: arda-credit-contract-deployer
      tag: "latest"
      pullPolicy: Always
```

### Environment Variables

The init container receives:
- `ETHEREUM_RPC_URL`: Anvil service URL (auto-configured)
- `ETHEREUM_PRIVATE_KEY`: Deployer private key (from secrets)

### Deployment File Location

The deployment file is written to:
```
/deployments/{environment}-deployment.json
```

And mounted in the API server at:
```
/app/contracts/deployments/{environment}-deployment.json
```

This matches the path expected by the faucet handler and other contract interaction code.

## Deployment File Format

```json
{
  "deployer": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
  "usdc": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
  "faucet": "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512",
  "verifier": "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
  "arda": "0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9",
  "chainId": 31337,
  "timestamp": "2025-10-02T12:34:56Z",
  "environment": "staging"
}
```

## Troubleshooting

### Init Container Fails

Check logs:
```bash
kubectl logs -l app.kubernetes.io/component=server -c contract-deployer -n arda-credit
```

Common issues:
- Anvil not ready: Init container waits up to 60 seconds
- Contract compilation errors: Check contract source
- Deployment script errors: Check ConfigMap

### Contracts Not Found

If the API server can't find contracts:

1. Check init container ran successfully:
   ```bash
   kubectl describe pod -l app.kubernetes.io/component=server -n arda-credit
   ```

2. Check deployment file exists:
   ```bash
   kubectl exec -it deployment/arda-credit-server -n arda-credit -- ls -la /app/contracts/deployments/
   ```

3. Check deployment file content:
   ```bash
   kubectl exec -it deployment/arda-credit-server -n arda-credit -- cat /app/contracts/deployments/staging-deployment.json
   ```

### Contract Addresses Changed

If Anvil restarted and addresses changed:

1. Restart the API server to trigger redeployment:
   ```bash
   kubectl rollout restart deployment arda-credit-server -n arda-credit
   ```

## Production vs Development

| Environment | Anvil | Contract Deployment | Contract Source |
|-------------|-------|---------------------|-----------------|
| local       | ✅ Yes | ✅ Automatic        | Init container  |
| staging     | ✅ Yes | ✅ Automatic        | Init container  |
| sandbox     | ❌ No  | ❌ Manual           | Testnet         |
| production  | ❌ No  | ❌ Manual           | Mainnet         |

In sandbox/production:
- Use real testnets/mainnet
- Deploy contracts manually with `make deploy-contracts`
- Configure contract addresses in secrets
- No init container runs

## CI/CD Integration

The contract deployer image should be built and pushed in your CI/CD pipeline alongside the main application image.

Example GitHub Actions workflow:
```yaml
- name: Build and push contract deployer
  run: |
    make docker-build-contract-deployer DOCKER_TAG=${{ github.sha }}
    make docker-push-contract-deployer DOCKER_TAG=${{ github.sha }}
```

## Benefits

✅ **Always Fresh Contracts**: Latest contract code automatically deployed  
✅ **No Manual Steps**: Eliminates deployment coordination issues  
✅ **Development Parity**: Local and staging work identically  
✅ **Fast Iteration**: Update contracts, restart pod, done  
✅ **Self-Healing**: Contracts redeploy if Anvil state is lost  

## See Also

- [Local Anvil Workflow](local-anvil-workflow.md)
- [Contract Development](../../contracts/README.md)
- [Deployment Guide](../deployment/development-setup.md)

