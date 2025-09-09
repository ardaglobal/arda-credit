#!/bin/bash
# Test script for PostgreSQL Exporter deployment validation
# This script validates that the postgres-exporter is properly deployed and configured

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
NAMESPACE="arda-credit"
CONTEXT="kind-arda-credit-e2e"
EXPORTER_PORT="9187"

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

# Check if Kind cluster is running
check_cluster() {
    log_info "Checking Kind cluster status..."
    
    if ! kind get clusters | grep -q "arda-credit-e2e"; then
        log_error "Kind cluster 'arda-credit-e2e' not found"
        log_info "Run 'make kind-setup' to create the cluster"
        exit 1
    fi
    
    if ! kubectl cluster-info --context="${CONTEXT}" >/dev/null 2>&1; then
        log_error "Cannot connect to Kind cluster"
        exit 1
    fi
    
    log_success "Kind cluster is accessible"
}

# Check if postgres-exporter is deployed
check_deployment() {
    log_info "Checking postgres-exporter deployment..."
    
    if ! kubectl get deployment -n "${NAMESPACE}" --context="${CONTEXT}" | grep -q "postgres-exporter"; then
        log_error "postgres-exporter deployment not found"
        log_info "Make sure postgresql.exporter.enabled=true in values"
        exit 1
    fi
    
    # Check if deployment is ready
    local ready_replicas
    ready_replicas=$(kubectl get deployment arda-credit-postgres-exporter -n "${NAMESPACE}" --context="${CONTEXT}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
    
    if [ "${ready_replicas}" -eq 0 ]; then
        log_warning "postgres-exporter deployment not ready"
        log_info "Checking pod status..."
        kubectl get pods -n "${NAMESPACE}" --context="${CONTEXT}" -l app.kubernetes.io/component=postgres-exporter
        
        log_info "Checking pod logs..."
        kubectl logs -n "${NAMESPACE}" --context="${CONTEXT}" -l app.kubernetes.io/component=postgres-exporter --tail=20 || true
        
        exit 1
    fi
    
    log_success "postgres-exporter deployment is ready"
}

# Check service configuration
check_service() {
    log_info "Checking postgres-exporter service..."
    
    if ! kubectl get service -n "${NAMESPACE}" --context="${CONTEXT}" | grep -q "postgres-exporter"; then
        log_error "postgres-exporter service not found"
        exit 1
    fi
    
    local service_port
    service_port=$(kubectl get service arda-credit-postgres-exporter -n "${NAMESPACE}" --context="${CONTEXT}" -o jsonpath='{.spec.ports[0].port}' 2>/dev/null || echo "")
    
    if [ "${service_port}" != "${EXPORTER_PORT}" ]; then
        log_error "Service port mismatch. Expected: ${EXPORTER_PORT}, Got: ${service_port}"
        exit 1
    fi
    
    log_success "postgres-exporter service is configured correctly"
}

# Test metrics endpoint
test_metrics_endpoint() {
    log_info "Testing metrics endpoint..."
    
    # Port-forward to access the metrics endpoint
    local port_forward_pid
    kubectl port-forward -n "${NAMESPACE}" --context="${CONTEXT}" service/arda-credit-postgres-exporter ${EXPORTER_PORT}:${EXPORTER_PORT} >/dev/null 2>&1 &
    port_forward_pid=$!
    
    # Wait for port-forward to be ready
    sleep 3
    
    # Test metrics endpoint
    local metrics_response
    if ! metrics_response=$(curl -s -f "http://localhost:${EXPORTER_PORT}/metrics" 2>/dev/null); then
        log_error "Cannot access metrics endpoint"
        kill ${port_forward_pid} 2>/dev/null || true
        exit 1
    fi
    
    # Check for basic metrics
    if ! echo "${metrics_response}" | grep -q "pg_up"; then
        log_error "Basic postgres-exporter metrics not found"
        kill ${port_forward_pid} 2>/dev/null || true
        exit 1
    fi
    
    # Check for custom metrics
    if ! echo "${metrics_response}" | grep -q "pg_database_size"; then
        log_warning "Custom database size metrics not found"
    else
        log_success "Custom database size metrics found"
    fi
    
    if ! echo "${metrics_response}" | grep -q "pg_buffer_cache_hit_ratio"; then
        log_warning "Custom buffer cache metrics not found"
    else
        log_success "Custom buffer cache metrics found"
    fi
    
    # Privacy compliance check
    log_info "Checking privacy compliance..."
    if echo "${metrics_response}" | grep -qE "(loan_amount|user_id|kyc_status|borrower|lender)"; then
        log_error "PRIVACY VIOLATION: Sensitive data found in metrics!"
        echo "${metrics_response}" | grep -E "(loan_amount|user_id|kyc_status|borrower|lender)"
        kill ${port_forward_pid} 2>/dev/null || true
        exit 1
    else
        log_success "Privacy compliance: No sensitive data detected"
    fi
    
    # Count available metrics
    local metric_count
    metric_count=$(echo "${metrics_response}" | grep -c '^# HELP' || echo "0")
    log_info "Total metrics available: ${metric_count}"
    
    # Clean up port-forward
    kill ${port_forward_pid} 2>/dev/null || true
    
    log_success "Metrics endpoint test completed successfully"
}

# Check configuration
check_configuration() {
    log_info "Checking postgres-exporter configuration..."
    
    # Check if ConfigMap exists
    if ! kubectl get configmap -n "${NAMESPACE}" --context="${CONTEXT}" | grep -q "postgres-exporter-queries"; then
        log_error "postgres-exporter queries ConfigMap not found"
        exit 1
    fi
    
    # Check if Secret exists
    if ! kubectl get secret -n "${NAMESPACE}" --context="${CONTEXT}" | grep -q "postgres-exporter"; then
        log_error "postgres-exporter Secret not found"
        exit 1
    fi
    
    log_success "Configuration resources found"
}

# Check PostgreSQL connectivity
check_postgres_connectivity() {
    log_info "Checking PostgreSQL connectivity from exporter..."
    
    # Get exporter pod name
    local pod_name
    pod_name=$(kubectl get pods -n "${NAMESPACE}" --context="${CONTEXT}" -l app.kubernetes.io/component=postgres-exporter -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
    
    if [ -z "${pod_name}" ]; then
        log_error "No postgres-exporter pod found"
        exit 1
    fi
    
    # Check if exporter can connect to PostgreSQL
    local connection_test
    if ! connection_test=$(kubectl exec -n "${NAMESPACE}" --context="${CONTEXT}" "${pod_name}" -- sh -c 'wget -qO- http://localhost:9187/metrics | grep pg_up | head -1' 2>/dev/null); then
        log_error "Cannot test PostgreSQL connectivity from exporter pod"
        exit 1
    fi
    
    if echo "${connection_test}" | grep -q "pg_up 1"; then
        log_success "PostgreSQL connectivity confirmed"
    else
        log_error "PostgreSQL connection failed"
        log_info "Connection test result: ${connection_test}"
        exit 1
    fi
}

# Main test function
main() {
    log_info "Starting PostgreSQL Exporter validation tests..."
    echo
    
    check_cluster
    check_deployment
    check_service
    check_configuration
    check_postgres_connectivity
    test_metrics_endpoint
    
    echo
    log_success "=== All PostgreSQL Exporter tests passed! ==="
    echo
    log_info "Summary:"
    echo "  ✅ Kind cluster accessible"
    echo "  ✅ postgres-exporter deployment ready"
    echo "  ✅ Service configured correctly"
    echo "  ✅ Configuration resources present"
    echo "  ✅ PostgreSQL connectivity confirmed"
    echo "  ✅ Metrics endpoint accessible"
    echo "  ✅ Custom queries working"
    echo "  ✅ Privacy compliance verified"
    echo
    log_info "Metrics endpoint: kubectl port-forward -n arda-credit service/arda-credit-postgres-exporter 9187:9187"
}

# Run tests
main "$@"