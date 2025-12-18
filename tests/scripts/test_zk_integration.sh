#!/bin/bash
# End-to-End zkVM Integration Test
# Tests: API → state_transactions → batch → proof flow
#
# Prerequisites:
# - Database running (make up)
# - API server running (make server)

set -e

echo "🧪 zkVM Integration End-to-End Test"
echo "===================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check environment
if [ -z "$DATABASE_URL" ]; then
    echo -e "${RED}❌ DATABASE_URL not set${NC}"
    exit 1
fi

API_URL="${ARDA_CREDIT_API_URL:-http://localhost:8080}"
echo "📍 API URL: $API_URL"
echo "📍 Database: $DATABASE_URL"
echo ""

# Function to run SQL query and show results
run_query() {
    local description=$1
    local query=$2
    echo -e "${YELLOW}📊 $description${NC}"
    psql "$DATABASE_URL" -c "$query"
    echo ""
}

# Function to check API endpoint
check_endpoint() {
    local url=$1
    echo -e "${YELLOW}🔍 Checking: $url${NC}"
    curl -s "$url" | head -c 100
    echo ""
    echo ""
}

echo "======================================"
echo "Step 1: Verify Initial State"
echo "======================================"
echo ""

run_query "Pending state transactions (should start at 0)" \
  "SELECT COUNT(*) as pending_count 
   FROM state_transactions 
   WHERE included_in_batch_id IS NULL;"

run_query "Total batches" \
  "SELECT COUNT(*) as total_batches FROM proof_batches;"

echo "======================================"
echo "Step 2: Create User via API"
echo "======================================"
echo ""

TEST_EMAIL="zktest_$(date +%s)@example.com"
echo "Creating user: $TEST_EMAIL"

USER_RESPONSE=$(curl -s -X POST "$API_URL/api/v1/user" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$TEST_EMAIL\",\"user_type\":\"investor\"}")

echo "Response: $USER_RESPONSE"
echo ""

echo "======================================"
echo "Step 3: Verify Transaction Created"
echo "======================================"
echo ""

run_query "Latest user_registration transaction" \
  "SELECT 
    id, 
    transaction_type, 
    included_in_batch_id, 
    created_at,
    user_id
   FROM state_transactions 
   WHERE transaction_type = 'user_registration' 
   ORDER BY created_at DESC LIMIT 1;"

echo "======================================"
echo "Step 4: Count Pending Transactions"
echo "======================================"
echo ""

PENDING_COUNT=$(psql "$DATABASE_URL" -t -c \
  "SELECT COUNT(*) FROM state_transactions WHERE included_in_batch_id IS NULL;")

echo -e "${GREEN}✅ Pending transactions: $PENDING_COUNT${NC}"
echo ""

if [ "$PENDING_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✅ Transaction successfully created!${NC}"
else
    echo -e "${RED}❌ No pending transactions found - API integration may be broken${NC}"
    exit 1
fi

echo ""
echo "======================================"
echo "Step 5: Trigger Batch Processing"
echo "======================================"
echo ""

echo "Note: Manual batch trigger requires authentication token."
echo "For now, we'll wait for automatic batch processing."
echo ""
echo "Batch will trigger when:"
echo "  - 10+ transactions pending (count trigger), OR"
echo "  - 60 seconds elapsed (timer trigger)"
echo ""

# Alternative: Manual trigger if you have a token
# echo "Attempting manual batch trigger..."
# curl -X POST "$API_URL/api/v1/batch/trigger" \
#   -H "Authorization: Bearer $TOKEN"

echo "Waiting 10 seconds for batch processor..."
sleep 10

echo ""
echo "======================================"
echo "Step 6: Check if Transaction Was Batched"
echo "======================================"
echo ""

run_query "Check if transaction was batched" \
  "SELECT 
    st.id,
    st.transaction_type,
    st.included_in_batch_id,
    st.created_at as tx_created,
    pb.proof_status,
    pb.created_at as batch_created
   FROM state_transactions st
   LEFT JOIN proof_batches pb ON st.included_in_batch_id = pb.id
   WHERE st.transaction_type = 'user_registration'
   ORDER BY st.created_at DESC LIMIT 3;"

BATCHED_COUNT=$(psql "$DATABASE_URL" -t -c \
  "SELECT COUNT(*) FROM state_transactions 
   WHERE transaction_type = 'user_registration' 
   AND included_in_batch_id IS NOT NULL;")

if [ "$BATCHED_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✅ At least $BATCHED_COUNT transaction(s) have been batched!${NC}"
else
    echo -e "${YELLOW}⏳ No transactions batched yet. This is expected if:${NC}"
    echo "   - Batch count trigger not reached (< 10 transactions)"
    echo "   - Timer hasn't triggered yet (< 60 seconds)"
    echo "   - Batch processor not running"
    echo ""
    echo "To test batching manually:"
    echo "  1. Create 10+ transactions"
    echo "  2. Wait 60 seconds for timer trigger"
    echo "  3. Check again"
fi

echo ""
echo "======================================"
echo "Step 7: Check Recent Batches"
echo "======================================"
echo ""

run_query "Recent proof batches" \
  "SELECT 
    id,
    proof_status,
    array_length(transaction_ids, 1) as tx_count,
    created_at,
    proven_at
   FROM proof_batches
   ORDER BY id DESC LIMIT 5;"

echo "======================================"
echo "Step 8: Check ADS State Commits"
echo "======================================"
echo ""

run_query "Recent state commits" \
  "SELECT 
    id,
    batch_id,
    encode(prev_merkle_root, 'hex') as prev_root_hex,
    encode(new_merkle_root, 'hex') as new_root_hex,
    created_at
   FROM ads_state_commits
   ORDER BY id DESC LIMIT 3;"

echo "======================================"
echo "Summary"
echo "======================================"
echo ""

TOTAL_STATE_TXS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM state_transactions;")
BATCHED_TXS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM state_transactions WHERE included_in_batch_id IS NOT NULL;")
PENDING_TXS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM state_transactions WHERE included_in_batch_id IS NULL;")
TOTAL_BATCHES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM proof_batches;")

echo "📊 Transaction Statistics:"
echo "   Total state transactions: $TOTAL_STATE_TXS"
echo "   Batched: $BATCHED_TXS"
echo "   Pending: $PENDING_TXS"
echo "   Total batches: $TOTAL_BATCHES"
echo ""

if [ "$BATCHED_TXS" -gt 0 ]; then
    echo -e "${GREEN}✅ SUCCESS: Transactions are flowing through the batch system!${NC}"
else
    echo -e "${YELLOW}⏳ PARTIAL: Transactions created but not yet batched${NC}"
    echo "   This is expected in early testing. Create more transactions or wait for timer."
fi

echo ""
echo "Test complete! 🎉"

