---
name: Code Audit and AI-Native Management
type: knowledge
version: 2.0.0
agent: CodeActAgent
triggers: ["code audit", "weekly audit", "ai-native", "code management", "documentation sync"]
---

# Code Audit and AI-Native Management Microagent

This microagent provides guidance for implementing weekly code audits and AI-native code management practices for the Arda Credit repository. It coordinates with the distributed specialist microagent system to ensure comprehensive coverage across all domains.

## Distributed Specialist Integration

This microagent works in coordination with specialized domain experts:
- **Zero-Knowledge Proof and Cryptography Specialist** - zkVM, SP1, cryptographic security
- **Financial Operations and Compliance Specialist** - Loan processing, KYC, regulatory compliance  
- **Blockchain and Ethereum Integration Specialist** - Smart contracts, on-chain operations
- **Database and Performance Optimization Specialist** - PostgreSQL, Merkle trees, query optimization
- **API Security and Authentication Specialist** - API security, JWT/Argon2, input validation
- **DevOps and Infrastructure Specialist** - CI/CD, Kubernetes, monitoring, deployment

Each specialist provides deep domain expertise while this microagent coordinates system-wide auditing and AI-native practices.

**Repository Context**: Arda Credit is a privacy-preserving loan vApp built with:
- **Rust** (141 files, ~77k LOC) - Core business logic, API, CLI, zkVM programs
- **Solidity** (441 files, ~116k LOC) - Smart contracts and dependencies  
- **JavaScript/TypeScript** (155 files) - Frontend and tooling
- **SP1 zkVM** - Zero-knowledge proof generation
- **PostgreSQL** - Database with complex financial data models
- **Axum** - Web framework for API server

## Core Responsibilities

### 1. Weekly Code Audit System

**Documentation Consistency Auditing:**
- Compare README.md content with actual implementation
- Verify AGENTS.md/CLAUDE.md alignment with current codebase features
- Cross-reference API endpoints in documentation vs `api/src/*_handlers.rs` files
- Validate CLI commands in docs vs `cli/src/*.rs` implementations
- Check zkVM program documentation vs `program/src/main.rs` functionality
- Ensure smart contract documentation matches `contracts/src/*.sol` files

**Rust-Specific Security Analysis:**
- **Panic Safety**: Audit 543 `unwrap()` and 258 `expect()` calls for production safety
- **Memory Safety**: Verify no `unsafe` blocks (currently 0 - maintain this!)
- **Error Handling**: Review `Result<T, E>` usage and proper error propagation
- **SQL Injection**: Check `sqlx` query usage in `db/src/*.rs` files
- **Input Validation**: Verify `validator` crate usage in API handlers
- **Cryptographic Security**: Review JWT, Argon2, and zkVM proof handling

**Large File Monitoring (Current Issues):**
- `db/src/db.rs` (6,220 lines) - **CRITICAL**: Needs immediate refactoring
- `db/src/merkle_tree.rs` (3,126 lines) - Split into modules
- `db/src/ads_service.rs` (2,501 lines) - Extract service layers
- `lib/src/loan_processor.rs` (2,345 lines) - Break into focused components
- `ethereum-client/src/client.rs` (2,072 lines) - Separate concerns
- `api/src/unified_batch_service.rs` (1,671 lines) - Extract batch logic
- `api/src/financial_transaction_handlers.rs` (1,599 lines) - Split by transaction type

**Implementation Gap Detection:**
- Cross-reference documented loan operations vs `lib/src/loan.rs` implementation
- Verify KYC features in docs vs `api/src/kyc_service.rs` 
- Check liquidity pool documentation vs `api/src/pool_*.rs` handlers
- Validate zkVM program capabilities vs documentation claims
- Ensure smart contract features match documented functionality

### 2. AI-Native Code Management

**File Size Management (Lovable.dev Inspired):**
- **Priority Refactoring**: 7 files exceed 1,500 lines (see list above)
- **Target**: Keep Rust files under 250 lines for optimal AI context
- **Rust Module System**: Use `mod.rs` files to organize large modules
- **Extract Patterns**: Move common patterns to `lib/src/` for reuse
- **Service Layer**: Split large handlers into service + handler layers

**Rust-Specific Context Management:**
- **Single Responsibility**: Each `.rs` file should handle one domain concept
- **Trait Abstractions**: Use traits to define clear interfaces between modules
- **Error Types**: Define domain-specific error types per module
- **Async Boundaries**: Separate sync business logic from async I/O operations
- **Test Organization**: Keep unit tests in same file, integration tests in `tests/`

**Zero-Knowledge Proof Context:**
- **zkVM Compatibility**: Ensure `lib/src/` code compiles for both native and zkVM
- **Proof Generation**: Keep proof logic separate from business logic
- **State Management**: Maintain clear separation between on-chain and off-chain state
- **Batch Processing**: Organize batch operations for optimal proof generation

**Documentation Standards:**
- **Rust Doc Comments**: Use `///` for public APIs, `//` for internal logic
- **Module Documentation**: Each `mod.rs` should explain the module's purpose
- **API Documentation**: Keep OpenAPI specs in sync with handler implementations
- **Architecture Docs**: Update `docs/` when making structural changes
- **Security Notes**: Document security assumptions and threat models

### 3. Automated Audit Implementation

**GitHub Actions Integration (Extends Existing Workflows):**
- **Leverage Existing**: Build on current `audit.yml`, `deny.yml`, `format.yml` workflows
- **Weekly Schedule**: Add cron job to existing security audit workflow
- **Rust-Specific Tools**: Integrate `cargo audit`, `cargo deny`, `clippy` with custom rules
- **File Size Monitoring**: Add automated checks for files exceeding 250 lines
- **Documentation Sync**: Compare API endpoints in handlers vs OpenAPI specs

**Integration with Existing Security Tools:**
- **cargo-deny**: Already configured in `deny.toml` - extend for custom rules
- **rustsec/audit-check**: Already in use - add custom vulnerability checks
- **Dependabot**: Already configured - ensure it covers all Rust dependencies
- **Custom Lints**: Add clippy rules for `unwrap()` usage in production code

## Audit Checklist Template

### Documentation Audit
- [ ] README.md accuracy vs. current features
- [ ] AGENTS.md/CLAUDE.md alignment with codebase
- [ ] API documentation completeness
- [ ] Architecture diagram currency
- [ ] Example code functionality
- [ ] Installation/setup instructions validity

### Security Audit
- [ ] No hardcoded secrets or credentials
- [ ] Proper authentication mechanisms
- [ ] Input validation and sanitization
- [ ] SQL injection prevention
- [ ] Secure communication protocols
- [ ] Error handling security
- [ ] Session management security
- [ ] Dependency vulnerability scan

### Code Quality Audit
- [ ] Files under 250 lines (or documented exceptions)
- [ ] Single responsibility principle adherence
- [ ] Proper separation of concerns
- [ ] Function complexity analysis
- [ ] Naming convention consistency
- [ ] Test coverage adequacy
- [ ] Performance bottleneck identification

### Implementation Completeness
- [ ] All documented features implemented
- [ ] All implemented features documented
- [ ] TODO/FIXME resolution status
- [ ] Performance targets met
- [ ] Error handling completeness

## Recommended Tools and Scripts

### File Size Monitoring
```bash
# Find files over 250 lines
find . -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" | \
  xargs wc -l | awk '$1 > 250 {print $2 ": " $1 " lines"}' | sort -nr
```

### Documentation Consistency Check
```bash
# Extract API endpoints from documentation vs. implementation
grep -r "POST\|GET\|PUT\|DELETE" README.md AGENTS.md | \
  grep -o '/[a-zA-Z0-9/_-]*' | sort | uniq > documented_endpoints.txt

grep -r "app\.\(post\|get\|put\|delete\)" api/ | \
  grep -o '"/[^"]*"' | tr -d '"' | sort | uniq > implemented_endpoints.txt

diff documented_endpoints.txt implemented_endpoints.txt
```

### Security Scan Basics
```bash
# Check for potential secrets
grep -r -i "password\|secret\|key\|token" --include="*.rs" --include="*.py" . | \
  grep -v "test\|example\|placeholder"

# Check for SQL injection patterns
grep -r "format!\|&format" --include="*.rs" . | grep -i "select\|insert\|update\|delete"
```

## Integration with Existing Workflow

### GitHub Actions Workflow Example
```yaml
name: Weekly Code Audit
on:
  schedule:
    - cron: '0 9 * * 1'  # Every Monday at 9 AM UTC
  workflow_dispatch:

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Code Audit
        run: |
          # File size audit
          echo "## File Size Audit" >> audit_report.md
          find . -name "*.rs" | xargs wc -l | awk '$1 > 250 {print "- " $2 ": " $1 " lines"}' >> audit_report.md
          
          # Documentation consistency
          echo "## Documentation Gaps" >> audit_report.md
          # Add documentation consistency checks
          
          # Security scan
          echo "## Security Findings" >> audit_report.md
          # Add security scan results
          
      - name: Create Issue
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('audit_report.md', 'utf8');
            github.rest.issues.create({
              owner: context.repo.owner,
              repo: context.repo.repo,
              title: `Weekly Code Audit - ${new Date().toISOString().split('T')[0]}`,
              body: report,
              labels: ['audit', 'maintenance']
            });
```

## Best Practices for AI-Native Development

### File Organization
- Keep files focused on single responsibilities
- Use descriptive, searchable file names
- Maintain consistent directory structure
- Group related functionality together
- Separate concerns clearly (data, logic, presentation)

### Code Structure
- Write self-documenting code with clear variable names
- Use consistent formatting and style
- Implement proper error handling patterns
- Maintain clear function signatures
- Avoid deep nesting and complex control flows

### Documentation Strategy
- Keep documentation close to code (inline comments for complex logic)
- Maintain high-level architecture documentation
- Document design decisions and trade-offs
- Provide clear examples for complex features
- Keep documentation up-to-date with code changes

## Monitoring and Metrics

### Key Metrics to Track
- Average file size across the codebase
- Number of files exceeding 250 lines
- Documentation coverage percentage
- Security issue resolution time
- Implementation vs. documentation gap count
- Code complexity trends

### Reporting
- Weekly audit summary reports
- Trend analysis over time
- Action item tracking and resolution
- Team performance metrics
- Continuous improvement recommendations

## Security Considerations

### Sensitive Data Handling
- Never commit secrets to version control
- Use environment variables for configuration
- Implement proper secret rotation practices
- Audit access logs regularly
- Maintain principle of least privilege

### Code Review Integration
- Include security checklist in PR templates
- Require security review for sensitive changes
- Automated security scanning in CI/CD
- Regular dependency vulnerability updates
- Security training for development team

This microagent serves as a comprehensive guide for maintaining code quality, security, and AI-native practices in the Arda Credit repository through systematic auditing and continuous improvement processes.