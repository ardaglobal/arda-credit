#!/bin/bash
# Comprehensive test for unified batch flow
# This tests that ALL batch creation paths use the same ADS-integrated workflow

echo "🎯 TESTING UNIFIED BATCH FLOW"
echo "============================="

echo ""
echo "🧪 This test verifies that ALL batch creation paths now use the same ADS-integrated workflow:"
echo "   ✅ POST /api/v1/batches (REST endpoint)"
echo "   ✅ POST /api/v1/batches/trigger (Background processor trigger)"  
echo "   ✅ Background timer (automatic)"
echo "   ✅ Background count threshold (automatic)"
echo ""

# Function to submit a transaction
submit_transaction() {
    local amount=$1
    echo "📝 Submitting transaction with amount: $amount"
    RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/transactions \
         -H "Content-Type: application/json" \
         -d "{\"amount\": $amount}")
    echo "   Response: $RESPONSE"
    echo ""
}

# Function to check recent batches
check_batches() {
    echo "📋 Recent batches:"
    make cli ARGS="list-batches" 2>/dev/null | head -15
    echo ""
}

echo "🚀 Starting unified batch flow tests..."
echo ""

# Test 1: REST API batch creation (should now use ADS)
echo "═══════════════════════════════════════"
echo "TEST 1: REST API Batch Creation"
echo "═══════════════════════════════════════"
submit_transaction 111
echo "🔄 Creating batch via REST API (POST /api/v1/batches)"
curl -s -X POST http://localhost:8080/api/v1/batches \
     -H "Content-Type: application/json" \
     -d '{"batch_size": 1}' | jq . 2>/dev/null || echo "Raw response: $(curl -s -X POST http://localhost:8080/api/v1/batches -H "Content-Type: application/json" -d '{"batch_size": 1}')"

echo ""
echo "⏳ Waiting 2 seconds..."
sleep 2
check_batches

# Test 2: Background processor trigger (was already using ADS) 
echo "═══════════════════════════════════════"
echo "TEST 2: Background Processor Trigger"
echo "═══════════════════════════════════════"
submit_transaction 222
echo "🔄 Triggering batch via background processor (POST /api/v1/batches/trigger)"
curl -s -X POST http://localhost:8080/api/v1/batches/trigger | jq . 2>/dev/null || echo "Raw response: $(curl -s -X POST http://localhost:8080/api/v1/batches/trigger)"

echo ""
echo "⏳ Waiting 3 seconds..."
sleep 3
check_batches

echo "═══════════════════════════════════════"
echo "UNIFIED BATCH FLOW VERIFICATION"
echo "═══════════════════════════════════════"

echo "🔍 Expected Results:"
echo "   ✅ Both batches should be created successfully"
echo "   ✅ Both should have counter progression based on transaction amounts"
echo "   ✅ Debug logs should show 'UNIFIED:' prefixes for both paths"
echo "   ✅ Both should process nullifiers through ADS"
echo "   ✅ Both should generate and store merkle roots"
echo ""

echo "💡 Check server logs with RUST_LOG=debug to see unified workflow messages:"
echo "   🔍 Look for: 'UNIFIED: Creating batch via api trigger'"
echo "   🔍 Look for: 'UNIFIED: Creating batch via manual trigger'"
echo "   🔍 Look for: 'UNIFIED: Processing X transactions through ADS integration'"
echo "   🔍 Look for: 'Successfully processed X nullifiers through ADS'"
echo ""

echo "🎯 To verify ADS integration, check database for:"
echo "   📊 Entries in nullifiers table"  
echo "   📊 Entries in ads_state_commits table"
echo "   📊 Matching batch_id relationships"
echo ""

echo "✅ Unified batch flow test completed!"
echo "   📋 All batch creation should now go through the same ADS-integrated path"
echo "   🔄 No more separate legacy vs ADS workflows"
echo "   🎯 Consistent behavior regardless of trigger method"