# Arda Credit - Privacy-Preserving Loan vApp

[![Sindri Circuit CI/CD](https://github.com/ardaglobal/arda-credit/actions/workflows/sindri.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/sindri.yml)
[![Build and Push Docker Image](https://github.com/ardaglobal/arda-credit/actions/workflows/docker.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/docker.yml)
[![Rust CI](https://github.com/ardaglobal/arda-credit/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/rust-ci.yml)
[![Foundry CI/CD](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry.yml)

A privacy-preserving loan platform built on SP1 zero-knowledge proofs, enabling secure loan origination, payments, and closeouts with KYC verification while maintaining transaction privacy through batch processing.

## Overview

**Arda Credit** is a verifiable application (vApp) that demonstrates privacy-preserving financial operations using zero-knowledge proofs. The platform enables:

### Core Features

**👥 User Management & KYC**
- **Investor** and **Lender** account types with KYC verification
- Privacy-preserving status proofs and flexible compliance requirements

**💰 Loan Operations**
- **USDC Deposits** - Direct smart contract deposits with automatic balance tracking
- **USDC Withdrawals** - KYC-validated withdrawals with balance verification
- **Loan Origination** - Create loans between verified parties
- **Payment Processing** - Private payment processing with cryptographic guarantees
- **Loan Closeout** - Final settlement and completion

**🔐 Privacy & Verification**
- **Transaction Inclusion Proofs** - Prove any transaction was processed without revealing others
- **Batch Privacy** - Individual loan details remain hidden in processing
- **Zero-Knowledge State Transitions** - Cryptographically verified loan state changes

## Architecture

This loan vApp demonstrates **privacy-preserving financial operations** with continuous state tracking:

### Privacy Model
```
Private Data (Hidden):
- Individual loan amounts and terms
- Payment amounts and frequencies
- User identities and KYC details
- Interest rates and loan metadata

Public Data (Verifiable):
- State root transitions (Merkle tree)
- Transaction count per batch
- Batch timestamps
- Proof verification results
```

### System Components
- **`program/`** - SP1 zkVM program for loan transaction processing
- **`api/`** - Production server with KYC, loan processing, and ZK proof generation
- **`db/`** - PostgreSQL with users, loans, KYC data, and Merkle tree state
- **`contracts/`** - Solidity contracts for on-chain loan state verification
- **`lib/`** - Shared loan computation logic (zkVM compatible)

## Requirements

- [Rust](https://rustup.rs/) toolchain
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install) zkVM
- [Foundry](https://book.getfoundry.sh/getting-started/installation) for smart contracts
- [Docker](https://docs.docker.com/get-docker/) for database
- [Node.js](https://nodejs.org/) for Sindri CLI

## Quick Start

**🚀 Complete Setup**: See [`docs/deployment/development-setup.md`](docs/deployment/development-setup.md) for detailed setup instructions.

**💡 Test the Complete Workflow**: See [`AGENTS.md#deal-workflow-examples`](AGENTS.md#deal-workflow-examples) for comprehensive API workflow examples.

**🔧 Development Commands**:
```bash
make help          # Show all available commands
make setup         # Complete setup from scratch
make server        # Start API server locally
```

## MCP Proxy Integration

Arda Credit includes an **MCP (Model Context Protocol) Proxy** for AI-powered document analysis and insights.

### Quick Start

```bash
# Start the MCP proxy service
docker-compose up -d mcp-proxy

# Check health
curl http://localhost:8000/health

# View logs
docker-compose logs -f mcp-proxy

# Stop the service
docker-compose stop mcp-proxy
```

### Configuration

The MCP proxy is configured via environment variables in `.env`:

```bash
# MCP Server URL (upstream MCP server)
MCP_SERVER_URL=https://arda-insights.fastmcp.app/mcp

# API Keys for authentication (comma-separated)
MCP_PROXY_API_KEYS=your-secure-key-1,your-secure-key-2

# CORS Origins (comma-separated)
MCP_CORS_ORIGINS=http://localhost:3000,http://localhost:5173,http://localhost:8080

# Optional: Timeout and logging
MCP_TIMEOUT=30.0
MCP_DEBUG=false
MCP_LOG_LEVEL=INFO
```

**Security Note**: Change default API keys in production! See `.env.example` for complete configuration options.

### Service Details

- **Port**: 8000
- **Health Check**: `GET http://localhost:8000/health`
- **Upstream**: `https://arda-insights.fastmcp.app/mcp`
- **Container**: `fastmcp_proxy`
- **Image**: `ghcr.io/ardaglobal/fastmcp-proxy:latest`

The MCP proxy enables:
- AI-powered document analysis via InfraNodus integration
- Knowledge graph generation from text
- Content gap analysis and research question generation
- Integration with Claude Code and other MCP-compatible tools

## Privacy Guarantees

### What Stays Private
- **Individual loan amounts** - Hidden in batch processing
- **User identities** - Protected through zero-knowledge proofs
- **Payment details** - Amounts and frequencies remain confidential
- **KYC information** - Status provable without revealing identity

### What Can Be Verified
- **Transaction inclusion** - Prove specific transactions were processed
- **State transitions** - Cryptographically verify loan state changes
- **KYC compliance** - Prove KYC requirements were met
- **Payment completion** - Verify payments without revealing amounts

## Authentication

Arda Credit uses **magic link authentication** for secure, passwordless access.

### Quick Start

To test the magic link authentication flow, you can use the following commands:

```bash
./create_user_and_magic_link.sh user@example.com
```

More details can be found in [`scripts/create-user-magic-link/README.md`](scripts/create-user-magic-link/README.md).

### Configuration

Required environment variables:
```bash
# AWS SES (Email Service)
SES_REGION=us-east-1
EMAIL_FROM=auth@arda.xyz

# Magic Link Settings
MAGIC_BASE_URL=https://app.arda.xyz/auth/verify
MAGIC_LINK_TTL_MINUTES=10

# Session Management
SESSION_EXPIRY_DAYS=30
```

See [`MAGIC_LINK_ENV_VARS.md`](MAGIC_LINK_ENV_VARS.md) for complete configuration.

## Development

See [`CONTRIBUTORS.md`](CONTRIBUTORS.md) for detailed development guidelines.

### 🎲 Test Data Population

To populate your database with realistic test data for development and testing:

```bash
# Start the database
make up

# Populate with comprehensive test dataset
cargo run -p db --bin populate_fake_data
```

This creates:
- **200 users**: 67 investors, 66 originators across 9 boroughs, 67 additional investors
- **20 liquidity pools**: $1M-$10M each with borough percentage limits
- **40 credit requests**: Revenue-based amounts from originators in different boroughs
- **Geographic distribution**: Manhattan, Brooklyn, Queens, Bronx, Staten Island, Long Island, Westchester, New Jersey, Connecticut
- **Borough limits**: Realistic percentage thresholds for risk diversification testing

Perfect for testing the borough percentage system, credit line requests, and pool management features.

### 🌐 Local Service URLs

When running locally via `docker-compose up` and `make server`, the following services are available:

| Service | URL | Description |
|---------|-----|-------------|
| **API Server** | http://0.0.0.0:8080 | Main REST API |
| **Swagger UI** | http://0.0.0.0:8080/swagger-ui/ | Interactive API documentation |
| **Health Check** | http://0.0.0.0:8080/api/v1/health | Service health status |
| **MCP Proxy** | http://localhost:8000 | MCP protocol proxy |
| **MCP Health** | http://localhost:8000/health | MCP proxy health check |
| **PostgreSQL** | localhost:5432 | Database (postgres/password) |
| **Prometheus** | http://localhost:9090 | Metrics collection |
| **Grafana** | http://localhost:3100 | Metrics visualization (admin/admin) |
| **Anvil** | http://localhost:8545 | Local Ethereum devnet (local/staging only) |

## Documentation

### 📚 Getting Started
- **[`docs/README.md`](./docs/README.md)** - Complete documentation hub with navigation
- **[`docs/deployment/development-setup.md`](./docs/deployment/development-setup.md)** - Development environment setup
- **[`AGENTS.md`](./AGENTS.md)** - Main entry point for AI Agents

### 🏗️ Architecture & Design
- **[`docs/architecture/three-component-overview.md`](./docs/architecture/three-component-overview.md)** - System architecture overview
- **[`docs/architecture/loan-system.md`](./docs/architecture/loan-system.md)** - Technical loan system details
- **[`docs/architecture/performance-targets.md`](./docs/architecture/performance-targets.md)** - Performance and capacity planning

### 🔧 Development & Operations
- **[`docs/integrations/magic-link-authentication.md`](./docs/integrations/magic-link-authentication.md)** - Magic link authentication integration guide
- **[`docs/integrations/api-swagger.md`](./docs/integrations/api-swagger.md)** - API documentation and OpenAPI specs
- **[`docs/development/local-testing.md`](./docs/development/local-testing.md)** - Local testing with GitHub Actions
- **[`docs/deployment/security-production.md`](./docs/deployment/security-production.md)** - Production deployment and security
- **[`docs/deployment/monitoring.md`](./docs/deployment/monitoring.md)** - OpenTelemetry monitoring setup

### 🏗️ Component Documentation
- **[`api/src/README.md`](./api/src/README.md)** - API server architecture and standards
- **[`db/README.md`](./db/README.md)** - Database layer and ADS integration
- **[`contracts/README.md`](./contracts/README.md)** - Smart contract deployment and testing
- **Swagger UI** - Interactive API documentation at http://0.0.0.0:8080/swagger-ui/

## Technology Stack

**Built on SP1 zkVM** - Uses Succinct's SP1 zero-knowledge virtual machine for:
- **RISC-V Compatibility** - Standard Rust code compiles to zkVM
- **Groth16 Proofs** - EVM-compatible proofs for on-chain verification
- **Sindri Integration** - Serverless proof generation at scale

**Merkle Tree State Management** - 32-level indexed trees provide:
- **Efficient Updates** - O(log n) operations with atomic transactions
- **Privacy-Preserving Commitments** - State roots hide individual transactions

## Security Model

### Trust Assumptions
- **SP1 zkVM Security** - Relies on SP1's cryptographic guarantees
- **Sindri Infrastructure** - Trusted for proof generation (verifiable outputs)
- **Smart Contract Security** - On-chain verification provides final settlement

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details.

## Status

**Current Version**: v0.0.1 (MVP Development)

This is a Minimum Viable Product (MVP) demonstrating privacy-preserving loan operations. The KYC process can be mocked for testing, but provides the foundation for production KYC integration.

