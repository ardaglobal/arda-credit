#!/bin/bash
# Test script for contract deployer init container
# Tests the deployment locally before pushing to Kubernetes

set -e

echo "🧪 Testing Contract Deployer Init Container"
echo "=========================================="
echo ""

# Configuration
IMAGE_NAME="ghcr.io/ardaglobal/arda-credit-contract-deployer:$(whoami)-dev"
ANVIL_CONTAINER="anvil-test-$$"
TEST_DIR="/tmp/arda-contract-deploy-test-$$"
ANVIL_PORT=8545

cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    docker stop "$ANVIL_CONTAINER" 2>/dev/null || true
    docker rm "$ANVIL_CONTAINER" 2>/dev/null || true
    rm -rf "$TEST_DIR"
    echo "✅ Cleanup complete"
}

trap cleanup EXIT

# Step 1: Build the contract deployer image
echo "📦 Step 1: Building contract deployer image..."
make docker-build-contract-deployer
echo ""

# Step 2: Start Anvil
echo "🔨 Step 2: Starting Anvil devnet..."
docker run -d \
    --name "$ANVIL_CONTAINER" \
    -p "$ANVIL_PORT:8545" \
    ghcr.io/foundry-rs/foundry:latest \
    anvil --host 0.0.0.0

# Wait for Anvil to be ready
echo "⏳ Waiting for Anvil to be ready..."
for i in {1..30}; do
    if cast client --rpc-url "http://127.0.0.1:$ANVIL_PORT" 2>/dev/null; then
        echo "✅ Anvil is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Anvil failed to start"
        exit 1
    fi
    sleep 1
done
echo ""

# Step 3: Create test directory
echo "📁 Step 3: Creating test directory..."
mkdir -p "$TEST_DIR"
echo ""

# Step 4: Create deployment script
echo "📝 Step 4: Creating deployment script..."
cat > "$TEST_DIR/deploy-contracts.sh" <<'DEPLOY_SCRIPT'
#!/bin/bash
set -e

echo "🚀 Contract Deployer Init Container Starting..."
echo "📋 Environment: test"

# Wait for Anvil to be ready
echo "⏳ Waiting for Anvil to be ready..."
ANVIL_URL="${ETHEREUM_RPC_URL:-http://host.docker.internal:8545}"
MAX_RETRIES=30
RETRY_COUNT=0

until cast client --rpc-url "$ANVIL_URL" 2>/dev/null || [ $RETRY_COUNT -eq $MAX_RETRIES ]; do
  RETRY_COUNT=$((RETRY_COUNT + 1))
  echo "Waiting for Anvil... (attempt $RETRY_COUNT/$MAX_RETRIES)"
  sleep 2
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
  echo "❌ Failed to connect to Anvil after $MAX_RETRIES attempts"
  exit 1
fi

echo "✅ Anvil is ready!"

# Check if contracts are already deployed
DEPLOYMENT_FILE="/deployments/test-deployment.json"
if [ -f "$DEPLOYMENT_FILE" ]; then
  echo "📦 Found existing deployment file: $DEPLOYMENT_FILE"
  ARDA_ADDRESS=$(jq -r '.arda' "$DEPLOYMENT_FILE" 2>/dev/null || echo "")
  if [ -n "$ARDA_ADDRESS" ] && [ "$ARDA_ADDRESS" != "null" ]; then
    echo "🔍 Verifying contract at $ARDA_ADDRESS..."
    if cast code "$ARDA_ADDRESS" --rpc-url "$ANVIL_URL" 2>/dev/null | grep -q "^0x"; then
      echo "✅ Contracts already deployed and verified!"
      exit 0
    else
      echo "⚠️  Contract not found at $ARDA_ADDRESS, redeploying..."
    fi
  fi
fi

echo "📦 Deploying contracts to Anvil..."
cd /contracts

# Deploy using forge script
PRIVATE_KEY="${ETHEREUM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"

# Run deployment
forge script script/Deploy.s.sol:Deploy \
  --rpc-url "$ANVIL_URL" \
  --private-key "$PRIVATE_KEY" \
  --broadcast \
  --legacy \
  -vvv

# Parse deployment addresses from broadcast file
BROADCAST_FILE="broadcast/Deploy.s.sol/31337/run-latest.json"

if [ ! -f "$BROADCAST_FILE" ]; then
  echo "❌ Deployment broadcast file not found"
  exit 1
fi

# Extract contract addresses using jq
DEPLOYER=$(jq -r '.transactions[0].transaction.from' "$BROADCAST_FILE")
MOCK_USDC=$(jq -r '[.transactions[] | select(.transactionType == "CREATE" and .contractName == "MockUSDC")] | first | .contractAddress' "$BROADCAST_FILE")
FAUCET=$(jq -r '[.transactions[] | select(.transactionType == "CREATE" and .contractName == "ARDAFaucet")] | first | .contractAddress' "$BROADCAST_FILE")
VERIFIER=$(jq -r '[.transactions[] | select(.transactionType == "CREATE" and (.contractName == "SP1MockVerifier" or .contractName == "SP1Verifier"))] | first | .contractAddress' "$BROADCAST_FILE")
ARDA=$(jq -r '[.transactions[] | select(.transactionType == "CREATE" and .contractName == "ARDA")] | first | .contractAddress' "$BROADCAST_FILE")

echo "📋 Deployed Contracts:"
echo "  Deployer: $DEPLOYER"
echo "  MockUSDC: $MOCK_USDC"
echo "  Faucet:   $FAUCET"
echo "  Verifier: $VERIFIER"
echo "  ARDA:     $ARDA"

# Create deployment JSON
mkdir -p /deployments
cat > "$DEPLOYMENT_FILE" <<EOF
{
  "deployer": "$DEPLOYER",
  "usdc": "$MOCK_USDC",
  "faucet": "$FAUCET",
  "verifier": "$VERIFIER",
  "arda": "$ARDA",
  "chainId": 31337,
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "environment": "test"
}
EOF

echo "✅ Deployment file created: $DEPLOYMENT_FILE"
cat "$DEPLOYMENT_FILE"
echo "🎉 Contract deployment complete!"
DEPLOY_SCRIPT

chmod +x "$TEST_DIR/deploy-contracts.sh"
echo ""

# Step 5: Run the deployer
echo "🚀 Step 5: Running contract deployer..."
docker run --rm \
    --add-host=host.docker.internal:host-gateway \
    -e ETHEREUM_RPC_URL="http://host.docker.internal:$ANVIL_PORT" \
    -e ETHEREUM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    -v "$TEST_DIR:/deployments" \
    -v "$TEST_DIR/deploy-contracts.sh:/scripts/deploy-contracts.sh:ro" \
    "$IMAGE_NAME" \
    /scripts/deploy-contracts.sh

echo ""

# Step 6: Verify deployment
echo "✅ Step 6: Verifying deployment..."
if [ -f "$TEST_DIR/test-deployment.json" ]; then
    echo "📄 Deployment file created successfully:"
    echo ""
    cat "$TEST_DIR/test-deployment.json" | jq .
    echo ""
    
    # Extract and verify contract addresses
    ARDA_ADDR=$(jq -r '.arda' "$TEST_DIR/test-deployment.json")
    FAUCET_ADDR=$(jq -r '.faucet' "$TEST_DIR/test-deployment.json")
    USDC_ADDR=$(jq -r '.usdc' "$TEST_DIR/test-deployment.json")
    
    echo "🔍 Verifying contracts on-chain..."
    echo "  ARDA at $ARDA_ADDR"
    if cast code "$ARDA_ADDR" --rpc-url "http://127.0.0.1:$ANVIL_PORT" | grep -q "^0x"; then
        echo "    ✅ Contract verified!"
    else
        echo "    ❌ Contract not found!"
        exit 1
    fi
    
    echo "  Faucet at $FAUCET_ADDR"
    if cast code "$FAUCET_ADDR" --rpc-url "http://127.0.0.1:$ANVIL_PORT" | grep -q "^0x"; then
        echo "    ✅ Contract verified!"
    else
        echo "    ❌ Contract not found!"
        exit 1
    fi
    
    echo "  MockUSDC at $USDC_ADDR"
    if cast code "$USDC_ADDR" --rpc-url "http://127.0.0.1:$ANVIL_PORT" | grep -q "^0x"; then
        echo "    ✅ Contract verified!"
    else
        echo "    ❌ Contract not found!"
        exit 1
    fi
    
    echo ""
    echo "🎉 All tests passed! Contract deployer is working correctly."
    echo ""
    echo "📦 Deployment artifacts saved to: $TEST_DIR"
    echo "💡 Run the following to see the results:"
    echo "    cat $TEST_DIR/test-deployment.json | jq ."
else
    echo "❌ Deployment file not created"
    exit 1
fi

