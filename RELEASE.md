# Release Management Strategy

This document outlines the comprehensive release management strategy for the arda-credit repository, which contains three distinct but interconnected components that require coordinated versioning.

## Component Overview

### 1. Smart Contracts (`contracts/`)
- **Location**: `contracts/src/Loan.sol` and related contracts
- **Dependencies**: SP1 contracts, OpenZeppelin libraries
- **Deployment Target**: Ethereum networks (mainnet, testnets)
- **Versioning Strategy**: Independent semantic versioning
- **Release Artifacts**: Compiled contracts, deployment addresses, ABIs

### 2. ZKVM Program (`program/`)
- **Location**: `program/src/main.rs`
- **Dependencies**: SP1 ZKVM runtime (`sp1-zkvm = "5.2.1"`)
- **Build Target**: SP1 ZKVM ELF binary
- **Versioning Strategy**: Independent semantic versioning
- **Release Artifacts**: Program verification key (vkey), ELF binary

### 3. Web Server Application (Main Repository)
- **Location**: Root workspace with `api/`, `db/`, `cli/`, etc.
- **Dependencies**: Both smart contracts and ZKVM program
- **Deployment Target**: K8s via helm chats `helm`
- **Versioning Strategy**: Main repository version that references specific versions of contracts and ZKVM program
- **Release Artifacts**: Docker images, helm charts, deployment configurations

## Semantic Versioning Strategy

### Version Format: `MAJOR.MINOR.PATCH`

#### Smart Contracts (`contracts-vX.Y.Z`)
- **MAJOR**: Breaking changes to contract interface or state structure
- **MINOR**: New features, additional functions, backward-compatible changes
- **PATCH**: Bug fixes, gas optimizations, security patches

#### ZKVM Program (`program-vX.Y.Z`)
- **MAJOR**: Changes to program logic that affect verification key or public inputs/outputs
- **MINOR**: Performance improvements, additional features that don't change verification
- **PATCH**: Bug fixes, optimizations that don't affect verification

**Circuit Versioning**: Each program release automatically creates a corresponding Sindri circuit with matching tag (e.g., `program-v2.1.0` → circuit tag `program-v2.1.0`). This ensures circuit deployments are version-locked with program releases.

#### Main Application (`vX.Y.Z`)
- **MAJOR**: Breaking changes to API, database schema, or deployment requirements
- **MINOR**: New features, API enhancements, backward-compatible changes
- **PATCH**: Bug fixes, dependency updates, minor improvements

## Release Process

### 1. Smart Contract Releases

#### Pre-Release Checklist
- [ ] Update contract version comments
- [ ] Run full test suite: `cd contracts && forge test`
- [ ] Gas optimization analysis
- [ ] Security audit (for major releases)
- [ ] Update deployment documentation

#### Release Steps
1. **Create RC tag**: `git tag contracts-v1.2.3-rc0`
2. **Push RC tag**: `git push origin contracts-v1.2.3-rc0`
3. **Automated deployment**: GitHub Actions deploys and captures address in `contracts/deployments/contracts-v1.2.3-rc0.json`, merge PR from GHA.
4. **Verify deployment**: Ensure Etherscan verification and test functionality
5. **Create final tag**: `git tag contracts-v1.2.3` (references RC deployment)
6. **Create GitHub release**: Include deployment addresses and ABI files from RC deployment

#### Post-Release
- Update dependent services with new contract addresses
- Notify integration partners of new contract versions

### 2. ZKVM Program Releases

#### Pre-Release Checklist
- [ ] Verify program compiles: `cd program && cargo build --release`
- [ ] Generate verification key: `cd script && cargo run --release -- --vkey`
- [ ] Test proof generation and verification
- [ ] Benchmark performance changes
- [ ] Update program documentation

#### Release Steps
1. **Create program tag**: `git tag program-v2.1.0`
2. **Push program tag**: `git push origin program-v2.1.0`
3. **Automated circuit deployment**: GitHub Actions builds and deploys to Sindri with tag `program-v2.1.0`
4. **Circuit testing**: Automated testing with deployed circuit
5. **Generate verification key**: Extract vkey from deployed circuit
6. **Test compatibility**: Ensure circuit works with current contracts
7. **Create GitHub release**: Include circuit tag and performance metrics

#### Post-Release
- Update contract deployments if vkey changed (major version)
- Update web server configuration to use new program version
- Update Sindri circuit references in dependent services

#### ZK Program Dependencies
The ZK program circuit depends on changes in these directories:
- **`program/`**: Core ZKVM program logic
- **`lib/`**: Business logic, transaction processing, ADS operations
- **`script/`**: CLI tools and testing infrastructure
- **`api/`**: Proof generation/verification (affects integration)
- **`ethereum-client/`**: On-chain proof submission
- **`db/`**: Proof storage and verification tracking

**Circuit Versioning Impact**: Changes to any of these can affect the verification key and require a new circuit deployment.

### 3. Main Application Releases

#### Pre-Release Checklist
- [ ] Specify contract and program versions in release notes
- [ ] Update Docker images: `docker build -t arda-credit:vX.Y.Z .`
- [ ] Database migration testing
- [ ] Integration testing with specified contract/program versions
- [ ] Load testing
- [ ] Update documentation and API specs

#### Release Steps
1. **Update version references**:
   ```toml
   # In api/Cargo.toml or environment configuration
   CONTRACT_VERSION = "contracts-v1.2.3"
   PROGRAM_VERSION = "program-v2.1.0"
   ```

2. **Tag the release**: `git tag v3.4.5`

3. **Build and push Docker images**:
   ```bash
   docker build -t ghcr.io/ardaglobal/arda-credit:v3.4.5 .
   docker push ghcr.io/ardaglobal/arda-credit:v3.4.5
   ```

4. **Update docker-compose.yml**:
   ```yaml
   services:
     server:
       image: ghcr.io/ardaglobal/arda-credit:v3.4.5
   ```

5. **Create GitHub release** with comprehensive release notes

## Version Compatibility Matrix

> [!NOTE] Contract versions reference RC deployments. The final version tag points to the last successful RC deployment.
> [!NOTE] this is a placeholder for the version compatibility matrix.

| Main App Version | Contract Version | Program Version | SP1 SDK Version |
|------------------|------------------|-----------------|-----------------|
| v1.0.0           | contracts-v1.0.0-rc2 | program-v1.0.0  | 5.2.1          |
| v1.1.0           | contracts-v1.0.0-rc2 | program-v1.1.0  | 5.2.1          |
| v1.2.0           | contracts-v1.1.0-rc1 | program-v1.1.0  | 5.2.1          |
| v2.0.0           | contracts-v2.0.0-rc0 | program-v2.0.0  | 5.3.0          |

**Program Circuit Tags**: Each program version corresponds to a Sindri circuit tag (e.g., `program-v1.0.0` → circuit `program-v1.0.0`).

## Configuration Management

### Environment Variables
```bash
# Contract Configuration
CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
CONTRACT_VERSION=contracts-v1.2.3-rc0  # References RC deployment

# Program Configuration
PROGRAM_VKEY=0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e
PROGRAM_VERSION=program-v2.1.0

# Application Configuration
APP_VERSION=v3.4.5
```

### Version Tracking in Code

#### Rust Configuration (`api/src/config.rs`)
```rust
#[derive(Debug, Clone)]
pub struct VersionConfig {
    pub app_version: &'static str,
    pub contract_version: &'static str,
    pub contract_address: String,
    pub program_version: &'static str,
    pub program_vkey: String,
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION"),
            contract_version: env!("CONTRACT_VERSION"), // e.g., "contracts-v1.2.3-rc0"
            contract_address: std::env::var("CONTRACT_ADDRESS")
                .expect("CONTRACT_ADDRESS must be set"), // From RC deployment
            program_version: env!("PROGRAM_VERSION"),
            program_vkey: std::env::var("PROGRAM_VKEY")
                .expect("PROGRAM_VKEY must be set"),
        }
    }
}
```

## Release Notes Template

### Main Application Release (v3.4.5)
**Component Versions:**
- Smart Contracts: contracts-v1.2.3-rc0 (0x1234...7890)
- ZKVM Program: program-v2.1.0 (circuit: program-v2.1.0, vkey: 0x0062...4e0e)
- SP1 SDK: 5.2.1

**Changes:**
- **Added**: New API endpoints for batch operations
- **Changed**: Improved database query performance
- **Fixed**: Memory leak in proof verification
- **Security**: Updated dependencies for CVE fixes

**Migration Notes:**
- Database migration required: `sqlx migrate run`
- Environment variable changes: Added `NEW_CONFIG_VAR`
- API breaking changes: `/v1/old-endpoint` deprecated
