# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Arda Credit** is a privacy-preserving loan vApp (verifiable app) built on SP1 zero-knowledge proofs. The MVP enables loan origination, payments, and closeouts with KYC verification, all while maintaining transaction privacy through batch processing and ZK proofs.

### Core Loan Features

**User Types & KYC:**
- **Investors** and **Lenders** - both can be KYC'd or non-KYC'd
- **KYC Status** - used in transaction flows for compliance
- **Account Registration** - user registration with KYC verification

**Loan Operations:**
- **Loan Origination** - create new loans between parties
- **Loan Payments** - make payments toward outstanding loans  
- **Loan Closeout** - final settlement and loan completion

**Privacy & Verification:**
- Users can **prove transaction inclusion** (loan payment, origination, closeout, KYC status)
- **Batch processing** keeps individual transaction details private
- **Zero-knowledge proofs** verify state transitions without revealing private data

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
cargo run --release                 # ~8s Core proofs: test loan operations

# Full stack development
make up                             # Start database + server (uses pre-built image)
make up-dev                         # Start with local Docker build
make down                           # Stop all services

# Local API server (alternative to Docker)
docker-compose up postgres -d      # Start just database
cargo run -p api --bin server --release  # Run server locally
```

### CLI Commands (Primary Interface)
```bash
# Health check
cargo run --bin cli -- health-check

# User & KYC Management
cargo run --bin cli -- register-user --user-type investor --email user@example.com
cargo run --bin cli -- submit-kyc --user-id 123
cargo run --bin cli -- get-user --user-id 123

# Loan Operations
cargo run --bin cli -- originate-loan --borrower-id 123 --lender-id 456 --amount 10000
cargo run --bin cli -- make-payment --loan-id 789 --amount 1000
cargo run --bin cli -- closeout-loan --loan-id 789

# Batch management
cargo run --bin cli -- view-pending          # View unbatched transactions
cargo run --bin cli -- trigger-batch --verbose  # Create batch + trigger ZK proof
cargo run --bin cli -- list-batches         # List all batches
cargo run --bin cli -- get-batch --batch-id 1  # Get batch details

# Transaction Inclusion Proofs (trustless verification)
cargo run --bin cli -- download-proof --batch-id 1  # Download proof data
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id abc123 \
  --transaction-type loan_payment
```

### Testing
```bash
# All tests
cargo test

# Database tests (requires PostgreSQL)
cd db && cargo test

# Smart contracts
make forge-build                    # Build contracts  
make forge-test                     # Run contract tests
cd contracts && forge test --match-contract LoanManagementTest  # Specific tests
```

### Smart Contract Deployment
```bash
# Deploy Arda Credit loan contract (set env vars first)
make deploy-contract
make deploy-contract-help           # Show required environment variables

# Build and test contracts
cd contracts && forge build
cd contracts && forge test
```

## High-Level Architecture

### Loan Processing Flow
1. **User Registration**: Users register as investors/lenders with optional KYC
2. **Loan Origination**: KYC'd users create loans (queued in `loan_transactions` table)
3. **Payment Processing**: Loan payments batched and processed via ZK proofs
4. **State Transitions**: Loan states (active, paid, closed) updated with cryptographic verification
5. **Contract Posting**: Background process posts proven loan state changes to Ethereum

### Zero-Knowledge Properties
```rust
// Public values (visible on-chain) 
struct LoanPublicValues {
    bytes32 previous_state_root;    // Previous Merkle root
    bytes32 new_state_root;         // New Merkle root  
    uint256 transaction_count;      // Number of transactions in batch
    // Individual loan amounts, KYC status, user details remain private
}
```

**Privacy Guarantees**: 
- Individual loan amounts, payments, and user details remain hidden
- Only state root transitions and transaction counts are public
- Users can prove their specific transaction was included without revealing others

### Key Components Integration

**Unified CLI Tool**: Single binary for complete loan workflow:
- User registration and KYC submission
- Loan origination, payments, and closeouts
- Local verification of transaction inclusion proofs
- Environment configuration via `ARDA_CREDIT_API_URL`

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

## Environment Configuration

### Required Environment Variables
```bash
# Database (Docker default)
DATABASE_URL=postgresql://postgres:password@127.0.0.1:5432/arda_credit_db

# Sindri (required for proof generation)
SINDRI_API_KEY=your_sindri_api_key_here     # Get from https://sindri.app
SINDRI_CIRCUIT_TAG=latest                   # Circuit version

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

## Loan Data Structures

### Core Types
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
    originated_at: DateTime<Utc>,
}

// Loan transaction (payments, origination, closeout)
struct LoanTransaction {
    transaction_id: uuid::Uuid,
    loan_id: uuid::Uuid,
    transaction_type: TransactionType,  // Origination | Payment | Closeout
    amount: Decimal,
    created_at: DateTime<Utc>,
}
```

## Common Development Patterns

### Adding New Loan Operations
1. Add transaction type to `TransactionType` enum in `lib/src/loan_types.rs`
2. Implement handler in `api/src/loan_handlers.rs`
3. Add CLI command in `cli/src/bin/cli.rs`
4. Add database migration if needed
5. Test with `cargo run --bin cli -- your-loan-command`

### KYC Integration
1. KYC status tracked in `users` table
2. KYC verification can be mocked via CLI/API
3. Transaction validation checks KYC status for restricted operations
4. Users can prove KYC status inclusion in batches

### Database Changes
1. Create migration in `db/migrations/`
2. Run `sqlx migrate run` to apply
3. Regenerate SQLx cache: `make initDB` or `cargo sqlx prepare --workspace`
4. Update relevant structs in `db/src/`

### Smart Contract Updates
1. Modify contracts in `contracts/src/`
2. Test: `cd contracts && forge test`
3. Deploy: `make deploy-contract` (after setting env vars)
4. Update `ETHEREUM_CONTRACT_ADDRESS` in environment

### Circuit Changes
1. Modify SP1 program in `program/src/main.rs` for loan transaction processing
2. Test locally: `cargo run --release`
3. Deploy to Sindri: `make deploy`
4. Update `SINDRI_CIRCUIT_TAG` if using specific version

## Loan Workflow Examples

### Complete Loan Lifecycle
```bash
# 1. Register users
cargo run --bin cli -- register-user --user-type lender --email lender@example.com
cargo run --bin cli -- register-user --user-type investor --email investor@example.com

# 2. Submit KYC (both users)
cargo run --bin cli -- submit-kyc --user-id lender-uuid
cargo run --bin cli -- submit-kyc --user-id investor-uuid

# 3. Originate loan (KYC required)
cargo run --bin cli -- originate-loan \
  --borrower-id investor-uuid \
  --lender-id lender-uuid \
  --amount 50000

# 4. Make payments
cargo run --bin cli -- make-payment --loan-id loan-uuid --amount 5000
cargo run --bin cli -- make-payment --loan-id loan-uuid --amount 5000

# 5. Create batch and generate proofs
cargo run --bin cli -- trigger-batch --verbose

# 6. Verify payment inclusion (trustless)
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id payment-uuid \
  --transaction-type loan_payment
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
- **Database connection errors**: Ensure PostgreSQL running via `docker-compose up postgres -d`
- **CLI errors**: Verify `ARDA_CREDIT_API_URL` environment variable
- **KYC validation errors**: Check user KYC status before loan operations
- **Contract deployment fails**: Check all Ethereum environment variables are set

### Service URLs
- Database: `localhost:5432`
- API Server: `http://localhost:8080`
- Health Check: `http://localhost:8080/api/v2/health`
- GraphQL Playground: `http://localhost:8080/playground` (dev only)