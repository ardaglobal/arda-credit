#!/bin/bash
# Setup Kind cluster for Arda Credit local e2e testing
# This script creates a Kind cluster, builds local images, and prepares the environment

set -euo pipefail

# Configuration
CLUSTER_NAME="arda-credit-e2e"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
KIND_CONFIG="${SCRIPT_DIR}/../cluster/kind-config.yaml"
VALUES_FILE="${SCRIPT_DIR}/../values/values-local.yaml"
CHART_PATH="${PROJECT_ROOT}/helm/charts/arda-credit"

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    if ! command -v kind &> /dev/null; then
        missing_tools+=("kind")
    fi
    
    if ! command -v kubectl &> /dev/null; then
        missing_tools+=("kubectl")
    fi
    
    if ! command -v helm &> /dev/null; then
        missing_tools+=("helm")
    fi
    
    if ! command -v docker &> /dev/null; then
        missing_tools+=("docker")
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install missing tools and try again"
        log_info "You can run 'make install' to install all dependencies"
        exit 1
    fi
    
    log_success "All prerequisites satisfied"
}

# Check if cluster already exists
check_existing_cluster() {
    if kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
        log_warning "Kind cluster '${CLUSTER_NAME}' already exists"
        
        # Check if cluster is healthy
        if kubectl cluster-info --context="kind-${CLUSTER_NAME}" >/dev/null 2>&1; then
            log_info "Cluster is healthy, skipping recreation"
            return 1  # Skip cluster creation
        else
            log_warning "Cluster exists but is unhealthy, recreating..."
            kind delete cluster --name="${CLUSTER_NAME}"
            log_success "Unhealthy cluster deleted"
        fi
    fi
    return 0  # Proceed with cluster creation
}

# Create Kind cluster
create_cluster() {
    log_info "Creating Kind cluster '${CLUSTER_NAME}'..."
    
    if ! kind create cluster --name="${CLUSTER_NAME}" --config="${KIND_CONFIG}"; then
        log_error "Failed to create Kind cluster"
        exit 1
    fi
    
    log_success "Kind cluster '${CLUSTER_NAME}' created successfully"
    
    # Wait for cluster to be ready
    log_info "Waiting for cluster to be ready..."
    kubectl wait --for=condition=Ready nodes --all --timeout=120s
    log_success "Cluster nodes are ready"
}

# Build and load Docker images
build_and_load_images() {
    log_info "Building and loading Docker images..."
    
    cd "${PROJECT_ROOT}"
    
    # Login to GHCR if credentials are available
    if [ -n "${GHCR_USERNAME:-}" ] && [ -n "${GHCR_TOKEN:-}" ]; then
        log_info "Logging into GHCR for Docker build..."
        echo "${GHCR_TOKEN}" | docker login ghcr.io -u "${GHCR_USERNAME}" --password-stdin
    else
        log_warning "No GHCR credentials found, build may fail if using private base images"
    fi
    
    # Build the main Arda Credit image
    log_info "Building arda-credit:local-dev image..."
    if ! docker build -t localhost/arda-credit:local-dev .; then
        log_error "Failed to build arda-credit image"
        log_info "Check Docker build logs above for details"
        exit 1
    fi
    
    # Load image into Kind cluster
    log_info "Loading arda-credit image into Kind cluster..."
    if ! kind load docker-image localhost/arda-credit:local-dev --name="${CLUSTER_NAME}"; then
        log_error "Failed to load arda-credit image into Kind cluster"
        exit 1
    fi
    
    # Note: App image would be built separately if it exists
    # For now, we'll use the server image for both services
    log_info "Tagging server image as app image for local testing..."
    docker tag localhost/arda-credit:local-dev localhost/arda-credit-app:local-dev
    kind load docker-image localhost/arda-credit-app:local-dev --name="${CLUSTER_NAME}"
    
    log_success "Docker images built and loaded successfully"
}

# Install dependencies (MetalLB + Docker registry secrets)
install_dependencies() {
    log_info "Installing cluster dependencies..."
    
    # Apply MetalLB
    log_info "Installing MetalLB..."
    kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/v0.14.8/config/manifests/metallb-native.yaml
    
    # Wait for MetalLB to be ready
    kubectl wait --namespace metallb-system \
                --for=condition=ready pod \
                --selector=app=metallb \
                --timeout=120s
    
    # Configure MetalLB address pool
    log_info "Configuring MetalLB address pool..."
    cat <<EOF | kubectl apply -f -
apiVersion: metallb.io/v1beta1
kind: IPAddressPool
metadata:
  name: example
  namespace: metallb-system
spec:
  addresses:
  - 172.19.255.200-172.19.255.250
---
apiVersion: metallb.io/v1beta1
kind: L2Advertisement
metadata:
  name: empty
  namespace: metallb-system
EOF

    # Install Docker registry secrets for pulling private images
    log_info "Installing Docker registry secrets..."
    
    # Source .env file if it exists
    if [ -f "${PROJECT_ROOT}/.env" ]; then
        log_info "Loading environment variables from .env..."
        set -a  # Export all variables
        source "${PROJECT_ROOT}/.env"
        set +a  # Stop exporting
    fi
    
    # Create ghcr-secret if credentials are available
    if [ -n "${GHCR_USERNAME:-}" ] && [ -n "${GHCR_TOKEN:-}" ]; then
        log_info "Creating ghcr-secret from environment variables..."
        kubectl create secret docker-registry ghcr-secret \
            --docker-server=ghcr.io \
            --docker-username="${GHCR_USERNAME}" \
            --docker-password="${GHCR_TOKEN}" \
            --docker-email="${GHCR_EMAIL:-noreply@ardaglobal.com}" \
            --dry-run=client -o yaml | kubectl apply -f -
    else
        log_warning "GHCR credentials not found in .env, skipping ghcr-secret creation"
    fi
    
    # Create arda-credit-secrets if variables are available
    if [ -n "${SINDRI_API_KEY:-}" ]; then
        log_info "Creating arda-credit-secrets from environment variables..."
        kubectl create secret generic arda-credit-secrets \
            --from-literal=sindri-api-key="${SINDRI_API_KEY}" \
            --from-literal=ethereum-rpc-url="${ETHEREUM_RPC_URL:-http://localhost:8545}" \
            --from-literal=ethereum-contract-address="${ETHEREUM_CONTRACT_ADDRESS:-0x0}" \
            --from-literal=ethereum-wallet-private-key="${ETHEREUM_WALLET_PRIVATE_KEY:-0x0}" \
            --from-literal=ethereum-deployer-address="${ETHEREUM_DEPLOYER_ADDRESS:-0x0}" \
            --dry-run=client -o yaml | kubectl apply -f -
    else
        log_warning "SINDRI_API_KEY not found in .env, using test values for local development"
        # Create with test values for local e2e testing
        kubectl create secret generic arda-credit-secrets \
            --from-literal=sindri-api-key="test-key" \
            --from-literal=ethereum-rpc-url="http://localhost:8545" \
            --from-literal=ethereum-contract-address="0x1234567890123456789012345678901234567890" \
            --from-literal=ethereum-wallet-private-key="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" \
            --from-literal=ethereum-deployer-address="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" \
            --dry-run=client -o yaml | kubectl apply -f -
    fi
    
    log_success "Dependencies installed and configured"
}

# Update Helm dependencies
update_helm_dependencies() {
    log_info "Updating Helm dependencies..."
    
    cd "${CHART_PATH}"
    
    # Add Bitnami repo if not already added
    if ! helm repo list | grep -q bitnami; then
        helm repo add bitnami https://charts.bitnami.com/bitnami
    fi
    
    helm repo update
    helm dependency update
    
    log_success "Helm dependencies updated"
}

# Deploy Arda Credit via Helm
deploy_arda_credit() {
    log_info "Deploying Arda Credit via Helm..."
    
    # Deploy to default namespace for simplicity
    helm upgrade --install arda-credit \
        "${CHART_PATH}" \
        --values="${VALUES_FILE}" \
        --wait \
        --timeout=300s \
        --debug
    
    if [ $? -eq 0 ]; then
        log_success "Arda Credit deployed successfully"
    else
        log_error "Failed to deploy Arda Credit"
        log_info "Check the logs above for details"
        exit 1
    fi
}

# Wait for services to be ready
wait_for_services() {
    log_info "Waiting for services to be ready..."
    
    # Wait for PostgreSQL
    log_info "Waiting for PostgreSQL to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=postgresql --timeout=120s
    
    # Wait for server
    log_info "Waiting for server to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --timeout=180s
    
    # Wait for app (if enabled)
    log_info "Waiting for app to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=app --timeout=120s
    
    log_success "All services are ready"
}

# Display access information
display_access_info() {
    log_success "=== Arda Credit E2E Environment Ready ==="
    echo
    log_info "Cluster Information:"
    echo "  Cluster Name: ${CLUSTER_NAME}"
    echo "  Kubectl Context: kind-${CLUSTER_NAME}"
    echo
    log_info "Service Access URLs:"
    echo "  🌐 Frontend App:    http://localhost:3000"
    echo "  🔧 API Server:      http://localhost:8080"
    echo "  🔧 Health Check:    http://localhost:8080/api/v1/health"
    echo "  🗄️  PostgreSQL:      localhost:5432"
    echo
    log_info "Kubernetes Access:"
    echo "  kubectl get pods"
    echo "  kubectl logs -f deployment/arda-credit-server"
    echo "  kubectl logs -f deployment/arda-credit-app"
    echo "  kubectl port-forward svc/arda-credit-postgresql 5432:5432"
    echo
    log_info "Database Connection:"
    echo "  Host: localhost"
    echo "  Port: 5432"
    echo "  Database: arda_credit_db_local"
    echo "  Username: postgres"
    echo "  Password: local-dev-password"
    echo
    log_warning "To tear down the environment, run: ./teardown-cluster.sh"
}

# Main execution
main() {
    log_info "Setting up Arda Credit e2e environment..."
    echo
    
    check_prerequisites
    if check_existing_cluster; then
        create_cluster
    fi
    build_and_load_images
    install_dependencies
    update_helm_dependencies
    deploy_arda_credit
    wait_for_services
    
    echo
    display_access_info
}

# Run main function
main "$@"