# USDC Deposit Monitor Service

This service monitors on-chain USDC deposit events from the Arda Credit smart contract and maintains accurate user balance tracking in the database.

## Overview

The USDC Deposit Monitor provides:
- **Real-time Event Listening**: Monitors smart contract for Deposit events
- **Transaction Confirmation**: Waits for required block confirmations before processing
- **Database Integration**: Maintains deposit records and user balance aggregations
- **Error Handling**: Robust retry mechanisms for failed operations
- **Idempotency**: Prevents duplicate processing of the same deposit
- **Audit Trail**: Complete logging of all processing events

## Architecture

### Components

1. **DepositMonitor**: Main service orchestrator
2. **DepositEventHandler**: Processes incoming deposit events
3. **Confirmation Checker**: Monitors pending deposits for sufficient confirmations
4. **Retry Processor**: Handles failed operations with exponential backoff
5. **Balance Updater**: Maintains user balance aggregations

### Database Schema

The service uses three main tables:

- `usdc_deposits`: Individual deposit records with processing status
- `user_deposit_balances`: Aggregated user balances 
- `deposit_processing_log`: Audit trail and error tracking

## Usage

### Running the Service

```bash
# Basic usage
cargo run --bin deposit_monitor -- \
  --database-url "postgresql://postgres:password@localhost:5432/arda_credit_db" \
  --ethereum-rpc-url "https://eth-mainnet.alchemyapi.io/v2/your-key" \
  --contract-address "0x..." \
  --confirmations 12

# With custom configuration
cargo run --bin deposit_monitor -- \
  --database-url $DATABASE_URL \
  --ethereum-rpc-url $ETHEREUM_RPC_URL \
  --contract-address $LOAN_CONTRACT_ADDRESS \
  --confirmations 12 \
  --check-interval 30 \
  --max-retries 3 \
  --retry-backoff 60 \
  --batch-size 100 \
  --log-level info
```

### Health Check

```bash
cargo run --bin deposit_monitor -- \
  --database-url $DATABASE_URL \
  --ethereum-rpc-url $ETHEREUM_RPC_URL \
  --contract-address $CONTRACT_ADDRESS \
  --health-check
```

### Statistics

```bash
cargo run --bin deposit_monitor -- \
  --database-url $DATABASE_URL \
  --stats
```

## Configuration

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string
- `ETHEREUM_RPC_URL`: Ethereum node RPC endpoint
- `LOAN_CONTRACT_ADDRESS`: Deployed Loan contract address

### Command Line Options

- `--confirmations`: Required block confirmations (default: 12)
- `--check-interval`: Confirmation check frequency in seconds (default: 30)  
- `--max-retries`: Maximum retry attempts (default: 3)
- `--retry-backoff`: Retry delay in seconds (default: 60)
- `--batch-size`: Processing batch size (default: 100)
- `--log-level`: Logging verbosity (default: info)

## Event Processing Flow

1. **Event Detection**: Service listens for `Deposit` events from smart contract
2. **Initial Storage**: Event stored in database with `pending` status
3. **Confirmation Monitoring**: Background task checks block confirmations
4. **Status Update**: Deposit marked as `confirmed` after sufficient confirmations
5. **Balance Aggregation**: User balance totals updated automatically via triggers
6. **Error Handling**: Failed operations logged and retried with backoff

## Monitoring & Observability

### Log Levels

- **ERROR**: Critical failures requiring immediate attention
- **WARN**: Recoverable issues (connection failures, retry attempts)
- **INFO**: Normal operational events (deposit confirmations, balance updates)
- **DEBUG**: Detailed processing information
- **TRACE**: Verbose event and database operation details

### Key Metrics

The service tracks:
- Total deposits processed
- Pending/confirmed/failed deposit counts
- Unique depositor addresses
- Total USDC amounts
- Processing error rates
- Confirmation timing statistics

### Database Queries

```sql
-- Check service health
SELECT status, COUNT(*) FROM usdc_deposits GROUP BY status;

-- Recent deposit activity
SELECT * FROM usdc_deposits ORDER BY created_at DESC LIMIT 10;

-- User balances
SELECT * FROM user_deposit_balances ORDER BY total_deposited DESC;

-- Processing errors
SELECT * FROM deposit_processing_log WHERE status = 'error' ORDER BY created_at DESC;
```

## Security Considerations

- **Private Key Management**: Service only reads from blockchain (no signing)
- **Database Security**: Uses connection pooling with prepared statements
- **Input Validation**: All event data validated before database storage
- **Error Handling**: Sensitive data masked in logs
- **Idempotency**: Duplicate events safely ignored

## Troubleshooting

### Common Issues

1. **Database Connection Failures**
   - Verify DATABASE_URL and PostgreSQL availability
   - Check connection limits and pool configuration

2. **Ethereum Node Issues** 
   - Verify RPC endpoint accessibility and API limits
   - Monitor connection stability and response times

3. **Missing Deposit Events**
   - Check contract address configuration
   - Verify event signature matches smart contract
   - Review block range synchronization

4. **Confirmation Delays**
   - Network congestion may slow block confirmations
   - Adjust `--confirmations` based on network conditions

### Recovery Procedures

1. **Restart Service**: Handle connection issues and temporary failures
2. **Manual Confirmation**: Force confirm deposits with sufficient blocks
3. **Retry Failed Operations**: Process stuck deposits manually
4. **Balance Reconciliation**: Re-aggregate user balances if needed

## Integration with Arda Credit

The deposit monitor integrates seamlessly with the existing Arda Credit infrastructure:

- Uses existing database schemas and migration system
- Leverages ethereum-client event handling framework  
- Connects with API server for real-time balance queries
- Supports CLI tools for operational management

## Performance

### Expected Throughput

- **Event Processing**: 1000+ deposits per minute
- **Confirmation Checks**: Sub-second database queries
- **Memory Usage**: <100MB baseline + event buffers
- **Database Impact**: Minimal with proper indexing

### Scaling Considerations

- Horizontal scaling via multiple service instances
- Database read replicas for statistics queries
- Event deduplication across multiple monitors
- Rate limiting for RPC provider quotas