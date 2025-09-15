# Arda Credit - Privacy-Preserving Loan vApp

[![Sindri Circuit CI/CD](https://github.com/ardaglobal/arda-credit/actions/workflows/sindri.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/sindri.yml)
[![Build and Push Docker Image](https://github.com/ardaglobal/arda-credit/actions/workflows/docker.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/docker.yml)
[![Build Program](https://github.com/ardaglobal/arda-credit/actions/workflows/prove.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/prove.yml)
[![Deploy Smart Contracts](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry-deploy.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry-deploy.yml)
[![Foundry Test](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry-test.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/foundry-test.yml)
[![Test](https://github.com/ardaglobal/arda-credit/actions/workflows/test.yml/badge.svg)](https://github.com/ardaglobal/arda-credit/actions/workflows/test.yml)

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
- **`cli/`** - Complete loan workflow interface + trustless verification
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

**💡 Test the Complete Workflow**: See [`AGENTS.md#loan-workflow-examples`](AGENTS.md#loan-workflow-examples) for comprehensive CLI workflow examples.

**🔧 Development Commands**:
```bash
make help          # Show all available commands
make setup         # Complete setup from scratch
make up           # Start services
make test         # Run all tests
make forge-test   # Test smart contracts
```

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

## Development

### Getting Started
- **Setup**: [`docs/deployment/development-setup.md`](docs/deployment/development-setup.md)
- **CLI Commands**: [`AGENTS.md#essential-development-commands`](AGENTS.md#essential-development-commands)
- **Local Testing**: [`docs/development/local-testing.md`](docs/development/local-testing.md)

### Quick Commands
```bash
make help          # Show all available commands
make setup         # Complete setup from scratch
make up           # Start services
make test         # Run all tests
make forge-test   # Test smart contracts
```

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
- **[`docs/development/local-testing.md`](./docs/development/local-testing.md)** - Local testing with GitHub Actions
- **[`docs/deployment/security-production.md`](./docs/deployment/security-production.md)** - Production deployment and security
- **[`docs/deployment/monitoring.md`](./docs/deployment/monitoring.md)** - OpenTelemetry monitoring setup

### 🏗️ Component Documentation
- **[`api/src/README.md`](./api/src/README.md)** - API server architecture and standards
- **[`cli/README.md`](./cli/README.md)** - CLI tool structure and conventions
- **[`db/README.md`](./db/README.md)** - Database layer and ADS integration
- **[`contracts/README.md`](./contracts/README.md)** - Smart contract deployment and testing

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

---

**🏗️ Built with SP1** | **🔐 Privacy by Design** | **⚡ Production Ready**
