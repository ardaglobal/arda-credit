# Three-Component Architecture Overview

This document explains the three-component structure of Arda Credit, a privacy-preserving loan verifiable application (vApp) built on SP1 zero-knowledge proofs. The system is designed with clean separation of concerns across three independent but coordinated components.

## Overview

Arda Credit implements a **three-component architecture** where each component serves distinct purposes but work together to enable privacy-preserving financial operations:

1. **Smart Contracts** (`contracts/`) - On-chain proof verification and state management
2. **ZKVM Program** (`program/`) - Zero-knowledge transaction processing
3. **Web Server Application** (Main Repository) - User interface, API, and orchestration

## Smart Contracts (`contracts/`)

### Location and Dependencies
- **Location**: `contracts/src/Loan.sol` and related Solidity contracts
- **Dependencies**: SP1 contracts, OpenZeppelin libraries
- **Deployment Target**: Ethereum networks (mainnet, testnets)
- **Build System**: Foundry (Forge)

### Purpose and Function
The smart contracts component handles:
- **SP1 Proof Verification** - Validates zero-knowledge proofs on-chain
- **Loan State Storage** - Maintains verified loan state transitions
- **Automated Proof Posting** - Background processes submit proven batches to Ethereum
- **Gas-Optimized Operations** - Efficient on-chain loan state management

### Versioning Strategy
- **Independent Semantic Versioning**: `contracts-vX.Y.Z` format
- **MAJOR**: Breaking changes to contract interface or state structure
- **MINOR**: New features, additional functions, backward-compatible changes
- **PATCH**: Bug fixes, gas optimizations, security patches

### Release Artifacts
- **Compiled Contracts**: Bytecode and ABI files
- **Deployment Addresses**: Contract addresses on target networks
- **Verification Data**: Etherscan verification records

## ZKVM Program (`program/`)

### Location and Dependencies
- **Location**: `program/src/main.rs`
- **Dependencies**: SP1 ZKVM runtime (`sp1-zkvm = "5.2.1"`)
- **Build Target**: SP1 ZKVM ELF binary
- **Runtime Environment**: RISC-V compatible execution

### Purpose and Function
The ZKVM program component processes:
- **Loan Transaction Logic** - Core business logic for loan operations
- **Privacy-Preserving Computations** - Zero-knowledge state transitions
- **Merkle Tree Operations** - Indexed Merkle tree state management
- **Batch Processing** - Multiple transactions proven together

### Versioning Strategy
- **Independent Semantic Versioning**: `program-vX.Y.Z` format
- **MAJOR**: Changes to program logic that affect verification key or public inputs/outputs
- **MINOR**: Performance improvements, additional features that don't change verification
- **PATCH**: Bug fixes, optimizations that don't affect verification

### Release Artifacts
- **Program Verification Key** (vkey): Cryptographic key for proof verification
- **ELF Binary**: Compiled RISC-V program for zkVM execution
- **Sindri Circuit**: Deployed circuit with matching tag (e.g., `program-v2.1.0`)

## Web Server Application (Main Repository)

### Location and Dependencies
- **Location**: Root workspace with `api/`, `db/`, `script/`, `lib/`, etc.
- **Dependencies**: Both smart contracts and ZKVM program components
- **Deployment Target**: Kubernetes via Helm charts
- **Build System**: Cargo workspace with API server and supporting tools

### Purpose and Function
The web server application component orchestrates:
- **User Management** - Registration, KYC verification, and account management
- **Loan Operations** - Origination, payments, closeouts, and withdrawals
- **Transaction Batching** - FIFO transaction processing with ZK proof generation
- **API Services** - REST and GraphQL endpoints for client interactions
- **Database Operations** - PostgreSQL with loan tables and Merkle tree state
- **Background Processing** - Automated proof generation and contract posting

### Versioning Strategy
- **Main Repository Version**: `vX.Y.Z` format that references specific versions of other components
- **MAJOR**: Breaking changes to API, database schema, or deployment requirements
- **MINOR**: New features, API enhancements, backward-compatible changes
- **PATCH**: Bug fixes, dependency updates, minor improvements

### Release Artifacts
- **Docker Images**: Multi-platform container images (`ghcr.io/ardaglobal/arda-credit:v3.4.5`)
- **Helm Charts**: Kubernetes deployment configurations
- **Database Migrations**: SQL schema updates for PostgreSQL

## Component Integration and Data Flow

### System Architecture Overview
```
User Interface (CLI/API)
        ↓
Web Server Application
    ├── Database (PostgreSQL)
    ├── Transaction Batching
    └── ZK Proof Generation (Sindri)
        ↓
ZKVM Program Execution
    ├── Privacy-Preserving Processing
    ├── State Transitions
    └── Proof Generation
        ↓
Smart Contract Verification
    ├── On-Chain Proof Validation
    ├── State Root Updates
    └── Settlement Execution
```

### Data Flow Patterns

#### Loan Origination Flow
1. **User Request** → Web Server API validates KYC and balance
2. **Transaction Queue** → Database stores pending loan transaction
3. **Batch Processing** → Web server batches transactions for ZK processing
4. **ZKVM Execution** → Program processes loan with privacy guarantees
5. **Proof Generation** → Sindri generates verifiable proof
6. **Contract Posting** → Smart contract verifies proof and updates state

#### USDC Withdrawal Flow
1. **Withdrawal Request** → Web server validates KYC and balance
2. **Balance Check** → Database verifies sufficient funds
3. **Transaction Processing** → ZKVM program validates withdrawal
4. **Proof Generation** → Batch includes withdrawal in privacy-preserving proof
5. **Smart Contract** → Executes USDC transfer with verified proof

### Communication Patterns

#### Synchronous Communication
- **API Requests** → Web server handles user interactions
- **Database Queries** → Direct PostgreSQL operations for state management
- **Contract Calls** → Direct Ethereum interactions for immediate operations

#### Asynchronous Communication
- **Batch Processing** → Background jobs process transaction batches
- **Proof Generation** → Sindri integration for ZK proof creation
- **Contract Posting** → Automated background processes for on-chain updates

#### Component Dependencies
- **Web Server** depends on ZKVM program for transaction processing
- **Web Server** depends on smart contracts for on-chain verification
- **ZKVM Program** depends on smart contracts for vkey consistency
- **Smart Contracts** depend on ZKVM program for proof validation

## Release Coordination

### Version Compatibility Matrix
The three components maintain coordinated versioning while allowing independent releases:

| Main App Version | Contract Version | Program Version | SP1 SDK Version |
|------------------|------------------|-----------------|-----------------|
| v1.0.0          | contracts-v1.0.0-rc2 | program-v1.0.0  | 5.2.1          |
| v1.1.0          | contracts-v1.0.0-rc2 | program-v1.1.0  | 5.2.1          |
| v1.2.0          | contracts-v1.1.0-rc1 | program-v1.1.0  | 5.2.1          |

### Release Process Coordination
1. **Component Releases** - Each component can be released independently
2. **Version Locking** - Main application locks to specific component versions
3. **Helm Integration** - Chart values reference locked component versions
4. **Environment Variables** - Runtime configuration injects versioned artifacts

### Configuration Management
```yaml
# Helm chart values with version locking
server:
  config:
    sindriCircuitTag: "program-v2.1.0"  # Locked to specific circuit
    contractAddress: "0x1234567890123456789012345678901234567890"

secrets:
  app:
    contractAddress: "0x1234567890123456789012345678901234567890"
    programVkey: "0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e"
```

## Development Workflow

### Component Development
- **Smart Contracts**: Local Foundry development with Anvil testing
- **ZKVM Program**: Fast SP1 Core proofs for local development
- **Web Server**: Full-stack development with Docker Compose

### Integration Testing
- **Unit Tests**: Component-specific test suites
- **Integration Tests**: Cross-component functionality validation
- **End-to-End Tests**: Complete workflow verification

### Deployment Strategy
- **Staged Rollouts**: Independent component deployment and testing
- **Blue-Green**: Main application deployment with version locking
- **Rollback Support**: Independent component rollback capabilities

This three-component architecture provides the foundation for Arda Credit's privacy-preserving financial operations while maintaining clean separation of concerns and enabling independent component evolution.
