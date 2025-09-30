# Local Anvil Development Workflow

## Overview

For local and staging environments, Arda Credit now uses a **local Anvil devnet** with deployed smart contracts instead of remote Ethereum testnets. This provides:

- ✅ **Faster development** - No network latency or gas costs
- ✅ **Consistent state** - Persisted blockchain state across restarts
- ✅ **Self-contained** - No external dependencies (Alchemy, Infura, etc.)
- ✅ **Realistic testing** - Uses actual smart contracts, not mocks
- ✅ **Easy reset** - Clean state with `make clean-anvil`

## Architecture

### Environment-Based Deployment

| Environment | Blockchain | Contracts Deployed | Faucet Available |
|------------|------------|-------------------|------------------|
| **local** | Anvil (localhost:8545) | MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier | ✅ Yes (1M USDC/hr) |
| **staging** | Anvil (staging server) | MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier | ✅ Yes (1M USDC/hr) |
| **sandbox** | Sepolia testnet | ARDA + Real USDC | ❌ Use Sepolia faucets |
| **production** | Ethereum mainnet | ARDA + Real USDC | ❌ Real USDC only |

### Deployed Contracts (Local/Staging)

1. **MockUSDC** - ERC20 token with 6 decimals (like real USDC)
2. **SP1MockVerifier** - Mock ZK proof verifier for testing
3. **ARDA** - Main contract (configured to use MockUSDC)
4. **ARDAFaucet** - Test USDC distribution (1M USDC per hour per address)

## Quick Start

### 1. Start Local Environment

```bash
# Start Postgres + Anvil
make up

# Deploy contracts to Anvil (one-time or after clean)
make deploy-contracts

# Start API server with all services
make server
```

The `make server` command now automatically:
- Starts Anvil if in local/staging environment
- Checks if contracts are deployed
- Deploys contracts if missing
- Loads contract addresses from deployment file
- Starts deposit monitor, withdrawal executor, and API server

### 2. Request Test USDC

```bash
# Request 1M USDC from faucet
make faucet-request ADDRESS=0xYourWalletAddress

# Check your balance and faucet status
make faucet-status ADDRESS=0xYourWalletAddress

# View overall faucet statistics
make faucet-stats
```

### 3. Use USDC in Application

Once you have USDC from the faucet, you can:

```bash
# Deposit USDC to ARDA contract (via MetaMask UI or cast)
# The deposit monitor will detect it and update your database balance

# Make deal payments, withdrawals, etc. - all using the local devnet!
```

## Faucet Details

### Configuration

- **Amount per request**: 1,000,000 USDC (1M)
- **Cooldown period**: 1 hour
- **Initial faucet balance**: 100,000,000 USDC (100M)
- **Access**: Anyone can request (rate-limited by address)

### Faucet Commands

#### Request Funds
```bash
make faucet-request ADDRESS=0x742d35Cc6...
```

**Output:**
```
💰 Requesting faucet funds for 0x742d35Cc6...
📋 Faucet contract: 0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9
📋 USDC contract: 0x5FbDB2315678afecb367f032d93F642f64180aa3
✅ Faucet request completed!
```

#### Check Status
```bash
make faucet-status ADDRESS=0x742d35Cc6...
```

**Output:**
```
📊 Checking faucet status for 0x742d35Cc6...

📋 Contracts:
  Faucet: 0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9
  USDC:   0x5FbDB2315678afecb367f032d93F642f64180aa3

💰 Faucet Balance:
99000000000000 [9.9e13] wei

💵 User Balance:
1000000000000 [1e12] wei

⏰ Can Request Now?:
false
```

#### View Statistics
```bash
make faucet-stats
```

**Output:**
```
📊 Faucet Statistics...
Faucet Contract: 0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9

99000000000000 [9.9e13]  # Faucet balance (wei)
1                         # Unique recipients
1000000000000 [1e12]      # Total distributed (wei)
```

## Anvil Management

### Start/Stop Anvil

```bash
# Start Anvil manually
make anvil-start

# Stop Anvil (preserves state)
make anvil-stop

# View Anvil logs
make anvil-logs
```

### Reset Blockchain State

```bash
# Clean Anvil state (deletes all transactions and contracts)
make clean-anvil

# Redeploy contracts after reset
make deploy-contracts
```

### Check Anvil Status

```bash
# Check if Anvil is running
docker ps | grep anvil

# Get current block number
cast block-number --rpc-url http://127.0.0.1:8545

# Get chain ID
cast chain-id --rpc-url http://127.0.0.1:8545
```

## Contract Deployment

### Automatic Deployment

The `make server` command automatically handles deployment for local/staging:

```bash
SERVER_ENVIRONMENT=local make server
```

This will:
1. Start Anvil if not running
2. Check for `contracts/deployments/local-deployment.json`
3. Deploy contracts if file doesn't exist
4. Load contract addresses from deployment file
5. Start all services with correct configuration

### Manual Deployment

```bash
# Deploy to local Anvil
SERVER_ENVIRONMENT=local make deploy-contracts

# Deploy to staging Anvil
SERVER_ENVIRONMENT=staging make deploy-contracts

# Deploy to Sepolia testnet
SERVER_ENVIRONMENT=sandbox make deploy-contracts

# Deploy to mainnet
SERVER_ENVIRONMENT=production make deploy-contracts
```

### Redeployment

To redeploy contracts (e.g., after contract changes):

```bash
# Clean Anvil state and redeploy
make clean-anvil && make deploy-contracts
```

## Deployment Files

Contract addresses are saved to environment-specific JSON files:

```bash
contracts/deployments/
  local-deployment.json      # Local Anvil deployment
  staging-deployment.json    # Staging Anvil deployment
  sandbox-deployment.json    # Sepolia testnet deployment (optional)
  production-deployment.json # Mainnet deployment (optional)
```

**Example `local-deployment.json`:**
```json
{
  "arda": "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
  "usdc": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
  "faucet": "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9",
  "verifier": "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512",
  "deployer": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
  "programVKey": "0x035a6b230490471fe1a84470ae9bf66a6521fd76d559d50488f30d5b1ccbfc2b",
  "chainId": 31337,
  "blockNumber": 5,
  "timestamp": 1759255043,
  "environment": "local"
}
```

## Anvil Configuration

### Docker Compose Settings

- **Host**: `0.0.0.0` (accessible from host machine)
- **Port**: `8545` (standard Ethereum RPC port)
- **Chain ID**: `31337` (standard Anvil chain ID)
- **Block Time**: `2 seconds` (faster than real Ethereum)
- **State Persistence**: `/anvil-state/state.json` (Docker volume)
- **State Save Interval**: Every 10 seconds

### Default Test Accounts

Anvil provides 10 test accounts with 10,000 ETH each:

```bash
Account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
Private Key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

Account #1: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
Private Key: 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d

# ... 8 more accounts available
```

## Integration with MetaMask (UI Testing)

### 1. Add Anvil Network to MetaMask

1. Open MetaMask → Settings → Networks → Add Network
2. Configure:
   - **Network Name**: Local Anvil Devnet
   - **RPC URL**: `http://127.0.0.1:8545`
   - **Chain ID**: `31337`
   - **Currency Symbol**: ETH

### 2. Import Test Account

1. MetaMask → Import Account
2. Paste Anvil private key: `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`
3. You now have 10,000 ETH on local devnet

### 3. Add MockUSDC Token to MetaMask

1. MetaMask → Import Token
2. Get USDC address from deployment file:
   ```bash
   cat contracts/deployments/local-deployment.json | jq -r '.usdc'
   ```
3. Paste address into MetaMask (symbol: USDC, decimals: 6)

### 4. Use Faucet via UI

**Option A: Direct Contract Interaction**
- In MetaMask, send transaction to faucet contract
- Call `requestFunds()` function
- Receive 1M USDC

**Option B: Via Arda Credit UI** (when built)
- UI can provide "Request Test USDC" button
- Calls faucet contract via Web3 provider
- Updates user balance automatically

## Troubleshooting

### Anvil Not Starting

**Check if Anvil container is running:**
```bash
docker ps | grep anvil
```

**View Anvil logs:**
```bash
make anvil-logs
```

**Restart Anvil:**
```bash
make anvil-stop && make anvil-start
```

### Contracts Not Deploying

**Error: "Connection refused" or "Empty reply from server"**
- Anvil may not be fully started yet
- Wait 3-5 seconds after `make anvil-start`
- Check Anvil logs for errors

**Error: "Deployment file not found"**
- Run `make deploy-contracts` manually
- Check `contracts/deployments/local-deployment.json` exists

### Faucet Issues

**Error: "CooldownNotElapsed"**
- You can only request once per hour
- Wait for cooldown period to complete
- Or use admin command `requestFundsImmediate()` (requires owner)

**Error: "InsufficientFaucetBalance"**
- Faucet ran out of test USDC
- Redeploy contracts: `make clean-anvil && make deploy-contracts`
- Or refill faucet manually with `cast send`

### State Persistence Issues

**Contracts disappeared after restart:**
- Check if Anvil volume exists: `docker volume ls | grep anvil`
- Verify state file: `docker exec arda_credit_anvil ls -la /anvil-state/`
- If state is corrupted, run `make clean-anvil && make deploy-contracts`

## Advanced Usage

### Direct Contract Interaction with Cast

```bash
# Get faucet balance
cast call FAUCET_ADDR "getFaucetBalance()" --rpc-url http://127.0.0.1:8545

# Get user USDC balance
cast call USDC_ADDR "balanceOf(address)(uint256)" YOUR_ADDRESS --rpc-url http://127.0.0.1:8545

# Request funds (from contract directly)
cast send FAUCET_ADDR "requestFunds()" \
  --rpc-url http://127.0.0.1:8545 \
  --private-key YOUR_PRIVATE_KEY

# Check cooldown time
cast call FAUCET_ADDR "getTimeUntilNextRequest(address)(uint256)" YOUR_ADDRESS \
  --rpc-url http://127.0.0.1:8545
```

### Admin Functions (Owner Only)

```bash
# Bypass cooldown (immediate funds)
cast send FAUCET_ADDR "requestFundsImmediate(address)" YOUR_ADDRESS \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Request custom amount
cast send FAUCET_ADDR "requestCustomAmount(address,uint256)" YOUR_ADDRESS 5000000000000 \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Disable/enable faucet
cast send FAUCET_ADDR "setEnabled(bool)" false \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### Monitoring Events

```bash
# Watch for deposit events
cast logs --address ARDA_CONTRACT_ADDR --rpc-url http://127.0.0.1:8545

# Watch for faucet requests
cast logs --address FAUCET_ADDR --rpc-url http://127.0.0.1:8545

# Get block number
cast block-number --rpc-url http://127.0.0.1:8545

# Get latest block
cast block latest --rpc-url http://127.0.0.1:8545
```

## Workflow Examples

### Complete Local Development Session

```bash
# 1. Start services (first time)
make up                          # Start Postgres + Anvil
make deploy-contracts            # Deploy contracts to Anvil
make server                      # Start API server

# 2. Request test USDC
make faucet-request ADDRESS=0xYourAddress

# 3. Verify balance
make faucet-status ADDRESS=0xYourAddress

# 4. Use Arda Credit normally
make cli ARGS="register-user --user-type investor --email test@example.com"
# ... normal deal operations ...

# 5. Clean up when done
make down                        # Stop all services
make clean-anvil                 # Optional: reset blockchain state
```

### Daily Development (After Initial Setup)

```bash
# Services already configured, just start them
make server

# Anvil state persisted, contracts still deployed!
# Continue where you left off yesterday
```

### Reset and Start Fresh

```bash
# Complete reset
make clean                       # Cleans Docker, SQLx, builds, AND Anvil
make setup                       # Reinstall, init DB, deploy contracts
make server                      # Start fresh
```

## Environment Variable Reference

### Local/Staging (Auto-configured)

These are automatically set by `make server` for local/staging environments:

```bash
ETHEREUM_RPC_URL=http://127.0.0.1:8545
ETHEREUM_CONTRACT_ADDRESS=<from deployment JSON>
ETHEREUM_DEPLOYER_ADDRESS=<from deployment JSON>
VERIFIER_CONTRACT_ADDRESS=<from deployment JSON>
USDC_CONTRACT_ADDRESS=<from deployment JSON>
FAUCET_CONTRACT_ADDRESS=<from deployment JSON>
ETHEREUM_WALLET_PRIVATE_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### Sandbox/Production (Manual Configuration)

Set these in your `.env` file for sandbox/production:

```bash
SERVER_ENVIRONMENT=sandbox  # or production
ETHEREUM_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY
ETHEREUM_CONTRACT_ADDRESS=<deployed contract address>
ETHEREUM_DEPLOYER_ADDRESS=<your deployer address>
VERIFIER_CONTRACT_ADDRESS=<SP1 verifier address>
PROGRAM_VKEY=<program verification key>
ETHEREUM_WALLET_PRIVATE_KEY=<your private key WITHOUT 0x prefix>
```

## Benefits Over Mock Service

This approach is superior to building a mock service because:

1. **Identical Code Paths**: Same Ethereum client code runs in all environments
2. **Real Contract Behavior**: Tests include gas costs, revert errors, event emission
3. **No Mock Maintenance**: No need to keep mock service in sync with real contracts
4. **Better Bug Detection**: Issues found locally will reproduce in production
5. **Faster Debugging**: Use Foundry tools (cast, anvil) for debugging
6. **State Inspection**: Query contract state directly with cast
7. **Event Testing**: Real events that deposit/withdrawal monitors can consume
8. **Frontend Testing**: MetaMask integration works identically to production

## Next Steps

- **Frontend Integration**: Add faucet UI button for easy token requests
- **CI/CD**: GitHub Actions can use Anvil for integration tests
- **Team Testing**: Staging environment with shared Anvil for team collaboration
- **Documentation**: API docs showing faucet endpoints for UI integration

## Related Documentation

- [Development Setup](development-setup.md) - Complete setup guide
- [SERVER_ENVIRONMENT Configuration](../SERVER_ENVIRONMENT.md) - Environment variable details
- [Contracts README](../../contracts/README.md) - Smart contract documentation
- [Ethereum Client README](../../ethereum-client/README.md) - Ethereum client usage

