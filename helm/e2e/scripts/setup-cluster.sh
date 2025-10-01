#!/bin/bash
# Setup Kind cluster for Arda Credit local e2e testing
# This script creates a Kind cluster, builds local images, and prepares the environment

set -euo pipefail

# Configuration
CLUSTER_NAME="arda-credit-e2e"
REGISTRY_NAME="kind-registry"
REGISTRY_PORT="5001"
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

# Setup local container registry
setup_local_registry() {
    log_info "Setting up local container registry..."

    # Always clean up any existing registry container first
    if docker ps -a --format "{{.Names}}" | grep -q "^${REGISTRY_NAME}$"; then
        log_info "Removing existing registry container '${REGISTRY_NAME}'..."
        docker stop "${REGISTRY_NAME}" 2>/dev/null || true
        docker rm -f "${REGISTRY_NAME}" 2>/dev/null || true
        log_success "Existing registry container cleaned up"
    fi

    # Start local registry
    log_info "Starting local registry on port ${REGISTRY_PORT}..."
    docker run -d --restart=always -p "127.0.0.1:${REGISTRY_PORT}:5000" --name "${REGISTRY_NAME}" registry:2

    # Wait for registry to be ready
    log_info "Waiting for registry to be ready..."
    local attempts=0
    local max_attempts=30
    until curl -s "http://localhost:${REGISTRY_PORT}/v2/" > /dev/null; do
        attempts=$((attempts + 1))
        if [ $attempts -ge $max_attempts ]; then
            log_error "Registry failed to start after ${max_attempts} attempts"
            exit 1
        fi
        sleep 1
    done

    log_success "Local registry is running at localhost:${REGISTRY_PORT}"
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
    if ! docker network inspect "${kind_network}" | grep -q "${REGISTRY_NAME}"; then
        docker network connect "${kind_network}" "${REGISTRY_NAME}"
        log_success "Registry connected to Kind network"
    else
        log_info "Registry already connected to Kind network"
    fi
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

# Build and push Docker images to local registry
build_and_push_images() {
    log_info "Building and pushing Docker images to local registry..."
    
    cd "${PROJECT_ROOT}"
    
    # Registry URL
    local registry_url="localhost:${REGISTRY_PORT}"
    
    # Check if main image already exists locally
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
    
    # Push server image only (app will use GHCR)
    docker tag localhost/arda-credit:local-dev "${registry_url}/arda-credit:local-dev"
    docker push "${registry_url}/arda-credit:local-dev"
    log_success "Pushed server image to ${registry_url}/arda-credit:local-dev"
    
    # Data-populator will use GHCR image, no need to build locally
    log_info "Data-populator will use production image from ghcr.io/ardaglobal/arda-credit-data-populator:latest"
    log_success "Local registry setup completed"
}

# Install dependencies (MetalLB + Docker registry secrets)
install_dependencies() {
    log_info "Installing cluster dependencies..."
    
    # Create arda-credit namespace first (needed for secrets)
    log_info "Creating arda-credit namespace..."
    kubectl create namespace arda-credit --dry-run=client -o yaml | kubectl apply -f - || true
    
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
        log_info "Creating ghcr-secret from environment variables in arda-credit namespace..."
        kubectl create secret docker-registry ghcr-secret \
            --docker-server=ghcr.io \
            --docker-username="${GHCR_USERNAME}" \
            --docker-password="${GHCR_TOKEN}" \
            --docker-email="${GHCR_EMAIL:-noreply@ardaglobal.com}" \
            --namespace=arda-credit \
            --dry-run=client -o yaml | kubectl apply -f -
        
        # Validate that the secret was created successfully
        if kubectl get secret ghcr-secret -n arda-credit >/dev/null 2>&1; then
            log_success "ghcr-secret created successfully in arda-credit namespace"
        else
            log_error "Failed to create ghcr-secret in arda-credit namespace"
            log_info "This will prevent pods from pulling private images from ghcr.io"
            exit 1
        fi
    else
        log_warning "GHCR credentials not found in .env file"
        log_warning "Set GHCR_USERNAME and GHCR_TOKEN in .env to enable private image pulls"
        log_warning "Pods may fail to pull private images from ghcr.io/ardaglobal/*"
        log_info "To fix this:"
        log_info "  1. Add GHCR_USERNAME=your-github-username to .env"
        log_info "  2. Add GHCR_TOKEN=your-github-token to .env"
        log_info "  3. Re-run 'make kind-setup'"
    fi
    
    # Create arda-credit-secrets for Helm lookup (external to Helm management)
    if [ -n "${SINDRI_API_KEY:-}" ]; then
        log_info "Creating arda-credit-secrets from environment variables for Helm lookup..."
        
        # Validate SigNoz key if provided
        local signoz_key="${SIGNOZ_INGESTION_KEY:-test-signoz-key}"
        if [ "${signoz_key}" != "test-signoz-key" ] && [ -n "${signoz_key}" ]; then
            if curl -X POST https://ingest.us.signoz.cloud:443/v1/metrics \
                -H "signoz-ingestion-key: ${signoz_key}" \
                -H "Content-Type: application/x-protobuf" \
                --data-binary "" --max-time 10 >/dev/null 2>&1; then
                log_success "SigNoz ingestion key validated successfully"
            else
                log_warning "SigNoz ingestion key validation failed, using test key"
                signoz_key="test-signoz-key"
            fi
        fi
        
        # For local development, use Anvil service URL; otherwise use ETHEREUM_RPC_URL from .env
        local RPC_URL="${ETHEREUM_RPC_URL:-http://arda-credit-anvil:8545}"

        kubectl create secret generic arda-credit-secrets-external \
            --from-literal=sindri-api-key="${SINDRI_API_KEY}" \
            --from-literal=ethereum-rpc-url="${RPC_URL}" \
            --from-literal=ethereum-contract-address="${ETHEREUM_CONTRACT_ADDRESS:-0x0}" \
            --from-literal=ethereum-wallet-private-key="${ETHEREUM_WALLET_PRIVATE_KEY:-0x0}" \
            --from-literal=ethereum-deployer-address="${ETHEREUM_DEPLOYER_ADDRESS:-0x0}" \
            --from-literal=signoz-ingestion-key="${signoz_key}" \
            --from-literal=aws-access-key-id="${AWS_ACCESS_KEY_ID:-test-aws-access-key-id}" \
            --from-literal=aws-secret-access-key="${AWS_SECRET_ACCESS_KEY:-test-aws-secret-access-key}" \
            --from-literal=email-from="${EMAIL_FROM:-no-reply@example.local}" \
            --from-literal=email-reply-to="${EMAIL_REPLY_TO:-support@example.local}" \
            --namespace=arda-credit \
            --dry-run=client -o yaml | kubectl apply -f -
        log_success "arda-credit-secrets-external created for Helm lookup"
    else
        log_warning "SINDRI_API_KEY not found, creating test secret for Helm lookup"
        kubectl create secret generic arda-credit-secrets-external \
            --from-literal=sindri-api-key="test-key" \
            --from-literal=ethereum-rpc-url="http://arda-credit-anvil:8545" \
            --from-literal=ethereum-contract-address="0x1234567890123456789012345678901234567890" \
            --from-literal=ethereum-wallet-private-key="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" \
            --from-literal=ethereum-deployer-address="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" \
            --from-literal=signoz-ingestion-key="test-signoz-key" \
            --from-literal=aws-access-key-id="test-aws-access-key-id" \
            --from-literal=aws-secret-access-key="test-aws-secret-access-key" \
            --from-literal=email-from="no-reply@example.local" \
            --from-literal=email-reply-to="support@example.local" \
            --namespace=arda-credit \
            --dry-run=client -o yaml | kubectl apply -f -
        log_success "Test arda-credit-secrets-external created for Helm lookup"
    fi
    
    log_success "Dependencies installed and configured"
}

# Validate ghcr-secret accessibility
validate_ghcr_secret() {
    log_info "Validating ghcr-secret accessibility..."
    
    # Check if ghcr-secret exists in arda-credit namespace
    if kubectl get secret ghcr-secret -n arda-credit >/dev/null 2>&1; then
        log_success "ghcr-secret found in arda-credit namespace"
        
        # Verify the secret has the expected keys
        local secret_keys
        secret_keys=$(kubectl get secret ghcr-secret -n arda-credit -o jsonpath='{.data}' | jq -r 'keys[]' 2>/dev/null || echo "")
        
        if echo "${secret_keys}" | grep -q ".dockerconfigjson"; then
            log_success "ghcr-secret contains valid Docker registry configuration"
        else
            log_warning "ghcr-secret exists but may not contain valid Docker registry configuration"
        fi
    else
        log_warning "ghcr-secret not found in arda-credit namespace"
        log_warning "Pods may fail to pull private images from ghcr.io"
        log_info "To create the secret manually:"
        log_info "  kubectl create secret docker-registry ghcr-secret \\"
        log_info "    --docker-server=ghcr.io \\"
        log_info "    --docker-username=YOUR_GITHUB_USERNAME \\"
        log_info "    --docker-password=YOUR_GITHUB_TOKEN \\"
        log_info "    --namespace=arda-credit"
    fi
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
    
    # Deploy to arda-credit namespace for proper isolation
    NAMESPACE="arda-credit"
    
    helm upgrade --install arda-credit \
        "${CHART_PATH}" \
        --values="${VALUES_FILE}" \
        --namespace="${NAMESPACE}" \
        --create-namespace \
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

# Configure PostgreSQL monitoring user
configure_postgresql_monitoring() {
    log_info "Configuring PostgreSQL monitoring user permissions..."
    
    # Wait for PostgreSQL to be ready for connections
    local max_attempts=60
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if kubectl exec -n arda-credit statefulset/arda-credit-postgresql -- env PGPASSWORD=local-dev-password psql -U postgres -c "SELECT 1;" >/dev/null 2>&1; then
            log_success "PostgreSQL is ready for configuration"
            break
        fi
        
        attempt=$((attempt + 1))
        log_info "Waiting for PostgreSQL to accept connections (attempt $attempt/$max_attempts)..."
        sleep 3
    done
    
    if [ $attempt -eq $max_attempts ]; then
        log_error "PostgreSQL failed to become ready for connections"
        exit 1
    fi
    
    # PostgreSQL init scripts should have created the monitoring user, but ensure it's properly configured
    log_info "Verifying and configuring monitoring user permissions..."
    kubectl exec -n arda-credit statefulset/arda-credit-postgresql -- env PGPASSWORD=local-dev-password psql -U postgres -c "
        -- Ensure monitoring user exists with correct password (init script should have created it)
        DO \$\$
        BEGIN
            -- Update password in case it needs refresh
            ALTER USER monitoring PASSWORD 'monitoring-password';
            RAISE NOTICE 'Monitoring user password updated';
        EXCEPTION WHEN undefined_object THEN
            -- Fallback: create user if init script failed
            CREATE USER monitoring WITH PASSWORD 'monitoring-password';
            RAISE NOTICE 'Monitoring user created as fallback';
        END
        \$\$;
        
        -- Ensure required permissions for PostgreSQL 10+ (SigNoz requirements)
        GRANT pg_monitor TO monitoring;
        GRANT SELECT ON pg_stat_database TO monitoring;
        
        -- Verify permissions
        SELECT 'Monitoring user configured successfully' AS status;
    " || {
        log_error "Failed to configure monitoring user"
        exit 1
    }
    
    # Test monitoring user connection with retries
    log_info "Testing monitoring user connection..."
    local test_attempts=0
    local max_test_attempts=5
    
    while [ $test_attempts -lt $max_test_attempts ]; do
        if kubectl exec -n arda-credit statefulset/arda-credit-postgresql -- env PGPASSWORD=monitoring-password psql -U monitoring -d arda_credit_db_local -c "SELECT count(*) FROM pg_stat_database;" >/dev/null 2>&1; then
            log_success "Monitoring user authentication and permissions verified"
            return 0
        fi
        
        test_attempts=$((test_attempts + 1))
        log_warning "Monitoring user test failed (attempt $test_attempts/$max_test_attempts), retrying..."
        sleep 2
    done
    
    log_error "Monitoring user authentication test failed after $max_test_attempts attempts"
    exit 1
}

# Validate monitoring configuration end-to-end
validate_monitoring_setup() {
    log_info "Validating end-to-end monitoring setup..."
    
    # Wait for OTEL collector to be ready
    log_info "Waiting for OTEL collector to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=opentelemetry-collector -n arda-credit --timeout=120s
    
    # Give OTEL collector time to start scraping
    log_info "Allowing OTEL collector to start metric collection..."
    sleep 30
    
    # Check for PostgreSQL receiver errors
    local error_count=$(kubectl logs deployment/arda-credit-opentelemetry-collector -n arda-credit --since=30s | grep -c "postgresql.*authentication.*failed" || echo "0")
    
    if [ "$error_count" -gt 0 ]; then
        log_warning "Found $error_count PostgreSQL authentication errors in OTEL collector logs"
        log_info "Attempting to restart OTEL collector to refresh connections..."
        kubectl rollout restart deployment/arda-credit-opentelemetry-collector -n arda-credit
        kubectl rollout status deployment/arda-credit-opentelemetry-collector -n arda-credit --timeout=60s
        sleep 20
        
        # Re-check for errors
        error_count=$(kubectl logs deployment/arda-credit-opentelemetry-collector -n arda-credit --since=20s | grep -c "postgresql.*authentication.*failed" || echo "0")
        
        if [ "$error_count" -gt 0 ]; then
            log_error "PostgreSQL monitoring still has authentication errors after restart"
            exit 1
        fi
    fi
    
    log_success "End-to-end monitoring validation completed successfully"
}

# Wait for services to be ready
wait_for_services() {
    log_info "Waiting for services to be ready..."
    
    # Wait for PostgreSQL
    log_info "Waiting for PostgreSQL to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=postgresql -n arda-credit --timeout=120s
    
    # Configure PostgreSQL monitoring user after PostgreSQL is ready
    configure_postgresql_monitoring
    
    # Wait for server
    log_info "Waiting for server to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server -n arda-credit --timeout=180s
    
    # Wait for app (if enabled)
    log_info "Waiting for app to be ready..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=app -n arda-credit --timeout=120s
    
    # Validate end-to-end monitoring setup
    validate_monitoring_setup
    
    log_success "All services are ready and monitoring is validated"
}

# Display access information
display_access_info() {
    log_success "=== Arda Credit E2E Environment Ready ==="
    echo
    log_info "Cluster Information:"
    echo "  Cluster Name: ${CLUSTER_NAME}"
    echo "  Kubectl Context: kind-${CLUSTER_NAME}"
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
    echo "  kubectl get pods -n arda-credit"
    echo "  kubectl logs -f deployment/arda-credit-server -n arda-credit"
    echo "  kubectl logs -f deployment/arda-credit-app -n arda-credit"
    echo "  kubectl logs -f deployment/arda-credit-prometheus-postgres-exporter -n arda-credit"
    echo "  kubectl port-forward svc/arda-credit-postgresql 5432:5432 -n arda-credit"
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
    setup_local_registry
    if check_existing_cluster; then
        create_cluster
    fi
    connect_registry_to_kind
    build_and_push_images
    install_dependencies
    update_helm_dependencies
    deploy_arda_credit
    wait_for_services
    validate_ghcr_secret
    
    echo
    display_access_info
}

# Run main function
main "$@"