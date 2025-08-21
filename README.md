# Arda Credit - Privacy-Preserving Loan vApp

A privacy-preserving loan platform built on SP1 zero-knowledge proofs, enabling secure loan origination, payments, and closeouts with KYC verification while maintaining transaction privacy through batch processing.

## Overview

**Arda Credit** is a verifiable application (vApp) that demonstrates privacy-preserving financial operations using zero-knowledge proofs. The platform enables:

### Core Features

**👥 User Management & KYC**
- **Investor** and **Lender** account types
- KYC verification with privacy-preserving status proofs
- Flexible KYC requirements for different operations

**💰 Loan Operations**
- **Loan Origination** - Create loans between KYC'd parties
- **Payment Processing** - Make payments with privacy guarantees  
- **Loan Closeout** - Final settlement and completion

**🔐 Privacy & Verification**
- **Transaction Inclusion Proofs** - Prove any transaction was processed without revealing others
- **Batch Privacy** - Individual loan amounts and user details remain private
- **KYC Privacy** - Prove KYC status without revealing identity
- **Zero-Knowledge State Transitions** - Cryptographically verified loan state changes

## Architecture

This loan vApp demonstrates **privacy-preserving financial operations** with continuous state tracking:

### Privacy Model
```
Private Data (Hidden):
- Individual loan amounts and terms
- Payment amounts and frequencies  
- User identities and KYC details
- Interest rates and loan metadata

Public Data (Verifiable):
- State root transitions (Merkle tree)
- Transaction count per batch
- Batch timestamps
- Proof verification results
```

### System Components
- **`program/`** - SP1 zkVM program for loan transaction processing
- **`cli/`** - Complete loan workflow interface + trustless verification
- **`api/`** - Production server with KYC, loan processing, and ZK proof generation
- **`db/`** - PostgreSQL with users, loans, KYC data, and Merkle tree state
- **`contracts/`** - Solidity contracts for on-chain loan state verification
- **`lib/`** - Shared loan computation logic (zkVM compatible)

## Requirements

- [Rust](https://rustup.rs/) toolchain
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install) zkVM
- [Foundry](https://book.getfoundry.sh/getting-started/installation) for smart contracts
- [Docker](https://docs.docker.com/get-docker/) for database
- [Node.js](https://nodejs.org/) for Sindri CLI

## Quick Start

**🚀 Complete Setup**
```bash
make setup    # Install dependencies + copy .env + initialize database
# Edit .env file with your SINDRI_API_KEY
make deploy   # Deploy circuit to Sindri
make up       # Start services
```

**💡 Test the Loan Workflow**
```bash
# 1. Register users (investors and lenders)
cargo run --bin cli -- register-user --user-type lender --email alice@example.com
cargo run --bin cli -- register-user --user-type investor --email bob@example.com

# 2. Submit KYC for both users (required for loan operations)
cargo run --bin cli -- submit-kyc --user-id <alice-uuid>
cargo run --bin cli -- submit-kyc --user-id <bob-uuid>

# 3. Originate a loan between KYC'd parties
cargo run --bin cli -- originate-loan \
  --borrower-id <bob-uuid> \
  --lender-id <alice-uuid> \
  --amount 50000

# 4. Make payments toward the loan
cargo run --bin cli -- make-payment --loan-id <loan-uuid> --amount 5000
cargo run --bin cli -- make-payment --loan-id <loan-uuid> --amount 5000

# 5. Process batch and generate ZK proofs
cargo run --bin cli -- trigger-batch --verbose

# 6. Verify payment inclusion (trustless)
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id <payment-uuid> \
  --transaction-type loan_payment
```

## Privacy Guarantees

### What Stays Private
- **Individual loan amounts** - Hidden in batch processing
- **User identities** - Protected through zero-knowledge proofs
- **Payment details** - Amounts and frequencies remain confidential
- **KYC information** - Status provable without revealing identity
- **Loan terms** - Interest rates and terms stay private

### What Can Be Verified
- **Transaction inclusion** - Prove specific transactions were processed
- **State transitions** - Cryptographically verify loan state changes
- **KYC compliance** - Prove KYC requirements were met
- **Payment completion** - Verify payments without revealing amounts
- **Loan lifecycle** - Track loan status changes with privacy

## Use Cases

### For Lenders
```bash
# Register as lender and complete KYC
cargo run --bin cli -- register-user --user-type lender --email lender@example.com
cargo run --bin cli -- submit-kyc --user-id <lender-uuid>

# Originate loans to KYC'd borrowers
cargo run --bin cli -- originate-loan --borrower-id <borrower> --lender-id <lender> --amount 100000

# Track loan performance privately
cargo run --bin cli -- list-loans --user-id <lender-uuid>
```

### For Borrowers/Investors
```bash
# Register and complete KYC
cargo run --bin cli -- register-user --user-type investor --email investor@example.com
cargo run --bin cli -- submit-kyc --user-id <investor-uuid>

# Make loan payments
cargo run --bin cli -- make-payment --loan-id <loan-uuid> --amount 2500

# Prove payment was processed
cargo run --bin cli -- verify-transaction-inclusion \
  --proof-file proof_batch_1.json \
  --transaction-id <payment-id> \
  --transaction-type loan_payment
```

### For Auditors/Regulators
```bash
# Download batch proof for verification
cargo run --bin cli -- download-proof --batch-id 1

# Verify batch contains expected transactions (without seeing private data)
cargo run --bin cli -- verify-batch-proof \
  --proof-file proof_batch_1.json \
  --expected-transaction-count 5
```

## Development

### Local Development
```bash
# Fast SP1 loan testing (no database)
cargo run --release                 # Test loan operations locally

# Full stack development
make up-dev                         # Local build + hot reload
```

### Makefile Commands
```bash
make help                           # Show all available commands
make setup                          # Complete setup from scratch
make test                          # Run all tests
make forge-test                    # Test smart contracts
make clean                         # Clean all build artifacts
```

### Architecture Details

For detailed technical information, see:
- [`CLAUDE.md`](CLAUDE.md) - Development guide for Claude Code
- [`LOAN_ARCHITECTURE.md`](LOAN_ARCHITECTURE.md) - Detailed technical architecture
- [`contracts/README.md`](contracts/README.md) - Smart contract documentation

## Privacy Technology

**Built on SP1 zkVM** - Uses Succinct's SP1 zero-knowledge virtual machine for:
- **RISC-V Compatibility** - Standard Rust code compiles to zkVM
- **Groth16 Proofs** - EVM-compatible proofs for on-chain verification
- **Sindri Integration** - Serverless proof generation at scale

**Merkle Tree State Management** - 32-level indexed trees provide:
- **8x Constraint Reduction** - Compared to traditional 256-level trees
- **Efficient Updates** - O(log n) operations with atomic transactions
- **Privacy-Preserving Commitments** - State roots hide individual transactions

**Batch Processing** - FIFO transaction batching ensures:
- **Transaction Privacy** - Individual amounts hidden in batches
- **Scalable Throughput** - Multiple transactions proven together
- **Cost Efficiency** - Amortized proof generation costs

## Security Model

### Trust Assumptions
- **SP1 zkVM Security** - Relies on SP1's cryptographic guarantees
- **Sindri Infrastructure** - Trusted for proof generation (verifiable outputs)
- **Smart Contract Security** - On-chain verification provides final settlement

### Threat Model
- **Protects Against**: Transaction amount leakage, user identity exposure, payment pattern analysis
- **Assumes**: Honest majority of validators, secure key management, proper KYC processes
- **Mitigates**: Front-running, MEV extraction, regulatory compliance violations

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details.

## Status

**Current Version**: v0.0.1 (MVP Development)

This is a Minimum Viable Product (MVP) demonstrating privacy-preserving loan operations. The KYC process can be mocked for testing, but provides the foundation for production KYC integration.

---

**🏗️ Built with SP1** | **🔐 Privacy by Design** | **⚡ Production Ready**