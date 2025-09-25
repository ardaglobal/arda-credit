# AGENTS.md

This file provides guidance to AI agents when working with code in this repository.

## Overview

**Arda Credit** is a privacy-preserving deal vApp (verifiable app) built on SP1 zero-knowledge proofs. The MVP enables deal origination, payments, and closeouts with KYC verification, plus **liquidity pool creation and management** for investors. All operations maintain transaction privacy through batch processing and ZK proofs.

### Core Deal Features

**User Types & KYC:**
- **Investors** and **Originators** - both can be KYC'd or non-KYC'd
- **KYC Status** - used in transaction flows for compliance
- **Account Registration** - user registration with KYC verification

**USDC Deposit & Withdrawal System:**
- **Smart Contract Integration** - Direct USDC deposits/withdrawals to/from vApp contract
- **Automatic Balance Tracking** - Database triggers update user balances
- **Deposit History** - Complete audit trail of all deposits and confirmations
- **Withdrawal Processing** - KYC validation, balance verification, and transaction tracking
- **Multi-table Architecture** - Separate deposit/withdrawal tracking and user balance systems

**Deal Operations:**
- **Deal Origination** - create new deals between parties
- **Deal Payments** - make payments toward outstanding deals
- **Deal Closeout** - final settlement and deal completion
- **Deal Marketplace** - list deals for sale, browse available deals, purchase deals
- **Deal Transfers** - transfer deal ownership between investors with validation

**Liquidity Pool Management:**
- **Pool Creation** - investors can create yield-bearing liquidity pools with deployment rules
- **Pool Funding** - deposit USDC into pools for credit line matching
- **Pool Configuration** - LTV ratios, interest rates, repayment periods per pool
- **Pool State Tracking** - total deposits, available/utilized liquidity, status management

**Privacy & Verification:**
- Users can **prove transaction inclusion** (deal payment, origination, closeout, KYC status)
- **Batch processing** keeps individual transaction details private
- **Zero-knowledge proofs** verify state transitions without revealing private data

### Core Architecture

**Three-Component Architecture:**
- **Smart Contracts** (`contracts/`) - On-chain proof verification and state management
- **ZKVM Program** (`program/`) - Zero-knowledge transaction processing
- **Web Server Application** (Main repo) - User interface, API, and orchestration

For detailed architecture information, see [`docs/architecture/three-component-overview.md`](docs/architecture/three-component-overview.md).

**Component Structure:**
- **`program/`** - SP1 zkVM program for privacy-preserving computations
- **`script/`** - Local testing with fast SP1 Core proofs
- **`cli/`** - Command-line interface for deal operations and verification
- **`api/`** - REST/GraphQL API server with ZK proof generation
- **`db/`** - PostgreSQL database with ADS (Authenticated Data Structures)
- **`contracts/`** - Solidity contracts for on-chain proof verification
- **`lib/`** - Shared computation logic (zkVM compatible)
- **`ethereum-client/`** - Ethereum integration utilities

## Essential Development Commands

### Quick Setup
```bash
# Complete automated setup
make setup                           # Install deps + copy .env + init database

# Individual steps
make install                         # Install all dependencies (Rust, SP1, Foundry, Docker, etc.)
make env                            # Copy .env.example to .env
make initDB                         # Initialize database with migrations + SQLx cache

# Deploy circuit to Sindri (required for proofs)
make deploy                         # Deploy circuit using SINDRI_CIRCUIT_TAG from .env
```

### Development Workflow
```bash
# Fast local SP1 development (no database required)
make run                           # ~8s Core proofs: test deal operations

# Full stack development
make up                             # Start database + server (uses pre-built image)
make up-dev                         # Start with local Docker build
make down                           # Stop all services

# Local API server (alternative to Docker)
docker-compose up postgres -d      # Start just database
make server                         # Run server locally with debug logging
```

### CLI Commands (Primary Interface)

**Make-based CLI usage:**
```bash
# Pattern: make cli ARGS="command arguments"
make cli ARGS="health-check"
make cli ARGS="register-user --user-type investor --email user@example.com"
make cli ARGS="create-pool --name \"My Pool\" --max-ltv-ratio 0.8"
```

**Key CLI commands:**
```bash
# Health and user management
make cli ARGS="health-check"
make cli ARGS="register-user --user-type investor --email user@example.com"
make cli ARGS="submit-kyc --user-id 123"
make cli ARGS="get-user --user-id 123"

# Liquidity pools
make cli ARGS="create-pool --name \"My Pool\" --max-ltv-ratio 0.8 --max-interest-rate 0.12 --repayment-period-days 365"
make cli ARGS="list-pools --verbose"
make cli ARGS="get-pool --pool-id <pool-id> --verbose"

# Deal operations (consolidated functionality)
make cli ARGS="deal originate --originator-id 123 --investor-id 456 --amount 10000"
make cli ARGS="deal payment --deal-id 789 --amount 1000"
make cli ARGS="deal closeout --deal-id 789"
make cli ARGS="deal list --status active --limit 20"
make cli ARGS="deal get --deal-id 789"

# Deal marketplace operations (consolidated into deal commands)
make cli ARGS="deal list-for-sale --deal-id 789 --asking-price 9500 --seller-id 456"
make cli ARGS="deal browse-marketplace --limit 10"
make cli ARGS="deal remove-from-sale --deal-id 789"
make cli ARGS="deal marketplace-stats"

# Deal transfer operations (consolidated into deal commands)
make cli ARGS="deal transfer --deal-id 789 --from-investor-id 456 --to-investor-id 101 --transfer-price 9500"
make cli ARGS="deal transfer-history --deal-id 789"
make cli ARGS="deal validate-transfer --deal-id 789 --current-investor-id 456"
make cli ARGS="deal user-transfer-history --user-id 456"

# USDC operations
make cli ARGS="get-user-balance-by-address --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB"
make cli ARGS="initiate-withdrawal --user-address 0x742E... --recipient-address 0x123... --amount 100.50"
make cli ARGS="get-withdrawal-status --request-id 1"

# Batch management
make cli ARGS="view-pending"
make cli ARGS="trigger-batch --verbose"
make cli ARGS="list-batches"
make cli ARGS="download-proof --batch-id 1"

# Transaction verification
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id abc123 --transaction-type deal_payment"
```

**Complete command reference:** Run `make help` to see all available commands.

### Testing & Building
```bash
# Test suites
make test                           # All tests with database
cargo test                          # Unit tests only (no database)
make cover                          # Generate code coverage report

# Smart contracts
make forge-build                    # Build contracts
make forge-test                     # Run contract tests
cd contracts && forge test --match-contract LoanManagementTest  # Specific tests

# Code quality
make fmt                            # Format code
make lint                           # Lint code
make lint-rust                      # Rust-specific linting
```

### Email Debug Mode (Bypass AWS SES)

**For development and testing without AWS SES:**
```bash
# Enable debug mode in environment
echo "EMAIL_DEBUG_MODE=true" >> .env

# Magic links will be logged to console instead of sent via email
# Example output:
# 🔗 DEBUG MODE: Magic link for matt@arda.xyz:
#       https://app.arda.xyz/auth/verify?token=abc123&state=xyz789
#       Expires in 10 minutes
#       Click this link to authenticate (expires in 10 minutes)

# Use the logged URL for manual testing of authentication flow
```

**Configuration Options:**
```bash
# Email configuration
EMAIL_DEBUG_MODE=true           # Skip AWS SES, log links only
EMAIL_ENABLED=true              # Must be true for debug mode to work
EMAIL_FROM=test@example.com    # From address for email templates
EMAIL_REPLY_TO=support@arda.xyz # Reply-to address
MAGIC_BASE_URL=https://app.arda.xyz/auth/verify  # Base URL for magic links
```

### Smart Contract Deployment

**Production deployment:**
```bash
# Deploy Arda Credit ARDA contract (set env vars first)
make deploy-contract
make deploy-contract-help           # Show required environment variables
```

**Local development with Anvil:**
```bash
# Start local Ethereum testnet
cd contracts
anvil                               # Runs on http://127.0.0.1:8545 (Chain ID: 31337)

# Build and deploy contracts
cd contracts
forge build                         # Compile all contracts

# Deploy SP1MockVerifier (for testing)
forge create lib/sp1-contracts/contracts/src/SP1MockVerifier.sol:SP1MockVerifier \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast

# Deploy ARDA contract (replace VERIFIER_ADDRESS with deployed address above)
forge create src/ARDA.sol:ARDA \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast \
  --constructor-args VERIFIER_ADDRESS 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

# Interact with deployed contracts
cast call CONTRACT_ADDRESS "verifier()" --rpc-url http://127.0.0.1:8545
cast call CONTRACT_ADDRESS "owner()" --rpc-url http://127.0.0.1:8545
```

## High-Level Architecture

### Three-Component Architecture

Arda Credit implements a **three-component architecture** with clean separation of concerns:

1. **Smart Contracts** (`contracts/`) - On-chain proof verification and state management
2. **ZKVM Program** (`program/`) - Zero-knowledge transaction processing
3. **Web Server Application** (Main repo) - User interface, API, and orchestration

For detailed architecture information, see [`docs/architecture/three-component-overview.md`](docs/architecture/three-component-overview.md).

### Deal Processing Flow
1. **User Registration**: Users register as investors/originators with optional KYC
2. **USDC Deposits**: Users deposit USDC to smart contract, automatically updating internal balances
3. **Deal Origination**: KYC'd users create deals (queued in `deal_transactions` table)
4. **Payment Processing**: Deal payments batched and processed via ZK proofs
5. **USDC Withdrawals**: KYC'd users withdraw USDC with balance validation and smart contract execution
6. **State Transitions**: Deal states (active, paid, closed) updated with cryptographic verification
7. **Marketplace Trading**: Deals can be listed for sale, browsed, and transferred between investors
8. **Ownership Transfers**: Deal ownership changes with validation and cryptographic verification
9. **Contract Posting**: Background process posts proven deal state changes to Ethereum

### Zero-Knowledge Properties

For detailed privacy guarantees and ZK properties, see [`docs/architecture/three-component-overview.md#zero-knowledge-properties`](docs/architecture/three-component-overview.md#zero-knowledge-properties).

**Core Privacy Guarantees:**
- Individual deal amounts, payments, and user details remain hidden
- Only state root transitions and transaction counts are public
- Users can prove their specific transaction was included without revealing others
- Batch processing maintains privacy while enabling regulatory compliance

## Environment Configuration

**Setup environment variables:**
```bash
# Copy and configure environment file
make env                            # Copies .env.example to .env
# Edit .env to add your SINDRI_API_KEY and other required variables
```

**Configuration details:** See `.env.example` for complete configuration options and required environment variables.

**Key variables for development:**
- `DATABASE_URL` - PostgreSQL connection string
- `SINDRI_API_KEY` - API key for proof generation (get from https://sindri.app)
- `SINDRI_CIRCUIT_TAG` - Circuit version for ZK proofs
- `ETHEREUM_RPC_URL` - Ethereum RPC endpoint for contract interactions
- `ETHEREUM_ARDA_CONTRACT_ADDRESS` - Deployed ARDA contract address (was LOAN_CONTRACT_ADDRESS)
- `DEAL_PROGRAM_VKEY` - Deal program verification key (was LOAN_PROGRAM_VKEY)
- `EMAIL_DEBUG_MODE` - Set to `true` to bypass AWS SES and log magic links to console
- `EMAIL_ENABLED` - Set to `true` to enable email functionality

**AI functionality variables (choose one provider):**
- `OPENAI_API_KEY` - OpenAI API key for GPT models
- `ANTHROPIC_API_KEY` - Anthropic API key for Claude models
- `AZURE_OPENAI_KEY` + `AZURE_OPENAI_ENDPOINT` - Azure OpenAI configuration
- `OPENROUTER_API_KEY` - OpenRouter API key for multi-model access
- `OPENROUTER_SITE_URL` + `OPENROUTER_SITE_NAME` - OpenRouter site info (optional)

For production deployment, see [`docs/deployment/security-production.md`](docs/deployment/security-production.md).

## Deal Data Structures

### Core Types
```rust
// User account with KYC status
struct User {
    user_id: uuid::Uuid,
    user_type: UserType,  // Investor | Originator
    email: String,
    kyc_status: KycStatus,  // NotSubmitted | Pending | Approved | Rejected
    kyc_verified_at: Option<DateTime<Utc>>,
}

// Deal record (converted from Loan)
struct Deal {
    deal_id: uuid::Uuid,
    originator_id: uuid::Uuid,  // was borrower_id
    investor_id: uuid::Uuid,    // was lender_id
    principal_amount: Decimal,
    outstanding_balance: Decimal,
    interest_rate_bps: i32,
    loan_status: String,  // keeping same field name: originated | active | paid_off | closed | defaulted
    originated_at: DateTime<Utc>,
    maturity_date: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// Deal payment (converted from Payment)
struct DealPayment {
    payment_id: uuid::Uuid,
    deal_id: uuid::Uuid,      // was loan_id
    payer_id: uuid::Uuid,
    amount: Decimal,
    payment_type: String, // regular | principal | interest | penalty
    transaction_hash: Option<String>,
    processed_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

// Deal transaction for ZK proof integration (converted from LoanTransaction)
struct DealTransaction {
    transaction_id: uuid::Uuid,
    deal_id_hash: Option<[u8; 32]>,  // was loan_id_hash
    transaction_type: TransactionType,  // DealOrigination | DealPayment | DealCloseout
    amount: Decimal,
    created_at: DateTime<Utc>,
}

// Deal marketplace listing (converted from LoanListing)
struct DealListing {
    listing_id: uuid::Uuid,
    deal_id: uuid::Uuid,      // was loan_id
    seller_id: uuid::Uuid,
    asking_price: Decimal,
    description: Option<String>,
    listed_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    status: String, // active | sold | cancelled | expired
}

// Deal ownership transfer (converted from LoanTransfer)
struct DealTransfer {
    transfer_id: uuid::Uuid,
    deal_id: uuid::Uuid,          // was loan_id
    from_investor_id: uuid::Uuid, // was from_lender_id
    to_investor_id: uuid::Uuid,   // was to_lender_id
    transfer_price: Decimal,
    transfer_date: DateTime<Utc>,
    status: String, // pending | completed | cancelled
}

// USDC deposit from smart contract (unchanged)
struct UsdcDeposit {
    id: i32,
    user_address: String,  // Ethereum wallet address
    amount: Decimal,       // Amount in wei (6 decimals for USDC)
    transaction_hash: String,
    status: String,        // pending | confirmed | processed | failed
    confirmation_count: i32,
    block_number: i64,
    created_at: DateTime<Utc>,
}
```

## Common Development Patterns

### Adding New Deal Operations
1. Add transaction type to `TransactionType` enum in `lib/src/transactions.rs`
2. Implement handler in `api/src/deal_handlers.rs`
3. Add CLI command in `cli/src/deals/mod.rs`
4. Add database migration if needed
5. Test with `make cli ARGS="deal your-new-command"`

### KYC Integration
1. KYC status tracked in `users` table
2. KYC verification can be mocked via CLI/API
3. Transaction validation checks KYC status for restricted operations
4. Users can prove KYC status inclusion in batches

### Database Changes
1. Create migration in `db/migrations/`
2. Run `make initDB` to apply and regenerate SQLx cache
3. Update relevant structs in `db/src/`

### Smart Contract Updates
1. Modify contracts in `contracts/src/`
2. Test: `make forge-test`
3. Deploy: `make deploy-contract` (after setting env vars)
4. Update `ETHEREUM_ARDA_CONTRACT_ADDRESS` in environment

### Circuit Changes
1. Modify SP1 program in `program/src/main.rs` for deal transaction processing
2. Test locally: `make run`
3. Deploy to Sindri: `make deploy`
4. Update `SINDRI_CIRCUIT_TAG` if using specific version

## Deal Workflow Examples

### USDC Deposit Flow
```bash
# 1. User deposits USDC to smart contract (triggers database update)
# 2. Check deposit balance
make cli ARGS="get-user-balance-by-address --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 3. Link wallet to user account
make cli ARGS="link-wallet --user-id user-uuid --wallet-address 0x742E..."

# 4. View deposit history (database queries)
psql $DATABASE_URL -c "SELECT * FROM usdc_deposits WHERE user_address = '0x742E...';"
psql $DATABASE_URL -c "SELECT * FROM user_deposit_balances WHERE user_address = '0x742E...';"
```

### Complete Deal Lifecycle
```bash
# 1. Register users
make cli ARGS="register-user --user-type originator --email originator@example.com"
make cli ARGS="register-user --user-type investor --email investor@example.com"

# 2. Submit KYC (both users)
make cli ARGS="submit-kyc --user-id originator-uuid"
make cli ARGS="submit-kyc --user-id investor-uuid"

# 3. Originate deal (KYC required)
make cli ARGS="deal originate --originator-id originator-uuid --investor-id investor-uuid --amount 50000"

# 4. Make payments
make cli ARGS="deal payment --deal-id deal-uuid --amount 5000"
make cli ARGS="deal payment --deal-id deal-uuid --amount 5000"

# 5. Create batch and generate proofs
make cli ARGS="trigger-batch --verbose"

# 6. Verify payment inclusion (trustless)
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id payment-uuid --transaction-type deal_payment"
```

### Complete Liquidity Pool Lifecycle
```bash
# 1. Register as investor and get KYC approval
make cli ARGS="register-user --user-type investor --email investor@example.com"
make cli ARGS="submit-kyc --user-id <investor-uuid>"

# 2. Create a new liquidity pool with deployment terms
make cli ARGS="create-pool --name \"Conservative Credit Pool\" --max-ltv-ratio 0.75 --max-interest-rate 0.10 --repayment-period-days 180"

# 3. List all pools (with status filter)
make cli ARGS="list-pools --status ACTIVE --verbose"

# 4. Check specific pool details
make cli ARGS="get-pool --pool-id <pool-uuid> --verbose"

# 5. Update pool status (pause/reactivate)
make cli ARGS="update-pool-status --pool-id <pool-uuid> --status PAUSED"

# 6. Find matching pools for credit deployment
make cli ARGS="find-matching-pools --required-liquidity 50000 --max-ltv 0.80 --max-rate 0.12 --min-repayment-days 90"

# 7. Pool operations are included in batch processing for ZK proofs
make cli ARGS="trigger-batch --verbose"

# 8. Verify pool creation was included in ZK proof (trustless verification)
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id <pool-creation-uuid> --transaction-type pool_creation"
```

### Complete Deal Marketplace Lifecycle
```bash
# 1. Create and fund a deal
make cli ARGS="deal originate --originator-id originator-uuid --investor-id investor-uuid --amount 50000"

# 2. List deal for sale (by current investor)
make cli ARGS="deal list-for-sale --deal-id deal-uuid --asking-price 47500 --seller-id investor-uuid --description \"Great investment opportunity\""

# 3. Browse marketplace
make cli ARGS="deal browse-marketplace --limit 10 --verbose"

# 4. Get marketplace statistics
make cli ARGS="deal marketplace-stats --verbose"

# 5. Remove from sale (if needed)
make cli ARGS="deal remove-from-sale --deal-id deal-uuid"

# 6. All marketplace operations are included in batch processing for ZK proofs
make cli ARGS="trigger-batch --verbose"

# 7. Verify deal listing was included in ZK proof (trustless verification)
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id <listing-uuid> --transaction-type deal_listing"
```

### Complete Deal Transfer Lifecycle
```bash
# 1. Validate transfer is possible
make cli ARGS="deal validate-transfer --deal-id deal-uuid --current-investor-id current-investor-uuid"

# 2. Execute deal ownership transfer
make cli ARGS="deal transfer --deal-id deal-uuid --from-investor-id current-investor-uuid --to-investor-id new-investor-uuid --transfer-price 47500"

# 3. Check transfer history for the deal
make cli ARGS="deal transfer-history --deal-id deal-uuid --limit 5"

# 4. Check user's transfer history
make cli ARGS="deal user-transfer-history --user-id current-investor-uuid --limit 10"

# 5. All transfer operations are included in batch processing for ZK proofs
make cli ARGS="trigger-batch --verbose"

# 6. Verify transfer inclusion in ZK proof (trustless verification)
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id transfer-transaction-uuid --transaction-type deal_transfer"
```

### Complete Withdrawal Lifecycle with ZK Proof Integration
```bash
# 1. Check available balance for withdrawals
make cli ARGS="get-available-balance --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 2. Initiate withdrawal (requires KYC approval + sufficient balance)
make cli ARGS="initiate-withdrawal --user-address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB --recipient-address 0x123456789012345678901234567890123456789 --amount 100.50"

# 3. Check withdrawal status
make cli ARGS="get-withdrawal-status --request-id 1"

# 4. View withdrawal history
make cli ARGS="get-withdrawal-history --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 5. Cancel withdrawal (only if not yet submitted to blockchain)
make cli ARGS="cancel-withdrawal --request-id 1"

# 6. Admin: View pending withdrawals
make cli ARGS="get-pending-withdrawals --limit 10"

# 7. Create batch and generate ZK proof (includes withdrawal transactions)
make cli ARGS="trigger-batch --verbose"

# 8. Verify withdrawal inclusion in ZK proof (trustless verification)
make cli ARGS="verify-transaction-inclusion --proof-file proof_batch_1.json --transaction-id withdrawal-transaction-uuid --transaction-type user_withdrawal"
```

## Debugging Tips

### SQLx Offline Mode
For development without database:
```bash
export SQLX_OFFLINE=true
cargo check --workspace     # Works without PostgreSQL
```

### Common Issues
- **Proof generation fails**: Check `SINDRI_API_KEY` and circuit deployment
- **Database connection errors**: Ensure PostgreSQL running via `make up`
- **CLI errors**: Verify `ARDA_CREDIT_API_URL` environment variable
- **KYC validation errors**: Check user KYC status before deal operations
- **Contract deployment fails**: Check all Ethereum environment variables are set

### Service URLs
- Database: `localhost:5432`
- API Server: `http://localhost:8080`
- Health Check: `http://localhost:8080/api/v1/health`
- GraphQL Playground: `http://localhost:8080/playground` (dev only)
- **Swagger/OpenAPI Documentation**: `http://localhost:8080/swagger-ui/`
- **AI Document Analysis**: `http://localhost:8080/api/v1/documents/{id}/analyze`
- **AI Chat Interface**: `http://localhost:8080/api/v1/ai/chat`
- **AI Analytics Dashboard**: `http://localhost:8080/api/v1/ai/analytics/dashboard`

### Key API Endpoints (Updated)
- **Deal Management**:
  - `POST /api/v1/deal` - Create deal
  - `GET /api/v1/deals` - List deals
  - `GET /api/v1/deal/{id}` - Get deal details
  - `PUT /api/v1/deal/{id}` - Update deal
  - `DELETE /api/v1/deal/{id}` - Delete deal
- **Deal Marketplace** (consolidated):
  - `GET /api/v1/deals/for-sale` - Browse marketplace
  - `POST /api/v1/deal/{id}/list-for-sale` - List deal for sale
  - `DELETE /api/v1/deal/{id}/remove-from-sale` - Remove from sale
  - `GET /api/v1/deals/marketplace/stats` - Marketplace statistics
- **Deal Transfers** (consolidated):
  - `POST /api/v1/deal/{id}/transfer` - Transfer ownership
  - `GET /api/v1/deal/{id}/transfers` - Get transfer history
  - `POST /api/v1/deal/{id}/transfer/validate` - Validate transfer
  - `GET /api/v1/user/{id}/deal-transfers` - User transfer history

## Documentation References

For detailed information on specific topics, see the [`docs/`](docs/) directory:

### Development & Setup
- **Local Development Setup**: [`docs/deployment/development-setup.md`](docs/deployment/development-setup.md)
- **Local Testing with GitHub Actions**: [`docs/development/local-testing.md`](docs/development/local-testing.md)

### Architecture & Design
- **Three-Component Architecture**: [`docs/architecture/three-component-overview.md`](docs/architecture/three-component-overview.md)
- **Deal System Architecture**: [`docs/architecture/deal-system.md`](docs/architecture/deal-system.md)
- **Performance Targets**: [`docs/architecture/performance-targets.md`](docs/architecture/performance-targets.md)

### Integration & APIs
- **API Documentation**: [`docs/integrations/api-swagger.md`](docs/integrations/api-swagger.md)
- **Authentication Integration**: [`docs/integrations/authentication.md`](docs/integrations/authentication.md)
- **Frontend Examples**: [`docs/integrations/frontend-examples.md`](docs/integrations/frontend-examples.md)

### Deployment & Operations
- **Production Security**: [`docs/deployment/security-production.md`](docs/deployment/security-production.md)
- **Monitoring Setup**: [`docs/deployment/monitoring.md`](docs/deployment/monitoring.md)

## System Conversion Summary

This system has been converted from loan-based terminology to deal-based terminology for improved clarity and business alignment.

### Key Changes Made

**Terminology Updates:**
- Loan → Deal (throughout system)
- Borrower → Originator (user who requests the deal)
- Lender → Investor (user who provides capital)
- All database tables: `loans` → `deals`, `loan_transactions` → `deal_transactions`, etc.
- All API endpoints: `/loans` → `/deals`, `/loan/{id}` → `/deal/{id}`, etc.
- All CLI commands: `loan originate` → `deal originate`, etc.

**Functional Consolidation:**
- **Marketplace functionality** consolidated into deal endpoints
- **Transfer functionality** consolidated into deal endpoints
- **Single unified CLI** for all deal operations (CRUD, marketplace, transfers)
- **Single unified API** for all deal operations

**Smart Contract Updates:**
- Contract renamed from "Loan" to "ARDA" for generic branding
- Internal functions use deal terminology (`verifyDealProof`, `DealBatchPublicValues`)
- Same security and functionality, updated terminology

**Maintained Compatibility:**
- Business logic and validation rules unchanged
- Database field `loan_status` kept for internal compatibility
- ZK proof structure and verification unchanged
- Same privacy guarantees and regulatory compliance

### Migration Impact

**What Changed:**
- All user-facing terminology (APIs, CLI, documentation)
- Database table and column names
- Function and variable names throughout codebase
- Smart contract name and internal references

**What Stayed the Same:**
- Business logic and transaction validation
- Zero-knowledge proof generation and verification
- Privacy guarantees and regulatory compliance features
- Database relationships and constraints
- Performance characteristics and scalability
- Security model and access controls

**Backward Compatibility:**
- CLI includes legacy commands with deprecation warnings
- Database migration handles the schema transition
- No breaking changes to core functionality

See [`docs/README.md`](docs/README.md) for the complete documentation hub.