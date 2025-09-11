# ZK Proof Pipeline Observability Implementation

This document describes the comprehensive tracing and metrics implementation for the Arda Credit ZK proof pipeline, designed for Sindri cost tracking and performance monitoring.

## Overview

The observability implementation provides comprehensive tracing across all stages of the ZK proof pipeline:

1. **Sindri API Cost & Performance Tracking** - Monitor API costs, response times, and success rates
2. **ZK Proof Generation Pipeline Stages** - Track each stage from batch creation to proof completion
3. **SP1 zkVM Execution Metrics** - Monitor zkVM program execution and resource usage
4. **Batch Processing Performance** - Track batch creation, transaction processing, and throughput
5. **ADS State Management** - Monitor Merkle tree operations and state transitions
6. **Smart Contract Integration** - Track proof posting to blockchain with gas costs
7. **Comprehensive Error Tracing** - Structured error handling with recovery guidance

## Implementation Components

### 1. Sindri API Tracing (`lib/src/proof.rs`)

**Key Metrics:**
```rust
// Span attributes for Sindri operations
sindri.circuit = %circuit_name
sindri.proof_system = ?proof_system  
sindri.transaction_count = %transaction_count
sindri.api_endpoint = %endpoint
sindri.request_size_bytes = %request_size
sindri.api_call_time_ms = %api_time
sindri.estimated_cost_usd = %cost
sindri.proof_id = %proof_id
sindri.success = %success
```

**Functions Instrumented:**
- `generate_batch_proof()` - Main proof generation with cost tracking
- `get_sindri_proof_info()` - Proof status checking with timing
- `get_sindri_proof_data()` - Proof data retrieval with size metrics

### 2. Batch Processing Pipeline (`api/src/batch_processor.rs`)

**Key Metrics:**
```rust
// Pipeline stage tracking
stage = "batch_creation" | "sindri_submission" | "proof_status_check" | "smart_contract_submission"
batch.trigger_source = %trigger_type
batch.transaction_count = %count
batch.id = %batch_id
sindri.submission_time_ms = %time
blockchain.contract_submission_time_ms = %time
blockchain.tx_hash = %hash
```

**Functions Instrumented:**
- `process_batch()` - Main batch processing workflow
- `submit_proof_fast()` - Sindri proof submission with timing
- `check_and_update_proof_status()` - Status polling with metrics
- `submit_batch_to_contract_with_confirmation()` - Smart contract integration

### 3. Unified Batch Service (`api/src/unified_batch_service.rs`)

**Key Metrics:**
```rust
// Batch processing performance
batch.tx_fetch_time_ms = %time
batch.nullifier_generation_time_ms = %time
ads.processing_time_ms = %time
batch.total_time_ms = %time
batch.transactions_per_second = %tps
batch.result = "success" | "no_transactions" | "error"
```

**Functions Instrumented:**
- `create_batch_with_ads()` - Unified batch creation with ADS integration
- Transaction fetching, nullifier generation, and ADS processing stages

### 4. ADS Operations (`db/src/ads_service.rs`)

**Key Metrics:**
```rust
// ADS operation tracking
ads.operation = "insert_nullifier" | "batch_insert" | "prove_membership"
ads.tree_type = "indexed_merkle_tree"
ads.lock_wait_time_ms = %time
ads.insertion_time_ms = %time
ads.throughput_ops_per_sec = %tps
ads.tree_height = %height
ads.successful_insertions = %count
```

**Functions Instrumented:**
- `insert()` - Single nullifier insertion with timing
- `batch_insert()` - Batch nullifier insertion with throughput metrics

### 5. Error Tracing (`lib/src/tracing_utils.rs`)

**Structured Error Categories:**
- `sindri_api_error` - Sindri API failures
- `proof_generation_error` - Proof generation failures  
- `batch_processing_error` - Batch processing issues
- `ads_operations_error` - ADS/Merkle tree errors
- `smart_contract_error` - Blockchain integration failures
- `configuration_error` - Configuration issues
- `network_error` - Network connectivity problems

**Error Context Fields:**
```rust
error.category = %category
error.operation = %operation
error.stage = %stage
error.is_recoverable = %recoverable
error.retry_count = %count
error.duration_ms = %time
```

## Key Performance Indicators (KPIs)

### Cost Metrics
- **Sindri API Cost per Batch**: `sindri.estimated_cost_usd`
- **Cost per Transaction**: `cost.estimated_usd / batch.transaction_count`
- **Monthly Sindri Spend**: Aggregate of all `sindri.estimated_cost_usd`

### Performance Metrics  
- **End-to-End Proof Time**: `batch.total_time_ms` (batch creation → proof ready)
- **Sindri API Response Time**: `sindri.api_call_time_ms`
- **Batch Processing Throughput**: `batch.transactions_per_second`
- **ADS Processing Performance**: `ads.throughput_ops_per_sec`

### Reliability Metrics
- **Proof Generation Success Rate**: `sindri.success` percentage
- **Batch Processing Success Rate**: `batch.result == "success"` percentage  
- **Smart Contract Posting Success**: `blockchain.success` percentage

### Business Logic Metrics
- **Transactions per Batch**: `batch.transaction_count`
- **Proof System Usage**: `sindri.proof_system` distribution
- **Circuit Deployment Tracking**: `sindri.circuit` and `sindri.circuit_tag`

## Usage Examples

### Querying Cost Metrics
```sql
-- Daily Sindri spend
SELECT DATE(timestamp) as date, 
       SUM(sindri.estimated_cost_usd) as daily_cost
FROM traces 
WHERE sindri.estimated_cost_usd IS NOT NULL
GROUP BY DATE(timestamp);

-- Cost per transaction
SELECT AVG(sindri.estimated_cost_usd / batch.transaction_count) as avg_cost_per_tx
FROM traces 
WHERE sindri.estimated_cost_usd IS NOT NULL 
  AND batch.transaction_count > 0;
```

### Querying Performance Metrics
```sql
-- Average proof generation time by proof system
SELECT sindri.proof_system,
       AVG(sindri.total_time_ms) as avg_time_ms,
       COUNT(*) as total_proofs
FROM traces 
WHERE sindri.proof_system IS NOT NULL
GROUP BY sindri.proof_system;

-- Batch processing throughput trends
SELECT DATE(timestamp) as date,
       AVG(batch.transactions_per_second) as avg_tps,
       MAX(batch.transactions_per_second) as max_tps
FROM traces
WHERE batch.transactions_per_second IS NOT NULL
GROUP BY DATE(timestamp);
```

### Querying Error Rates
```sql
-- Sindri API error rate
SELECT error.category,
       COUNT(*) as error_count,
       COUNT(*) * 100.0 / (SELECT COUNT(*) FROM traces WHERE sindri.api_endpoint IS NOT NULL) as error_rate_pct
FROM traces 
WHERE error.category LIKE '%sindri%'
GROUP BY error.category;

-- Batch processing failure analysis
SELECT error.stage,
       COUNT(*) as failures,
       AVG(error.duration_ms) as avg_failure_time_ms
FROM traces
WHERE error.category = 'batch_processing_error'
GROUP BY error.stage;
```

## Privacy & Security Considerations

### Safe to Log
✅ Batch IDs, transaction counts, proof IDs  
✅ Processing durations, costs, error types  
✅ System performance metrics, API response codes  
✅ Merkle roots and state transitions  

### Never Log  
❌ Individual transaction amounts or details  
❌ User IDs or personal information  
❌ Private keys or secrets  
❌ Loan-specific data within batches  

### Redaction Pattern
```rust
#[instrument(skip(sensitive_data), fields(
    operation_id = %operation_id,
    user_count = %user_count,
    // DO NOT LOG: individual_amounts, user_details
), level = "info")]
```

## Configuration

### Environment Variables
- `SINDRI_API_KEY` - Required for Sindri operations
- `SINDRI_CIRCUIT_TAG` - Circuit version for cost tracking (default: "latest")

### Tracing Levels
- `ERROR` - Critical failures requiring immediate attention
- `WARN` - Recoverable errors and performance issues  
- `INFO` - Normal operations and key metrics
- `DEBUG` - Detailed timing and diagnostic information

## Integration with Monitoring Systems

The structured tracing output is compatible with:

- **OpenTelemetry** - Export to OTLP collectors
- **Jaeger** - Distributed tracing visualization  
- **Prometheus** - Metrics collection and alerting
- **Grafana** - Dashboard creation and monitoring
- **Datadog/New Relic** - APM and observability platforms

## Success Criteria

After implementation, you can answer:

1. **Cost Questions**:
   - "What's our daily Sindri spend?"
   - "Which batch sizes are most cost-effective?" 
   - "Are proof costs trending up or down?"

2. **Performance Questions**:
   - "What's our average proof generation time?"
   - "Where are the bottlenecks in our ZK pipeline?"
   - "How many transactions per second can we process?"

3. **Reliability Questions**:
   - "What's our proof generation success rate?"
   - "How often do Sindri API calls fail?"
   - "Are batches getting stuck in any stage?"

## Next Steps

1. **Deploy** the observability implementation to staging environment
2. **Configure** monitoring dashboards and alerts
3. **Validate** cost tracking accuracy with actual Sindri billing
4. **Optimize** performance based on collected metrics
5. **Scale** monitoring to production workloads