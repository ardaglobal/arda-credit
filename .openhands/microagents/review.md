---
name: review
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: []
---

# Review Microagent

This microagent provides specialized capabilities for conducting comprehensive code reviews in the Arda Credit repository. It focuses on reviewing code quality, security, performance, and adherence to project standards.

## Capabilities

### Code Quality Review
- **Code Structure**: Analyze code organization, modularity, and maintainability
- **Best Practices**: Ensure adherence to Rust best practices and idiomatic code patterns
- **Documentation**: Review code documentation, comments, and inline explanations
- **Error Handling**: Evaluate error handling patterns and robustness

### Security Review
- **Vulnerability Assessment**: Identify potential security vulnerabilities in smart contracts and API endpoints
- **Access Control**: Review authentication and authorization mechanisms
- **Input Validation**: Ensure proper input sanitization and validation
- **Cryptographic Implementation**: Review zero-knowledge proof implementations and cryptographic operations

### Performance Review
- **Optimization Opportunities**: Identify performance bottlenecks and optimization opportunities
- **Resource Usage**: Analyze memory and computational efficiency
- **Database Queries**: Review SQL queries and database interactions for efficiency
- **Smart Contract Gas Optimization**: Evaluate gas usage in Ethereum smart contracts

### Financial Logic Review
- **Credit Scoring**: Review credit scoring algorithms and financial calculations
- **Risk Assessment**: Analyze risk management and assessment logic
- **Compliance**: Ensure adherence to financial regulations and standards
- **Data Integrity**: Verify financial data handling and integrity measures

## Review Process

### 1. Initial Assessment
- Understand the purpose and scope of the code changes
- Identify the components affected (API, smart contracts, database, etc.)
- Review related documentation and specifications

### 2. Technical Review
- Analyze code structure and design patterns
- Check for proper error handling and edge cases
- Verify test coverage and quality
- Review dependencies and external integrations

### 3. Security Analysis
- Conduct security vulnerability assessment
- Review access controls and permissions
- Analyze input validation and sanitization
- Check for common security anti-patterns

### 4. Performance Evaluation
- Identify potential performance issues
- Review algorithmic complexity
- Analyze resource utilization
- Suggest optimization opportunities

### 5. Compliance Check
- Ensure adherence to project coding standards
- Verify compliance with financial regulations
- Check documentation completeness
- Validate test coverage requirements

## Review Checklist

### General Code Quality
- [ ] Code follows project style guidelines
- [ ] Functions and variables have descriptive names
- [ ] Code is properly documented
- [ ] Error handling is comprehensive
- [ ] Tests are present and comprehensive

### Security
- [ ] No hardcoded secrets or sensitive data
- [ ] Input validation is implemented
- [ ] Access controls are properly implemented
- [ ] Cryptographic operations are secure
- [ ] Smart contracts follow security best practices

### Performance
- [ ] No obvious performance bottlenecks
- [ ] Database queries are optimized
- [ ] Memory usage is efficient
- [ ] Smart contract gas usage is optimized

### Financial Logic
- [ ] Financial calculations are accurate
- [ ] Risk assessment logic is sound
- [ ] Compliance requirements are met
- [ ] Data integrity is maintained

## Tools and Resources

### Static Analysis
- Use `cargo clippy` for Rust code analysis
- Run `cargo audit` for security vulnerability scanning
- Use `sqlx` for database query validation

### Testing
- Ensure unit tests pass with `cargo test`
- Run integration tests for API endpoints
- Test smart contracts with Foundry framework
- Verify zero-knowledge proof generation and verification

### Documentation
- Review README files and documentation
- Check API documentation completeness
- Verify smart contract documentation
- Ensure deployment and setup instructions are clear

## Common Issues to Watch For

### Rust-Specific
- Unnecessary cloning or memory allocations
- Improper error handling with `unwrap()` or `expect()`
- Missing lifetime annotations where needed
- Inefficient string handling

### Smart Contract Issues
- Reentrancy vulnerabilities
- Integer overflow/underflow
- Gas limit issues
- Improper access control

### API Security
- Missing input validation
- Improper authentication/authorization
- SQL injection vulnerabilities
- Sensitive data exposure

### Financial Logic
- Rounding errors in calculations
- Missing edge case handling
- Incorrect risk assessment logic
- Compliance violations

## Reporting

When conducting a review, provide:
1. **Summary**: Brief overview of the review findings
2. **Critical Issues**: High-priority security or functionality issues
3. **Recommendations**: Specific suggestions for improvement
4. **Positive Feedback**: Acknowledge good practices and well-written code
5. **Action Items**: Clear next steps for addressing identified issues

## Integration with CI/CD

This microagent can be integrated with the existing GitHub Actions workflows:
- Automated code quality checks
- Security vulnerability scanning
- Performance regression testing
- Compliance validation

The review process should complement existing automated checks and provide human-level analysis and recommendations.