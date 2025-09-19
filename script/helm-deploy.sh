#!/bin/bash
# Universal Helm deployment script that works with both Kind and EKS
# Detects environment and uses appropriate configuration

set -euo pipefail

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

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CHART_PATH="${PROJECT_ROOT}/helm/charts/arda-credit"

# Default values
NAMESPACE="arda-credit"
RELEASE_NAME="arda-credit"
EXTERNAL_SECRET_NAME="arda-credit-secrets-external"

# Detect environment (Kind vs EKS)
detect_environment() {
    local current_context
    current_context=$(kubectl config current-context 2>/dev/null || echo "none")
    
    if [[ "$current_context" == *"kind"* ]]; then
        ENVIRONMENT="kind"
        VALUES_FILE="${PROJECT_ROOT}/helm/e2e/values/values-local.yaml"
        log_info "Detected Kind cluster environment"
    elif [[ "$current_context" == *"eks"* ]] || [[ "$current_context" == *"arn:aws"* ]]; then
        ENVIRONMENT="eks"
        VALUES_FILE="${PROJECT_ROOT}/helm/charts/arda-credit/values.yaml"
        log_info "Detected EKS cluster environment"
    else
        log_error "Cannot detect cluster environment. Current context: $current_context"
        log_info "Supported environments: Kind (local) or EKS (AWS)"
        exit 1
    fi
    
    log_success "Using environment: $ENVIRONMENT"
    log_success "Using values file: $VALUES_FILE"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    if ! command -v kubectl &> /dev/null; then
        missing_tools+=("kubectl")
    fi
    
    if ! command -v helm &> /dev/null; then
        missing_tools+=("helm")
    fi
    
    # Kind-specific tools
    if [ "$ENVIRONMENT" = "kind" ]; then
        if ! command -v kind &> /dev/null; then
            missing_tools+=("kind")
        fi
        
        if ! command -v docker &> /dev/null; then
            missing_tools+=("docker")
        fi
        
        if ! command -v curl &> /dev/null; then
            missing_tools+=("curl")
        fi
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install missing tools and try again"
        log_info "You can run 'make install' to install all dependencies"
        exit 1
    fi
    
    # Check if cluster is accessible (skip for Kind if cluster doesn't exist yet)
    if [ "$ENVIRONMENT" != "kind" ]; then
        if ! kubectl cluster-info &> /dev/null; then
            log_error "Cannot access Kubernetes cluster"
            exit 1
        fi
    fi
    
    log_success "All prerequisites satisfied"
}

# Create namespace with protection
create_namespace() {
    log_info "Creating and protecting namespace..."
    kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
    kubectl annotate namespace "$NAMESPACE" "helm.sh/resource-policy=keep" --overwrite
    log_success "Namespace $NAMESPACE created and protected"
}

# Create external secret from .env
create_external_secret() {
    log_info "Creating external secret from .env file..."
    
    if [ ! -f "$PROJECT_ROOT/.env" ]; then
        log_error ".env file not found at $PROJECT_ROOT/.env"
        exit 1
    fi
    
    if kubectl get secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
        log_info "External secret already exists, updating..."
        kubectl delete secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE"
    fi
    
    # Source .env file
    set -a
    source "$PROJECT_ROOT/.env"
    set +a
    
    kubectl create secret generic "$EXTERNAL_SECRET_NAME" \
        --from-literal=sindri-api-key="${SINDRI_API_KEY}" \
        --from-literal=ethereum-rpc-url="${ETHEREUM_RPC_URL}" \
        --from-literal=ethereum-contract-address="${ETHEREUM_CONTRACT_ADDRESS}" \
        --from-literal=ethereum-wallet-private-key="${ETHEREUM_WALLET_PRIVATE_KEY}" \
        --from-literal=ethereum-deployer-address="${ETHEREUM_DEPLOYER_ADDRESS}" \
        --from-literal=signoz-ingestion-key="${SIGNOZ_INGESTION_KEY}" \
        --namespace="$NAMESPACE"
    
    kubectl annotate secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE" "helm.sh/resource-policy=keep" --overwrite
    log_success "External secret created and protected"
}

# Create storage class for EKS
create_storage_class() {
    if [ "$ENVIRONMENT" = "eks" ]; then
        log_info "Creating EKS storage class..."
        if kubectl get storageclass arda-credit-storage >/dev/null 2>&1; then
            log_info "Storage class already exists"
        else
            kubectl apply -f - <<EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: arda-credit-storage
  labels:
    app.kubernetes.io/name: arda-credit
    app.kubernetes.io/component: storage
provisioner: ebs.csi.eks.amazonaws.com
volumeBindingMode: WaitForFirstConsumer
reclaimPolicy: Retain
allowVolumeExpansion: true
parameters:
  type: gp3
  encrypted: "true"
  iops: "3000"
  throughput: "125"
EOF
            log_success "Storage class created"
        fi
    else
        log_info "Skipping storage class creation for Kind environment"
    fi
}

# Setup GHCR secret for Kind
setup_ghcr_secret() {
    if [ "$ENVIRONMENT" = "kind" ]; then
        log_info "Setting up GHCR secret for Kind..."
        
        # Source .env for GHCR credentials  
        set -a
        source "$PROJECT_ROOT/.env"
        set +a
        
        if [ -n "${GHCR_USERNAME:-}" ] && [ -n "${GHCR_TOKEN:-}" ]; then
            kubectl create secret docker-registry ghcr-secret \
                --docker-server=ghcr.io \
                --docker-username="${GHCR_USERNAME}" \
                --docker-password="${GHCR_TOKEN}" \
                --docker-email="${GHCR_EMAIL:-noreply@ardaglobal.com}" \
                --namespace="$NAMESPACE" \
                --dry-run=client -o yaml | kubectl apply -f -
            log_success "GHCR secret created"
        else
            log_warning "GHCR credentials not found in .env"
        fi
    fi
}

# Update Helm dependencies
update_helm_deps() {
    log_info "Updating Helm dependencies..."
    cd "$CHART_PATH"
    helm dependency update
    log_success "Helm dependencies updated"
}

# Lint Helm chart
lint_helm_chart() {
    log_info "Linting Helm chart..."
    helm lint "$CHART_PATH" --values "$VALUES_FILE"
    log_success "Helm chart lint passed"
}

# Smart Helm deployment
deploy_helm_chart() {
    log_info "Deploying Helm chart..."
    
    local helm_cmd="helm"
    local action="install"
    local context_args=""
    
    # Set kubectl context for Kind
    if [ "$ENVIRONMENT" = "kind" ]; then
        context_args="--kube-context=kind-${KIND_CLUSTER_NAME}"
    fi
    
    # Check if release exists
    if helm list -n "$NAMESPACE" $context_args | grep -q "$RELEASE_NAME"; then
        log_info "Release exists, upgrading..."
        action="upgrade"
    else
        log_info "Release doesn't exist, installing..."
        action="install"
    fi
    
    # Deploy with appropriate action
    if [ "$action" = "upgrade" ]; then
        helm upgrade "$RELEASE_NAME" "$CHART_PATH" \
            --namespace "$NAMESPACE" \
            --values "$VALUES_FILE" \
            --timeout=600s \
            --wait \
            $context_args
    else
        helm install "$RELEASE_NAME" "$CHART_PATH" \
            --namespace "$NAMESPACE" \
            --values "$VALUES_FILE" \
            --timeout=600s \
            --wait \
            $context_args
    fi
    
    log_success "Helm chart deployed successfully!"
}

# =============================================================================
# Kind-specific Functions
# =============================================================================

# Kind configuration
KIND_CLUSTER_NAME="arda-credit-e2e"
KIND_REGISTRY_NAME="kind-registry"
KIND_REGISTRY_PORT="5001"
KIND_CONFIG="${PROJECT_ROOT}/helm/e2e/cluster/kind-config.yaml"

# Check if Kind cluster exists and is healthy
check_kind_cluster() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    log_info "Checking Kind cluster status..."
    
    if kind get clusters | grep -q "^${KIND_CLUSTER_NAME}$"; then
        log_info "Kind cluster '${KIND_CLUSTER_NAME}' exists"
        
        # Check if cluster is healthy
        if kubectl cluster-info --context="kind-${KIND_CLUSTER_NAME}" >/dev/null 2>&1; then
            log_success "Kind cluster is healthy"
            return 0
        else
            log_warning "Kind cluster exists but is unhealthy, will recreate..."
            kind delete cluster --name="${KIND_CLUSTER_NAME}"
            log_success "Unhealthy cluster deleted"
        fi
    fi
    
    log_info "Kind cluster needs to be created"
    return 1
}

# Create Kind cluster
create_kind_cluster() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    log_info "Creating Kind cluster '${KIND_CLUSTER_NAME}'..."
    
    if ! kind create cluster --name="${KIND_CLUSTER_NAME}" --config="${KIND_CONFIG}"; then
        log_error "Failed to create Kind cluster"
        exit 1
    fi
    
    log_success "Kind cluster '${KIND_CLUSTER_NAME}' created successfully"
    
    # Wait for cluster to be ready
    log_info "Waiting for cluster to be ready..."
    kubectl wait --for=condition=Ready nodes --all --timeout=120s --context="kind-${KIND_CLUSTER_NAME}"
    log_success "Cluster nodes are ready"
}

# Setup local container registry for Kind
setup_kind_registry() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    log_info "Setting up local container registry..."

    # Clean up any existing registry container first
    if docker ps -a --format "{{.Names}}" | grep -q "^${KIND_REGISTRY_NAME}$"; then
        log_info "Removing existing registry container '${KIND_REGISTRY_NAME}'..."
        docker stop "${KIND_REGISTRY_NAME}" 2>/dev/null || true
        docker rm -f "${KIND_REGISTRY_NAME}" 2>/dev/null || true
        log_success "Existing registry container cleaned up"
    fi

    # Start local registry
    log_info "Starting local registry on port ${KIND_REGISTRY_PORT}..."
    docker run -d --restart=always -p "127.0.0.1:${KIND_REGISTRY_PORT}:5000" --name "${KIND_REGISTRY_NAME}" registry:2

    # Wait for registry to be ready
    log_info "Waiting for registry to be ready..."
    local attempts=0
    local max_attempts=30
    until curl -s "http://localhost:${KIND_REGISTRY_PORT}/v2/" > /dev/null; do
        attempts=$((attempts + 1))
        if [ $attempts -ge $max_attempts ]; then
            log_error "Registry failed to start after ${max_attempts} attempts"
            exit 1
        fi
        sleep 1
    done

    log_success "Local registry is running at localhost:${KIND_REGISTRY_PORT}"
    
    # Connect registry to Kind network
    connect_registry_to_kind
}

# Connect registry to Kind network
connect_registry_to_kind() {
    log_info "Connecting registry to Kind network..."
    
    # Get Kind network name
    local kind_network
    kind_network=$(docker network ls --format "{{.Name}}" | grep "^kind$" || echo "")
    
    if [ -z "${kind_network}" ]; then
        log_error "Kind network not found"
        exit 1
    fi
    
    # Connect registry to Kind network if not already connected
    if ! docker network inspect "${kind_network}" | grep -q "${KIND_REGISTRY_NAME}"; then
        docker network connect "${kind_network}" "${KIND_REGISTRY_NAME}"
        log_success "Registry connected to Kind network"
    else
        log_info "Registry already connected to Kind network"
    fi
}

# Build and push images for Kind
build_and_push_kind_images() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    log_info "Building and pushing Docker images for Kind..."
    
    cd "${PROJECT_ROOT}"
    
    # Registry URL
    local registry_url="localhost:${KIND_REGISTRY_PORT}"
    
    # Check if image already exists locally
    if docker images localhost/arda-credit:local-dev --format "table {{.Repository}}:{{.Tag}}" | grep -q "localhost/arda-credit:local-dev"; then
        log_info "Image localhost/arda-credit:local-dev already exists, skipping build..."
    else
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
    fi
    
    # Tag and push server image to local registry
    log_info "Tagging and pushing server image to local registry..."
    
    # Push server image to local registry
    docker tag localhost/arda-credit:local-dev "${registry_url}/arda-credit:local-dev"
    docker push "${registry_url}/arda-credit:local-dev"
    log_success "Pushed server image to ${registry_url}/arda-credit:local-dev"
    
    log_info "App will use production image from ghcr.io/ardaglobal/arda-credit-app:latest"
    log_success "Local image build and push completed"
}

# Install Kind dependencies
install_kind_dependencies() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    log_info "Installing Kind cluster dependencies..."
    
    # Use Kind cluster context
    local context="kind-${KIND_CLUSTER_NAME}"
    
    # Install MetalLB
    log_info "Installing MetalLB..."
    kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/v0.14.8/config/manifests/metallb-native.yaml --context="$context"
    
    # Wait for MetalLB to be ready
    kubectl wait --namespace metallb-system \
                --for=condition=ready pod \
                --selector=app=metallb \
                --timeout=120s \
                --context="$context"
    
    # Configure MetalLB address pool
    log_info "Configuring MetalLB address pool..."
    cat <<EOF | kubectl apply -f - --context="$context"
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

    log_success "Kind dependencies installed successfully"
}

# Kind-specific namespace and secret setup
setup_kind_namespace_and_secrets() {
    if [ "$ENVIRONMENT" != "kind" ]; then
        return 0
    fi
    
    local context="kind-${KIND_CLUSTER_NAME}"
    
    log_info "Setting up Kind namespace and secrets..."
    
    # Create namespace
    kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - --context="$context"
    kubectl annotate namespace "$NAMESPACE" "helm.sh/resource-policy=keep" --overwrite --context="$context"
    
    # Create external secret for Kind with test values if needed
    if kubectl get secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE" --context="$context" >/dev/null 2>&1; then
        log_info "External secret already exists in Kind, updating..."
        kubectl delete secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE" --context="$context"
    fi
    
    # Source .env file
    set -a
    source "$PROJECT_ROOT/.env" 2>/dev/null || true
    set +a
    
    # Use test values for missing secrets in Kind
    local sindri_key="${SINDRI_API_KEY:-test-key}"
    local eth_rpc="${ETHEREUM_RPC_URL:-http://localhost:8545}"
    local eth_contract="${ETHEREUM_CONTRACT_ADDRESS:-0x1234567890123456789012345678901234567890}"
    local eth_private_key="${ETHEREUM_WALLET_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
    local eth_deployer="${ETHEREUM_DEPLOYER_ADDRESS:-0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266}"
    local signoz_key="${SIGNOZ_INGESTION_KEY:-test-signoz-key}"
    
    kubectl create secret generic "$EXTERNAL_SECRET_NAME" \
        --from-literal=sindri-api-key="$sindri_key" \
        --from-literal=ethereum-rpc-url="$eth_rpc" \
        --from-literal=ethereum-contract-address="$eth_contract" \
        --from-literal=ethereum-wallet-private-key="$eth_private_key" \
        --from-literal=ethereum-deployer-address="$eth_deployer" \
        --from-literal=signoz-ingestion-key="$signoz_key" \
        --namespace="$NAMESPACE" \
        --context="$context"
    
    kubectl annotate secret "$EXTERNAL_SECRET_NAME" -n "$NAMESPACE" "helm.sh/resource-policy=keep" --overwrite --context="$context"
    log_success "Kind external secret created"
}

# Display Kind access information
display_kind_access_info() {
    log_success "=== Kind Environment Ready ==="
    echo
    log_info "Cluster Information:"
    echo "  Cluster Name: ${KIND_CLUSTER_NAME}"
    echo "  Kubectl Context: kind-${KIND_CLUSTER_NAME}"
    echo "  Namespace: arda-credit"
    echo
    log_info "Service Access URLs:"
    echo "  🌐 Frontend App:    http://localhost:3000"
    echo "  🔧 API Server:      http://localhost:8080"
    echo "  🔧 Health Check:    http://localhost:8080/api/v1/health"
    echo "  🗄️  PostgreSQL:      localhost:5432"
    echo "  📊 Postgres Metrics: kubectl port-forward svc/arda-credit-prometheus-postgres-exporter 9187:9187 -n arda-credit"
    echo
    log_info "Kubernetes Access:"
    echo "  kubectl get pods -n arda-credit --context=kind-${KIND_CLUSTER_NAME}"
    echo "  kubectl logs -f deployment/arda-credit-server -n arda-credit --context=kind-${KIND_CLUSTER_NAME}"
    echo "  kubectl logs -f deployment/arda-credit-app -n arda-credit --context=kind-${KIND_CLUSTER_NAME}"
    echo "  kubectl port-forward svc/arda-credit-postgresql 5432:5432 -n arda-credit --context=kind-${KIND_CLUSTER_NAME}"
    echo
    log_info "Database Connection:"
    echo "  Host: localhost"
    echo "  Port: 5432"
    echo "  Database: arda_credit_db_local"
    echo "  Username: postgres"
    echo "  Password: local-dev-password"
    echo
}

# Verify deployment
verify_deployment() {
    log_info "Verifying deployment..."
    
    local context_args=""
    if [ "$ENVIRONMENT" = "kind" ]; then
        context_args="--context=kind-${KIND_CLUSTER_NAME}"
    fi
    
    echo ""
    log_info "Pod Status:"
    kubectl get pods -n "$NAMESPACE" -o wide $context_args
    
    echo ""
    log_info "Services:"
    kubectl get svc -n "$NAMESPACE" $context_args
    
    echo ""
    log_info "Ingress:"  
    kubectl get ingress -n "$NAMESPACE" $context_args 2>/dev/null || echo "No ingress found"
    
    echo ""
    log_info "Secrets:"
    kubectl get secrets -n "$NAMESPACE" $context_args | grep -E "(external|arda-credit)" || echo "No matching secrets found"
    
    echo ""
    log_success "Deployment verification complete!"
    
    if [ "$ENVIRONMENT" = "eks" ]; then
        log_info "🌐 Application: https://app.stag.aws.arda.xyz"
        log_info "🔍 Health Check: https://app.stag.aws.arda.xyz/api/v1/health"
    else
        log_info "🌐 Application: http://localhost:3000 (port-forward required)"
        log_info "🔍 Health Check: http://localhost:8080/api/v1/health (port-forward required)"
    fi
}

# Main execution
main() {
    log_info "Starting universal Helm deployment..."
    
    detect_environment
    check_prerequisites
    
    if [ "$ENVIRONMENT" = "kind" ]; then
        # Kind-specific setup flow
        log_info "Setting up Kind environment..."
        
        # Check and create Kind cluster if needed
        if ! check_kind_cluster; then
            create_kind_cluster
        fi
        
        # Setup local registry and build images
        setup_kind_registry
        build_and_push_kind_images
        
        # Install cluster dependencies
        install_kind_dependencies
        
        # Setup namespace and secrets for Kind
        setup_kind_namespace_and_secrets
        setup_ghcr_secret
    else
        # EKS-specific setup flow
        log_info "Setting up EKS environment..."
        create_namespace
        create_external_secret
        create_storage_class
        setup_ghcr_secret
    fi
    
    # Common deployment steps
    update_helm_deps
    lint_helm_chart
    deploy_helm_chart
    verify_deployment
    
    # Environment-specific completion messages
    if [ "$ENVIRONMENT" = "kind" ]; then
        display_kind_access_info
    fi
    
    log_success "🎉 Deployment complete!"
}

# Handle command line arguments
case "${1:-deploy}" in
    "deploy")
        main
        ;;
    "status")
        detect_environment
        verify_deployment
        ;;
    "clean")
        detect_environment
        log_warning "Cleaning up deployment..."
        
        local context_args=""
        if [ "$ENVIRONMENT" = "kind" ]; then
            context_args="--kube-context=kind-${KIND_CLUSTER_NAME}"
            
            # Ask for confirmation before destroying Kind cluster
            echo ""
            log_warning "This will remove the Helm release from Kind cluster."
            read -p "Do you also want to delete the entire Kind cluster? (y/N): " -n 1 -r
            echo
            
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                log_warning "Deleting entire Kind cluster and local registry..."
                helm uninstall "$RELEASE_NAME" -n "$NAMESPACE" $context_args || true
                kind delete cluster --name="$KIND_CLUSTER_NAME" || true
                docker stop "$KIND_REGISTRY_NAME" 2>/dev/null || true
                docker rm -f "$KIND_REGISTRY_NAME" 2>/dev/null || true
                log_success "Kind cluster and registry completely removed"
            else
                log_info "Removing only Helm release, keeping Kind cluster..."
                helm uninstall "$RELEASE_NAME" -n "$NAMESPACE" $context_args || true
                log_success "Helm release removed, Kind cluster preserved"
            fi
        else
            helm uninstall "$RELEASE_NAME" -n "$NAMESPACE" || true
            log_success "Cleanup complete"
        fi
        ;;
    *)
        echo "Usage: $0 [deploy|status|clean]"
        echo "  deploy - Deploy or upgrade the application (default)"
        echo "  status - Check deployment status"
        echo "  clean  - Clean up deployment"
        exit 1
        ;;
esac