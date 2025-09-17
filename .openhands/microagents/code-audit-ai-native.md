---
name: Code Audit and AI-Native Management
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: []
---

# Code Audit and AI-Native Management Microagent

This microagent provides guidance for implementing weekly code audits and AI-native code management practices for the Arda Credit repository. It focuses on maintaining documentation consistency, identifying security issues, and implementing best practices for AI-friendly codebase management.

## Core Responsibilities

### 1. Weekly Code Audit System

**Documentation Consistency Auditing:**
- Compare README.md content with actual implementation
- Verify AGENTS.md/CLAUDE.md alignment with current codebase features
- Identify gaps between documented APIs and actual API endpoints
- Check for outdated examples or deprecated functionality references
- Validate that architectural diagrams match current system design

**Security Gap Analysis:**
- Scan for hardcoded secrets or credentials in code
- Review authentication and authorization implementations
- Check for SQL injection vulnerabilities in database queries
- Analyze input validation and sanitization practices
- Verify secure communication protocols (HTTPS, TLS)
- Review error handling to prevent information leakage
- Check for proper session management and token handling

**Implementation Gap Detection:**
- Identify features mentioned in documentation but not implemented
- Find implemented features not documented
- Check for TODO/FIXME comments that indicate incomplete work
- Verify test coverage matches documented functionality
- Analyze performance characteristics vs. documented targets

### 2. AI-Native Code Management

**File Size Management (Lovable.dev Inspired):**
- Monitor files exceeding 250 lines of code
- Suggest refactoring opportunities for large files
- Identify functions that could be extracted into separate modules
- Recommend splitting complex files into focused components
- Track file growth trends over time

**Context Management Best Practices:**
- Ensure each file has a single, clear responsibility
- Verify proper separation of concerns across modules
- Check for appropriate abstraction levels
- Identify overly complex functions that need decomposition
- Recommend consistent naming conventions for AI readability

**Documentation Standards:**
- Maintain clear, concise function and module documentation
- Ensure API documentation is up-to-date with implementation
- Verify that complex business logic is well-documented
- Check for proper error handling documentation
- Maintain architectural decision records (ADRs)

### 3. Automated Audit Implementation

**GitHub Actions Integration:**
- Create weekly scheduled workflow for automated audits
- Generate audit reports in markdown format
- Post findings as GitHub issues with appropriate labels
- Track audit history and trend analysis
- Integrate with existing CI/CD pipeline

**Linear Integration (Alternative):**
- Create recurring Linear issues for manual audit reviews
- Template-based audit checklists
- Automated assignment to team members
- Progress tracking and completion metrics

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