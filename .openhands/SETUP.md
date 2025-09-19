# Arda Credit Development Environment Setup

This document explains the automated setup process for the Arda Credit development environment using the provided `setup.sh` script.

## Quick Start

```bash
# Clone the repository
git clone https://github.com/ardaglobal/arda-credit.git
cd arda-credit

# Run the setup script
./.openhands/setup.sh
```

## What the Setup Script Does

The `setup.sh` script provides a comprehensive, automated setup for the entire Arda Credit development stack:

### 1. System Dependencies
- **Build Tools**: GCC, Make, pkg-config for compiling native dependencies
- **SSL Libraries**: OpenSSL development headers for cryptographic operations
- **Database**: PostgreSQL server and client libraries
- **Containerization**: Docker and Docker Compose for deployment
- **Utilities**: curl, wget, git, jq for various development tasks

### 2. DevOps and Kubernetes Tools
- **kubectl**: Kubernetes command-line tool for cluster management
- **KIND**: Kubernetes IN Docker for local cluster development
- **Helm**: Kubernetes package manager for application deployment
- **Docker**: Container runtime and image management
- **Container Orchestration**: Complete local Kubernetes development environment

### 3. Rust Ecosystem
- **Rust Toolchain**: Latest stable Rust via rustup
- **Components**: clippy (linting), rustfmt (formatting)
- **Tools**: 
  - `cargo-audit` - Security vulnerability scanning
  - `cargo-deny` - Dependency policy enforcement
  - `sqlx-cli` - Database migration and query management
  - `cargo-watch` - File watching for development
  - `cargo-expand` - Macro expansion for debugging

### 4. Zero-Knowledge Proof Stack
- **SP1 zkVM**: Complete SP1 toolchain for zero-knowledge proof generation
- **Prover Tools**: `cargo-prove` for SP1 program compilation and proving
- **Dependencies**: All required libraries for zkVM operations

### 5. Blockchain Development
- **Foundry**: Complete Ethereum development toolkit
  - `forge` - Smart contract compilation and testing
  - `cast` - Ethereum RPC interaction
  - `anvil` - Local Ethereum node for testing
- **Solidity Support**: Full Solidity compilation and testing environment

### 6. Frontend Development
- **Node.js**: Version 18+ for frontend tooling
- **TypeScript**: Type-safe JavaScript development
- **Development Tools**: Prettier, ESLint for code quality

### 7. Database Setup
- **PostgreSQL Configuration**: Automated database and user creation
- **Environment Variables**: Pre-configured `.env` file with sensible defaults
- **Migration Support**: SQLx migration system setup

### 8. Development Tools
- **Git Hooks**: Pre-commit hooks for code quality enforcement
- **VS Code Configuration**: Optimized settings and recommended extensions
- **Docker Environment**: Container orchestration setup

## Supported Platforms

### Ubuntu/Debian
- Uses `apt-get` for system package management
- Installs PostgreSQL via system packages
- Configures systemd services

### macOS
- Uses Homebrew for package management
- Installs PostgreSQL@14 via Homebrew
- Configures Homebrew services

### Other Linux Distributions
- Provides manual installation guidance
- Supports Arch Linux (pacman) and CentOS (yum) detection
- Falls back to manual instructions for unsupported distributions

## Environment Configuration

The script creates a comprehensive `.env` file with all necessary configuration:

```bash
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
```

## Post-Setup Configuration

After running the setup script, you'll need to:

### 1. Update Environment Variables
Edit the `.env` file to include your actual API keys and configuration:
- **Ethereum RPC URL**: Replace with your Alchemy/Infura endpoint
- **Private Key**: Use a development-only private key
- **JWT Secret**: Generate a secure random string for production
- **Sindri API Key**: Add your Sindri API key for proof generation

### 2. Database Migrations
Run database migrations to set up the schema:
```bash
sqlx migrate run
```

### 3. Verify Installation
Test that all components are working:
```bash
# Test Rust compilation
cargo check --workspace

# Test SP1 zkVM
cd program && cargo prove build

# Test smart contracts
cd contracts && forge test

# Test DevOps tools
kubectl version --client
kind version
helm version

# Test API server
cargo run --bin server
```

### 4. Optional: Setup Local Kubernetes Cluster
The setup script can optionally create a local Kubernetes cluster:
```bash
# Create KIND cluster with ingress support
kind create cluster --name arda-credit --config .openhands/kind-config.yaml

# Verify cluster is running
kubectl cluster-info --context kind-arda-credit

# Deploy applications using Helm
helm install arda-credit ./charts/arda-credit
```

## Development Workflow

The setup script configures several development workflow enhancements:

### Pre-commit Hooks
Automatically runs before each commit:
- **Code Formatting**: `cargo fmt` ensures consistent code style
- **Linting**: `clippy` catches common mistakes and suggests improvements
- **Security Audit**: `cargo audit` checks for known vulnerabilities

### VS Code Integration
Optimized VS Code configuration includes:
- **Rust Analyzer**: Advanced Rust language support
- **Solidity Support**: Smart contract development tools
- **Debugging**: LLDB integration for Rust debugging
- **Extensions**: Recommended extensions for the full development stack

### Docker Development
Docker configuration for:
- **Local Development**: Consistent development environment
- **Testing**: Isolated testing environment
- **Deployment**: Production-ready containerization

## Troubleshooting

### Common Issues

**SP1 Installation Fails**
```bash
# Manually install SP1
curl -L https://sp1.succinct.xyz | bash
source ~/.bashrc  # or ~/.zshrc
sp1up
```

**PostgreSQL Connection Issues**
```bash
# Check PostgreSQL status
sudo systemctl status postgresql  # Ubuntu
brew services list | grep postgresql  # macOS

# Reset database
sudo -u postgres dropdb arda_credit
sudo -u postgres createdb arda_credit
```

**Docker Permission Issues (Ubuntu)**
```bash
# Add user to docker group
sudo usermod -aG docker $USER
# Log out and back in, or run:
newgrp docker
```

### Verification Commands

Verify each component is working:
```bash
# Rust toolchain
rustc --version
cargo --version

# SP1 zkVM
cargo prove --version

# Foundry
forge --version

# DevOps tools
kubectl version --client
kind version
helm version

# Node.js
node --version
npm --version

# Database
psql -h localhost -U arda_user -d arda_credit -c "SELECT version();"

# Docker
docker --version
docker-compose --version

# Kubernetes cluster (if created)
kubectl get nodes --context kind-arda-credit
```

## Security Considerations

The setup script follows security best practices:

### Development vs Production
- **Default Passwords**: Uses development-only passwords that must be changed for production
- **Local Services**: Configures services for local development only
- **Environment Separation**: Clear separation between development and production configuration

### Dependency Management
- **Verified Sources**: All tools installed from official sources
- **Version Pinning**: Uses specific versions where possible
- **Security Scanning**: Includes automated security vulnerability scanning

### Access Control
- **Database Users**: Creates dedicated database user with minimal privileges
- **File Permissions**: Sets appropriate permissions on configuration files
- **Service Isolation**: Configures services with appropriate isolation

## Integration with Microagents

The setup script is designed to work seamlessly with the distributed microagent system:

### DevOps Integration
- Supports the **DevOps and Infrastructure Specialist** with complete CI/CD setup
- Provides monitoring and observability tools configuration
- Includes Docker and Kubernetes development support

### Security Integration  
- Supports the **API Security and Authentication Specialist** with security tools
- Includes automated security scanning and vulnerability detection
- Provides secure development environment configuration

### Database Integration
- Supports the **Database and Performance Optimization Specialist** with PostgreSQL setup
- Includes SQLx CLI for database management and migrations
- Provides performance monitoring and optimization tools

This comprehensive setup ensures that developers can quickly get started with Arda Credit development while maintaining the high standards of security, performance, and code quality expected in a privacy-preserving financial system.