# Running the ZK Circuit - Quick Start Guide

This guide explains how to run the Arda Credit ZK circuit with test data to verify deal validation.

## Overview

The ZK circuit is located in `/workspace/program/src/main.rs` and can be executed via the test script at `/workspace/script/src/bin/main.rs`.

## Prerequisites

### Required Dependencies
```bash
# Install system dependencies
sudo apt-get update
sudo apt-get install -y build-essential libssl-dev pkg-config libstdc++-13-dev g++

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install SP1 toolchain
curl -L https://sp1.succinct.xyz | bash
sp1up

# Install Foundry (for smart contracts)
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

## Running the Circuit

### Method 1: Using Make (Recommended)
```bash
# Run with local Core proofs (fast, ~8 seconds)
make run

# OR with explicit environment
SERVER_ENVIRONMENT=local make run
```

### Method 2: Using Cargo Directly
```bash
# Run the test script with SQLX offline mode
SQLX_OFFLINE=true cargo run --release --bin arda-credit
```

### Method 3: With Sindri Integration (Production)
```bash
# Requires SINDRI_API_KEY in .env
SERVER_ENVIRONMENT=sandbox make run
```

## Test Scenario

The circuit executes a comprehensive test demonstrating the full deal lifecycle:

### Transactions Processed

1. **User Registration** - Alice (Originator)
2. **User Registration** - Bob (Investor)  
3. **KYC Submission** - Alice
4. **KYC Submission** - Bob
5. **KYC Verification** - Alice (Approved)
6. **KYC Verification** - Bob (Approved)
7. **Deal Origination** - $50,000 deal (Bob invests with Alice)
8. **Deal Payment #1** - $2,500
9. **Deal Payment #2** - $2,500

### What the Test Validates

✅ **Deal Creation Happy Path**:
- Both parties exist and are registered
- Both parties complete KYC and get approved
- Investor has sufficient balance ($100,000 pre-funded)
- Deal is created with valid terms (7.5% APR, 36 months)
- Deal ID is unique (no duplicates)
- Balance transfers execute correctly:
  - Investor: $100,000 → $50,000 (lends $50,000)
  - Originator: $0 → $50,000 (receives $50,000)

✅ **Deal Payment Processing**:
- Payments reduce outstanding balance correctly
- First payment changes status from `Originated` to `Active`
- Balance updates reflect payments:
  - Payment $2,500: Outstanding drops to $47,500
  - Payment $2,500: Outstanding drops to $45,000

✅ **Privacy Preservation**:
- Individual deal amounts remain hidden
- User identities are hashed
- Only state root transitions are public

✅ **State Transition Verification**:
- Previous state root ≠ New state root
- Transaction count matches batch size (9 transactions)
- Batch timestamp recorded
- Transaction types hash computed

## Expected Output

When successful, you should see:

```
🌍 Server Environment: LOCAL
🏦 Starting local SP1 deal processing test
🔐 Testing complete deal lifecycle with privacy preservation

📊 Test scenario:
  Initial users: 0
  Initial deals: 0
  Batch transactions: 9
  Transaction types: [UserRegistration, UserRegistration, KycSubmission, ...]

🔄 Generating Core proof (fast, for development)...
✅ Core proof generated successfully!

🔍 Verifying deal processing proof...
✅ Proof verification passed!

📤 Public outputs (what goes on-chain):
  Previous state root: 0x...
  New state root: 0x...
  Transaction count: 9
  Batch timestamp: 1640995200
  Transaction types hash: 0x...

✅ State transition verified:
  State successfully changed from 0x... to 0x...
🔐 Individual deal amounts, user identities, and transaction details remain private!
🎉 Only state root transitions and transaction count are public

🎉 Local SP1 deal processing test completed successfully!
```

## Troubleshooting

### Build Issues

If you encounter build errors, try:

```bash
# Clean and rebuild
cargo clean
SQLX_OFFLINE=true cargo build --release --bin arda-credit

# OR use the offline SQLx mode
export SQLX_OFFLINE=true
```

### Missing Dependencies

```bash
# Ensure all system libraries are installed
sudo apt-get install -y \
    build-essential \
    libssl-dev \
    pkg-config \
    libstdc++-13-dev \
    g++ \
    clang
```

### SP1 Setup Issues

```bash
# Reinstall SP1 toolchain
sp1up --force

# Verify SP1 installation
cargo prove --version
```

## Alternative: Using Docker

If local setup has issues, use Docker:

```bash
# Build and run via Docker
make docker-build
docker run --rm -v $(pwd):/workspace arda-credit make run
```

## Performance Notes

- **Local Core proofs**: ~8 seconds for 9 transactions
- **Sindri Groth16**: ~28-30 seconds for 100 transactions  
- **Scaling**: Approximately O(n^1.3) for large batches

## What's Being Proven

The ZK circuit proves:

1. ✅ Valid deal creation with KYC checks
2. ✅ Correct balance transfers (investor → originator)
3. ✅ Proper deal state transitions (Originated → Active)
4. ✅ Payment processing with balance validation
5. ✅ Privacy of deal amounts and user identities
6. ✅ Cryptographic state root transitions via ADS

See `docs/ZK_CIRCUIT_AUDIT.md` for complete details.

## Next Steps

1. Review the audit document: `docs/ZK_CIRCUIT_AUDIT.md`
2. Examine the test code: `script/src/bin/main.rs`
3. Review the circuit implementation: `program/src/main.rs`
4. Deploy to production: `make deploy` (requires `SINDRI_API_KEY`)

## References

- **Circuit Audit**: `/workspace/docs/ZK_CIRCUIT_AUDIT.md`
- **Main Program**: `/workspace/program/src/main.rs`
- **Test Script**: `/workspace/script/src/bin/main.rs`
- **Deal Logic**: `/workspace/lib/src/deal.rs`
- **Transaction Types**: `/workspace/lib/src/transactions.rs`
