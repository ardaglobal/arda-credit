# Arda Credit Distributed Microagent System

This directory contains a comprehensive distributed microagent system designed specifically for the Arda Credit privacy-preserving loan vApp. Each microagent provides deep domain expertise while working together to maintain system-wide quality, security, and performance.

## Architecture Overview

The system consists of **7 specialized microagents** that cover all major domains of the Arda Credit codebase:

```
┌─────────────────────────────────────────────────────────────┐
│                 Distributed Agent Coordinator               │
│                    (System-wide coordination)               │
└─────────────────────────┬───────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   ZK/Crypto │  │  Financial  │  │ Blockchain  │
│ Specialist  │  │ Compliance  │  │  Ethereum   │
│             │  │ Specialist  │  │ Specialist  │
└─────────────┘  └─────────────┘  └─────────────┘
        │                 │                 │
        └─────────────────┼─────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  Database   │  │ API Security│  │   DevOps    │
│Performance  │  │    Auth     │  │Infrastructure│
│ Specialist  │  │ Specialist  │  │ Specialist  │
└─────────────┘  └─────────────┘  └─────────────┘
```

## Microagent Specifications

### 1. [Zero-Knowledge Proof and Cryptography Specialist](zkvm-cryptography-specialist.md)
- **Domain**: SP1 zkVM, zero-knowledge proofs, cryptographic operations
- **Triggers**: `zkvm`, `sp1`, `proof`, `zero-knowledge`, `cryptography`, `sindri`, `groth16`
- **Coverage**: 60+ files with zkVM/proof logic, Merkle trees, privacy-preserving operations
- **Key Responsibilities**: Proof generation optimization, cryptographic security, privacy guarantees

### 2. [Financial Operations and Compliance Specialist](financial-compliance-specialist.md)
- **Domain**: Loan processing, KYC compliance, regulatory requirements
- **Triggers**: `loan`, `kyc`, `financial`, `compliance`, `usdc`, `deposit`, `withdrawal`, `credit`
- **Coverage**: Financial handlers, loan processors, compliance services, liquidity pools
- **Key Responsibilities**: Financial precision, regulatory compliance, audit trails

### 3. [Blockchain and Ethereum Integration Specialist](blockchain-ethereum-specialist.md)
- **Domain**: Smart contracts, Ethereum client, on-chain operations
- **Triggers**: `ethereum`, `blockchain`, `smart contract`, `solidity`, `alloy`, `foundry`, `usdc`, `evm`
- **Coverage**: 441 Solidity files, Ethereum client (2,072 lines), contract integration
- **Key Responsibilities**: Smart contract security, state synchronization, gas optimization

### 4. [Database and Performance Optimization Specialist](database-performance-specialist.md)
- **Domain**: PostgreSQL operations, performance optimization, ADS management
- **Triggers**: `database`, `postgresql`, `sqlx`, `performance`, `merkle`, `ads`, `indexing`, `query`
- **Coverage**: Database layer (6,220+ lines), Merkle trees, 706 SQLx references
- **Key Responsibilities**: Query optimization, ACID compliance, large file refactoring

### 5. [API Security and Authentication Specialist](api-security-specialist.md)
- **Domain**: API security, authentication, authorization, input validation
- **Triggers**: `api`, `security`, `authentication`, `jwt`, `argon2`, `validation`, `middleware`, `axum`
- **Coverage**: 15+ API handler files, authentication system, security middleware
- **Key Responsibilities**: Secure authentication, input validation, authorization

### 6. [DevOps and Infrastructure Specialist](devops-infrastructure-specialist.md)
- **Domain**: CI/CD pipelines, infrastructure management, deployment automation
- **Triggers**: `devops`, `ci/cd`, `docker`, `kubernetes`, `github actions`, `deployment`, `monitoring`
- **Coverage**: GitHub Actions workflows, Docker configs, Kubernetes manifests
- **Key Responsibilities**: Build optimization, deployment automation, monitoring

### 7. [Distributed Agent Coordinator](distributed-agent-coordinator.md)
- **Domain**: System-wide coordination, cross-domain integration
- **Triggers**: `microagent`, `distributed`, `coordination`, `specialist`, `architecture`
- **Coverage**: All domains and their interactions
- **Key Responsibilities**: Conflict resolution, architecture consistency, system optimization

## Key Features

### Domain-Specific Expertise
Each microagent provides deep, specialized knowledge in their domain:
- **Comprehensive Coverage**: All major aspects of the Arda Credit codebase
- **Specific Triggers**: Targeted activation based on relevant keywords
- **Best Practices**: Domain-specific coding standards and security practices
- **Performance Optimization**: Specialized performance tuning for each domain

### Cross-Domain Coordination
The distributed system ensures seamless collaboration:
- **Interface Management**: Clean interfaces between domains
- **Dependency Coordination**: Proper management of cross-domain dependencies
- **Conflict Resolution**: Systematic resolution of competing requirements
- **System-Wide Optimization**: Holistic optimization across all domains

### AI-Native Design
Optimized for AI-assisted development:
- **File Size Management**: Target <250 lines per file for optimal AI context
- **Clear Separation**: Single responsibility principle for each file
- **Comprehensive Documentation**: Detailed documentation for AI understanding
- **Consistent Patterns**: Standardized patterns across all domains

## Critical Refactoring Priorities

The system has identified several large files requiring immediate attention:

1. **`db/src/db.rs` (6,220 lines)** - Database + Performance Specialist
2. **`db/src/merkle_tree.rs` (3,126 lines)** - Database + ZK Specialist  
3. **`db/src/ads_service.rs` (2,501 lines)** - Database + ZK Specialist
4. **`lib/src/loan_processor.rs` (2,345 lines)** - Financial Specialist
5. **`ethereum-client/src/client.rs` (2,072 lines)** - Blockchain Specialist
6. **`api/src/unified_batch_service.rs` (1,671 lines)** - API + ZK Specialist
7. **`api/src/financial_transaction_handlers.rs` (1,599 lines)** - API + Financial Specialist

## Usage Guidelines

### Activating Specialists
Each microagent is activated by specific trigger words in your conversation:
- Use domain-specific terms to engage the relevant specialist
- Multiple specialists can be engaged simultaneously for cross-domain issues
- The coordinator automatically manages complex multi-domain scenarios

### Best Practices
- **Single Domain Issues**: Engage the specific specialist for focused expertise
- **Cross-Domain Issues**: Let the coordinator manage multi-specialist collaboration
- **Architecture Decisions**: Always involve the coordinator for system-wide changes
- **Performance Issues**: Coordinate between relevant specialists and performance specialist

## Security and Compliance

The distributed system maintains comprehensive security coverage:
- **Domain-Specific Security**: Each specialist maintains security standards for their domain
- **Cross-Domain Security**: Coordinator ensures security measures work cohesively
- **Compliance Monitoring**: Financial specialist ensures regulatory compliance
- **Audit Trails**: Complete audit coverage across all domains

## Monitoring and Observability

System-wide monitoring coordinated across all specialists:
- **Performance Metrics**: Domain-specific and system-wide performance monitoring
- **Security Metrics**: Comprehensive security monitoring across all components
- **Business Metrics**: Financial and operational metrics
- **Infrastructure Metrics**: Complete infrastructure health monitoring

This distributed microagent system ensures that Arda Credit maintains the highest standards of security, performance, and code quality while leveraging deep domain expertise in each specialized area.