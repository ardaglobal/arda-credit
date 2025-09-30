# Local Anvil Implementation Summary

## Executive Summary

We have successfully implemented a **local Anvil devnet integration** for Arda Credit's local and staging environments. This replaces the need for a complex mock contract service with actual smart contracts running on a local blockchain, providing a cleaner, more maintainable, and more realistic testing environment.

## What Was Built

### 1. Docker Compose Integration

**File**: `docker-compose.yml`

Added Anvil as a Docker service with:
- State persistence across restarts
- 2-second block time (faster than real Ethereum)
- Automatic startup in local/staging environments via profiles
- Port 8545 exposed for RPC access

**Key Features:**
- Anvil only starts in local/staging (via `profiles`)
- State saved to Docker volume every 10 seconds
- Can be reset with `make clean-anvil`

### 2. Smart Contracts

**New Contract**: `contracts/src/ARDAFaucet.sol`

A production-quality faucet contract with:
- Rate limiting (1M USDC per hour per address)
- Admin override functions for testing
- Statistics tracking (total distributed, unique recipients)
- SafeERC20 integration
- Comprehensive error handling

**Existing Contract Used**: `contracts/test/mocks/MockUSDC.sol`
- ERC20 token with 6 decimals (matches real USDC)
- Anyone can mint (for testing)
- Fully compatible with ARDA contract

### 3. Deployment Script

**New File**: `contracts/script/Deploy.s.sol`

Environment-aware Foundry deployment script that:
- Reads `SERVER_ENVIRONMENT` variable
- Deploys different contract sets based on environment
- Handles private key parsing (with or without `0x` prefix)
- Saves deployment info to JSON files
- Provides detailed deployment output

**Deployment Matrix:**

| Environment | Deploys | Target |
|------------|---------|--------|
| local | MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier | Anvil (localhost:8545) |
| staging | MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier | Staging Anvil |
| sandbox | ARDA only | Sepolia testnet |
| production | ARDA only | Ethereum mainnet |

### 4. Makefile Enhancements

**Updated Commands:**

#### Environment-Aware Deployment
```bash
make deploy-contracts    # Deploys based on SERVER_ENVIRONMENT
```
- Auto-selects contracts to deploy
- Forces localhost RPC for local/staging
- Requires full env vars for sandbox/production

#### Anvil Management
```bash
make anvil-start     # Start Anvil devnet
make anvil-stop      # Stop Anvil (preserves state)
make anvil-logs      # View logs
make clean-anvil     # Reset blockchain state
```

#### Faucet Commands
```bash
make faucet-request ADDRESS=0x...   # Request 1M USDC
make faucet-status ADDRESS=0x...    # Check balance/status
make faucet-stats                   # View statistics
```

#### Enhanced Server Startup
```bash
make server
```
Now automatically:
1. Starts Anvil (if local/staging)
2. Checks if contracts deployed
3. Deploys if missing
4. Loads addresses from JSON
5. Starts deposit monitor
6. Starts withdrawal executor
7. Starts API server

### 5. Documentation

**New Files:**
- `docs/development/local-anvil-workflow.md` - Complete workflow guide
- `LOCAL_ANVIL_IMPLEMENTATION.md` - This summary

**Updated Files:**
- `AGENTS.md` - Added Anvil and faucet commands
- `Makefile` - Inline help text updated

## Architectural Benefits

### Unified Code Path

**Before:**
- Batch processor: checks `SERVER_ENVIRONMENT`, logs or calls Ethereum client
- Ethereum client: always calls real contract

**After:**
- Batch processor: always calls Ethereum client (no environment check)
- Ethereum client: routes to local Anvil or remote RPC based on configuration
- **No code branching needed** - same execution path in all environments

### Realistic Testing

**What We Get:**
- ✅ Actual contract execution (gas costs, reverts, etc.)
- ✅ Real event emission (deposit/withdrawal monitors work identically)
- ✅ State persistence (blockchain survives restarts)
- ✅ Transaction history (can query past blocks)
- ✅ MetaMask integration (add local network, works like mainnet)

**What We Avoid:**
- ❌ Building mock contract service in Rust
- ❌ Keeping mock behavior in sync with real contracts
- ❌ Different code paths for local vs production
- ❌ Mock-specific bugs that don't reproduce in production

### Developer Experience

**One-Command Setup:**
```bash
make server  # Does everything: starts Anvil, deploys contracts, loads config, runs services
```

**Easy Reset:**
```bash
make clean-anvil  # Reset blockchain, keep everything else
```

**Instant Feedback:**
- 2-second block time (vs 12 seconds on mainnet)
- No gas costs
- No rate limiting
- Immediate transaction finality

## Testing the Implementation

### Successful Test Results

✅ **Anvil startup** - Runs in Docker, accessible on port 8545  
✅ **Contract deployment** - All 4 contracts deployed successfully  
✅ **Faucet request** - Successfully distributed 1M USDC  
✅ **Balance tracking** - User balance shows 1M USDC  
✅ **Rate limiting** - Cooldown enforced correctly  
✅ **State persistence** - Deployment JSON created with all addresses  

### Deployment Output (Successful)

```
==================================
LOCAL DEPLOYMENT COMPLETE
==================================

Contract Addresses:
  MockUSDC:     0x5FbDB2315678afecb367f032d93F642f64180aa3
  Verifier:     0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
  ARDA:         0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0
  Faucet:       0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9

Configuration:
  Deployer:     0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
  Program VKey: 0x035a6b230490471fe1a84470ae9bf66a6521fd76d559d50488f30d5b1ccbfc2b
  Faucet Amount: 1000000 USDC per request
  Cooldown:     1 hour(s)
```

### Faucet Test Results

```bash
$ make faucet-request ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
✅ Faucet request completed!

$ make faucet-status ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
💰 Faucet Balance: 99000000000000 [9.9e13] wei (99M USDC)
💵 User Balance: 1000000000000 [1e12] wei (1M USDC)
⏰ Can Request Now?: false (cooldown active)

$ make faucet-stats
Faucet Balance: 99000000000000 [9.9e13]  # 99M USDC remaining
Unique Recipients: 1                     # 1 address served
Total Distributed: 1000000000000 [1e12]  # 1M USDC distributed
```

## Implementation Details

### Contract Interactions

**Faucet Request Flow:**
1. User calls `faucet.requestFunds()`
2. Faucet checks cooldown period (1 hour)
3. Transfers 1M USDC from faucet to user
4. Emits `FundsRequested` event
5. Updates tracking (last request time, total distributed)

**Deposit Flow:**
1. User deposits MockUSDC to ARDA contract (via UI/MetaMask)
2. ARDA emits `Deposit` event
3. Deposit monitor listens for events
4. Updates `usdc_deposits` table
5. Updates `user_deposit_balances` via trigger

**Withdrawal Flow:**
1. User creates withdrawal request (API)
2. Withdrawal executor calls `arda.withdraw()`
3. ARDA transfers USDC to user
4. ARDA emits `Withdrawal` event
5. Withdrawal monitor updates database

### Environment Variable Handling

**Local/Staging** (automatically set by Makefile):
```bash
ETHEREUM_RPC_URL=http://127.0.0.1:8545
ETHEREUM_CONTRACT_ADDRESS=<loaded from deployment JSON>
ETHEREUM_DEPLOYER_ADDRESS=<loaded from deployment JSON>
VERIFIER_CONTRACT_ADDRESS=<loaded from deployment JSON>
USDC_CONTRACT_ADDRESS=<loaded from deployment JSON>
FAUCET_CONTRACT_ADDRESS=<loaded from deployment JSON>
ETHEREUM_WALLET_PRIVATE_KEY=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

**Sandbox/Production** (must be set by developer):
```bash
ETHEREUM_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/...
ETHEREUM_CONTRACT_ADDRESS=<your deployed contract>
ETHEREUM_DEPLOYER_ADDRESS=<your deployer address>
VERIFIER_CONTRACT_ADDRESS=<SP1 verifier on that chain>
PROGRAM_VKEY=<your program verification key>
ETHEREUM_WALLET_PRIVATE_KEY=<your private key WITHOUT 0x>
```

## Files Changed

### New Files
- `contracts/src/ARDAFaucet.sol` - Faucet smart contract
- `contracts/script/Deploy.s.sol` - Deployment automation script
- `contracts/deployments/local-deployment.json` - Local deployment addresses (generated)
- `api/src/handlers/faucet.rs` - Backend API handler for faucet info
- `docs/development/local-anvil-workflow.md` - Complete workflow documentation
- `docs/integrations/frontend-faucet-integration.md` - Frontend implementation guide
- `test_local_anvil_setup.sh` - End-to-end test suite
- `LOCAL_ANVIL_IMPLEMENTATION.md` - This summary

### Modified Files
- `docker-compose.yml` - Added Anvil service with profiles
- `Makefile` - Added anvil/faucet commands, updated deployment
- `AGENTS.md` - Updated workflow documentation
- `api/src/handlers/mod.rs` - Added faucet module
- `api/src/config/paths.rs` - Added faucet API paths
- `api/src/api/router.rs` - Added faucet routes
- `api/src/services/batch_processor.rs` - Removed `SERVER_ENVIRONMENT` check
- `.gitignore` - Ignore local deployment files

## Next Steps

### Immediate (For This PR)
- [x] Implement Anvil Docker integration
- [x] Create faucet contract
- [x] Build deployment script
- [x] Update Makefile with new commands
- [x] Test end-to-end workflow
- [x] Document the changes

### Future Enhancements
- [x] **Backend API Endpoints**: Faucet info, status, and config endpoints created
- [ ] **Frontend Faucet UI**: Implement using `docs/integrations/frontend-faucet-integration.md`
- [ ] **Auto-Faucet on Registration**: Give new users test USDC automatically
- [ ] **Staging Environment**: Deploy shared staging Anvil for team testing
- [ ] **CI Integration**: Use Anvil in GitHub Actions for integration tests
- [ ] **MetaMask Guide**: User-facing step-by-step guide
- [ ] **Multi-Chain Support**: Support Base, Arbitrum, Optimism local devnets

### Migration Path

For existing developers:

```bash
# Clean old setup
make down
make clean

# Setup with new Anvil workflow
make up                  # Now includes Anvil
make deploy-contracts    # Deploy to local Anvil
make server              # Works with local contracts
```

## FAQ

**Q: Do I need to change my .env file?**  
A: No! For local development, the Makefile automatically overrides to use Anvil. Your existing .env with Sepolia settings will be used for sandbox/production.

**Q: Where does the private key in the Makefile come from?**  
A: It's the standard Anvil test account private key (`0xac09...`). This is a publicly-known test key that Anvil generates from its default mnemonic ("test test test..."). It's safe to use in local development because it only controls test ETH on your local blockchain. **Never use this key on real networks!** For sandbox/production, we use your actual private key from `.env`.

**Q: Can I change the Anvil test accounts?**  
A: Yes! You can modify the Anvil configuration in `docker-compose.yml` to use a custom mnemonic or generate different test accounts. However, using the default is recommended for consistency across the team.

**Q: How do I reset everything?**  
A: `make clean-anvil && make deploy-contracts`

**Q: Can I use MetaMask with local Anvil?**  
A: Yes! Add network with RPC `http://127.0.0.1:8545`, Chain ID `31337`. Import the test account with private key `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` to get 10,000 test ETH.

**Q: Will my deposit/withdrawal monitors work?**  
A: Yes! They listen to real contract events from Anvil, just like on mainnet.

**Q: How much USDC can I get from the faucet?**  
A: 1M USDC per hour, per address. Faucet has 100M total (100 requests).

**Q: What if I need more USDC?**  
A: Wait 1 hour for cooldown, or redeploy contracts (fresh 100M faucet), or use admin `requestFundsImmediate()`

**Q: Does this work in CI?**  
A: Not yet, but it's easy to add! Anvil can run in GitHub Actions for integration tests.

**Q: How does the frontend request faucet funds?**  
A: The backend provides faucet info via API (`/api/v1/faucet/info`), then the frontend calls the faucet contract directly via MetaMask. See `docs/integrations/frontend-faucet-integration.md` for complete implementation guide.

## Conclusion

This implementation provides a **production-like local development environment** without the complexity of building mock services. By using real contracts on a local devnet, we get:

1. **Consistency** - Same code, same contracts, same behavior across environments
2. **Speed** - Fast iteration without network calls
3. **Simplicity** - No mock service to maintain
4. **Realism** - Catches contract-related bugs early
5. **Developer Joy** - One command to start everything: `make server`

The architecture is clean, well-documented, and ready for team adoption.

