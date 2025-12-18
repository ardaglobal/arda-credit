#!/bin/bash
# Validate that populate_deal_data creates all required transaction types

set -e

echo "=== Validating zkVM Transaction Coverage ==="

# Required transaction types (Priority 1)
REQUIRED_TYPES=(
    "user_registration"
    "deal_origination"
    "deal_payment"
    "deal_closeout"
    "collateral_creation"
    "collateral_deal_association"
)

# Check each required type
MISSING=0
for TYPE in "${REQUIRED_TYPES[@]}"; do
    COUNT=$(psql $DATABASE_URL -t -c "
        SELECT COUNT(*) FROM state_transactions
        WHERE transaction_type = '$TYPE';
    " | tr -d ' ')

    if [ "$COUNT" -gt "0" ]; then
        echo "✅ $TYPE: $COUNT transactions"
    else
        echo "❌ $TYPE: MISSING"
        MISSING=$((MISSING + 1))
    fi
done

echo ""
echo "=== Summary ==="
echo "Required types found: $((6 - MISSING))/6"

if [ $MISSING -eq 0 ]; then
    echo "✅ PASS: All required transaction types present"
    echo ""
    echo "Coverage: 60% (6/10 types) - Minimum for testing"
    exit 0
else
    echo "❌ FAIL: Missing $MISSING required transaction types"
    exit 1
fi
