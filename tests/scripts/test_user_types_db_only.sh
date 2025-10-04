#!/bin/bash

# Arda Credit User Type System Database-Only Integration Test
# Tests database setup, invite codes, and CLI functionality without requiring API server

set -e

echo "🧪 Arda Credit User Type Database-Only Integration Test Script"
echo "=============================================================="
echo

# Configuration
DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:password@localhost:5432/arda_credit_db"}

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

# Check if database is running
check_database() {
    log_test_start "Checking if database is running"

    if pg_isready -h localhost -p 5432 -U postgres >/dev/null 2>&1; then
        log_success "PostgreSQL database is running"
    else
        log_failure "PostgreSQL database is not running on localhost:5432"
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
            local uses=$(echo "$result" | grep -o '[0-9]*' | head -1)
            log_success "Invite code $code is valid ($uses uses available)"
        else
            log_failure "Invite code $code is invalid or not found"
            return 1
        fi
    done
}

# Test user type constraints
test_user_type_constraints() {
    log_test_start "Testing user type constraints"

    echo "   Testing valid user types..."

    # Test valid user types (should succeed)
    local valid_types=("investor" "originator" "admin")
    for user_type in "${valid_types[@]}"; do
        local result=$(psql "${DATABASE_URL}" -t -c "
            INSERT INTO identities (id, user_type, email, created_at, updated_at, email_verified)
            VALUES (gen_random_uuid(), '$user_type', 'test-$user_type@example.com', NOW(), NOW(), false)
            RETURNING id;
        " 2>/dev/null)

        if [[ -n "$result" ]]; then
            log_success "User type '$user_type' accepted by database"
            # Clean up
            psql "${DATABASE_URL}" -c "DELETE FROM identities WHERE email = 'test-$user_type@example.com';" >/dev/null 2>&1
        else
            log_failure "User type '$user_type' rejected by database"
        fi
    done

    echo "   Testing invalid user types..."

    # Test invalid user types (should fail)
    local invalid_types=("regular" "user" "admin")
    for user_type in "${invalid_types[@]}"; do
        local result=$(psql "${DATABASE_URL}" -t -c "
            INSERT INTO identities (id, user_type, email, created_at, updated_at, email_verified)
            VALUES (gen_random_uuid(), '$user_type', 'test-invalid-$user_type@example.com', NOW(), NOW(), false);
        " 2>/dev/null)

        if [[ -z "$result" ]]; then
            log_success "Invalid user type '$user_type' properly rejected"
        else
            log_failure "Invalid user type '$user_type' was accepted (should have failed)"
        fi
    done
}

# Test invite code generation via CLI
test_cli_invite_code_generation() {
    log_test_start "Testing CLI invite code generation"

    # Test generating invite codes for each user type
    local user_types=("investor" "originator" "admin")

    for user_type in "${user_types[@]}"; do
        echo "   Generating invite code for user type: $user_type"
        if make cli ARGS="generate-invite-code --user-type $user_type --max-uses 5" >/dev/null 2>&1; then
            log_success "Generated invite code for $user_type"
        else
            log_failure "Failed to generate invite code for $user_type"
        fi
    done

    # Test invalid user type (should fail)
    echo "   Testing invalid user type rejection..."
    if ! make cli ARGS="generate-invite-code --user-type regular --max-uses 5" >/dev/null 2>&1; then
        log_success "Invalid user type 'regular' properly rejected"
    else
        log_failure "Invalid user type 'regular' was accepted (should have failed)"
    fi
}

# Test invite code validation via CLI
test_cli_invite_code_validation() {
    log_test_start "Testing CLI invite code validation"

    # Test validating existing 2025 codes
    local codes=("INVESTOR2025" "ORIGINATOR2025" "ADMIN2025")

    for code in "${codes[@]}"; do
        echo "   Validating invite code: $code"
        if make cli ARGS="check-invite-code --code $code" >/dev/null 2>&1; then
            log_success "Invite code $code validated successfully"
        else
            log_failure "Failed to validate invite code $code"
        fi
    done

    # Test invalid code (should fail)
    echo "   Testing invalid invite code rejection..."
    if ! make cli ARGS="check-invite-code --code INVALID2025" >/dev/null 2>&1; then
        log_success "Invalid invite code properly rejected"
    else
        log_failure "Invalid invite code was accepted (should have failed)"
    fi
}

# Test user registration via CLI (this will fail without API server, but let's see the error)
test_cli_user_registration() {
    log_test_start "Testing CLI user registration (API server required)"

    echo "   Note: This test requires the API server to be running"

    # Try to register a user (will likely fail without API server)
    local result=$(make cli ARGS="register-user --email cli-test-$(date +%s)@arda.xyz --invite-code INVESTOR2025" 2>&1)

    if echo "$result" | grep -q "error\|Error\|ERROR"; then
        log_info "CLI user registration failed (expected without API server): $(echo "$result" | head -1)"
    else
        log_warning "CLI user registration unexpectedly succeeded without API server"
    fi
}

# Verify database state after all operations
verify_database_state() {
    log_test_start "Verifying database state"

    echo "   Checking invite code usage..."

    # Show current state of 2025 invite codes
    local result=$(psql "${DATABASE_URL}" -c "
        SELECT code, current_uses, max_uses, is_active,
               CASE WHEN expires_at > NOW() THEN 'ACTIVE' ELSE 'EXPIRED' END as expiry_status
        FROM invite_codes
        WHERE code IN ('INVESTOR2025', 'ORIGINATOR2025', 'ADMIN2025')
        ORDER BY code;
    " 2>/dev/null)

    if [[ -n "$result" ]]; then
        log_success "Retrieved invite code status from database"
        echo "$result"
    else
        log_failure "Failed to retrieve invite code status"
    fi

    # Check identities table constraints
    echo "   Checking identities table constraints..."
    local constraint_count=$(psql "${DATABASE_URL}" -t -c "
        SELECT COUNT(*)
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
        WHERE tc.table_name = 'identities'
        AND tc.constraint_type = 'CHECK'
        AND kcu.column_name = 'user_type';
    " 2>/dev/null | tr -d ' ')

    if [[ "$constraint_count" -gt 0 ]]; then
        log_success "User type constraint exists on identities table"
    else
        log_failure "User type constraint missing from identities table"
    fi
}

# Print test summary
print_summary() {
    echo
    echo "📊 TEST RESULTS"
    echo "==============="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}🎉 All database and CLI tests passed!${NC}"
        echo
        echo "🚀 Next Steps:"
        echo "1. Start the API server: make server"
        echo "2. Run full integration test: ./test_user_types_integration.sh"
        echo "3. Test magic link authentication manually"
    else
        echo -e "${RED}💥 $TESTS_FAILED test(s) failed${NC}"
        exit 1
    fi
}

# Main test execution
main() {
    echo "Starting database-only integration tests..."
    echo

    # Run all tests
    check_database
    test_database_setup
    test_user_type_constraints
    test_cli_invite_code_generation
    test_cli_invite_code_validation
    test_cli_user_registration
    verify_database_state

    # Print summary
    print_summary
}

# Run main function
main "$@"
