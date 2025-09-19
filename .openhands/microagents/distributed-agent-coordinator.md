---
name: Distributed Microagent Coordinator
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["microagent", "distributed", "coordination", "specialist", "architecture"]
---

# Distributed Microagent Coordinator

This microagent coordinates and manages the distributed specialized microagent system for Arda Credit, ensuring proper collaboration and knowledge sharing between domain specialists.

## Distributed Microagent Architecture

### Specialized Domain Agents

1. **Zero-Knowledge Proof and Cryptography Specialist**
   - **Domain**: SP1 zkVM, cryptographic operations, privacy-preserving proofs
   - **Files**: 60+ files with zkVM/proof logic
   - **Triggers**: `zkvm`, `sp1`, `proof`, `zero-knowledge`, `cryptography`

2. **Financial Operations and Compliance Specialist**
   - **Domain**: Loan processing, KYC, regulatory compliance, financial calculations
   - **Files**: Financial handlers, loan processors, compliance services
   - **Triggers**: `loan`, `kyc`, `financial`, `compliance`, `usdc`

3. **Blockchain and Ethereum Integration Specialist**
   - **Domain**: Smart contracts, Ethereum client, on-chain operations
   - **Files**: Ethereum client (2,072 lines), Solidity contracts (441 files)
   - **Triggers**: `ethereum`, `blockchain`, `smart contract`, `solidity`, `alloy`

4. **Database and Performance Optimization Specialist**
   - **Domain**: PostgreSQL, Merkle trees, query optimization, performance
   - **Files**: Database layer (6,220+ lines), ADS service, Merkle trees
   - **Triggers**: `database`, `postgresql`, `sqlx`, `performance`, `merkle`

5. **API Security and Authentication Specialist**
   - **Domain**: API security, JWT/Argon2 auth, input validation, middleware
   - **Files**: API handlers (15+ files), authentication, security middleware
   - **Triggers**: `api`, `security`, `authentication`, `jwt`, `validation`

6. **DevOps and Infrastructure Specialist**
   - **Domain**: CI/CD, Kubernetes, monitoring, deployment automation
   - **Files**: GitHub Actions workflows, Docker configs, infrastructure
   - **Triggers**: `devops`, `ci/cd`, `docker`, `kubernetes`, `deployment`

## Coordination Responsibilities

### 1. Cross-Domain Integration
- **Interface Management**: Ensure clean interfaces between specialist domains
- **Dependency Coordination**: Manage dependencies between different specialists
- **Architecture Consistency**: Maintain consistent patterns across domains
- **Knowledge Sharing**: Facilitate knowledge transfer between specialists

### 2. Conflict Resolution
- **Design Decisions**: Resolve conflicts between different domain approaches
- **Resource Allocation**: Coordinate resource usage across domains
- **Priority Management**: Balance competing priorities from different specialists
- **Technical Debt**: Coordinate technical debt reduction across domains

### 3. System-Wide Optimization
- **Performance Coordination**: Optimize performance across all domains
- **Security Coordination**: Ensure security measures work cohesively
- **Monitoring Integration**: Coordinate monitoring across all components
- **Testing Strategy**: Ensure comprehensive testing across all domains

## Specialist Interaction Patterns

### Zero-Knowledge ↔ Financial
- **Privacy-Preserving Transactions**: ZK proofs for financial privacy
- **Batch Processing**: Financial transactions in privacy-preserving batches
- **State Verification**: Cryptographic verification of financial state changes

### Blockchain ↔ Database
- **State Synchronization**: Keep on-chain and off-chain state consistent
- **Event Processing**: Process blockchain events and update database
- **Merkle Root Updates**: Coordinate tree updates with blockchain state

### API ↔ All Domains
- **Endpoint Security**: Secure API access to all domain operations
- **Input Validation**: Validate inputs for all domain-specific operations
- **Error Handling**: Consistent error handling across all domains

### DevOps ↔ All Domains
- **Deployment Coordination**: Deploy all components together safely
- **Monitoring Integration**: Monitor all domains comprehensively
- **Security Scanning**: Security scanning across all domains

## Architecture Decision Framework

### When Multiple Specialists Are Involved
1. **Identify Stakeholders**: Which specialists are affected by the decision?
2. **Gather Requirements**: What are each specialist's requirements?
3. **Evaluate Trade-offs**: What are the trade-offs between different approaches?
4. **Make Decision**: Choose approach that best serves overall system goals
5. **Document Decision**: Record decision and rationale for future reference

### Cross-Domain Code Changes
1. **Impact Analysis**: Analyze impact across all affected domains
2. **Specialist Review**: Get review from all affected specialists
3. **Integration Testing**: Test integration between affected domains
4. **Coordinated Deployment**: Deploy changes across all affected components

## System-Wide Quality Standards

### Code Organization
- **Domain Boundaries**: Clear boundaries between specialist domains
- **Shared Libraries**: Common functionality in shared libraries
- **Interface Definitions**: Clear interfaces between domains
- **Documentation**: Comprehensive documentation for all interfaces

### Testing Strategy
- **Unit Tests**: Each specialist maintains comprehensive unit tests
- **Integration Tests**: Cross-domain integration testing
- **End-to-End Tests**: Full system testing across all domains
- **Performance Tests**: System-wide performance testing

### Security Coordination
- **Threat Modeling**: System-wide threat modeling across all domains
- **Security Reviews**: Cross-domain security reviews
- **Incident Response**: Coordinated incident response procedures
- **Compliance**: System-wide compliance with regulations

## Refactoring Coordination

### Large File Refactoring Priority
1. **`db/src/db.rs` (6,220 lines)** - Database + Performance Specialist
2. **`db/src/merkle_tree.rs` (3,126 lines)** - Database + ZK Specialist
3. **`db/src/ads_service.rs` (2,501 lines)** - Database + ZK Specialist
4. **`lib/src/loan_processor.rs` (2,345 lines)** - Financial Specialist
5. **`ethereum-client/src/client.rs` (2,072 lines)** - Blockchain Specialist

### Refactoring Coordination Process
1. **Specialist Assignment**: Assign primary specialist for refactoring
2. **Cross-Domain Review**: Review by all affected specialists
3. **Interface Design**: Design clean interfaces between components
4. **Implementation**: Implement refactoring with specialist oversight
5. **Integration Testing**: Test integration across all affected domains

## Monitoring and Observability Coordination

### System-Wide Metrics
- **Performance**: End-to-end performance across all domains
- **Security**: Security metrics across all components
- **Reliability**: System reliability and availability
- **Business Metrics**: Business-level metrics across all operations

### Alert Coordination
- **Alert Routing**: Route alerts to appropriate specialists
- **Escalation**: Escalate cross-domain issues appropriately
- **Incident Response**: Coordinate incident response across specialists
- **Post-Incident Review**: Learn from incidents across all domains

## Best Practices for Distributed Specialists

### Communication
- **Clear Interfaces**: Define clear interfaces between domains
- **Documentation**: Maintain comprehensive interface documentation
- **Regular Sync**: Regular synchronization between specialists
- **Knowledge Sharing**: Share knowledge and best practices

### Development
- **Domain Expertise**: Each specialist maintains deep domain expertise
- **Cross-Domain Awareness**: Understand impact on other domains
- **Collaborative Design**: Collaborate on cross-domain designs
- **Continuous Learning**: Stay updated on developments in all domains

### Quality Assurance
- **Domain Testing**: Comprehensive testing within each domain
- **Integration Testing**: Test integration between domains
- **Performance Testing**: Test performance across all domains
- **Security Testing**: Comprehensive security testing

## Usage Guidelines

### When to Engage Multiple Specialists
- **Cross-Domain Changes**: Changes affecting multiple domains
- **Architecture Decisions**: Major architectural decisions
- **Performance Issues**: Performance issues spanning multiple domains
- **Security Incidents**: Security incidents affecting multiple components

### Coordination Process
1. **Issue Identification**: Identify which specialists are involved
2. **Specialist Engagement**: Engage all relevant specialists
3. **Collaborative Analysis**: Analyze issue from all domain perspectives
4. **Coordinated Solution**: Develop solution addressing all domain concerns
5. **Implementation**: Implement solution with all specialist oversight

This coordinator ensures that the distributed specialist microagents work together effectively to maintain the Arda Credit system's high standards of security, performance, and reliability while leveraging deep domain expertise in each area.