# SERVER_ENVIRONMENT Configuration

## Overview

The `SERVER_ENVIRONMENT` variable controls which proof generation method is used for ZK batch processing. This allows for fast local development while maintaining production-ready Sindri integration.

## Configuration

Set the environment variable in your `.env` file or shell:

```bash
export SERVER_ENVIRONMENT=local  # Options: local, staging, sandbox, production
```

## Environment Options

### `local` (Default)
- **Proof Generation**: Local SP1 prover
- **Speed**: Fast (~8 seconds for Core proofs)
- **Use Case**: Local development and testing
- **Requirements**: None (no Sindri API key needed)
- **Proof Type**: SP1 Core proofs
- **Storage**: Proofs not uploaded to Sindri

### `staging`
- **Proof Generation**: Local SP1 prover
- **Speed**: Fast (~8 seconds for Core proofs)
- **Use Case**: Staging environment testing
- **Requirements**: None (no Sindri API key needed)
- **Proof Type**: SP1 Core proofs
- **Storage**: Proofs not uploaded to Sindri

### `sandbox`
- **Proof Generation**: Sindri service
- **Speed**: Slower (~28-60 seconds depending on batch size)
- **Use Case**: Pre-production testing with real proof infrastructure
- **Requirements**: Valid `SINDRI_API_KEY` required
- **Proof Type**: Groth16 (configurable)
- **Storage**: Proofs stored in Sindri for retrieval

### `production`
- **Proof Generation**: Sindri service
- **Speed**: Slower (~28-60 seconds depending on batch size)
- **Use Case**: Production deployments
- **Requirements**: Valid `SINDRI_API_KEY` required
- **Proof Type**: Groth16 (configurable)
- **Storage**: Proofs stored in Sindri for retrieval

## How It Works

### Proof Generation Flow

```mermaid
graph TD
    A[Batch Created] --> B{Check SERVER_ENVIRONMENT}
    B -->|local/staging| C[Use Local SP1 Prover]
    B -->|sandbox/production| D[Use Sindri Service]
    C --> E[Generate Core Proof Locally]
    E --> F[Verify Proof Locally]
    F --> G[Mark as 'Ready' Immediately]
    D --> H[Submit to Sindri API]
    H --> I[Monitor Proof Status]
    I --> J[Update Batch When Ready]
    G --> K[Batch Processing Complete]
    J --> K
```

### Implementation Details

The environment check happens in `lib/src/proof.rs`:

```rust
pub async fn generate_batch_proof(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    let server_env = std::env::var("SERVER_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .to_lowercase();

    match server_env.as_str() {
        "local" | "staging" => generate_batch_proof_local(request).await,
        "sandbox" | "production" => generate_batch_proof_sindri(request).await,
        _ => generate_batch_proof_local(request).await, // Default to local
    }
}
```

## Local Proof Generation

For `local` and `staging` environments:

1. **Setup**: `ProverClient::from_env()` initializes the local prover
2. **Input**: Transaction data is serialized via `SP1Stdin`
3. **Proving**: Core proofs generated locally (~8 seconds)
4. **Verification**: Immediate local verification
5. **Result**: Proof marked as "Ready" with pseudo ID `local-<timestamp>`

**Advantages**:
- Fast iteration during development
- No network dependency
- No Sindri API costs
- Immediate feedback

**Limitations**:
- Proofs not production-ready
- No on-chain verification support
- Not stored for later retrieval

## Sindri Proof Generation

For `sandbox` and `production` environments:

1. **Setup**: `SindriClient::default()` connects to Sindri API
2. **Input**: Transaction data serialized to JSON
3. **Proving**: Submitted to Sindri asynchronously
4. **Monitoring**: Background service polls for completion
5. **Result**: Production-ready Groth16 proofs

**Advantages**:
- Production-ready proofs
- On-chain verification support
- Proofs stored for retrieval
- Benchmark data included

**Limitations**:
- Slower (28-60+ seconds depending on batch size)
- Requires Sindri API key
- Network dependency
- API costs apply

## Testing

### Local Testing

#### Via API Server
```bash
# Set environment for local proofs
export SERVER_ENVIRONMENT=local

# Start the API server
make server

# In another terminal, trigger a batch
make cli ARGS="batch trigger --verbose"
```

#### Via Script (Direct)
```bash
# Set environment for local proofs
export SERVER_ENVIRONMENT=local

# Run the test script (fast ~8s)
make run
```

### Sindri Testing

#### Via API Server
```bash
# Set environment for Sindri proofs
export SERVER_ENVIRONMENT=sandbox
export SINDRI_API_KEY=your_api_key_here

# Start the API server
make server

# In another terminal, trigger a batch
make cli ARGS="batch trigger --verbose"
```

#### Via Script (Direct)
```bash
# Set environment for Sindri proofs
export SERVER_ENVIRONMENT=sandbox
export SINDRI_API_KEY=your_api_key_here

# Run the test script (slower, uses Sindri)
make run
```

#### Benchmarking
```bash
# Sindri benchmark with custom parameters
export SERVER_ENVIRONMENT=production
export SINDRI_API_KEY=your_key_here
make bench-zk NUM_PROOFS=10 NUM_TRANSACTIONS=100

# Local benchmark
export SERVER_ENVIRONMENT=local
make bench-zk NUM_PROOFS=10 NUM_TRANSACTIONS=100
```

## Migration Notes

### Changed Behavior

1. **CLI `submit-transaction` command removed**: Transactions are now only created through proper API operations (user registration, deal operations, etc.)

2. **Automatic proof method selection**: No need to pass `--sindri` flag; environment variable determines the method

3. **script/src/bin/main.rs updated**: Now uses `SERVER_ENVIRONMENT` instead of compile-time `--features sindri` flag

4. **Makefile benchmarks updated**: `make bench-zk` no longer uses `--features sindri`, uses `SERVER_ENVIRONMENT` instead

### Breaking Changes

- ❌ **Removed**: `cli batch submit-transaction` command
- ❌ **Removed**: `BatchApiClient::submit_transaction()` method
- ✅ **Added**: `SERVER_ENVIRONMENT` variable for proof method selection
- ✅ **Added**: Local SP1 proof generation for fast development

## Environment Variable Reference

```bash
# Required for all environments
DATABASE_URL=postgresql://user:pass@localhost:5432/arda_credit_db

# Proof generation configuration
SERVER_ENVIRONMENT=local  # local, staging, sandbox, or production

# Required only for sandbox/production (Sindri environments)
SINDRI_API_KEY=your_sindri_api_key_here
SINDRI_CIRCUIT_TAG=latest  # or specific version like v1.0.0

# Optional: Smart contract configuration (for posting proofs on-chain)
ETHEREUM_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY
ETHEREUM_CONTRACT_ADDRESS=0x...
```

## Troubleshooting

### Local Proof Generation Fails

**Problem**: `error: couldn't read 'build/program'`

**Solution**: Build the program first:
```bash
make run  # This builds the program and tests locally
```

### Sindri Proof Generation Fails

**Problem**: `SINDRI_API_KEY not set`

**Solution**: Add your Sindri API key:
```bash
export SINDRI_API_KEY=your_key_here
```

**Problem**: `Circuit not found: arda-credit:latest`

**Solution**: Deploy the circuit first:
```bash
make deploy  # Deploys circuit to Sindri
```

### Environment Not Recognized

**Problem**: `Unknown SERVER_ENVIRONMENT 'dev'`

**Solution**: Use one of the valid options:
```bash
export SERVER_ENVIRONMENT=local      # For local development
export SERVER_ENVIRONMENT=staging    # For staging
export SERVER_ENVIRONMENT=sandbox    # For sandbox/testing
export SERVER_ENVIRONMENT=production # For production
```

## Related Files

- `lib/src/proof.rs` - Proof generation logic with environment routing
- `api/src/config/types.rs` - `ServerEnvironment` enum definition
- `api/src/services/batch_processor.rs` - Batch processing service
- `cli/src/batch.rs` - CLI batch commands (submit-transaction removed)
- `AGENTS.md` - Full environment variable documentation
