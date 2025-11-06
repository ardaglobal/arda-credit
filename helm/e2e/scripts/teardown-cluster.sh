#!/bin/bash
# Teardown Kind cluster for Arda Credit local e2e testing
# This script safely removes the Kind cluster and cleans up resources

set -euo pipefail

# Configuration
CLUSTER_NAME="arda-credit-e2e"
REGISTRY_NAME="kind-registry"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Check if cluster exists
check_cluster_exists() {
    if ! kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
        log_warning "Kind cluster '${CLUSTER_NAME}' does not exist"
        log_info "Nothing to tear down"
        exit 0
    fi
}

# Uninstall Helm release (optional cleanup)
uninstall_helm_release() {
    log_info "Checking for Helm release 'arda-credit'..."
    
    # Switch to the correct kubectl context
    kubectl config use-context "kind-${CLUSTER_NAME}" 2>/dev/null || true
    
    # Check if Helm release exists
    if helm list | grep -q arda-credit; then
        log_info "Uninstalling Helm release 'arda-credit'..."
        if helm uninstall arda-credit --timeout=60s; then
            log_success "Helm release uninstalled"
        else
            log_warning "Failed to uninstall Helm release cleanly (cluster will still be deleted)"
        fi
    else
        log_info "No Helm release 'arda-credit' found"
    fi
}

# Delete Kind cluster
delete_cluster() {
    log_info "Deleting Kind cluster '${CLUSTER_NAME}'..."
    
    if kind delete cluster --name="${CLUSTER_NAME}"; then
        log_success "Kind cluster '${CLUSTER_NAME}' deleted successfully"
    else
        log_error "Failed to delete Kind cluster"
        exit 1
    fi
}

# Clean up local registry container
cleanup_registry() {
    log_info "Cleaning up local container registry..."

    # Check if registry container exists (running or stopped)
    if docker ps -a --format "table {{.Names}}" | grep -q "^${REGISTRY_NAME}$"; then
        log_info "Removing registry container: ${REGISTRY_NAME}"
        docker stop "${REGISTRY_NAME}" 2>/dev/null || log_warning "Registry container was not running"
        docker rm "${REGISTRY_NAME}" 2>/dev/null || log_warning "Could not remove registry container: ${REGISTRY_NAME}"
        log_success "Registry container removed"
    else
        log_info "Registry container '${REGISTRY_NAME}' not found"
    fi
}

# Clean up Docker images (optional)
cleanup_docker_images() {
    log_info "Cleaning up local Docker images..."

    # Remove local development images
    local images_to_remove=(
        "localhost/arda-credit:local-dev"
    )

    for image in "${images_to_remove[@]}"; do
        if docker images --format "table {{.Repository}}:{{.Tag}}" | grep -q "$image"; then
            log_info "Removing image: $image"
            docker rmi "$image" 2>/dev/null || log_warning "Could not remove image: $image"
        fi
    done

    log_success "Docker image cleanup completed"
}

# Display cleanup summary
display_summary() {
    log_success "=== Arda Credit E2E Environment Cleanup Complete ==="
    echo
    log_info "Cleaned up resources:"
    echo "  ✓ Kind cluster '${CLUSTER_NAME}' deleted"
    echo "  ✓ Helm release 'arda-credit' uninstalled"
    echo "  ✓ Local container registry removed"
    echo "  ✓ Local Docker images removed"
    echo "  ✓ Kubectl context cleaned up"
    echo
    log_info "To recreate the environment, run: ./setup-cluster.sh"
}

# Confirm deletion (with option to skip confirmation)
confirm_deletion() {
    if [[ "${1:-}" == "--force" || "${1:-}" == "-f" ]]; then
        log_info "Force mode enabled, skipping confirmation"
        return 0
    fi
    
    log_warning "This will delete the Kind cluster '${CLUSTER_NAME}' and all associated resources"
    read -p "Are you sure you want to proceed? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Teardown cancelled by user"
        exit 0
    fi
}

# Main execution
main() {
    log_info "Tearing down Arda Credit e2e environment..."
    echo

    check_cluster_exists
    confirm_deletion "$@"
    uninstall_helm_release
    delete_cluster
    cleanup_registry
    cleanup_docker_images

    echo
    display_summary
}

# Show help information
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Teardown the Arda Credit Kind cluster e2e testing environment.

OPTIONS:
    -f, --force    Skip confirmation prompt
    -h, --help     Show this help message

EXAMPLES:
    $0              # Interactive teardown with confirmation
    $0 --force      # Non-interactive teardown
    $0 --help       # Show this help message

EOF
}

# Handle command line arguments
case "${1:-}" in
    -h|--help)
        show_help
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac