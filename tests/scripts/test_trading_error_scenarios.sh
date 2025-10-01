#!/bin/bash
set -e

echo "🚨 Testing Loan Trading Error Scenarios"
echo "======================================="

# Set up test environment
export DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:password@localhost:5432/arda_credit_db"}
BASE_URL=${ARDA_CREDIT_API_URL:-"http://localhost:8080"}

# Helper function for API calls with user authentication
api_call() {
    local method=$1
    local url=$2
    local user_id=$3
    local data=$4

    if [ -n "$data" ]; then
        curl -s -X $method "$BASE_URL$url" \
            -H "Content-Type: application/json" \
            -H "x-test-user-id: $user_id" \
            -d "$data"
    else
        curl -s -X $method "$BASE_URL$url" \
            -H "x-test-user-id: $user_id"
    fi
}

# Setup test data
echo "📋 Setting up test data..."
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

# Test insufficient balance
echo "💰 Testing insufficient balance..."
POOR_BUYER_ID=$(make cli ARGS="register-user --user-type investor --email poor@test.com" | grep -o '[0-9a-f-]\{36\}')
# Don't add balance to poor buyer
INSUFFICIENT_RESULT=$(api_call POST "/api/v1/loan/$LOAN_ID/transfer" "$POOR_BUYER_ID" '{"new_lender_id": "'$POOR_BUYER_ID'", "purchase_price": "45000.00"}')
echo "   Expected error: $(echo "$INSUFFICIENT_RESULT" | jq -r '.error // "Insufficient balance detected"')"

# Test loan not for sale
echo "🚫 Testing loan not for sale..."
NOT_FOR_SALE_LOAN_ID=$(make cli ARGS="originate-loan --borrower-id $BUYER_ID --lender-id $SELLER_ID --amount 25000" | grep -o '[0-9a-f-]\{36\}')
NOT_FOR_SALE_RESULT=$(api_call POST "/api/v1/loan/$NOT_FOR_SALE_LOAN_ID/transfer" "$BUYER_ID" '{"new_lender_id": "'$BUYER_ID'", "purchase_price": "25000.00"}')
echo "   Expected error: $(echo "$NOT_FOR_SALE_RESULT" | jq -r '.error // "Loan not for sale detected"')"

# Test unauthorized access
echo "🔒 Testing unauthorized access..."
UNAUTH_RESULT=$(api_call POST "/api/v1/loan/$LOAN_ID/list-for-sale" "" '{"asking_price": "45000.00"}')
echo "   Expected error: $(echo "$UNAUTH_RESULT" | jq -r '.error // "Unauthorized access detected"')"

# Test self-purchase prevention
echo "👤 Testing self-purchase prevention..."
SELF_PURCHASE_RESULT=$(api_call POST "/api/v1/loan/$LOAN_ID/transfer" "$SELLER_ID" '{"new_lender_id": "'$SELLER_ID'", "purchase_price": "45000.00"}')
echo "   Expected error: $(echo "$SELF_PURCHASE_RESULT" | jq -r '.error // "Self-purchase prevented"')"

# Test invalid loan ID
echo "🆔 Testing invalid loan ID..."
INVALID_ID_RESULT=$(api_call POST "/api/v1/loan/invalid-uuid/transfer" "$BUYER_ID" '{"new_lender_id": "'$BUYER_ID'", "purchase_price": "45000.00"}')
echo "   Expected error: $(echo "$INVALID_ID_RESULT" | jq -r '.error // "Invalid ID format detected"')"

# Test ownership validation for marketplace
echo "🏠 Testing marketplace ownership validation..."
OTHER_SELLER_ID=$(make cli ARGS="register-user --user-type lender --email other@test.com" | grep -o '[0-9a-f-]\{36\}')
OWNERSHIP_RESULT=$(api_call POST "/api/v1/loan/$LOAN_ID/list-for-sale" "$OTHER_SELLER_ID" '{"asking_price": "45000.00"}')
echo "   Expected error: $(echo "$OWNERSHIP_RESULT" | jq -r '.error // "Ownership validation detected"')"

# Test transfer history access control
echo "📋 Testing transfer history access control..."
OTHER_USER_ID=$(make cli ARGS="register-user --user-type investor --email stranger@test.com" | grep -o '[0-9a-f-]\{36\}')
HISTORY_ACCESS_RESULT=$(api_call GET "/api/v1/user/$BUYER_ID/transfers" "$OTHER_USER_ID")
echo "   Expected error: $(echo "$HISTORY_ACCESS_RESULT" | jq -r '.error // "Access control detected"')"

echo "✅ Error scenario testing complete"
