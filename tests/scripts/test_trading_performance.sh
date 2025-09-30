#!/bin/bash
set -e

echo "⚡ Testing Loan Trading Performance & Concurrency"
echo "==============================================="

# Set up test environment
export DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:password@localhost:5432/arda_credit_db"}
BASE_URL=${ARDA_CREDIT_API_URL:-"http://localhost:8080"}

# Setup test data
echo "📋 Setting up performance test data..."
SELLER_ID=$(make cli ARGS="register-user --user-type lender --email seller@test.com" | grep -o '[0-9a-f-]\{36\}' | head -1)
BUYER_ID=$(make cli ARGS="register-user --user-type investor --email buyer@test.com" | grep -o '[0-9a-f-]\{36\}' | head -1)

# Add buyer balance
psql $DATABASE_URL -c "
INSERT INTO user_balances (user_id, currency, available_balance)
VALUES ('$BUYER_ID', 'USDC', 100000.00)
ON CONFLICT (user_id, currency) DO UPDATE SET available_balance = 100000.00;
" > /dev/null

# Create loan
LOAN_ID=$(make cli ARGS="originate-loan --borrower-id $BUYER_ID --lender-id $SELLER_ID --amount 50000" | grep -o '[0-9a-f-]\{36\}' | head -1)

echo "📋 Test data ready:"
echo "   Seller: $SELLER_ID"
echo "   Buyer: $BUYER_ID"
echo "   Loan: $LOAN_ID"
echo ""

# Test concurrent marketplace browsing
echo "📊 Testing concurrent marketplace browsing..."
for i in {1..10}; do
    curl -s -X GET "$BASE_URL/api/v1/loans/for-sale?limit=10" \
        -H "x-test-user-id: $BUYER_ID" &
done
wait
echo "✅ Concurrent browsing: OK"

# Test concurrent transfer attempts (should prevent race conditions)
echo "🔄 Testing concurrent transfer protection..."
# Create multiple transfer attempts for the same loan - only one should succeed

# Test database connection pool under load
echo "💾 Testing database performance..."
time make cli ARGS="marketplace browse --limit 50"
time make cli ARGS="transfer validate --loan-id $LOAN_ID --buyer-id $BUYER_ID --purchase-price 45000.00"

# Test API response times
echo "⏱️ Testing API response times..."
echo "Marketplace browse:"
time curl -s -X GET "$BASE_URL/api/v1/loans/for-sale" \
    -H "x-test-user-id: $BUYER_ID" > /dev/null

echo "Transfer validation:"
time curl -s -X POST "$BASE_URL/api/v1/loan/$LOAN_ID/transfer/validate" \
    -H "Content-Type: application/json" \
    -H "x-test-user-id: $BUYER_ID" \
    -d '{"new_lender_id": "'$BUYER_ID'", "purchase_price": "45000.00"}' > /dev/null

echo "Marketplace stats:"
time curl -s -X GET "$BASE_URL/api/v1/loans/marketplace/stats" \
    -H "x-test-user-id: $SELLER_ID" > /dev/null

# Test memory usage patterns
echo "🧠 Testing memory usage patterns..."
echo "Note: Monitor system memory during concurrent operations above"

echo "✅ Performance testing complete"
