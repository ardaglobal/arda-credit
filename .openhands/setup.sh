#!/bin/bash

# Arda Credit Development Environment Setup Script
# This script sets up the complete development environment for the Arda Credit
# privacy-preserving loan vApp, including all required tools and dependencies.

set -e  # Exit on any error

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

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command_exists apt-get; then
            OS="ubuntu"
        elif command_exists yum; then
            OS="centos"
        elif command_exists pacman; then
            OS="arch"
        else
            OS="linux"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
    else
        OS="unknown"
    fi
    log_info "Detected OS: $OS"
}

# Install system dependencies
install_system_deps() {
    log_info "Installing system dependencies..."
    
    case $OS in
        ubuntu)
            sudo apt-get update
            sudo apt-get install -y \
                curl \
                wget \
                git \
                build-essential \
                pkg-config \
                libssl-dev \
                libpq-dev \
                postgresql \
                postgresql-contrib \
                docker.io \
                docker-compose \
                jq \
                unzip \
                ca-certificates \
                gnupg \
                lsb-release
            ;;
        macos)
            if ! command_exists brew; then
                log_info "Installing Homebrew..."
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            fi
            
            brew update
            brew install \
                curl \
                wget \
                git \
                pkg-config \
                openssl \
                libpq \
                postgresql@14 \
                docker \
                docker-compose \
                jq
            ;;
        *)
            log_warning "Unsupported OS. Please install dependencies manually:"
            log_warning "- curl, wget, git, build tools"
            log_warning "- OpenSSL development headers"
            log_warning "- PostgreSQL client and server"
            log_warning "- Docker and Docker Compose"
            ;;
    esac
    
    log_success "System dependencies installed"
}

# Install Kubernetes and DevOps tools
install_devops_tools() {
    log_info "Installing DevOps tools (kubectl, KIND, Helm)..."
    
    case $OS in
        ubuntu)
            # Install kubectl
            if ! command_exists kubectl; then
                log_info "Installing kubectl..."
                curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
                chmod +x kubectl
                sudo mv kubectl /usr/local/bin/
            fi
            
            # Install KIND
            if ! command_exists kind; then
                log_info "Installing KIND..."
                curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.22.0/kind-linux-amd64
                chmod +x ./kind
                sudo mv ./kind /usr/local/bin/
            fi
            
            # Install Helm
            if ! command_exists helm; then
                log_info "Installing Helm..."
                curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
            fi
            ;;
        macos)
            # Install kubectl
            if ! command_exists kubectl; then
                log_info "Installing kubectl..."
                brew install kubectl
            fi
            
            # Install KIND
            if ! command_exists kind; then
                log_info "Installing KIND..."
                brew install kind
            fi
            
            # Install Helm
            if ! command_exists helm; then
                log_info "Installing Helm..."
                brew install helm
            fi
            ;;
        *)
            log_warning "Please install DevOps tools manually:"
            log_warning "- kubectl: https://kubernetes.io/docs/tasks/tools/"
            log_warning "- KIND: https://kind.sigs.k8s.io/docs/user/quick-start/"
            log_warning "- Helm: https://helm.sh/docs/intro/install/"
            return
            ;;
    esac
    
    # Verify installations
    if command_exists kubectl && command_exists kind && command_exists helm; then
        log_success "DevOps tools installed successfully"
        kubectl version --client --output=yaml 2>/dev/null | grep gitVersion || log_info "kubectl installed"
        kind version || log_info "KIND installed"
        helm version --short || log_info "Helm installed"
    else
        log_warning "Some DevOps tools may not have installed correctly"
    fi
}

# Install Rust toolchain
install_rust() {
    log_info "Installing Rust toolchain..."
    
    if ! command_exists rustc; then
        log_info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    else
        log_info "Rust already installed, updating..."
        rustup update
    fi
    
    # Install required Rust components
    log_info "Installing Rust components..."
    rustup component add clippy rustfmt
    
    # Install required Rust tools
    log_info "Installing Rust tools..."
    cargo install --locked \
        cargo-audit \
        cargo-deny \
        sqlx-cli \
        cargo-watch \
        cargo-expand
    
    log_success "Rust toolchain installed"
}

# Install SP1 zkVM toolchain
install_sp1() {
    log_info "Installing SP1 zkVM toolchain..."
    
    # Install SP1 toolchain
    if ! command_exists sp1up; then
        log_info "Installing SP1 toolchain..."
        curl -L https://sp1.succinct.xyz | bash
        source ~/.bashrc 2>/dev/null || source ~/.zshrc 2>/dev/null || true
        sp1up
    else
        log_info "SP1 already installed, updating..."
        sp1up
    fi
    
    # Verify SP1 installation
    if command_exists cargo-prove; then
        log_success "SP1 zkVM toolchain installed successfully"
    else
        log_warning "SP1 installation may not be complete. Please restart your shell and run 'sp1up'"
    fi
}

# Install Foundry for Solidity development
install_foundry() {
    log_info "Installing Foundry for Solidity development..."
    
    if ! command_exists forge; then
        log_info "Installing Foundry..."
        curl -L https://foundry.paradigm.xyz | bash
        source ~/.bashrc 2>/dev/null || source ~/.zshrc 2>/dev/null || true
        foundryup
    else
        log_info "Foundry already installed, updating..."
        foundryup
    fi
    
    log_success "Foundry installed"
}

# Install Node.js and npm
install_nodejs() {
    log_info "Installing Node.js and npm..."
    
    if ! command_exists node; then
        case $OS in
            ubuntu)
                curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
                sudo apt-get install -y nodejs
                ;;
            macos)
                brew install node@18
                ;;
            *)
                log_warning "Please install Node.js 18+ manually"
                return
                ;;
        esac
    fi
    
    # Install global npm packages
    npm install -g \
        typescript \
        @types/node \
        prettier \
        eslint
    
    log_success "Node.js and npm installed"
}

# Setup PostgreSQL database
setup_database() {
    log_info "Setting up PostgreSQL database..."
    
    # Start PostgreSQL service
    case $OS in
        ubuntu)
            sudo systemctl start postgresql
            sudo systemctl enable postgresql
            ;;
        macos)
            brew services start postgresql@14
            ;;
    esac
    
    # Create database and user
    log_info "Creating database and user..."
    sudo -u postgres psql -c "CREATE DATABASE arda_credit;" 2>/dev/null || log_info "Database already exists"
    sudo -u postgres psql -c "CREATE USER arda_user WITH PASSWORD 'arda_password';" 2>/dev/null || log_info "User already exists"
    sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE arda_credit TO arda_user;" 2>/dev/null || true
    
    # Create .env file with database configuration
    if [ ! -f .env ]; then
        log_info "Creating .env file..."
        cat > .env << EOF
# Database Configuration
DATABASE_URL=postgresql://arda_user:arda_password@localhost/arda_credit

# API Configuration
API_HOST=0.0.0.0
API_PORT=8080

# JWT Configuration
JWT_SECRET=your-super-secret-jwt-key-change-this-in-production

# OpenTelemetry Configuration
ENABLE_OPENTELEMETRY=false
OTEL_SERVICE_NAME=arda-credit-api
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Ethereum Configuration
ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/your-api-key
PRIVATE_KEY=your-private-key-for-development-only

# SP1 Configuration
SP1_PROVER=local
SINDRI_API_KEY=your-sindri-api-key
EOF
        log_success ".env file created"
    else
        log_info ".env file already exists"
    fi
    
    log_success "PostgreSQL database setup complete"
}

# Setup Docker environment
setup_docker() {
    log_info "Setting up Docker environment..."
    
    # Start Docker service
    case $OS in
        ubuntu)
            sudo systemctl start docker
            sudo systemctl enable docker
            sudo usermod -aG docker $USER
            
            # Start Docker daemon if not running in container environment
            if ! pgrep dockerd > /dev/null; then
                log_info "Starting Docker daemon..."
                sudo dockerd > /tmp/docker.log 2>&1 &
                sleep 5
            fi
            ;;
        macos)
            if ! pgrep -f "Docker Desktop" > /dev/null; then
                log_warning "Please start Docker Desktop manually"
            fi
            ;;
    esac
    
    # Verify Docker is working
    if command_exists docker; then
        docker --version || log_warning "Docker may not be properly configured"
    fi
    
    log_success "Docker setup complete"
}

# Setup local Kubernetes cluster with KIND
setup_kubernetes() {
    log_info "Setting up local Kubernetes cluster with KIND..."
    
    if ! command_exists kind || ! command_exists kubectl; then
        log_warning "KIND or kubectl not installed, skipping Kubernetes setup"
        return
    fi
    
    # Check if Docker is running
    if ! docker info > /dev/null 2>&1; then
        log_warning "Docker is not running, skipping Kubernetes setup"
        return
    fi
    
    # Create KIND cluster if it doesn't exist
    if ! kind get clusters | grep -q "arda-credit"; then
        log_info "Creating KIND cluster 'arda-credit'..."
        
        # Create KIND cluster configuration
        cat > /tmp/kind-config.yaml << 'EOF'
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
name: arda-credit
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
  - containerPort: 8080
    hostPort: 8080
    protocol: TCP
- role: worker
EOF
        
        kind create cluster --config /tmp/kind-config.yaml
        rm /tmp/kind-config.yaml
        
        # Wait for cluster to be ready
        log_info "Waiting for cluster to be ready..."
        kubectl wait --for=condition=Ready nodes --all --timeout=300s
        
        log_success "KIND cluster 'arda-credit' created successfully"
    else
        log_info "KIND cluster 'arda-credit' already exists"
    fi
    
    # Set kubectl context
    kubectl cluster-info --context kind-arda-credit
    
    # Install NGINX Ingress Controller
    log_info "Installing NGINX Ingress Controller..."
    kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
    
    # Wait for ingress controller to be ready
    kubectl wait --namespace ingress-nginx \
        --for=condition=ready pod \
        --selector=app.kubernetes.io/component=controller \
        --timeout=90s
    
    log_success "Kubernetes cluster setup complete"
}

# Build the project
build_project() {
    log_info "Building the Arda Credit project..."
    
    # Run database migrations
    if [ -f .env ]; then
        log_info "Running database migrations..."
        source .env
        sqlx migrate run --database-url "$DATABASE_URL" || log_warning "Migration failed - database may not be ready"
    fi
    
    # Build Rust workspace
    log_info "Building Rust workspace..."
    cargo build --workspace
    
    # Build smart contracts
    if [ -d "contracts" ]; then
        log_info "Building smart contracts..."
        cd contracts
        forge build
        cd ..
    fi
    
    # Install npm dependencies if package.json exists
    if [ -f "package.json" ]; then
        log_info "Installing npm dependencies..."
        npm install
    fi
    
    log_success "Project build complete"
}

# Run tests
run_tests() {
    log_info "Running tests..."
    
    # Run Rust tests
    log_info "Running Rust tests..."
    cargo test --workspace
    
    # Run Foundry tests
    if [ -d "contracts" ]; then
        log_info "Running Foundry tests..."
        cd contracts
        forge test
        cd ..
    fi
    
    log_success "Tests completed"
}

# Setup development tools
setup_dev_tools() {
    log_info "Setting up development tools..."
    
    # Setup git hooks
    if [ -d ".git" ]; then
        log_info "Setting up git hooks..."
        cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
# Pre-commit hook for Arda Credit

echo "Running pre-commit checks..."

# Format Rust code
cargo fmt --all -- --check
if [ $? -ne 0 ]; then
    echo "Code formatting issues found. Run 'cargo fmt' to fix."
    exit 1
fi

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
if [ $? -ne 0 ]; then
    echo "Clippy warnings found. Please fix them."
    exit 1
fi

# Run security audit
cargo audit
if [ $? -ne 0 ]; then
    echo "Security vulnerabilities found. Please address them."
    exit 1
fi

echo "Pre-commit checks passed!"
EOF
        chmod +x .git/hooks/pre-commit
        log_success "Git hooks installed"
    fi
    
    # Setup VS Code configuration
    if [ ! -d ".vscode" ]; then
        mkdir -p .vscode
        cat > .vscode/settings.json << 'EOF'
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.allTargets": false,
    "editor.formatOnSave": true,
    "files.associations": {
        "*.rs": "rust"
    },
    "solidity.defaultCompiler": "localFoundry",
    "solidity.packageDefaultDependenciesContractsDirectory": "contracts/src",
    "solidity.packageDefaultDependenciesDirectory": "contracts/lib"
}
EOF
        
        cat > .vscode/extensions.json << 'EOF'
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "vadimcn.vscode-lldb",
        "serayuzgur.crates",
        "JuanBlanco.solidity",
        "ms-vscode.vscode-typescript-next",
        "esbenp.prettier-vscode",
        "ms-vscode.vscode-json"
    ]
}
EOF
        log_success "VS Code configuration created"
    fi
}

# Main setup function
main() {
    log_info "Starting Arda Credit development environment setup..."
    log_info "This script will install all required dependencies and tools."
    
    # Detect OS
    detect_os
    
    # Ask for confirmation
    read -p "Continue with setup? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Setup cancelled"
        exit 0
    fi
    
    # Run setup steps
    install_system_deps
    install_devops_tools
    install_rust
    install_sp1
    install_foundry
    install_nodejs
    setup_database
    setup_docker
    setup_dev_tools
    build_project
    
    # Optional: setup Kubernetes cluster
    read -p "Setup local Kubernetes cluster with KIND? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        setup_kubernetes
    fi
    
    # Optional: run tests
    read -p "Run tests to verify setup? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        run_tests
    fi
    
    log_success "Arda Credit development environment setup complete!"
    log_info ""
    log_info "🚀 Development Environment Ready!"
    log_info ""
    log_info "Next steps:"
    log_info "1. Restart your shell or run: source ~/.bashrc (or ~/.zshrc)"
    log_info "2. Review and update the .env file with your configuration"
    log_info "3. Start the API server: cargo run --bin server"
    log_info "4. Run the CLI: cargo run --bin cli -- --help"
    log_info ""
    log_info "🛠️  Available Tools:"
    log_info "• Rust toolchain with SP1 zkVM support"
    log_info "• Foundry for smart contract development"
    log_info "• PostgreSQL database with migrations"
    log_info "• Docker and Docker Compose"
    log_info "• kubectl, KIND, and Helm for Kubernetes development"
    log_info "• Node.js and TypeScript for frontend development"
    log_info ""
    log_info "🔧 DevOps Commands:"
    log_info "• Create KIND cluster: kind create cluster --name arda-credit"
    log_info "• Deploy to Kubernetes: kubectl apply -f k8s/"
    log_info "• Manage with Helm: helm install arda-credit ./charts/arda-credit"
    log_info "• Build containers: docker-compose build"
    log_info ""
    log_info "📚 For more information, see:"
    log_info "• Setup guide: .openhands/SETUP.md"
    log_info "• Microagents: .openhands/microagents/README.md"
    log_info "• Project README: README.md"
    
    if [[ $OS == "ubuntu" ]]; then
        log_warning "⚠️  Note: You may need to log out and back in for Docker group membership to take effect"
    fi
}

# Run main function
main "$@"