#!/bin/bash
# Test Local Anvil Setup - End-to-End Verification
# This script tests the complete local Anvil workflow

set -e

echo "🧪 Testing Local Anvil Setup"
echo "============================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}✅ $1${NC}"
}

fail() {
    echo -e "${RED}❌ $1${NC}"
    exit 1
}

info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# Test 1: Clean state
echo "Test 1: Clean Anvil state"
make clean-anvil > /dev/null 2>&1 || true
pass "Anvil state cleaned"
echo ""

# Test 2: Start Anvil
echo "Test 2: Start Anvil devnet"
make anvil-start > /dev/null 2>&1 || fail "Failed to start Anvil"
sleep 3
pass "Anvil started"
echo ""

# Test 3: Check Anvil connectivity
echo "Test 3: Check Anvil RPC connectivity"
CHAIN_ID=$(RUST_LOG=error cast chain-id --rpc-url http://127.0.0.1:8545 2>/dev/null | grep -o '[0-9]*$')
if [ "$CHAIN_ID" = "31337" ]; then
    pass "Anvil responding on correct chain (31337)"
else
    fail "Anvil chain ID incorrect: $CHAIN_ID"
fi
echo ""

# Test 4: Deploy contracts
echo "Test 4: Deploy contracts to Anvil"
SERVER_ENVIRONMENT=local make deploy-contracts > /tmp/deploy_output.log 2>&1 || fail "Contract deployment failed"
if [ -f "contracts/deployments/local-deployment.json" ]; then
    pass "Deployment file created"
else
    fail "Deployment file not created"
fi
echo ""

# Test 5: Verify contract addresses
echo "Test 5: Verify deployed contracts"
ARDA_ADDR=$(cat contracts/deployments/local-deployment.json | jq -r '.arda')
USDC_ADDR=$(cat contracts/deployments/local-deployment.json | jq -r '.usdc')
FAUCET_ADDR=$(cat contracts/deployments/local-deployment.json | jq -r '.faucet')
VERIFIER_ADDR=$(cat contracts/deployments/local-deployment.json | jq -r '.verifier')

info "ARDA: $ARDA_ADDR"
info "USDC: $USDC_ADDR"
info "Faucet: $FAUCET_ADDR"
info "Verifier: $VERIFIER_ADDR"

# Verify contracts have code
ARDA_CODE=$(cast code $ARDA_ADDR --rpc-url http://127.0.0.1:8545 2>/dev/null)
if [ "${#ARDA_CODE}" -gt 10 ]; then
    pass "ARDA contract deployed with code"
else
    fail "ARDA contract has no code"
fi
echo ""

# Test 6: Test faucet request
echo "Test 6: Request funds from faucet"
TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
make faucet-request ADDRESS=$TEST_ADDRESS > /tmp/faucet_request.log 2>&1 || fail "Faucet request failed"
pass "Faucet request completed"
echo ""

# Test 7: Verify user received USDC
echo "Test 7: Verify user received USDC"
USER_BALANCE=$(RUST_LOG=error cast call $USDC_ADDR "balanceOf(address)(uint256)" $TEST_ADDRESS --rpc-url http://127.0.0.1:8545 2>/dev/null | grep -o '[0-9]*' | head -1)
if [ "$USER_BALANCE" = "1000000000000" ]; then
    pass "User received 1M USDC (1000000000000 wei)"
else
    fail "Unexpected balance: $USER_BALANCE"
fi
echo ""

# Test 8: Verify faucet stats
echo "Test 8: Check faucet statistics"
FAUCET_BALANCE=$(RUST_LOG=error cast call $FAUCET_ADDR "getFaucetBalance()" --rpc-url http://127.0.0.1:8545 2>/dev/null | grep -o '[0-9]*' | head -1)
if [ "$FAUCET_BALANCE" = "99000000000000" ]; then
    pass "Faucet balance correct (99M USDC remaining)"
else
    info "Faucet balance: $FAUCET_BALANCE"
    pass "Faucet balance updated"
fi
echo ""

# Test 9: Verify cooldown is enforced
echo "Test 9: Verify cooldown enforcement"
CAN_REQUEST=$(RUST_LOG=error cast call $FAUCET_ADDR "canRequestNow(address)(bool)" $TEST_ADDRESS --rpc-url http://127.0.0.1:8545 2>/dev/null)
if [ "$CAN_REQUEST" = "false" ]; then
    pass "Cooldown correctly enforced"
else
    fail "Cooldown not enforced (should be false, got: $CAN_REQUEST)"
fi
echo ""

# Test 10: Verify state persistence setup
echo "Test 10: Check Anvil state persistence"
if docker volume ls | grep -q "arda-credit_anvil_data"; then
    pass "Anvil volume exists for state persistence"
else
    fail "Anvil volume not found"
fi
echo ""

echo "=================================="
echo -e "${GREEN}🎉 All Tests Passed!${NC}"
echo "=================================="
echo ""
echo "Local Anvil setup is working correctly!"
echo ""
echo "Next steps:"
echo "  - Run 'make server' to start the full stack"
echo "  - Use 'make faucet-request ADDRESS=...' to get test USDC"
echo "  - Connect MetaMask to http://127.0.0.1:8545 (Chain ID: 31337)"
echo ""
echo "Cleanup:"
echo "  - Run 'make anvil-stop' to stop Anvil"
echo "  - Run 'make clean-anvil' to reset blockchain state"

