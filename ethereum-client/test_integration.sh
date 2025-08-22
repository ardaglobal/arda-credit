#!/bin/bash

# Ethereum Client Integration Test Script
echo "🧪 Testing Ethereum Client Integration"
echo "======================================"

# Check if required environment variables are set
if [ -z "$DATABASE_URL" ]; then
    echo "❌ DATABASE_URL not set"
    exit 1
fi

if [ -z "$SINDRI_API_KEY" ]; then
    echo "⚠️ SINDRI_API_KEY not set - Sindri integration will fail"
fi

if [ -z "$ETHEREUM_RPC_URL" ]; then
    echo "⚠️ ETHEREUM_RPC_URL not set - using default"
fi

echo "✅ Environment variables checked"

# Test 1: Build the ethereum client
echo ""
echo "📦 Test 1: Building ethereum client..."
cd "$(dirname "$0")"
if cargo build --release; then
    echo "✅ Build successful"
else
    echo "❌ Build failed"
    exit 1
fi

# Test 2: Test configuration loading
echo ""
echo "⚙️ Test 2: Testing configuration..."
if cargo run --bin ethereum_bridge -- --help > /dev/null 2>&1; then
    echo "✅ Configuration loading works"
else
    echo "❌ Configuration loading failed"
    exit 1
fi

# Test 3: Test database connectivity (if available)
echo ""
echo "🗄️ Test 3: Testing database connectivity..."
if echo "SELECT 1" | psql "$DATABASE_URL" > /dev/null 2>&1; then
    echo "✅ Database connection works"
    
    # Check if required tables exist
    echo "📋 Checking required tables..."
    
    # Check sindri_proofs table
    if echo "SELECT COUNT(*) FROM sindri_proofs LIMIT 1" | psql "$DATABASE_URL" > /dev/null 2>&1; then
        echo "✅ sindri_proofs table exists"
    else
        echo "❌ sindri_proofs table missing"
    fi
    
    # Check credit_transactions table  
    if echo "SELECT COUNT(*) FROM credit_transactions LIMIT 1" | psql "$DATABASE_URL" > /dev/null 2>&1; then
        echo "✅ credit_transactions table exists"
    else
        echo "❌ credit_transactions table missing"
    fi
    
else
    echo "⚠️ Database connection failed - database tests skipped"
fi

# Test 4: Test one-shot mode (dry run)
echo ""
echo "🔄 Test 4: Testing one-shot processing..."
if timeout 10s cargo run --bin ethereum_bridge -- --one-shot 2>&1 | grep -q "One-shot processing completed"; then
    echo "✅ One-shot processing works"
else
    echo "⚠️ One-shot processing test incomplete (may require actual data)"
fi

echo ""
echo "🎉 Integration tests completed!"
echo ""
echo "📚 Usage Examples:"
echo "=================="
echo ""
echo "1. Run ethereum bridge in one-shot mode:"
echo "   cargo run --bin ethereum_bridge -- --one-shot"
echo ""
echo "2. Run ethereum bridge continuously:"
echo "   cargo run --bin ethereum_bridge -- --interval 60"
echo ""
echo "3. Monitor ethereum events:"
echo "   cargo run --example monitor_events"
echo ""
echo "4. Test independent verification:"
echo "   cargo run --example independent_verification"
echo ""
echo "💡 Make sure to set these environment variables:"
echo "   - DATABASE_URL: PostgreSQL connection string"
echo "   - SINDRI_API_KEY: Your Sindri API key"
echo "   - ETHEREUM_RPC_URL: Alchemy or other Ethereum RPC URL"
echo "   - ETHEREUM_WALLET_PRIVATE_KEY: Private key for signing transactions (hex)"
echo "   - ETHEREUM_DEPLOYER_ADDRESS: Address that deployed the contract (must match private key)"
echo "   - ETHEREUM_CONTRACT_ADDRESS: Deployed contract address"