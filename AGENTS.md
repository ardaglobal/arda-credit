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

# Full stack development with local Anvil devnet
make up                             # Start Postgres + Anvil (local/staging) or just Postgres (sandbox/production)
make deploy-contracts               # Deploy contracts (environment-aware)
make server                         # Run server with deposit/withdrawal monitors

# Alternative: Server auto-deploys contracts if needed
make server                         # One command to start everything

# Stop services
make down                           # Stop all services
make clean-anvil                    # Reset Anvil blockchain state
```

### Local Anvil Devnet (Local/Staging Environments)

For local and staging environments, Arda Credit uses a local Anvil devnet instead of remote testnets:

```bash
# Anvil management
make anvil-start                    # Start Anvil devnet
make anvil-stop                     # Stop Anvil (preserves state)
make anvil-logs                     # View Anvil logs
make clean-anvil                    # Reset blockchain state

# Faucet commands (get test USDC)
make faucet-request ADDRESS=0x742d35Cc6...   # Request 1M USDC (1 hour cooldown)
make faucet-status ADDRESS=0x742d35Cc6...    # Check balance and cooldown status
make faucet-stats                            # View overall faucet statistics

# Contract deployment (environment-aware)
SERVER_ENVIRONMENT=local make deploy-contracts      # Deploy to local Anvil
SERVER_ENVIRONMENT=sandbox make deploy-contracts    # Deploy to Sepolia testnet
```

**What gets deployed:**
- **Local/Staging**: MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier (to Anvil)
- **Sandbox/Production**: ARDA only (to Sepolia/Mainnet with real USDC)

**Faucet Configuration:**
- Amount: 1,000,000 USDC per request
- Cooldown: 1 hour between requests
- Initial balance: 100,000,000 USDC

See [`docs/development/local-anvil-workflow.md`](docs/development/local-anvil-workflow.md) for complete details.

### API Testing with curl

The Arda Credit API server provides a comprehensive REST API for all operations. Instead of using a CLI, interact directly with the API using standard HTTP tools.

**Basic API Examples:**
```bash
# Health check (no authentication)
curl http://localhost:8080/api/v1/health

# GET request with authentication (list deals)
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/deals

# POST request with authentication and body (create deal)
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"originator_id": "uuid", "investor_id": "uuid", "amount": 10000}' \
  http://localhost:8080/api/v1/deal

# PUT request with authentication (update deal)
curl -X PUT \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"status": "active"}' \
  http://localhost:8080/api/v1/deal/{deal_id}

# DELETE request with authentication
curl -X DELETE \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/deal/{deal_id}

# GET request with URL parameters
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/deals?status=active&limit=20"
```

**For comprehensive API documentation, see:**
- **Swagger UI**: http://localhost:8080/swagger-ui/ (interactive API documentation)
- **OpenAPI Spec**: http://localhost:8080/api-docs/openapi.json
- **Integration Guide**: [`docs/integrations/api-swagger.md`](docs/integrations/api-swagger.md)

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

**Environment-aware deployment** (recommended):
```bash
# Deploy based on SERVER_ENVIRONMENT
make deploy-contracts               # Uses SERVER_ENVIRONMENT (default: local)

# Or specify environment explicitly
SERVER_ENVIRONMENT=local make deploy-contracts      # → Anvil (with mocks)
SERVER_ENVIRONMENT=sandbox make deploy-contracts    # → Sepolia testnet
SERVER_ENVIRONMENT=production make deploy-contracts # → Mainnet

make deploy-contract-help           # Show deployment help
```

**What gets deployed:**
- **Local/Staging**: MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier (automated)
- **Sandbox/Production**: ARDA only (requires env vars: RPC URL, verifier, program key)

**Local development with Anvil** (automated):
```bash
# Anvil is automatically managed by docker-compose
make up                             # Starts Anvil + Postgres
make deploy-contracts               # Deploys all contracts to Anvil
make server                         # Auto-deploys if needed, then starts server

# Or manual Anvil management
make anvil-start                    # Start Anvil manually
make anvil-stop                     # Stop Anvil (preserves state)
make anvil-logs                     # View Anvil logs
make clean-anvil                    # Reset blockchain state

# Faucet for test USDC (local/staging only)
make faucet-request ADDRESS=0x...   # Request 1M USDC (1hr cooldown)
make faucet-status ADDRESS=0x...    # Check balance and status
make faucet-stats                   # View overall statistics
```

**Manual deployment** (legacy):
```bash
# Start Anvil manually
anvil                               # Runs on http://127.0.0.1:8545 (Chain ID: 31337)

# Deploy with forge create
cd contracts
forge create src/ARDA.sol:ARDA \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --constructor-args VERIFIER_ADDRESS PROGRAM_VKEY

# Better: Use automated deployment script
make deploy-contracts
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
- `SERVER_ENVIRONMENT` - Server environment: `local`, `staging`, `sandbox`, or `production` (default: `local`)
  - `local` and `staging` use local SP1 prover for fast development
  - `sandbox` and `production` use Sindri service for proof generation
- `SINDRI_API_KEY` - API key for proof generation (get from https://sindri.app) - required for `sandbox`/`production`
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
3. Add API endpoint route in `api/src/api/routes.rs`
4. Add database migration if needed
5. Test with curl or Swagger UI at http://localhost:8080/swagger-ui/

### KYC Integration
1. KYC status tracked in `users` table
2. KYC verification can be mocked via API for testing
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

# 2. Check deposit balance via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/user/balance?address=0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 3. Link wallet to user account via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-uuid", "wallet_address": "0x742E..."}' \
  http://localhost:8080/api/v1/user/link-wallet

# 4. View deposit history via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/deposits?address=0x742E..."
```

**Note:** For complete API endpoint details, see the Swagger UI at http://localhost:8080/swagger-ui/

### Complete Deal Lifecycle
```bash
# 1. Register users via API
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"email": "originator@example.com", "user_type": "originator"}' \
  http://localhost:8080/api/v1/user

curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"email": "investor@example.com", "user_type": "investor"}' \
  http://localhost:8080/api/v1/user

# 2. Submit KYC (both users) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "originator-uuid"}' \
  http://localhost:8080/api/v1/kyc/submit

# 3. Originate deal (KYC required) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"originator_id": "originator-uuid", "investor_id": "investor-uuid", "amount": 50000}' \
  http://localhost:8080/api/v1/deal

# 4. Make payments via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"deal_id": "deal-uuid", "amount": 5000}' \
  http://localhost:8080/api/v1/deal/payment

# 5. Create batch and generate proofs via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/batch/trigger

# 6. Verify payment inclusion via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "payment-uuid", "transaction_type": "deal_payment", "proof_file": "proof_batch_1.json"}' \
  http://localhost:8080/api/v1/verify/inclusion
```

**Note:** These are example API calls. See Swagger UI for exact endpoint paths and parameters.

### Complete Liquidity Pool Lifecycle
```bash
# 1. Register as investor and get KYC approval via API
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"email": "investor@example.com", "user_type": "investor"}' \
  http://localhost:8080/api/v1/user

curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "investor-uuid"}' \
  http://localhost:8080/api/v1/kyc/submit

# 2. Create a new liquidity pool with deployment terms via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "Conservative Credit Pool", "max_ltv_ratio": 0.75, "max_interest_rate": 0.10, "repayment_period_days": 180}' \
  http://localhost:8080/api/v1/pool

# 3. List all pools (with status filter) via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/pools?status=ACTIVE"

# 4. Check specific pool details via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/pool/pool-uuid

# 5. Update pool status (pause/reactivate) via API
curl -X PUT \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"status": "PAUSED"}' \
  http://localhost:8080/api/v1/pool/pool-uuid

# 6. Find matching pools for credit deployment via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/pools/match?required_liquidity=50000&max_ltv=0.80&max_rate=0.12&min_repayment_days=90"

# 7. Pool operations are included in batch processing for ZK proofs
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/batch/trigger

# 8. Verify pool creation was included in ZK proof
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "pool-creation-uuid", "transaction_type": "pool_creation", "proof_file": "proof_batch_1.json"}' \
  http://localhost:8080/api/v1/verify/inclusion
```

### Complete Deal Marketplace Lifecycle
```bash
# 1. Create and fund a deal via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"originator_id": "originator-uuid", "investor_id": "investor-uuid", "amount": 50000}' \
  http://localhost:8080/api/v1/deal

# 2. List deal for sale (by current investor) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"deal_id": "deal-uuid", "asking_price": 47500, "seller_id": "investor-uuid", "description": "Great investment opportunity"}' \
  http://localhost:8080/api/v1/deal/deal-uuid/list-for-sale

# 3. Browse marketplace via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/deals/for-sale?limit=10"

# 4. Get marketplace statistics via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/deals/marketplace/stats

# 5. Remove from sale (if needed) via API
curl -X DELETE \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/deal/deal-uuid/remove-from-sale

# 6. All marketplace operations are included in batch processing for ZK proofs
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/batch/trigger

# 7. Verify deal listing was included in ZK proof
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "listing-uuid", "transaction_type": "deal_listing", "proof_file": "proof_batch_1.json"}' \
  http://localhost:8080/api/v1/verify/inclusion
```

### Complete Deal Transfer Lifecycle
```bash
# 1. Validate transfer is possible via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"deal_id": "deal-uuid", "current_investor_id": "current-investor-uuid"}' \
  http://localhost:8080/api/v1/deal/deal-uuid/transfer/validate

# 2. Execute deal ownership transfer via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"deal_id": "deal-uuid", "from_investor_id": "current-investor-uuid", "to_investor_id": "new-investor-uuid", "transfer_price": 47500}' \
  http://localhost:8080/api/v1/deal/deal-uuid/transfer

# 3. Check transfer history for the deal via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/deal/deal-uuid/transfers?limit=5"

# 4. Check user's transfer history via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/user/current-investor-uuid/deal-transfers?limit=10"

# 5. All transfer operations are included in batch processing for ZK proofs
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/batch/trigger

# 6. Verify transfer inclusion in ZK proof
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "transfer-transaction-uuid", "transaction_type": "deal_transfer", "proof_file": "proof_batch_1.json"}' \
  http://localhost:8080/api/v1/verify/inclusion
```

### Complete Withdrawal Lifecycle with ZK Proof Integration
```bash
# 1. Check available balance for withdrawals via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/user/balance?address=0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 2. Initiate withdrawal (requires KYC approval + sufficient balance) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_address": "0x742E4C1D5B5B8Ac5dC12345678901234567890aB", "recipient_address": "0x123456789012345678901234567890123456789", "amount": 100.50}' \
  http://localhost:8080/api/v1/withdrawal/initiate

# 3. Check withdrawal status via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/withdrawal/status/1

# 4. View withdrawal history via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/withdrawals?address=0x742E4C1D5B5B8Ac5dC12345678901234567890aB"

# 5. Cancel withdrawal (only if not yet submitted to blockchain) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/withdrawal/1/cancel

# 6. Admin: View pending withdrawals via API
curl -H "Authorization: Bearer YOUR_TOKEN" \
  "http://localhost:8080/api/v1/withdrawals/pending?limit=10"

# 7. Create batch and generate ZK proof (includes withdrawal transactions) via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  http://localhost:8080/api/v1/batch/trigger

# 8. Verify withdrawal inclusion in ZK proof via API
curl -X POST \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "withdrawal-transaction-uuid", "transaction_type": "user_withdrawal", "proof_file": "proof_batch_1.json"}' \
  http://localhost:8080/api/v1/verify/inclusion
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
- **API authentication errors**: Verify your Bearer token is valid and not expired
- **KYC validation errors**: Check user KYC status before deal operations via API
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