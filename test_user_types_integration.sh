#!/bin/bash

# Arda Credit User Type System Integration Test Script
# Tests the complete flow: invite codes → registration → magic link auth

set -e

echo "🧪 Arda Credit User Type Integration Test Script"
echo "================================================"
echo

# Configuration
BASE_URL="http://localhost:8080"
TEST_EMAIL="matt@arda.xyz"
DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:password@localhost:5432/arda_credit"}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
log_test_start() {
    echo -e "${BLUE}🔄 $1${NC}"
    ((TESTS_RUN++))
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
    ((TESTS_PASSED++))
}

log_failure() {
    echo -e "${RED}❌ $1${NC}"
    ((TESTS_FAILED++))
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# Check if services are running
check_services() {
    log_test_start "Checking if required services are running"

    # Check database
    if pg_isready -h localhost -p 5432 -U postgres >/dev/null 2>&1; then
        log_success "PostgreSQL database is running"
    else
        log_failure "PostgreSQL database is not running on localhost:5432"
        exit 1
    fi

    # Check API server
    if curl -s --max-time 5 "${BASE_URL}/api/v1/health" >/dev/null; then
        log_success "API server is running at ${BASE_URL}"
    else
        log_failure "API server is not running at ${BASE_URL}"
        exit 1
    fi
}

# Test database setup and invite codes
test_database_setup() {
    log_test_start "Testing database setup and invite codes"

    echo "   Checking invite codes in database..."

    # Test each invite code
    local codes=("INVESTOR2025" "ORIGINATOR2025" "ADMIN2025")
    for code in "${codes[@]}"; do
        echo "   Testing invite code: $code"

        # Use psql to check invite code exists and is valid
        local result=$(psql "${DATABASE_URL}" -t -c "
            SELECT
                CASE
                    WHEN is_active AND (expires_at IS NULL OR expires_at > NOW()) AND current_uses < max_uses
                    THEN 'VALID'
                    ELSE 'INVALID'
                END as status,
                max_uses - current_uses as available_uses
            FROM invite_codes
            WHERE code = '$code';
        " 2>/dev/null | tr -d ' ')

        if [[ $result == *"VALID"* ]]; then
            local uses=$(echo "$result" | grep -o '[0-9]*')
            log_success "Invite code $code is valid ($uses uses available)"
        else
            log_failure "Invite code $code is invalid or not found"
            return 1
        fi
    done
}

# Test user registration with each invite code
test_user_registration() {
    log_test_start "Testing user registration with all invite codes"

    local test_cases=(
        "investor-test-${RANDOM}@arda.xyz:INVESTOR2025:investor"
        "originator-test-${RANDOM}@arda.xyz:ORIGINATOR2025:originator"
        "admin-test-${RANDOM}@arda.xyz:ADMIN2025:admin"
    )

    for test_case in "${test_cases[@]}"; do
        IFS=':' read -r email code expected_type <<< "$test_case"

        echo "   Registering user: $email with code: $code (expected type: $expected_type)"

        # Make registration request
        local response=$(curl -s -w "\n%{http_code}" -X POST "${BASE_URL}/api/v1/user" \
            -H "Content-Type: application/json" \
            -d "{
                \"email\": \"$email\",
                \"password\": \"SecureTestPass123!\",
                \"invite_code\": \"$code\"
            }")

        local http_code=$(echo "$response" | tail -n1)
        local body=$(echo "$response" | head -n -1)

        if [ "$http_code" -eq 200 ]; then
            # Check if user type matches expected
            local actual_type=$(echo "$body" | jq -r '.user_type' 2>/dev/null || echo "unknown")
            if [ "$actual_type" = "$expected_type" ]; then
                log_success "User $email registered successfully as $actual_type"
            else
                log_failure "User $email registered but type mismatch: expected $expected_type, got $actual_type"
            fi
        else
            log_failure "Registration failed for $email (HTTP $http_code): $body"
        fi
    done
}

# Test magic link request
test_magic_link_request() {
    log_test_start "Testing magic link request"

    echo "   Requesting magic link for: $TEST_EMAIL"

    local response=$(curl -s -w "\n%{http_code}" -X POST "${BASE_URL}/api/v1/auth/magic-link" \
        -H "Content-Type: application/json" \
        -d "{
            \"email\": \"$TEST_EMAIL\",
            \"redirect_uri\": \"http://localhost:3000/dashboard\"
        }")

    local http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" -eq 200 ]; then
        log_success "Magic link request successful for $TEST_EMAIL"
        log_info "Check your email for the magic link to complete the authentication test"
        echo "   Response: $body"
    else
        log_failure "Magic link request failed (HTTP $http_code): $body"
    fi
}

# Test error scenarios
test_error_scenarios() {
    log_test_start "Testing error scenarios"

    # Test 1: Registration without invite code
    echo "   Testing registration without invite code..."
    local response=$(curl -s -w "\n%{http_code}" -X POST "${BASE_URL}/api/v1/user" \
        -H "Content-Type: application/json" \
        -d "{
            \"email\": \"error-test-${RANDOM}@arda.xyz\",
            \"password\": \"SecureTestPass123!\"
        }")

    local http_code=$(echo "$response" | tail -n1)
    if [ "$http_code" -eq 400 ]; then
        log_success "Registration without invite code properly rejected (HTTP 400)"
    else
        log_failure "Registration without invite code should fail with 400, got $http_code"
    fi

    # Test 2: Registration with invalid invite code
    echo "   Testing registration with invalid invite code..."
    local response=$(curl -s -w "\n%{http_code}" -X POST "${BASE_URL}/api/v1/user" \
        -H "Content-Type: application/json" \
        -d "{
            \"email\": \"error-test-${RANDOM}@arda.xyz\",
            \"password\": \"SecureTestPass123!\",
            \"invite_code\": \"INVALID2025\"
        }")

    local http_code=$(echo "$response" | tail -n1)
    if [ "$http_code" -eq 400 ]; then
        log_success "Registration with invalid invite code properly rejected (HTTP 400)"
    else
        log_failure "Registration with invalid invite code should fail with 400, got $http_code"
    fi
}

# Test CLI commands
test_cli_commands() {
    log_test_start "Testing CLI commands"

    # Test invite code generation
    echo "   Testing invite code generation..."
    local user_types=("investor" "originator" "admin")

    for user_type in "${user_types[@]}"; do
        echo "   Generating invite code for user type: $user_type"
        if make cli ARGS="generate-invite-code --user-type $user_type --max-uses 5" >/dev/null 2>&1; then
            log_success "Generated invite code for $user_type"
        else
            log_failure "Failed to generate invite code for $user_type"
        fi
    done

    # Test invalid user type
    echo "   Testing invalid user type rejection..."
    if ! make cli ARGS="generate-invite-code --user-type regular --max-uses 5" >/dev/null 2>&1; then
        log_success "Invalid user type 'regular' properly rejected"
    else
        log_failure "Invalid user type 'regular' was accepted (should have failed)"
    fi
}

# Verify database state
verify_database_state() {
    log_test_start "Verifying database state"

    echo "   Checking user registrations in database..."

    # Query for test users created during this run
    local user_count=$(psql "${DATABASE_URL}" -t -c "
        SELECT COUNT(*) FROM identities
        WHERE email LIKE '%-test-%@arda.xyz'
        AND created_at > NOW() - INTERVAL '1 hour';
    " 2>/dev/null | tr -d ' ')

    if [ "$user_count" -gt 0 ]; then
        log_success "Found $user_count test users created recently"
    else
        log_warning "No test users found - this might be expected if tests failed earlier"
    fi

    # Check invite code usage
    echo "   Checking invite code usage..."
    local codes=("INVESTOR2025" "ORIGINATOR2025" "ADMIN2025")

    for code in "${codes[@]}"; do
        local usage=$(psql "${DATABASE_URL}" -t -c "
            SELECT current_uses || '/' || max_uses || ' used'
            FROM invite_codes
            WHERE code = '$code';
        " 2>/dev/null | tr -d ' ')
        echo "   $code: $usage"
    done

    log_success "Database state verification completed"
}

# Manual testing instructions
print_manual_test_instructions() {
    echo
    echo "📋 MANUAL TESTING INSTRUCTIONS"
    echo "=============================="
    echo
    log_info "To complete the magic link authentication test:"
    echo "1. Check your email ($TEST_EMAIL) for the magic link"
    echo "2. Copy the token from the magic link URL"
    echo "3. Run this command to verify the magic link:"
    echo
    echo "   curl -X POST '${BASE_URL}/api/v1/auth/verify' \\"
    echo "     -H 'Content-Type: application/json' \\"
    echo "     -d '{\"token\":\"YOUR_TOKEN_HERE\",\"state\":\"YOUR_STATE_HERE\"}' \\"
    echo "     -c cookies.txt"
    echo
    echo "4. Test the authenticated endpoint:"
    echo
    echo "   curl -X GET '${BASE_URL}/api/v1/auth/me' \\"
    echo "     -b cookies.txt"
    echo
    echo "5. Verify the response contains correct user information"
}

# Main test execution
main() {
    echo "Starting integration tests..."
    echo

    # Run all tests
    check_services
    test_database_setup
    test_user_registration
    test_magic_link_request
    test_error_scenarios
    test_cli_commands
    verify_database_state

    # Print results
    echo
    echo "📊 TEST RESULTS"
    echo "==============="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}🎉 All tests passed!${NC}"
    else
        echo -e "${RED}💥 $TESTS_FAILED test(s) failed${NC}"
        exit 1
    fi

    # Print manual testing instructions
    print_manual_test_instructions
}

# Run main function
main "$@"
