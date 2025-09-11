# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Arda Credit** is a privacy-preserving loan vApp (verifiable app) built on SP1 zero-knowledge proofs. The MVP enables loan origination, payments, and closeouts with KYC verification, plus **liquidity pool creation and management** for investors. All operations maintain transaction privacy through batch processing and ZK proofs.

### Core Loan Features

**User Types & KYC:**
- **Investors** and **Lenders** - both can be KYC'd or non-KYC'd
- **KYC Status** - used in transaction flows for compliance
- **Account Registration** - user registration with KYC verification

**USDC Deposit & Withdrawal System:**
- **Smart Contract Integration** - Direct USDC deposits/withdrawals to/from vApp contract
- **Automatic Balance Tracking** - Database triggers update user balances
- **Deposit History** - Complete audit trail of all deposits and confirmations
- **Withdrawal Processing** - KYC validation, balance verification, and transaction tracking
- **Multi-table Architecture** - Separate deposit/withdrawal tracking and user balance systems

**Loan Operations:**
- **Loan Origination** - create new loans between parties
- **Loan Payments** - make payments toward outstanding loans  
- **Loan Closeout** - final settlement and loan completion

**Liquidity Pool Management:**
- **Pool Creation** - investors can create yield-bearing liquidity pools with deployment rules
- **Pool Funding** - deposit USDC into pools for credit line matching
- **Pool Configuration** - LTV ratios, interest rates, repayment periods per pool
- **Pool State Tracking** - total deposits, available/utilized liquidity, status management

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

# Liquidity Pool Management (ZK-integrated)
# Create and manage liquidity pools with full ZK proof integration
cargo run --bin cli -- create-pool --name "My Pool" --max-ltv-ratio 0.8 --max-interest-rate 0.12 --repayment-period-days 365
cargo run --bin cli -- list-pools --verbose
cargo run --bin cli -- get-pool --pool-id <pool-id> --verbose
cargo run --bin cli -- get-investor-pool-metrics --investor-id <investor-id> --verbose
cargo run --bin cli -- update-pool-status --pool-id <pool-id> --status ACTIVE
cargo run --bin cli -- find-matching-pools --required-liquidity 10000 --max-ltv 0.8 --max-rate 0.12 --min-repayment-days 30

# User & KYC Management
cargo run --bin cli -- register-user --user-type investor --email user@example.com
cargo run --bin cli -- submit-kyc --user-id 123
cargo run --bin cli -- get-user --user-id 123

# USDC Balance Management
cargo run --bin cli -- get-user-balance-by-address --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB
cargo run --bin cli -- get-user-balance-by-id --user-id 123
cargo run --bin cli -- link-wallet --user-id 123 --wallet-address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB

# USDC Withdrawal Management
cargo run --bin cli -- get-available-balance --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB
cargo run --bin cli -- initiate-withdrawal --user-address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB --recipient-address 0x123... --amount 100.50
cargo run --bin cli -- get-withdrawal-status --request-id 1
cargo run --bin cli -- get-withdrawal-history --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB
cargo run --bin cli -- cancel-withdrawal --request-id 1
cargo run --bin cli -- get-pending-withdrawals --limit 10  # Admin command

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

#### Production Deployment
```bash
# Deploy Arda Credit loan contract (set env vars first)
make deploy-contract
make deploy-contract-help           # Show required environment variables
```

#### Local Development with Anvil
```bash
# Start local Ethereum testnet
cd contracts
anvil                               # Runs on http://127.0.0.1:8545 (Chain ID: 31337)

# In a new terminal, build and deploy contracts
cd contracts
forge build                         # Compile all contracts

# Deploy SP1MockVerifier (for testing)
forge create lib/sp1-contracts/contracts/src/SP1MockVerifier.sol:SP1MockVerifier \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast

# Deploy Loan contract (replace VERIFIER_ADDRESS with deployed address above)
forge create src/Loan.sol:Loan \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast \
  --constructor-args VERIFIER_ADDRESS 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

# Interact with deployed contracts
cast call CONTRACT_ADDRESS "verifier()" --rpc-url http://127.0.0.1:8545
cast call CONTRACT_ADDRESS "owner()" --rpc-url http://127.0.0.1:8545
```

#### Contract Testing
```bash
# Build and test contracts
cd contracts && forge build
cd contracts && forge test
forge test --fork-url http://127.0.0.1:8545  # Test against local testnet
```

## High-Level Architecture

### Loan Processing Flow
1. **User Registration**: Users register as investors/lenders with optional KYC
2. **USDC Deposits**: Users deposit USDC to smart contract, automatically updating internal balances
3. **Loan Origination**: KYC'd users create loans (queued in `loan_transactions` table)
4. **Payment Processing**: Loan payments batched and processed via ZK proofs
5. **USDC Withdrawals**: KYC'd users withdraw USDC with balance validation and smart contract execution
6. **State Transitions**: Loan states (active, paid, closed) updated with cryptographic verification
7. **Contract Posting**: Background process posts proven loan state changes to Ethereum

### ZK Program Withdrawal Flow Integration
The ZK program in `program/src/main.rs` fully supports the withdrawal flow through:

**Transaction Processing:**
- `UserWithdrawal` (TransactionType = 8) transactions are processed in mixed batches
- KYC verification required for all withdrawal transactions
- Balance validation ensures sufficient funds before processing
- Withdrawal metadata contains recipient address for smart contract execution

**ADS Integration:**
- All withdrawal transactions are converted to nullifiers using `transaction_to_nullifier()` 
- Nullifiers are inserted into the Indexed Merkle Tree (IMT) for cryptographic consistency
- State root transitions (`previous_state_root` → `new_state_root`) include withdrawal proofs
- Off-chain and ZK program state roots remain synchronized through ADS operations

**Balance State Tracking:**
- `DebitFromWithdrawal` balance updates automatically created for each withdrawal
- Balance updates include recipient wallet address in metadata for audit trail
- Total system balance decreases correctly when USDC exits the system via withdrawals
- Individual user balances updated atomically with withdrawal processing

**Privacy Guarantees:**
- Withdrawal amounts and recipient addresses remain private within ZK execution
- Only state root changes and transaction counts are published on-chain
- Users can prove withdrawal inclusion without revealing other batch transactions
- Batch processing maintains privacy while enabling regulatory compliance

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

// USDC deposit from smart contract
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

// User deposit balance aggregation
struct UserDepositBalance {
    user_address: String,
    total_deposited: Decimal,    // Cumulative deposits in wei
    total_deposited_usdc: Decimal, // Human-readable USDC amount  
    deposit_count: i32,
    first_deposit_at: Option<DateTime<Utc>>,
    last_deposit_at: Option<DateTime<Utc>>,
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

### USDC Deposit Flow
```bash
# 1. User deposits USDC to smart contract (triggers database update)
# 2. Check deposit balance
cargo run --bin cli -- get-user-balance-by-address --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB

# 3. Link wallet to user account
cargo run --bin cli -- link-wallet --user-id user-uuid --wallet-address 0x742E...

# 4. View deposit history (database queries)
psql $DATABASE_URL -c "SELECT * FROM usdc_deposits WHERE user_address = '0x742E...';"
psql $DATABASE_URL -c "SELECT * FROM user_deposit_balances WHERE user_address = '0x742E...';"
```

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

### Complete Liquidity Pool Lifecycle
```bash
# 1. Register as investor and get KYC approval
cargo run --bin cli -- register-user --user-type investor --email investor@example.com
cargo run --bin cli -- submit-kyc --user-id <investor-uuid>

# 2. Create a new liquidity pool with deployment terms
cargo run --bin cli -- create-pool \
  --name "Conservative Credit Pool" \
  --max-ltv-ratio 0.75 \
  --max-interest-rate 0.10 \
  --repayment-period-days 180

# 3. List all pools (with status filter)
cargo run --bin cli -- list-pools --status ACTIVE --verbose

# 4. Check specific pool details
cargo run --bin cli -- get-pool --pool-id <pool-uuid> --verbose

# 5. Update pool status (pause/reactivate)
cargo run --bin cli -- update-pool-status --pool-id <pool-uuid> --status PAUSED

# 6. Find matching pools for credit deployment
cargo run --bin cli -- find-matching-pools \
  --required-liquidity 50000 \
  --max-ltv 0.80 \
  --max-rate 0.12 \
  --min-repayment-days 90

# 7. Pool operations are included in batch processing for ZK proofs
cargo run --bin cli -- trigger-batch --verbose

# 8. Verify pool creation was included in ZK proof (trustless verification)
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id <pool-creation-uuid> \
  --transaction-type pool_creation
```

### Complete Withdrawal Lifecycle with ZK Proof Integration
```bash
# 1. Check available balance for withdrawals
cargo run --bin cli -- get-available-balance --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB

# 2. Initiate withdrawal (requires KYC approval + sufficient balance)
cargo run --bin cli -- initiate-withdrawal \
  --user-address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB \
  --recipient-address 0x123456789012345678901234567890123456789 \
  --amount 100.50

# 3. Check withdrawal status
cargo run --bin cli -- get-withdrawal-status --request-id 1

# 4. View withdrawal history
cargo run --bin cli -- get-withdrawal-history --address 0x742E4C1D5B5B8Ac5dC12345678901234567890aB

# 5. Cancel withdrawal (only if not yet submitted to blockchain)
cargo run --bin cli -- cancel-withdrawal --request-id 1

# 6. Admin: View pending withdrawals
cargo run --bin cli -- get-pending-withdrawals --limit 10

# 7. Create batch and generate ZK proof (includes withdrawal transactions)
cargo run --bin cli -- trigger-batch --verbose

# 8. Verify withdrawal inclusion in ZK proof (trustless verification)
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id withdrawal-transaction-uuid \
  --transaction-type user_withdrawal

# 9. View withdrawal database records with ZK proof data
psql $DATABASE_URL -c "SELECT * FROM withdrawal_requests WHERE user_address = '0x742E...';"
psql $DATABASE_URL -c "SELECT * FROM withdrawal_request_history WHERE withdrawal_request_id = 1;"
psql $DATABASE_URL -c "SELECT * FROM loan_transactions WHERE transaction_type = 'UserWithdrawal';"
```

### ZK Program State Changes for Withdrawals
The withdrawal flow integrates completely with the ZK program's ADS operations:

**Core Flow Verification:**
1. User requests withdrawal → Creates `UserWithdrawal` transaction (type 8)
2. System performs boolean KYC check → Validated in `validate_transaction_with_state()`
3. Validate sufficient balance → Checked in `BalanceState::create_withdrawal_update()`
4. Smart contract transfers USDC → External to ZK program (triggered by withdrawal processing)
5. Database balance updated → `DebitFromWithdrawal` balance update applied
6. Balance reflects in CLI → ZK-verified state transition with privacy preservation

**ZK Program Processing (lib/src/loan_processor.rs:225-242):**
```rust
TransactionType::UserWithdrawal => {
    // Extract recipient address from metadata
    let metadata = transaction.metadata.as_bytes();
    let mut recipient_address = [0u8; 32];
    recipient_address.copy_from_slice(&metadata[0..32]);

    // Create balance update (validates sufficient balance)
    let balance_update = self.balance_state.create_withdrawal_update(
        transaction.transaction_id,
        transaction.user_id,
        recipient_address,
        u128::from(transaction.amount),
        transaction.created_at,
    )?;

    // Apply balance update atomically
    self.balance_state.apply_update(balance_update)?;
}
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
- Health Check: `http://localhost:8080/api/v1/health`
- GraphQL Playground: `http://localhost:8080/playground` (dev only)