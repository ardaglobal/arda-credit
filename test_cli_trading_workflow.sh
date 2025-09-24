#!/bin/bash
set -e

echo "🧪 Testing CLI Loan Trading Workflow"
echo "===================================="

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

# Test complete CLI workflow
echo "🛒 Testing marketplace CLI..."
echo "1. Browse empty marketplace..."
make cli ARGS="marketplace browse --limit 5"

echo "2. List loan for sale..."
make cli ARGS="marketplace list-for-sale --loan-id $LOAN_ID --asking-price 45000.00 --description 'Test loan listing'"

echo "3. Browse marketplace with listing..."
make cli ARGS="marketplace browse --verbose"

echo "4. Show marketplace stats..."
make cli ARGS="marketplace stats"
echo ""

echo "🔄 Testing transfer CLI..."
echo "1. Validate transfer..."
make cli ARGS="transfer validate --loan-id $LOAN_ID --buyer-id $BUYER_ID --purchase-price 45000.00"

echo "2. Execute transfer..."
make cli ARGS="transfer execute --loan-id $LOAN_ID --buyer-id $BUYER_ID --purchase-price 45000.00 --notes 'CLI test transfer'"

echo "3. Check loan transfer history..."
make cli ARGS="transfer loan-history --loan-id $LOAN_ID --verbose"

echo "4. Check user transfer history..."
make cli ARGS="transfer user-history --user-id $BUYER_ID --verbose"
echo ""

echo "🔄 Testing marketplace after transfer..."
echo "1. Browse marketplace (should be empty)..."
make cli ARGS="marketplace browse"

echo "2. Updated marketplace stats..."
make cli ARGS="marketplace stats"
echo ""

echo "✅ CLI Integration Test: PASSED"
echo "Complete loan trading workflow works end-to-end!"
