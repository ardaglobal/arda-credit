# CONTRIBUTORS.md

This document serves as the comprehensive guide for contributors to the arda-credit repository, consolidating coding standards, development practices, and release management procedures.

## Overview

## Table of Contents

- [Development Standards](#development-standards)
- [Release Management](#release-management)
- [AI Agent Interaction Guidelines](#ai-agent-interaction-guidelines)

## Development Standards

### Primary Interface: Make Commands

The repository uses Make commands as the primary interface for all development tasks. Essential commands include:

#### Setup & Configuration
```bash
make setup                           # Complete automated setup (install deps + copy .env + init database)
make install                         # Install all dependencies (Rust, SP1, Foundry, Docker, etc.)
make env                            # Copy .env.example to .env
make initDB                         # Initialize database with migrations + SQLx cache
make setup-hooks                    # Setup git hooks for pre-commit linting and formatting
```

#### Development Workflow
```bash
make run                           # Fast local SP1 development (~3.5s Core proofs)
make up                            # Start database + server (uses pre-built image)
make down                          # Stop all services
make server                        # Start API server locally (debug mode with verbose logging)
# Test API via curl or Swagger UI at http://localhost:8080/swagger-ui/
```

#### Code Quality & Testing
```bash
make fmt                           # Format code (Rust + YAML + GitHub Actions)
make lint                          # Lint code (Rust + YAML + GitHub Actions + Security)
make test                          # Run all tests with database
make cover                         # Generate code coverage report
make deny                          # Security audit dependencies
```

#### Smart Contracts
```bash
make forge-build                   # Build smart contracts
make forge-test                    # Run smart contract tests
make deploy-contract               # Deploy Arda Credit loan contract
make deploy-contract-help          # Show deployment requirements
```

#### Infrastructure Management
```bash
make clean                         # Clean up all build artifacts and resources
make docker-build                  # Build Docker image locally
make docker-push                   # Build and push Docker image
make helm-deps                     # Update Helm chart dependencies
make helm-upgrade                  # Upgrade Arda Credit Helm deployment
```

### Git Hooks Setup

Pre-commit and pre-push hooks ensure code quality:

```bash
make setup-hooks                    # Configures automatic linting and formatting
```

- **Pre-commit**: Runs `make fmt` and `make lint` automatically
- **Pre-push**: Runs `make test` automatically

### File Organization Standards

#### Core Architecture
```
arda-credit/
├── program/                        # SP1 zkVM program (RISC-V)
├── script/                         # Local SP1 testing and utilities
├── cli/                           # Unified CLI tool
├── api/                           # Production server with ZK proof generation
├── db/                            # PostgreSQL with Merkle tree state management
├── contracts/                     # Solidity proof verification contracts
├── lib/                           # Shared computation logic (zkVM compatible)
├── ethereum-client/               # Ethereum integration
└── helm/                          # Kubernetes deployment charts
```

#### CLI Module Organization

**Single-File Modules** (under 1000 lines):
```
cli/src/
├── lib.rs                         # Main exports and module declarations
├── cli.rs                         # Core CLI setup and middleware
├── balance.rs                     # Balance and wallet operations
├── batch.rs                       # Batch processing and proof operations
├── pools.rs                       # Liquidity pool operations
├── user.rs                        # User management and KYC operations
```

**Directory Modules** (when functionality exceeds 1000 lines):
```
├── loans/                         # Credit and loan operations
│   ├── mod.rs                     # Main module with command routing
│   ├── investors.rs               # Investor registration and profiles
│   ├── merchants.rs               # Merchant registration and turnover
│   ├── credit.rs                  # Credit line eligibility and requests
│   └── operations.rs              # Loan origination, payments, closeouts
```

**Feature-Gated Modules** (optional functionality):
```
├── benchmarks/                    # Performance benchmarks (--features testing)
│   ├── mod.rs                     # Main benchmark module
│   ├── analytics.rs               # Analytics and statistics
│   └── performance.rs             # Performance testing
├── tests/                         # Integration tests
│   └── integration_tests.rs       # End-to-end workflow tests
```

#### Single Responsibility Principle

- **Files must not exceed 1000 lines of code**
- Each file handles one clear, focused responsibility
- Large modules split into logical, focused sub-modules

#### Module Splitting Pattern

When a module exceeds 1000 lines, follow this pattern:

```rust
// loans/mod.rs - Main module with command routing
pub mod investors;
pub mod merchants;
pub mod credit;
pub mod operations;

pub use investors::*;
pub use merchants::*;
pub use credit::*;
pub use operations::*;

#[derive(Subcommand)]
pub enum LoanCommands {
    // All loan-related commands
}

pub async fn handle_loan_command(/* ... */) -> Result<()> {
    // Route to appropriate sub-module
}
```

**Sub-module organization by responsibility**:
- `investors.rs` (343 lines) - Investor registration and profiles
- `merchants.rs` (287 lines) - Merchant registration and turnover data
- `credit.rs` (283 lines) - Credit line eligibility and requests
- `operations.rs` (286 lines) - Loan origination, payments, closeouts

### Coding Standards

#### API Endpoint Documentation

All API endpoints must have comprehensive documentation:

```rust
/// Create a new user account
Create {
    /// User type (user, investor, originator, admin)
    #[arg(long)]
    user_type: String,
    /// User email address
    #[arg(long)]
    email: String,
    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
},
```

#### Function Documentation

All public functions must include:
- Purpose and functionality
- Parameter validation requirements
- Return values and error conditions
- Privacy implications (for ZK operations)

```rust
/// Create a new user account with KYC initialization
///
/// This function creates a user account and initializes their KYC status.
/// All user data is handled with privacy preservation in mind.
///
/// # Arguments
/// * `client` - API client for server communication
/// * `user_type` - Type of user (investor, lender, etc.)
/// * `email` - User's email address (validated)
/// * `verbose` - Enable detailed output
///
/// # Privacy Considerations
/// User data is processed with ZK privacy guarantees
pub async fn create_user(
    client: &BatchApiClient,
    user_type: &str,
    email: &str,
    verbose: bool,
) -> Result<()> {
    // Implementation with comprehensive error handling
}
```

#### Error Handling Standards

Consistent error handling across all modules:

```rust
// ✅ GOOD - Consistent error responses with user-friendly messages
match operation().await {
    Ok(result) => {
        println!("✅ Operation successful!");
        println!("   Details: {}", result);
    },
    Err(e) => {
        eprintln!("❌ Operation failed: {}", e);
        eprintln!("💡 Try: Alternative approach or troubleshooting steps");
    }
}
```

#### Output Standards

Use consistent emoji and formatting for CLI output:

```rust
// Status indicators
println!("✅ Success message");
println!("⚠️  Warning message");
println!("❌ Error message");
println!("🔄 Processing message");
println!("💡 Helpful tip");

// Section headers
println!("👤 User Operations");
println!("💰 Balance Information");
println!("🏦 Pool Details");
println!("🔧 Technical Details (verbose)");
```

### Testing Structure

#### Unit Tests
Each module should include comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_function_success() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_function_error_cases() {
        // Test error handling
    }
}
```

#### Integration Tests
- Place integration tests in `tests/` directory at the CLI package root
- Test complete command workflows end-to-end
- Validate CLI argument parsing and error handling

#### Code Review Checklist

Before submitting code for review:

- [ ] Function is in the correct module (single responsibility)
- [ ] Function does not exceed reasonable length (~100 lines)
- [ ] All parameters are validated appropriately
- [ ] Error handling follows established patterns
- [ ] Output formatting uses consistent emoji and structure
- [ ] Documentation is comprehensive and helpful
- [ ] Tests cover success and error cases

## Release Management

### Component Overview

Arda Credit consists of three interconnected components requiring coordinated versioning:

#### 1. Smart Contracts (`contracts/`)
- **Location**: `contracts/src/Loan.sol` and related contracts
- **Dependencies**: SP1 contracts, OpenZeppelin libraries
- **Deployment Target**: Ethereum networks (mainnet, testnets)
- **Versioning**: Independent semantic versioning (`contracts-vX.Y.Z`)

#### 2. ZKVM Program (`program/`)
- **Location**: `program/src/main.rs`
- **Dependencies**: SP1 ZKVM runtime (`sp1-zkvm = "5.2.1"`)
- **Build Target**: SP1 ZKVM ELF binary
- **Versioning**: Independent semantic versioning (`program-vX.Y.Z`)

#### 3. Web Server Application (Main Repository)
- **Location**: Root workspace (`api/`, `db/`, `cli/`, etc.)
- **Dependencies**: Both smart contracts and ZKVM program
- **Deployment Target**: Kubernetes via Helm charts
- **Versioning**: Main repository version referencing specific component versions

### Semantic Versioning Strategy

#### Version Format: `MAJOR.MINOR.PATCH`

**Smart Contracts (`contracts-vX.Y.Z`)**:
- **MAJOR**: Breaking changes to contract interface or state structure
- **MINOR**: New features, additional functions, backward-compatible changes
- **PATCH**: Bug fixes, gas optimizations, security patches

**ZKVM Program (`program-vX.Y.Z`)**:
- **MAJOR**: Changes affecting verification key or public inputs/outputs
- **MINOR**: Performance improvements, features not changing verification
- **PATCH**: Bug fixes, optimizations not affecting verification

**Main Application (`vX.Y.Z`)**:
- **MAJOR**: Breaking changes to API, database schema, or deployment requirements
- **MINOR**: New features, API enhancements, backward-compatible changes
- **PATCH**: Bug fixes, dependency updates, minor improvements

### Release Process

#### Smart Contract Releases

**Pre-Release Checklist**:
- [ ] Update contract version comments
- [ ] Run full test suite: `cd contracts && forge test`
- [ ] Gas optimization analysis
- [ ] Security audit (for major releases)
- [ ] Update deployment documentation

**Release Steps**:
1. **Create RC tag**: `git tag contracts-v1.2.3-rc0`
2. **Push RC tag**: `git push origin contracts-v1.2.3-rc0`
3. **Automated deployment**: GitHub Actions deploys and captures address in `contracts/deployments/contracts-v1.2.3-rc0.json`, merge PR from GHA.
4. **Verify deployment**: Ensure Etherscan verification and test functionality
5. **Create final tag**: `git tag contracts-v1.2.3` (references RC deployment)
6. **Create GitHub release**: Include deployment addresses and ABI files from RC deployment

#### Post-Release
- Update dependent services with new contract addresses
- Notify integration partners of new contract versions

#### ZKVM Program Releases

**Pre-Release Checklist**:
- [ ] Verify program compiles: `cd program && cargo build --release`
- [ ] Generate verification key: `cd script && cargo run --release -- --vkey`
- [ ] Test proof generation and verification
- [ ] Benchmark performance changes
- [ ] Update program documentation

**Release Steps**:
1. Create program tag: `git tag program-v2.1.0`
2. Push program tag: `git push origin program-v2.1.0`
3. Automated circuit deployment to Sindri with tag `program-v2.1.0`
4. Circuit testing and verification key extraction
5. Test compatibility with current contracts
6. Create GitHub release with circuit tag and performance metrics

**Post-Release**:
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

#### Main Application Releases

**Pre-Release Checklist**:
- [ ] Specify contract and program versions in release notes
- [ ] Ensure Docker images build via GitHub Actions
- [ ] Database migration testing with Helm chart
- [ ] Integration testing with specified contract/program versions
- [ ] Load testing with Helm deployment
- [ ] Update documentation and API specs

**Release Steps**:
1. **Update version references in Helm chart**:
   ```yaml
   # In helm/charts/arda-credit/values.yaml
   server:
     image:
       tag: "v3.4.5"
     config:
       sindriCircuitTag: "program-v2.1.0"  # Lock to specific circuit version

   secrets:
     app:
       ethereumContractAddress: "0x1234567890123456789012345678901234567890"  # From RC deployment
   ```

2. **Update Helm chart version**:
   ```yaml
   # In helm/charts/arda-credit/Chart.yaml
   version: 0.4.3  # Increment chart version
   appVersion: "v3.4.5"  # Lock to app version
   ```

3. **Tag the release**: `git tag v3.4.5`

4. **Automated Docker builds**: GitHub Actions builds and pushes versioned tags

5. **Test Helm deployment**:
   ```bash
   # Update Helm dependencies
   make helm-deps

   # Test deployment in staging
   helm upgrade --install arda-credit ./helm/charts/arda-credit \
     --namespace arda-staging \
     --values helm/charts/arda-credit/values-staging.yaml \
     --set server.image.tag=v3.4.5 \
     --set app.image.tag=v3.4.5
   ```

6. **Create GitHub release** with comprehensive release notes

#### Helm Chart Releases

**Pre-Release Checklist**:
- [ ] Update Helm chart dependencies: `make helm-deps`
- [ ] Test chart rendering: `helm template ./helm/charts/arda-credit`
- [ ] Validate with kubeconform or similar tools
- [ ] Update chart documentation and README
- [ ] Test deployment in development environment

**Release Steps**:
1. **Update Chart.yaml version**:
   ```yaml
   # Increment chart version for new features
   version: 0.5.0  # Major chart changes
   # or
   version: 0.4.4  # Minor chart changes
   ```

2. **Tag Helm chart release**: `git tag helm-v0.5.0`

3. **Push tag**: `git push origin helm-v0.5.0`

4. **Create GitHub release** for Helm chart with:
   - Chart version changes
   - New features added
   - Breaking changes
   - Migration instructions

#### Chart Versioning Strategy
- **Chart Version**: Independent from app version, follows SemVer
- **App Version**: References the main application version
- **Tag Format**: `helm-vX.Y.Z`
- **Release Cadence**: As needed when chart features change

### Version Compatibility Matrix

> [!NOTE] Contract versions reference RC deployments. The final version tag points to the last successful RC deployment.
> [!NOTE] this is a placeholder for the version compatibility matrix.

| Main App Version | Contract Version | Program Version | SP1 SDK Version | Helm Chart |
|------------------|------------------|-----------------|-----------------|------------|
| v1.0.0           | contracts-v1.0.0-rc2 | program-v1.0.0  | 5.2.1          | 0.1.0     |
| v1.1.0           | contracts-v1.0.0-rc2 | program-v1.1.0  | 5.2.1          | 0.2.0     |
| v1.2.0           | contracts-v1.1.0-rc1 | program-v1.1.0  | 5.2.1          | 0.3.0     |
| v2.0.0           | contracts-v2.0.0-rc0 | program-v2.0.0  | 5.3.0          | 0.4.0     |

**Program Circuit Tags**: Each program version corresponds to a Sindri circuit tag (e.g., `program-v1.0.0` → circuit `program-v1.0.0`).

**Helm Chart Compatibility**: Each main app version includes a compatible Helm chart version for deployment.

**Notes**:
- Contract versions reference RC deployments
- Program versions correspond to Sindri circuit tags
- Helm chart versions are independent from app versions

### Configuration Management

#### Contract Deployment Integration

##### vkey.json Updates
The `vk.json` file in the repository root is automatically updated during program releases:
- **Location**: `vk.json` (committed to repository)
- **Update Trigger**: Program tag pushes (`program-v*`)
- **Source**: Extracted from deployed Sindri circuit after successful deployment
- **Usage**: Used by contract deployments to ensure vkey consistency

##### Contract Address Capture
Contract deployments automatically capture and store deployment information:
- **Deployment Records**: `contracts/deployments/contracts-v1.2.3-rc0.json`
- **Captured Data**: Contract address, transaction hash, block number, network
- **Git Integration**: Records are committed back to repository
- **Helm Integration**: Addresses are injected into Helm chart values

#### Environment Variables in Helm

**Non-sensitive Configuration** (ConfigMap):
```yaml
# helm/charts/arda-credit/values.yaml
server:
  config:
    rustLog: "debug"
    serverPort: "8080"
    sindriCircuitTag: "program-v2.1.0"  # Locked to specific circuit
```

**Sensitive Configuration** (Secrets):
```yaml
# helm/charts/arda-credit/values.yaml
secrets:
  app:
    sindriApiKey: "your-sindri-api-key"
    ethereumRpcUrl: "https://eth-mainnet.alchemyapi.io/v2/demo"
    ethereumContractAddress: "0x1234567890123456789012345678901234567890"  # From RC deployment
    ethereumWalletPrivateKey: "0x..."
    ethereumDeployerAddress: "0x..."
```

#### Runtime Environment Variables
```bash
# Automatically constructed by Helm
DATABASE_URL=postgresql://user:password@host:5432/db
CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
PROGRAM_VKEY=0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e
APP_VERSION=v3.4.5
```

#### Version Tracking in Code

##### Rust Configuration (`api/src/config.rs`)
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

### Docker Build Automation

#### GitHub Actions Docker Workflow
Docker images are automatically built and published via GitHub Actions:

##### Trigger Conditions
- **Main Branch**: Pushes to `main` build `latest` tags
- **Release Tags**: Tags like `v3.4.5` build versioned tags
- **Manual Dispatch**: Manual trigger for testing

##### Image Registry
- **Server Image**: `ghcr.io/ardaglobal/arda-credit:v3.4.5`
- **Frontend Image**: `ghcr.io/ardaglobal/arda-credit-app:v3.4.5`
- **Multi-Platform**: `linux/amd64`, `linux/arm64` for releases

##### Build Configuration
```yaml
# .github/workflows/docker.yml key settings:
env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ardaglobal/arda-credit
  PLATFORM: linux/amd64,linux/arm64  # Multi-platform for releases
```

##### Cache Strategy
- **S3 Cache**: `codebuild-s3-cache-arda` bucket
- **Prefix**: `output/arda-credit`
- **Mode**: `max` (maximize cache reuse)

### Release Notes Template

#### Main Application Release (v3.4.5)

**Component Versions:**
- Smart Contracts: contracts-v1.2.3-rc0 (0x1234...7890)
- ZKVM Program: program-v2.1.0 (circuit: program-v2.1.0, vkey: 0x0062...4e0e)
- SP1 SDK: 5.2.1
- Helm Chart: 0.4.3

**Docker Images:**
- Server: `ghcr.io/ardaglobal/arda-credit:v3.4.5`
- Frontend: `ghcr.io/ardaglobal/arda-credit-app:v3.4.5`

**Changes:**
- **Added**: New API endpoints for batch operations
- **Changed**: Improved database query performance
- **Fixed**: Memory leak in proof verification
- **Security**: Updated dependencies for CVE fixes

**Migration Notes:**
- Database migration required: `sqlx migrate run`
- Helm chart values updated for new image tags
- Environment variable changes: Added `NEW_CONFIG_VAR`
- API breaking changes: `/v1/old-endpoint` deprecated

**Deployment Instructions:**
```bash
# Update Helm chart
helm upgrade --install arda-credit ./helm/charts/arda-credit \
  --set server.image.tag=v3.4.5 \
  --set app.image.tag=v3.4.5 \
  --set secrets.app.ethereumContractAddress=0x1234...7890
```

## AI Agent Interaction Guidelines

### Project Overview

**Arda Credit** is a privacy-preserving loan vApp built on SP1 zero-knowledge proofs. The MVP enables loan origination, payments, and closeouts with KYC verification, plus liquidity pool creation and management for investors. All operations maintain transaction privacy through batch processing and ZK proofs.

### Core Architecture

**Clean separation of concerns:**
- **`program/`** - RISC-V program for SP1 zkVM loan transaction processing
- **`script/`** - Local SP1 testing with fast Core proofs (development)
- **`cli/`** - Unified CLI for loan operations + local verification tool
- **`api/`** - Production server with loan processing, ZK proof generation via Sindri
- **`db/`** - PostgreSQL with loan tables, KYC data, and Merkle tree state management
- **`contracts/`** - Solidity proof verification with loan state management
- **`lib/`** - Shared computation logic (zkVM compatible)
- **`ethereum-client/`** - Ethereum integration for automated contract posting

### Essential Development Commands

#### Quick Setup
```bash
make setup                           # Install deps + copy .env + init database
make install                         # Install all dependencies
make env                            # Copy .env.example to .env
make initDB                         # Initialize database with migrations + SQLx cache
make deploy                         # Deploy circuit to Sindri (required for proofs)
```

#### Development Workflow
```bash
make run                            # Fast local SP1 development (~3.5s Core proofs)
make up                             # Start database + server (uses pre-built image)
make down                           # Stop all services
```

### API Testing (Primary Interface)

For comprehensive API documentation and testing, use the Swagger UI at `http://localhost:8080/swagger-ui/`

```bash
# Health check
curl http://localhost:8080/api/v1/health

# Liquidity Pool Management
curl -X POST http://localhost:8080/api/v1/pool \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"name":"My Pool","max_ltv_ratio":0.8,"max_interest_rate":0.12,"repayment_period_days":365}'

curl "http://localhost:8080/api/v1/pools" \
  -H "Authorization: Bearer YOUR_TOKEN"

# User & KYC Management
curl -X POST http://localhost:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{"email":"user@example.com","user_type":"investor"}'

curl -X POST http://localhost:8080/api/v1/user/123/kyc \
  -H "Authorization: Bearer YOUR_TOKEN"

# USDC Balance Management
curl "http://localhost:8080/api/v1/balance/0x742E4C1D5B5B8Ac5dC12345678901234567890aB" \
  -H "Authorization: Bearer YOUR_TOKEN"

# USDC Withdrawal Management
curl -X POST http://localhost:8080/api/v1/withdrawal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"user_address":"0x742E...","recipient_address":"0x123...","amount":"100.50"}'

curl "http://localhost:8080/api/v1/withdrawal/1" \
  -H "Authorization: Bearer YOUR_TOKEN"

# Loan/Deal Operations
curl -X POST http://localhost:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"originator_id":"123","investor_id":"456","amount":10000}'

curl -X POST http://localhost:8080/api/v1/deal/789/payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"amount":1000}'

# Batch management
curl "http://localhost:8080/api/v1/transactions/pending" \
  -H "Authorization: Bearer YOUR_TOKEN"

curl -X POST http://localhost:8080/api/v1/batch/trigger \
  -H "Authorization: Bearer YOUR_TOKEN"

curl "http://localhost:8080/api/v1/batches" \
  -H "Authorization: Bearer YOUR_TOKEN"

# For complete documentation, see: http://localhost:8080/swagger-ui/
```

### Testing
```bash
make test                           # All tests with database
make cover                          # Generate coverage report
cd db && cargo test                 # Database tests only
cd contracts && forge test          # Smart contract tests
```

### Smart Contract Deployment
```bash
make deploy-contract                # Deploy Arda Credit loan contract
make deploy-contract-help           # Show required environment variables
```

### Zero-Knowledge Properties
```rust
// Public values (visible on-chain)
struct LoanBatchPublicValues {
    bytes32 previous_state_root;       // Previous Merkle root
    bytes32 new_state_root;            // New Merkle root
    uint32 transaction_count;          // Number of transactions in batch
    uint64 batch_timestamp;            // Timestamp when batch was processed
    bytes32 transaction_types_hash;    // Hash of transaction types in batch
    uint128 previousTotalBalance;      // Previous total balance across all loans
    uint128 currentTotalBalance;       // Current total balance across all loans
    // Individual loan amounts, KYC status, user details remain private
}
```

**Privacy Guarantees**:
- Individual loan amounts, payments, and user details remain hidden
- Only state root transitions and transaction counts are public
- Users can prove their specific transaction was included without revealing others

### Key Components Integration

**API Server** (`api/src/bin/server.rs`):
- Loan transaction processing and batching
- KYC verification and user management
- Sindri integration for ZK proof generation
- Background processing with automated Ethereum posting
- Contract data generation (public/private split)

**Database Layer** (`db/`):
- User accounts with KYC status tracking
- Loan records with payment history
- 32-level indexed Merkle trees for state commitments
- Complete audit trail for all loan operations

**Smart Contracts** (`contracts/src/ArdaCredit.sol`):
- SP1 proof verification with loan state storage
- Automated background posting of proven loan batches
- Gas-optimized loan state operations

### Environment Configuration

**Required Environment Variables**:
```bash
# Database
DATABASE_URL=postgresql://postgres:password@127.0.0.1:5432/arda_credit_db

# Sindri (required for proof generation)
SINDRI_API_KEY=your_sindri_api_key_here
SINDRI_CIRCUIT_TAG=latest

# Server
RUST_LOG=info
SERVER_PORT=8080

# Smart contract integration (optional)
ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/demo
ETHEREUM_CONTRACT_ADDRESS=0x...
ETHEREUM_WALLET_PRIVATE_KEY=...
ETHEREUM_DEPLOYER_ADDRESS=0x...
VERIFIER_CONTRACT_ADDRESS=0x...
PROGRAM_VKEY=0x...
```

### Core Data Structures
```rust
// User account with KYC status
struct User {
    user_id: uuid::Uuid,
    user_type: UserType,  // Investor | Lender
    email: String,
    kyc_status: KycStatus,  // NotSubmitted | Pending | Approved | Rejected
    kyc_verified_at: Option<DateTime<Utc>>,
}

// Loan record
struct Loan {
    loan_id: uuid::Uuid,
    borrower_id: uuid::Uuid,
    lender_id: uuid::Uuid,
    principal_amount: Decimal,
    outstanding_balance: Decimal,
    interest_rate: Decimal,
    loan_status: LoanStatus,  // Originated | Active | PaidOff | Closed
    originated_at: DateTime<Utc>>,
}

// Loan transaction (payments, origination, closeout)
struct LoanTransaction {
    transaction_id: uuid::Uuid,
    loan_id: uuid::Uuid,
    transaction_type: TransactionType,  // Origination | Payment | Closeout
    amount: Decimal,
    created_at: DateTime<Utc>>,
}
```

### Common Development Patterns

#### Adding New Loan Operations
1. Add transaction type to `TransactionType` enum in `lib/src/loan_types.rs`
2. Implement handler in `api/src/loan_handlers.rs`
3. Add API endpoint route in `api/src/routes.rs`
4. Add database migration if needed
5. Test with curl or Swagger UI at `http://localhost:8080/swagger-ui/`

#### KYC Integration
1. KYC status tracked in `users` table
2. KYC verification can be managed via API endpoints
3. Transaction validation checks KYC status for restricted operations
4. Users can prove KYC status inclusion in batches

#### Database Changes
1. Create migration in `db/migrations/`
2. Run `sqlx migrate run` to apply
3. Regenerate SQLx cache: `make initDB` or `cargo sqlx prepare --workspace`
4. Update relevant structs in `db/src/`

#### Smart Contract Updates
1. Modify contracts in `contracts/src/`
2. Test: `cd contracts && forge test`
3. Deploy: `make deploy-contract` (after setting env vars)
4. Update `ETHEREUM_CONTRACT_ADDRESS` in environment

#### Circuit Changes
1. Modify SP1 program in `program/src/main.rs`
2. Test locally: `cargo run --release`
3. Deploy to Sindri: `make deploy`
4. Update `SINDRI_CIRCUIT_TAG` if using specific version

### Debugging Tips

#### SQLx Offline Mode
```bash
export SQLX_OFFLINE=true
cargo check --workspace     # Works without PostgreSQL
```

#### Common Issues
- **Proof generation fails**: Check `SINDRI_API_KEY` and circuit deployment
- **Database connection errors**: Ensure PostgreSQL running via `docker-compose up postgres -d`
- **CLI errors**: Verify `ARDA_CREDIT_API_URL` environment variable
- **KYC validation errors**: Check user KYC status before loan operations
- **Contract deployment fails**: Check all Ethereum environment variables are set

#### Service URLs
- Database: `localhost:5432`
- API Server: `http://localhost:8080`
- Health Check: `http://localhost:8080/api/v1/health`
- GraphQL Playground: `http://localhost:8080/playground` (dev only)

---

This document serves as the canonical reference for all contributors. For the most up-to-date information, always refer to this file and the AGENTS.md guide.
