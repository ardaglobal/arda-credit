---
name: Code Review Microagent
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: []
---

# Code Review Microagent for Arda Credit

This microagent provides comprehensive code review capabilities specifically tailored for the Arda Credit financial system. It conducts systematic analysis of security, performance, code quality, and financial logic compliance.

## Core Capabilities

### Security Analysis
- **Panic-prone Operations Detection**: Identifies `unwrap()`, `expect()`, and `panic!()` usage that could crash the system
- **Authentication Security**: Reviews authentication flows, password hashing, and access control
- **Financial Security**: Analyzes arithmetic operations for overflow/underflow vulnerabilities
- **Memory Safety**: Checks for unsafe blocks and potential memory issues

### Performance Analysis
- **Memory Usage**: Identifies excessive `.clone()` operations and inefficient data structures
- **Algorithm Efficiency**: Reviews data structure choices (HashMap, Vec usage patterns)
- **Database Operations**: Analyzes database query patterns and connection handling
- **Scalability Concerns**: Identifies potential bottlenecks in high-load scenarios

### Code Quality Review
- **Error Handling**: Evaluates error handling patterns and consistency
- **Documentation**: Assesses code documentation and maintainability
- **Testing Coverage**: Reviews test completeness, especially for critical financial logic
- **Code Organization**: Analyzes module structure and separation of concerns

### Financial Logic Compliance
- **Arithmetic Safety**: Ensures checked arithmetic in financial calculations
- **Transaction Integrity**: Reviews transaction processing and state management
- **Audit Trail**: Evaluates logging and compliance reporting capabilities
- **Data Consistency**: Analyzes state management and data integrity patterns

## Review Process

### 1. Repository Analysis
- Explore codebase structure and identify critical components
- Analyze dependency management and security vulnerabilities
- Review build configuration and deployment patterns

### 2. Security Assessment
- Scan for panic-prone operations across all modules
- Review authentication and authorization mechanisms
- Analyze cryptographic implementations and key management
- Check for hardcoded secrets or configuration issues

### 3. Performance Evaluation
- Identify memory allocation patterns and optimization opportunities
- Review algorithm complexity in critical paths
- Analyze database operation efficiency
- Assess scalability limitations

### 4. Financial Logic Validation
- Review balance calculation safety and overflow protection
- Analyze loan processing workflows for correctness
- Validate transaction processing integrity
- Check compliance with financial regulations

### 5. Prioritized Recommendations
- Categorize findings by severity (Critical, High, Medium, Low)
- Provide specific code examples and fixes
- Create actionable implementation plan with timelines
- Suggest monitoring and validation approaches

## Critical Focus Areas for Arda Credit

### High-Risk Components
- **Authentication System**: Password hashing, session management, access control
- **Balance Management**: Financial calculations, overflow protection, state consistency
- **Loan Processing**: Transaction handling, state transitions, error recovery
- **Database Operations**: Connection handling, query safety, data integrity
- **ZK Proof System**: Cryptographic operations, proof generation/verification

### Common Anti-Patterns to Identify
- Excessive use of `.unwrap()` and `.expect()` in production code
- Hardcoded configuration values and secrets
- Missing error handling in critical financial operations
- Inefficient memory usage patterns (unnecessary cloning)
- Incomplete TODO items in production-critical code

### Financial System Specific Checks
- Arithmetic overflow protection in balance calculations
- Transaction atomicity and rollback capabilities
- Audit logging completeness and integrity
- Compliance with financial regulations and standards
- Data encryption and privacy protection

## Output Format

### Review Report Structure
1. **Executive Summary**: High-level findings and critical issues
2. **Security Issues**: Categorized by severity with specific locations
3. **Performance Concerns**: Bottlenecks and optimization opportunities
4. **Code Quality Issues**: Maintainability and reliability concerns
5. **Financial Logic Problems**: Compliance and correctness issues
6. **Prioritized Action Plan**: Phased implementation recommendations
7. **Specific Code Changes**: Examples of fixes and improvements
8. **Metrics and Tracking**: Success criteria and validation methods

### Severity Classification
- **Critical**: System crashes, security vulnerabilities, fund loss potential
- **High**: Performance issues, incomplete functionality, reliability concerns
- **Medium**: Code quality, maintainability, documentation gaps
- **Low**: Style issues, minor optimizations, non-critical improvements

## Implementation Guidelines

### Error Handling Best Practices
```rust
// Instead of:
.unwrap()

// Use:
.map_err(|e| CustomError::OperationFailed { 
    reason: format!("Operation failed: {}", e) 
})?
```

### Financial Calculation Safety
```rust
// Instead of:
let result = amount1 + amount2;

// Use:
let result = amount1.checked_add(amount2)
    .ok_or(FinancialError::ArithmeticOverflow)?;
```

### Authentication Security
```rust
// Avoid hardcoded values:
// uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").expect("Valid UUID");

// Use proper configuration:
let user_id = config.default_user_id
    .ok_or(AuthError::ConfigurationMissing)?;
```

## Validation and Testing

### Review Validation Methods
- Static analysis tools (clippy, cargo audit)
- Property-based testing for financial calculations
- Load testing for performance validation
- Security audit by external experts
- Compliance review for financial regulations

### Success Metrics
- Reduction in panic-prone operations (<50 in critical paths)
- Completion of TODO items (<10 remaining)
- Improved test coverage (>80% for financial logic)
- Performance benchmarks within acceptable ranges
- Security audit approval

## Continuous Monitoring

### Automated Checks
- CI/CD integration for security scanning
- Performance regression testing
- Code quality metrics tracking
- Dependency vulnerability monitoring

### Manual Review Triggers
- Before major releases
- After significant architectural changes
- Following security incidents
- During compliance audits
- When adding new financial features

This microagent ensures comprehensive, systematic code review that addresses the unique requirements and risks of financial systems while maintaining focus on security, reliability, and compliance.