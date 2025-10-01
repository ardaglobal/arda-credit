# Arda Credit Development Environment Setup

This document explains how to set up your local development environment for the Arda Credit project.

## Quick Start

```bash
# Clone the repository
git clone https://github.com/ardaglobal/arda-credit.git
cd arda-credit

# Run the automated setup
make setup
```

## What the Setup Does

The `make setup` command provides a streamlined setup for the Arda Credit development environment:

1. **Install Dependencies**: Installs required system dependencies and tools
2. **Environment Configuration**: Creates a `.env` file from the template
3. **Database Setup**: Initializes the PostgreSQL database and runs migrations
4. **Git Hooks**: Sets up pre-commit hooks for code quality

## Prerequisites

Before running `make setup`, ensure you have the following installed on your system:

### Required Tools

- **Rust**: Version 1.70+ (install via [rustup](https://rustup.rs/))
- **Docker**: For running PostgreSQL and containerized services
- **Docker Compose**: For orchestrating local services
- **Node.js**: Version 18+ for frontend tooling (if developing frontend)

### Platform-Specific Installation

#### Ubuntu/Debian
```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo apt-get install docker-compose

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### macOS
```bash
# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Docker Desktop
brew install --cask docker

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Optional Tools (for full development)

- **SP1 zkVM**: For zero-knowledge proof development
  ```bash
  curl -L https://sp1.succinct.xyz | bash
  sp1up
  ```

- **Foundry**: For smart contract development
  ```bash
  curl -L https://foundry.paradigm.xyz | bash
  foundryup
  ```

- **kubectl, KIND, Helm**: For Kubernetes development
  ```bash
  # Install via package manager or make install
  make install
  ```

## Detailed Setup Steps

### 1. Install Dependencies

```bash
make install
```

This installs:
- Kubernetes tools (kubectl, KIND, Helm)
- DevOps utilities required for deployment

### 2. Configure Environment

```bash
make env
```

This creates a `.env` file from `.env.example`. Edit the file to add your configuration:
- **SINDRI_API_KEY**: Your Sindri API key for SP1 proving
- **Database credentials**: Default values work for local development
- **API configuration**: Host and port settings
- **OpenTelemetry**: Tracing and monitoring configuration

### 3. Initialize Database

```bash
make initDB
```

This will:
- Start the PostgreSQL database via Docker Compose
- Run all database migrations
- Generate SQLx metadata cache

### 4. Setup Git Hooks

```bash
make setup-hooks
```

This configures pre-commit hooks that run:
- Code formatting checks (`cargo fmt`)
- Linting (`cargo clippy`)
- Security audits (`cargo audit`)

### 5. Start Development

```bash
# Start all background services
make up

# Run the API server
make server
```

The API server will be available at http://localhost:8080

## Development Workflow

### Daily Development
1. `make up` - Starts database and required services
2. `make server` - Runs the API server with debug logging
3. Make your changes
4. Test with `curl` or your frontend
5. Run tests: `make test`

### Available Make Commands

Run `make help` to see all available commands:

```bash
make help          # Show all available commands
make setup         # Complete setup from scratch
make server        # Start API server locally
make test          # Run all tests
make check         # Run type checking and linting
make fmt           # Format code
make clean         # Clean build artifacts
```

## Component Setup

### Rust Ecosystem
- **Rust Toolchain**: Latest stable Rust via rustup
- **Components**: clippy (linting), rustfmt (formatting)
- **Tools**: 
  - `cargo-audit` - Security vulnerability scanning
  - `cargo-deny` - Dependency policy enforcement
  - `sqlx-cli` - Database migration and query management
  - `cargo-watch` - File watching for development

### Zero-Knowledge Proof Stack
- **SP1 zkVM**: Complete SP1 toolchain for zero-knowledge proof generation
- **Prover Tools**: `cargo-prove` for SP1 program compilation and proving
- **Configuration**: Set `SP1_PROVER=local` in `.env` for local proving

### Blockchain Development
- **Foundry**: Complete Ethereum development toolkit
  - `forge` - Smart contract compilation and testing
  - `cast` - Ethereum RPC interaction
  - `anvil` - Local Ethereum node for testing

### Database
- **PostgreSQL**: Runs in Docker with persistent storage
- **Migrations**: SQLx-based migration system
- **Connection**: Default connection string in `.env`

## Services Overview

| Service | Port | Purpose | URL |
|---------|------|---------|-----|
| Arda Credit API | 8080 | Main application | http://localhost:8080 |
| PostgreSQL | 5432 | Database | localhost:5432 |

## Testing and Verification

### Verify Installation

Test that all components are working:

```bash
# Test Rust compilation
cargo check --workspace

# Test SP1 zkVM (if installed)
cd program && cargo prove build

# Test smart contracts (if Foundry is installed)
cd contracts && forge test

# Test DevOps tools (if installed)
kubectl version --client
kind version
helm version

# Test API server
make server
```

### Run Tests

```bash
# Run all tests
make test

# Run specific tests
cargo test --package api
cargo test --package db
```

## Kubernetes Development

For local Kubernetes development and testing:

```bash
# Setup local KIND cluster
make kind-setup

# Deploy to local cluster
make kind-deploy

# Check cluster status
make kind-status

# Cleanup cluster
make kind-cleanup
```

## Troubleshooting

### Database Connection Issues
```bash
# Check if PostgreSQL is running
docker-compose ps postgres

# Check database logs
docker logs arda_credit_postgres

# Restart database
docker-compose restart postgres
```

### Docker Permission Issues (Ubuntu)
```bash
# Add user to docker group
sudo usermod -aG docker $USER
# Log out and back in, or run:
newgrp docker
```

### Port Conflicts
```bash
# Check what's using port 8080 (API)
lsof -i :8080

# Check what's using port 5432 (PostgreSQL)
lsof -i :5432
```

### SQLx Offline Mode
If you don't have a database connection, you can use SQLx offline mode:
```bash
# Generate SQLx metadata
make sqlx-prepare

# Build with offline mode
SQLX_OFFLINE=true cargo build
```

## Security Considerations

### Development vs Production
- **Default Passwords**: Development passwords must be changed for production
- **Local Services**: Configured for local development only
- **Environment Separation**: Clear separation between dev and production config

### Access Control
- **Database Users**: Uses dedicated database user
- **Service Isolation**: Proper service isolation in Docker
- **Secrets Management**: Never commit `.env` to version control

## Integration with Microagents

The development environment is designed to work with the distributed microagent system located in `.openhands/microagents/`:

- **API Security Specialist**: Security tools and automated scanning
- **Database Specialist**: PostgreSQL setup and performance tools
- **DevOps Specialist**: CI/CD, monitoring, and Kubernetes support
- **zkVM Specialist**: SP1 development and proof generation
- **Blockchain Specialist**: Ethereum and smart contract tools

For more information on microagents, see [`.openhands/microagents/README.md`](./microagents/README.md)

## Additional Resources

- **Development Setup**: [`docs/deployment/development-setup.md`](../docs/deployment/development-setup.md)
- **Production Deployment**: [`docs/deployment/security-production.md`](../docs/deployment/security-production.md)
- **Testing Guide**: [`docs/development/local-testing.md`](../docs/development/local-testing.md)
- **Architecture**: [`docs/architecture/`](../docs/architecture/)
- **Main README**: [`README.md`](../README.md)
- **Agent Guide**: [`AGENTS.md`](../AGENTS.md)

## Quick Reference

```bash
# Complete setup from scratch
make setup

# Daily development
make up && make server

# Run tests
make test

# Format and lint
make fmt && make check

# View all commands
make help
```

This streamlined setup ensures developers can quickly start working on Arda Credit while maintaining high standards of security, performance, and code quality.
