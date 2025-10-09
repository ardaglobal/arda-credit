# GitHub Actions Workflows

This directory contains all GitHub Actions workflows for the Arda Credit platform.

## Container Build Workflows

### 1. Main Server (`docker.yml`)
**Purpose:** Build and publish the main API server container

- **Image:** `ghcr.io/ardaglobal/arda-credit`
- **Dockerfile:** `Dockerfile`
- **Platform:** `linux/amd64`
- **Triggers:**
  - Push to `main` branch
  - Release tags (`v*`, `program-v*`, `contracts-v*`)
- **Key Features:**
  - SP1 zkVM program building
  - SQLx offline mode compilation
  - Database migrations included
  - S3 build cache for faster builds

### 2. Frontend App (`docker-app.yml`)
**Purpose:** Build and publish the frontend application container

- **Image:** `ghcr.io/ardaglobal/arda-credit-app`
- **Dockerfile:** `Dockerfile.app`
- **Platforms:** `linux/amd64`, `linux/arm64`
- **Triggers:**
  - Push to `main` branch (when `app/**` changes)
  - Release tags
- **Key Features:**
  - Multi-platform builds for broader compatibility
  - Nginx-based serving
  - API proxying configuration

### 3. Contract Deployer (`docker-contract-deployer.yml`)
**Purpose:** Build and publish the contract deployment init container

- **Image:** `ghcr.io/ardaglobal/arda-credit-contract-deployer`
- **Dockerfile:** `Dockerfile.contract-deployer`
- **Platforms:** `linux/amd64`, `linux/arm64`
- **Triggers:**
  - Push to `main` branch (when `contracts/**` changes)
  - Release tags
- **Key Features:**
  - Foundry-based contract deployment
  - Anvil devnet support
  - Submodule inclusion (forge-std, sp1-contracts)
  - Multi-platform builds

### 4. Data Populator (`data-populator.yml`)
**Purpose:** Build and publish the test data population container

- **Image:** `ghcr.io/ardaglobal/arda-credit-data-populator`
- **Dockerfile:** `Dockerfile.data-populator`
- **Platform:** `linux/amd64`
- **Triggers:**
  - Push to `main` branch (when data populator script changes)
  - Release tags
- **Key Features:**
  - Based on main server image
  - Includes fake data generation script
  - Database migrations included

## CI/CD Workflows

### Code Quality

- **`rust-ci.yml`** - Rust linting, testing, and building
- **`coverage.yml`** - Code coverage reporting
- **`deny.yml`** - Security auditing of dependencies
- **`actionlint.yml`** - GitHub Actions workflow linting
- **`yamllint.yml`** - YAML file linting
- **`semantic-pr.yml`** - Semantic PR title validation

### Smart Contracts

- **`foundry.yml`** - Foundry contract testing and building
- **`docker-anvil-contract-deploy.yml`** - Anvil deployment testing

### ZK Proofs

- **`sindri.yml`** - Circuit deployment to Sindri service

### Code Review

- **`claude.yml`** - AI-powered code review with Claude
- **`claude-code-review.yml`** - Detailed code analysis

### Helm Charts

- **`helm-lint.yml`** - Helm chart validation and linting

## Build Optimization

All Docker workflows use:

- **AWS CodeBuild Runners:** High-performance self-hosted runners
  - Main server: `2xlarge` instance (72 vCPUs) for Rust compilation
  - Other containers: `large` instance (8 vCPUs)

- **S3 Build Cache:** Persistent caching for faster builds
  - Bucket: `codebuild-s3-cache-arda`
  - Prefixes: `output/arda-credit*`
  - Mode: `max` for maximum cache reuse

- **Build Optimizations:**
  - Parallel Cargo builds (72 jobs for main server)
  - Native CPU targeting (`-C target-cpu=native`)
  - BuildKit log buffering (50MB)
  - Network optimizations for Cargo

## Image Registry

All images are published to **GitHub Container Registry (GHCR)**:

```
ghcr.io/ardaglobal/arda-credit:latest
ghcr.io/ardaglobal/arda-credit-app:latest
ghcr.io/ardaglobal/arda-credit-contract-deployer:latest
ghcr.io/ardaglobal/arda-credit-data-populator:latest
```

### Tagging Strategy

Each workflow generates multiple tags:

- `latest` - Latest build from main branch
- `main` - Explicit main branch tag
- `v3.4.5` - Semver version tags
- `v3.4` - Major.minor tags
- `v3` - Major version tags
- `sha-abc123` - Git commit SHA tags

## Release Process

When a release is published (e.g., `v3.4.5`):

1. All container build workflows are triggered
2. Each workflow builds and pushes versioned tags
3. Images are available at:
   - `ghcr.io/ardaglobal/arda-credit:v3.4.5`
   - `ghcr.io/ardaglobal/arda-credit-app:v3.4.5`
   - `ghcr.io/ardaglobal/arda-credit-contract-deployer:v3.4.5`
   - `ghcr.io/ardaglobal/arda-credit-data-populator:v3.4.5`

## Path-Based Triggering

Workflows only run when relevant files change:

- **Main Server:** Any Rust code, Dockerfile changes
- **Frontend App:** `app/**` directory changes
- **Contract Deployer:** `contracts/**` directory changes
- **Data Populator:** `db/src/bin/populate_fake_data.rs` changes

This minimizes unnecessary builds and speeds up CI/CD.

## Troubleshooting

### Build Failures

1. **Check runner availability:** AWS CodeBuild runners must be provisioned
2. **Verify S3 cache access:** Ensure IAM permissions for cache bucket
3. **Review Dockerfile syntax:** Use `hadolint` for linting
4. **Check submodules:** Contract deployer requires submodules

### Authentication Issues

- **GHCR Push:** Uses `GITHUB_TOKEN` with `packages: write` permission
- **Pull Private Images:** Same token provides read access
- **DockerHub (legacy):** Uses `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` secrets

### Cache Issues

Clear S3 cache if builds fail with cache corruption:
```bash
aws s3 rm s3://codebuild-s3-cache-arda/output/arda-credit --recursive
```

## Local Testing

Test workflows locally using [act](https://github.com/nektos/act):

```bash
# Install act
brew install act

# Test docker workflow
act -j build-and-push -W .github/workflows/docker.yml

# Test with specific event
act push -W .github/workflows/docker-app.yml
```

## Adding New Workflows

When adding new container builds:

1. Create `Dockerfile.<name>` in repository root
2. Create `.github/workflows/docker-<name>.yml` workflow
3. Follow existing patterns for consistency:
   - Use hadolint for Dockerfile linting
   - Configure S3 caching
   - Use appropriate runner size
   - Add path-based triggering
   - Include in CONTRIBUTORS.md documentation

## Maintenance

### Regular Updates

- **Update action versions:** Check for security updates monthly
- **Review runner images:** Update CodeBuild image versions
- **Optimize cache:** Monitor S3 costs and cleanup old cache entries
- **Update base images:** Keep Foundry, Rust, Node.js images current
