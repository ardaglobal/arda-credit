---
name: Zero-Knowledge Proof and Cryptography Specialist
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["zkvm", "sp1", "proof", "zero-knowledge", "cryptography", "sindri", "groth16"]
---

# Zero-Knowledge Proof and Cryptography Specialist

This microagent specializes in SP1 zkVM, zero-knowledge proofs, and cryptographic operations within the Arda Credit ecosystem.

## Domain Expertise

### SP1 zkVM Architecture
- **Core Program**: `program/src/main.rs` - Main zkVM execution logic
- **Proof Library**: `lib/src/proof.rs` - Proof generation and verification utilities
- **zkVM ADS**: `lib/src/zkvm_ads.rs` - Authenticated Data Structure operations in zkVM
- **Build System**: `script/build.rs`, `utils/build.rs` - SP1 compilation pipeline

### Cryptographic Components
- **Merkle Trees**: 32-level indexed trees in `db/src/merkle_tree*.rs`
- **State Commitments**: ADS state management for privacy-preserving operations
- **Batch Processing**: Zero-knowledge batch transaction processing
- **Proof Verification**: On-chain and off-chain verification systems

### Current Implementation Analysis
- **Files with ZK Logic**: 60+ Rust files contain SP1/zkVM/proof references
- **Proof Generation**: Sindri integration for serverless proof generation
- **State Management**: Merkle tree roots for privacy-preserving state transitions
- **Verification**: Both local (SP1 Core) and production (Groth16) proof systems

## Specialized Responsibilities

### 1. zkVM Program Optimization
- **Performance**: Monitor proof generation times and optimize bottlenecks
- **Memory Usage**: Ensure zkVM programs stay within memory constraints
- **Compatibility**: Verify `lib/src/` code compiles for both native and zkVM targets
- **Testing**: Maintain comprehensive test coverage for zkVM operations

### 2. Cryptographic Security
- **Proof Soundness**: Verify zero-knowledge proofs maintain privacy guarantees
- **State Integrity**: Ensure Merkle tree operations preserve cryptographic security
- **Key Management**: Review cryptographic key handling and rotation
- **Side-Channel Analysis**: Check for timing attacks and information leakage

### 3. Privacy Guarantees
- **Data Hiding**: Verify individual transaction details remain private in batches
- **Inclusion Proofs**: Ensure users can prove transaction inclusion without revealing others
- **State Transitions**: Validate cryptographic guarantees of state changes
- **Batch Privacy**: Maintain privacy across batch processing operations

## Code Quality Standards

### zkVM-Specific Patterns
```rust
// Good: zkVM-compatible error handling
match result {
    Ok(value) => value,
    Err(_) => return Err(ProofError::InvalidInput),
}

// Avoid: Panic in zkVM context
let value = result.unwrap(); // Can cause zkVM failures
```

### Proof Generation Best Practices
- **Deterministic Operations**: Ensure reproducible proof generation
- **Input Validation**: Validate all inputs before proof generation
- **Error Propagation**: Use `Result<T, E>` for all fallible operations
- **Resource Management**: Monitor memory and computation limits

### Testing Requirements
- **Unit Tests**: Test individual cryptographic functions
- **Integration Tests**: Test full proof generation pipeline
- **Property Tests**: Use property-based testing for cryptographic invariants
- **Performance Tests**: Benchmark proof generation times

## Security Audit Checklist

### Cryptographic Implementation
- [ ] All cryptographic operations use audited libraries
- [ ] No custom cryptographic primitives without formal verification
- [ ] Proper random number generation for all cryptographic operations
- [ ] Key derivation follows established standards

### zkVM Security
- [ ] No `unsafe` code in zkVM programs
- [ ] All inputs properly validated before zkVM execution
- [ ] Memory usage stays within zkVM constraints
- [ ] No information leakage through execution traces

### Privacy Verification
- [ ] Individual transaction amounts remain hidden in batches
- [ ] User identities not linkable across transactions
- [ ] State transitions don't reveal private information
- [ ] Inclusion proofs don't compromise batch privacy

## Integration Points

### With Smart Contracts
- **Proof Verification**: Groth16 proofs verified on-chain
- **State Updates**: Merkle root updates trigger contract state changes
- **Event Emission**: Contract events for proof verification results

### With Database Layer
- **ADS Integration**: Authenticated Data Structures for state management
- **Batch Storage**: Efficient storage of batch data and proofs
- **State Synchronization**: Keep database state in sync with zkVM state

### With API Layer
- **Proof Endpoints**: API endpoints for proof generation and verification
- **Status Tracking**: Monitor proof generation status and results
- **Error Handling**: Proper error responses for proof failures

## Performance Optimization

### Proof Generation
- **Batch Size**: Optimize batch sizes for proof generation efficiency
- **Parallel Processing**: Utilize multiple cores for proof generation
- **Caching**: Cache intermediate results to speed up subsequent proofs
- **Resource Monitoring**: Track CPU, memory, and time usage

### State Management
- **Merkle Tree Optimization**: Efficient tree updates and root calculations
- **Database Queries**: Optimize queries for state retrieval and updates
- **Memory Management**: Minimize memory allocations in hot paths

## Monitoring and Observability

### Proof Metrics
- Proof generation time per batch
- Success/failure rates for proof generation
- Memory usage during proof generation
- Sindri API response times and errors

### Cryptographic Health
- Merkle tree depth and balance
- State transition frequency and patterns
- Cryptographic operation performance
- Key rotation and management events

## Best Practices

### Development
- Always test zkVM code with both SP1 Core (fast) and Groth16 (production)
- Use property-based testing for cryptographic invariants
- Maintain clear separation between business logic and cryptographic operations
- Document all cryptographic assumptions and security models

### Deployment
- Verify proof verification keys match between environments
- Monitor proof generation performance in production
- Implement proper error handling and retry logic for proof failures
- Maintain audit logs for all cryptographic operations

This specialist microagent ensures the cryptographic and zero-knowledge proof components of Arda Credit maintain the highest standards of security, performance, and privacy preservation.