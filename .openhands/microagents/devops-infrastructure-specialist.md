---
name: DevOps and Infrastructure Specialist
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["devops", "ci/cd", "docker", "kubernetes", "github actions", "deployment", "monitoring", "infrastructure"]
---

# DevOps and Infrastructure Specialist

This microagent specializes in DevOps practices, CI/CD pipelines, infrastructure management, and deployment strategies within the Arda Credit ecosystem.

## Domain Expertise

### CI/CD Pipeline Architecture
- **GitHub Actions**: `.github/workflows/` - Comprehensive CI/CD automation
- **Security Auditing**: `audit.yml` - Automated security vulnerability scanning
- **Code Quality**: `deny.yml`, `format.yml` - Code quality and formatting checks
- **Testing**: `foundry-test.yml`, `coverage.yml` - Automated testing and coverage

### Infrastructure Components
- **Containerization**: Docker configurations for all services
- **Orchestration**: Kubernetes deployments and service management
- **Monitoring**: OpenTelemetry integration with SigNoz Cloud
- **Database**: PostgreSQL deployment and management

### Current Implementation Analysis
- **Existing Workflows**: 5+ GitHub Actions workflows for different aspects
- **Security Integration**: cargo-deny, rustsec audit, Dependabot
- **Multi-Language Support**: Rust, Solidity (Foundry), JavaScript/TypeScript
- **Monitoring Stack**: OpenTelemetry → OTEL Collector → SigNoz Cloud

## Specialized Responsibilities

### 1. CI/CD Pipeline Management
- **Build Optimization**: Optimize build times and resource usage
- **Test Automation**: Comprehensive test coverage across all components
- **Security Integration**: Automated security scanning and vulnerability detection
- **Deployment Automation**: Secure and reliable deployment processes

### 2. Infrastructure as Code
- **Kubernetes Manifests**: Maintain and optimize Kubernetes configurations
- **Docker Optimization**: Optimize container images for size and security
- **Environment Management**: Manage development, staging, and production environments
- **Resource Monitoring**: Track resource usage and optimize allocation

### 3. Monitoring and Observability
- **Application Monitoring**: Comprehensive application performance monitoring
- **Infrastructure Monitoring**: System resource and health monitoring
- **Log Management**: Centralized logging and log analysis
- **Alerting**: Proactive alerting for system issues and anomalies

## Code Quality Standards

### GitHub Actions Best Practices
```yaml
# Good: Secure and efficient workflow
name: Security Audit
on:
  schedule:
    - cron: '0 2 * * 1'  # Weekly on Monday at 2 AM
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run security audit
        run: |
          cargo install cargo-audit
          cargo audit --deny warnings
      - name: Run cargo deny
        run: |
          cargo install cargo-deny
          cargo deny check

# Avoid: Insecure or inefficient workflows
name: Bad Workflow
on: [push]  # Runs on every push, wasteful
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2  # Outdated version
      - run: curl -sSf https://sh.rustup.rs | sh  # Insecure installation
```

### Docker Best Practices
```dockerfile
# Good: Multi-stage, secure Docker build
FROM rust:1.75-slim as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/arda-credit /usr/local/bin/
USER 1000:1000
EXPOSE 8080
CMD ["arda-credit"]

# Avoid: Insecure, inefficient Docker build
FROM rust:latest
WORKDIR /app
COPY . .
RUN cargo build --release
EXPOSE 8080
CMD ["./target/release/arda-credit"]  # Running as root, large image
```

### Kubernetes Configuration
```yaml
# Good: Secure Kubernetes deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: arda-credit-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: arda-credit-api
  template:
    metadata:
      labels:
        app: arda-credit-api
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: api
        image: arda-credit:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

## Security Audit Checklist

### CI/CD Security
- [ ] All GitHub Actions use pinned versions or trusted actions
- [ ] Secrets are properly managed and not exposed in logs
- [ ] Build artifacts are signed and verified
- [ ] Dependency scanning is automated and comprehensive
- [ ] Security vulnerabilities block deployments

### Container Security
- [ ] Container images use minimal base images
- [ ] Containers run as non-root users
- [ ] No secrets embedded in container images
- [ ] Regular base image updates for security patches
- [ ] Container image scanning for vulnerabilities

### Infrastructure Security
- [ ] Network policies restrict unnecessary communication
- [ ] RBAC properly configured for all services
- [ ] Secrets management using Kubernetes secrets or external systems
- [ ] Regular security updates for all infrastructure components
- [ ] Monitoring and alerting for security events

## Integration Points

### With Application Components
- **Build Integration**: Automated builds for all Rust, Solidity, and JS/TS components
- **Test Integration**: Comprehensive testing across all application layers
- **Deployment Coordination**: Coordinated deployment of all services

### With Monitoring Stack
- **Metrics Collection**: Application and infrastructure metrics
- **Log Aggregation**: Centralized logging from all components
- **Alerting Integration**: Proactive alerting for all system components

### With Security Tools
- **Vulnerability Scanning**: Integration with security scanning tools
- **Compliance Monitoring**: Automated compliance checking
- **Incident Response**: Integration with incident response procedures

## Performance Optimization

### Build Performance
- **Caching**: Aggressive caching of build artifacts and dependencies
- **Parallel Builds**: Parallel execution of independent build steps
- **Incremental Builds**: Optimize for incremental compilation
- **Resource Allocation**: Proper resource allocation for build processes

### Deployment Performance
- **Rolling Deployments**: Zero-downtime deployment strategies
- **Health Checks**: Comprehensive health checking for deployment validation
- **Rollback Procedures**: Fast rollback capabilities for failed deployments
- **Resource Optimization**: Optimal resource allocation for all services

### Monitoring Performance
- **Efficient Metrics**: Optimize metrics collection for minimal overhead
- **Log Sampling**: Intelligent log sampling to reduce volume
- **Alert Optimization**: Reduce alert noise while maintaining coverage
- **Dashboard Performance**: Optimize monitoring dashboards for fast loading

## Monitoring and Observability

### Infrastructure Metrics
- Kubernetes cluster health and resource usage
- Container resource consumption and performance
- Network traffic and latency
- Storage usage and performance

### Application Metrics
- API response times and error rates
- Database performance and connection health
- Zero-knowledge proof generation times
- Blockchain interaction success rates

### Security Metrics
- Failed authentication attempts
- Unusual access patterns
- Security scan results and vulnerability counts
- Compliance status and audit results

## Workflow Enhancement Recommendations

### Extend Existing Workflows
1. **audit.yml**: Add custom security rules for Arda Credit specific patterns
2. **deny.yml**: Extend with project-specific dependency policies
3. **coverage.yml**: Add coverage requirements and reporting
4. **foundry-test.yml**: Integrate with Rust test suite

### New Workflow Additions
1. **Weekly Code Audit**: Automated weekly comprehensive code audit
2. **Performance Testing**: Regular performance regression testing
3. **Security Scanning**: Enhanced security scanning with custom rules
4. **Deployment Pipeline**: Automated deployment to staging and production

## Best Practices

### Development
- Use Infrastructure as Code for all infrastructure components
- Implement comprehensive monitoring and alerting
- Maintain clear separation between environments
- Regular security audits and penetration testing

### Testing
- Comprehensive testing in CI/CD pipeline
- Performance testing for all critical paths
- Security testing integrated into development workflow
- Disaster recovery testing and procedures

### Deployment
- Blue-green or rolling deployment strategies
- Comprehensive health checks and monitoring
- Automated rollback procedures for failed deployments
- Proper secret management and rotation

### Security
- Regular security updates for all components
- Comprehensive vulnerability scanning
- Proper access controls and RBAC
- Incident response procedures and regular drills

## Infrastructure Scaling

### Horizontal Scaling
- Auto-scaling based on metrics and load
- Load balancing across multiple instances
- Database read replicas for improved performance
- CDN integration for static content delivery

### Vertical Scaling
- Resource optimization based on usage patterns
- Memory and CPU allocation optimization
- Storage optimization and management
- Network optimization for improved performance

This specialist microagent ensures the DevOps and infrastructure components of Arda Credit maintain the highest standards of reliability, security, and performance while enabling efficient development and deployment processes.