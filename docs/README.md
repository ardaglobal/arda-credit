# Arda Credit Documentation Hub

Welcome to the Arda Credit documentation hub! This directory contains comprehensive guides for understanding, deploying, and extending the Arda Credit privacy-preserving loan platform.

## Overview

The `docs/` directory serves as the central navigation point for all Arda Credit documentation. It complements the detailed component-specific READMEs by providing:

### 🎯 What This Documentation Covers

- **Cross-cutting concerns** - Architecture patterns, deployment strategies, and integration approaches
- **Task-oriented guides** - Step-by-step workflows for common development and operational tasks
- **Reference materials** - Standards, best practices, and maintenance guidelines
- **Integration patterns** - Frontend, API, and third-party service integration examples

### 🚫 What This Documentation Doesn't Cover

- **API reference** - See [`api/src/README.md`](../api/src/README.md) and Swagger documentation at http://localhost:8080/swagger-ui/
- **Component internals** - See individual component READMEs in their respective directories
- **Code-level details** - See inline documentation and source code comments

## 📁 Documentation Structure

### 🏗️ Architecture & Design
```
docs/architecture/
├── loan-system.md              # Technical loan system architecture
├── performance-targets.md      # Performance and capacity planning
└── three-component-overview.md # Smart contracts, ZK program, web server
```

### 🚀 Deployment & Operations
```
docs/deployment/
├── security-production.md      # Security setup and production deployment
├── development-setup.md        # Local development environment setup
└── monitoring.md               # OpenTelemetry and monitoring setup
```

### 🧪 Development & Testing
```
docs/development/
└── local-testing.md           # GitHub Actions local testing with act
```

### 🔗 Integrations
```
docs/integrations/
├── authentication.md          # Frontend authentication integration
├── api-swagger.md            # API documentation and Swagger
└── frontend-examples.md      # Frontend integration examples
```

## 🧭 Quick Reference Guide

| Task Category | Common Tasks | Recommended Documentation |
|---------------|--------------|---------------------------|
| **Getting Started** | First-time setup, environment config | [`docs/deployment/development-setup.md`](deployment/development-setup.md) |
| | Understanding system architecture | [`docs/architecture/three-component-overview.md`](architecture/three-component-overview.md) |
| **Development** | Local testing with GitHub Actions | [`docs/development/local-testing.md`](development/local-testing.md) |
| | Frontend integration patterns | [`docs/integrations/frontend-examples.md`](integrations/frontend-examples.md) |
| | API integration and authentication | [`docs/integrations/authentication.md`](integrations/authentication.md) |
| **Operations** | Production deployment and security | [`docs/deployment/security-production.md`](deployment/security-production.md) |
| | Monitoring and observability | [`docs/deployment/monitoring.md`](deployment/monitoring.md) |
| | Performance optimization | [`docs/architecture/performance-targets.md`](architecture/performance-targets.md) |
| **Architecture** | Loan system design and patterns | [`docs/architecture/loan-system.md`](architecture/loan-system.md) |
| | Component interaction overview | [`docs/architecture/three-component-overview.md`](architecture/three-component-overview.md) |
| **Integration** | API documentation and Swagger | [`docs/integrations/api-swagger.md`](integrations/api-swagger.md) |
| | Authentication flows | [`docs/integrations/authentication.md`](integrations/authentication.md) |

## 📚 Component-Specific Documentation

For detailed implementation details, refer to these component READMEs:

### Core Components
- **[`README.md`](../README.md)** - Project overview, quick start, and use cases
- **[`AGENTS.md`](../AGENTS.md)** - AI agent context and development guidance
- **[`docs/architecture/loan-system.md`](architecture/loan-system.md)** - Detailed technical architecture

### System Components
- **[`api/src/README.md`](../api/src/README.md)** - API server architecture and endpoints
- **[`contracts/README.md`](../contracts/README.md)** - Smart contract deployment and testing
- **[`db/README.md`](../db/README.md)** - Database schema and operations
- **[`lib/`](../lib/)** - Shared computation logic and ZKVM compatibility
- **[`program/`](../program/)** - SP1 ZK program for loan transaction processing

### Development Tools
- **[`ethereum-client/`](../ethereum-client/)** - Ethereum integration utilities
- **[`scripts/`](../scripts/)** - Development and deployment scripts
- **[`utils/`](../utils/)** - Testing and utility tools

## 🤖 AI Agent Context

This documentation is designed to work seamlessly with AI development agents. For comprehensive context about the Arda Credit system, see:

- **[`AGENTS.md`](../AGENTS.md)** - Essential context for AI agents working with this codebase
- **[`CLAUDE.md`](../CLAUDE.md)** - Claude-specific development guidance and patterns

The `AGENTS.md` file provides:
- **Project overview** and core concepts
- **Architecture breakdown** and component relationships
- **Development workflows** and best practices
- **Common patterns** and conventions
- **Essential commands** and quick references

## 🔄 Documentation Maintenance

### Update Guidelines

**When to update documentation:**
- **New features** - Add guides for significant new functionality
- **Architecture changes** - Update relevant architecture documents
- **Process changes** - Modify deployment and development guides
- **Breaking changes** - Update integration guides and quick references

**Documentation standards:**
- **Keep it current** - Update docs when code changes
- **Cross-reference** - Link between related documents and component READMEs
- **Task-oriented** - Focus on practical workflows and common tasks
- **Accessible** - Use clear language, examples, and navigation

### Contribution Process

1. **Identify the need** - Determine if changes require new docs or updates to existing ones
2. **Follow the structure** - Use the established directory and naming conventions
3. **Cross-reference** - Link to related component READMEs and other docs
4. **Update navigation** - Modify this README if adding/removing documents
5. **Test workflows** - Ensure documented processes work as described

### Quality Checklist

Before committing documentation changes:
- [ ] Content is accurate and up-to-date
- [ ] Links to related documentation are working
- [ ] Quick reference table reflects current documentation
- [ ] Examples are tested and functional
- [ ] Navigation is clear and logical

## 🚀 Getting Started

1. **Project Overview** - Start with [`../README.md`](../README.md) for project overview and quick start
2. **Development Setup** - Follow [`docs/deployment/development-setup.md`](deployment/development-setup.md) for environment setup
3. **System Architecture** - Read [`docs/architecture/three-component-overview.md`](architecture/three-component-overview.md)
4. **API Workflows** - Review [`../AGENTS.md`](../AGENTS.md) for comprehensive API usage examples and Swagger documentation

## 📞 Support and Resources

- **Issues and bugs** - Use GitHub Issues with appropriate labels
- **Discussions** - Use GitHub Discussions for questions and design discussions
- **Contributing** - See contribution guidelines in component READMEs
- **Security** - See [`docs/deployment/security-production.md`](deployment/security-production.md) for security-related documentation

---

**📖 Quick Navigation:** [Architecture](architecture/) • [Deployment](deployment/) • [Development](development/) • [Integrations](integrations/) • [Main README](../README.md)
