#!/bin/bash
# Basic deployment validation tests for Arda Credit in Kind cluster
# Tests core functionality and service health after deployment

set -euo pipefail

# Configuration
CLUSTER_NAME="arda-credit-e2e"
KUBECTL_CONTEXT="kind-${CLUSTER_NAME}"
NAMESPACE="arda-credit"
API_BASE_URL="http://localhost:8080"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test tracking
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test result tracking
test_passed() {
    ((TESTS_RUN++))
    ((TESTS_PASSED++))
    log_success "✓ $1"
}

test_failed() {
    ((TESTS_RUN++))
    ((TESTS_FAILED++))
    log_error "✗ $1"
}

test_skipped() {
    ((TESTS_RUN++))
    log_warning "⚠ $1 (SKIPPED)"
}

test_warning() {
    ((TESTS_RUN++))
    log_warning "⚠ $1"
}

# Check if cluster is running
check_cluster_running() {
    log_info "Checking if Kind cluster is running..."
    
    if ! kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
        log_error "Kind cluster '${CLUSTER_NAME}' not found"
        log_info "Run 'make kind-setup' to create the cluster first"
        exit 1
    fi
    
    if ! kubectl cluster-info --context="${KUBECTL_CONTEXT}" >/dev/null 2>&1; then
        log_error "Cannot connect to cluster '${CLUSTER_NAME}'"
        exit 1
    fi
    
    test_passed "Kind cluster is running and accessible"
}

# Test 1: Kubernetes cluster health
test_cluster_health() {
    log_info "Testing cluster health..."
    
    # Check nodes are ready
    local ready_nodes
    ready_nodes=$(kubectl get nodes --context="${KUBECTL_CONTEXT}" --no-headers | grep -c "Ready" || echo "0")
    
    if [ "$ready_nodes" -ge 2 ]; then
        test_passed "All cluster nodes are Ready ($ready_nodes nodes)"
    else
        test_failed "Not all nodes are Ready (found $ready_nodes)"
    fi
    
    # Check system pods
    local system_pods
    system_pods=$(kubectl get pods -n kube-system --context="${KUBECTL_CONTEXT}" --field-selector=status.phase=Running --no-headers | wc -l)
    
    if [ "$system_pods" -ge 5 ]; then
        test_passed "System pods are running ($system_pods pods)"
    else
        test_failed "Insufficient system pods running ($system_pods)"
    fi
}

# Test 2: PostgreSQL deployment
test_postgresql() {
    log_info "Testing PostgreSQL deployment..."
    
    # Check PostgreSQL pod is running
    if kubectl get pods --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=postgresql --field-selector=status.phase=Running --no-headers | grep -q postgresql; then
        test_passed "PostgreSQL pod is running"
    else
        test_failed "PostgreSQL pod is not running"
        kubectl get pods --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=postgresql || true
        return
    fi
    
    # Check PostgreSQL service exists
    if kubectl get service --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" arda-credit-postgresql >/dev/null 2>&1; then
        test_passed "PostgreSQL service exists"
    else
        test_failed "PostgreSQL service not found"
    fi
    
    # Test database connection (basic connectivity)
    log_info "Testing PostgreSQL connectivity..."
    if timeout 10 bash -c "</dev/tcp/localhost/5432"; then
        test_passed "PostgreSQL is accepting connections on port 5432"
    else
        test_failed "Cannot connect to PostgreSQL on localhost:5432"
    fi
}

# Test 3: Arda Credit Server deployment
test_server() {
    log_info "Testing Arda Credit Server deployment..."
    
    # Check server pod is running
    if kubectl get pods --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --field-selector=status.phase=Running --no-headers | grep -q server; then
        test_passed "Server pod is running"
    else
        test_failed "Server pod is not running"
        kubectl get pods --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server || true
        return
    fi
    
    # Check server service
    if kubectl get service --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" arda-credit-server >/dev/null 2>&1; then
        test_passed "Server service exists"
    else
        test_failed "Server service not found"
    fi
    
    # Test server readiness
    local ready_replicas
    ready_replicas=$(kubectl get deployment arda-credit-server --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
    
    if [ "$ready_replicas" -ge 1 ]; then
        test_passed "Server deployment has ready replicas ($ready_replicas)"
    else
        test_failed "Server deployment has no ready replicas"
    fi
}


# Test 4: API health endpoint
test_api_health() {
    log_info "Testing API health endpoint..."
    
    # Wait a moment for services to be fully ready
    sleep 2
    
    # Test health endpoint
    if curl -f -s "${API_BASE_URL}/api/v1/health" >/dev/null; then
        test_passed "API health endpoint responds"
        
        # Get health status details
        local health_response
        health_response=$(curl -s "${API_BASE_URL}/api/v1/health" 2>/dev/null || echo "{}")
        log_info "Health response: $health_response"
        
    else
        test_failed "API health endpoint not responding"
        log_info "Trying alternative health paths..."
        
        # Try alternative paths
        for path in "/health" "/api/health" "/api/v1/health"; do
            if curl -f -s "${API_BASE_URL}${path}" >/dev/null 2>&1; then
                test_passed "API responds on alternative path: $path"
                return
            fi
        done
        
        log_error "No health endpoint found"
    fi
}


# Test 5: Service discovery and networking
test_networking() {
    log_info "Testing internal service networking..."
    
    # Test internal service DNS resolution
    if kubectl exec --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" deployment/arda-credit-server -- nslookup arda-credit-postgresql >/dev/null 2>&1; then
        test_passed "Internal DNS resolution works (server -> postgresql)"
    else
        test_failed "Internal DNS resolution failed"
    fi
    
    # Test NodePort services
    local server_nodeport
    server_nodeport=$(kubectl get service arda-credit-server --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo "")
    
    if [ -n "$server_nodeport" ] && [ "$server_nodeport" != "null" ]; then
        test_passed "Server NodePort configured: $server_nodeport"
    else
        test_failed "Server NodePort not configured"
    fi
}

# Test 6: Resource usage
test_resources() {
    log_info "Testing resource usage..."
    
    # Check pod resource usage
    local server_cpu
    server_cpu=$(kubectl top pod --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --no-headers 2>/dev/null | awk '{print $2}' | head -1 || echo "")
    
    if [ -n "$server_cpu" ]; then
        test_passed "Server pod resource metrics available: CPU $server_cpu"
    else
        test_skipped "Server pod resource metrics not available (metrics-server may not be installed)"
    fi
    
    # Check memory usage
    local memory_usage
    memory_usage=$(kubectl get pods --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit --no-headers | wc -l)
    
    if [ "$memory_usage" -ge 2 ]; then
        test_passed "Expected number of Arda Credit pods running ($memory_usage)"
    else
        test_failed "Unexpected number of Arda Credit pods ($memory_usage)"
    fi
}

# Test 7: Logs validation
test_logs() {
    log_info "Testing application logs..."
    
    # Check server logs for errors
    local server_errors
    server_errors=$(kubectl logs --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --tail=20 2>/dev/null | grep -i "error\|panic\|fatal" | wc -l || echo "0")
    
    if [ "$server_errors" -eq 0 ]; then
        test_passed "No errors found in server logs"
    else
        test_warning "Found $server_errors potential errors in server logs"
    fi
    
    # Check if server is logging startup messages
    if kubectl logs --context="${KUBECTL_CONTEXT}" -n "${NAMESPACE}" -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --tail=10 2>/dev/null | grep -q "Starting\|Listening\|Started\|Ready"; then
        test_passed "Server shows startup messages in logs"
    else
        test_failed "No startup messages found in server logs"
    fi
}

# Display test summary
display_summary() {
    echo
    log_info "=== Test Summary ==="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo
    
    if [ "$TESTS_FAILED" -eq 0 ]; then
        log_success "🎉 All tests passed! Arda Credit deployment is healthy."
        echo
        log_info "🌐 Access URLs:"
        echo "  API: ${API_BASE_URL}"
        echo "  Health: ${API_BASE_URL}/api/v1/health"
        echo
        log_info "🔧 Debug commands:"
        echo "  make kind-status    # Show cluster status"
        echo "  make kind-logs      # Show service logs"
        echo "  kubectl get pods -n arda-credit --context=${KUBECTL_CONTEXT}"
        return 0
    else
        log_error "❌ ${TESTS_FAILED} test(s) failed. Check the output above for details."
        echo
        log_info "🔧 Debugging tips:"
        echo "  make kind-logs      # Check service logs"
        echo "  make kind-status    # Check cluster status"
        echo "  kubectl describe pods -n arda-credit --context=${KUBECTL_CONTEXT}"
        return 1
    fi
}

# Main test execution
main() {
    log_info "Starting Arda Credit deployment validation tests..."
    echo
    
    check_cluster_running
    
    # Run all tests
    test_cluster_health
    test_postgresql
    test_server
    test_api_health
    test_networking
    test_resources
    test_logs
    
    display_summary
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        cat << EOF
Usage: $0 [OPTIONS]

Run deployment validation tests for Arda Credit in Kind cluster.

This script validates that the Arda Credit application has been deployed
correctly in the local Kind cluster and all services are functioning.

Tests include:
  - Cluster health and node status
  - PostgreSQL deployment and connectivity
  - Arda Credit server deployment and API health
  - Internal networking and service discovery
  - Resource usage and logs validation

OPTIONS:
    --help, -h    Show this help message

EXAMPLES:
    $0            # Run all deployment tests
    $0 --help     # Show this help

PREREQUISITES:
    - Kind cluster must be running (make kind-setup)
    - Arda Credit must be deployed via Helm

EOF
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac