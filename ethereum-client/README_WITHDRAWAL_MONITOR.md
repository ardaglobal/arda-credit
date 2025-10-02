# Withdrawal Event Monitoring & Database Update Service

This document describes the USDC Withdrawal Event Monitoring service that monitors on-chain withdrawal events and updates user balances in the database upon confirmation.

## Overview

The Withdrawal Monitor Service extends the existing deposit event monitoring pattern (ARDGBL-212) to handle withdrawal events from the smart contract. It provides:

- **Event Listening**: Monitor smart contract for withdrawal events
- **Transaction Polling**: Poll for withdrawal transaction confirmations  
- **Database Updates**: Debit user accounts with confirmed withdrawal amounts
- **Error Handling**: Manage failed withdrawals and retry logic
- **Balance Consistency**: Ensure database balance matches on-chain state
- **Audit Trail**: Track all withdrawal confirmations for compliance

## Event Structure

The service monitors for the following withdrawal event:

```solidity
event Withdrawal(address indexed user, address indexed to, uint256 amount, uint256 timestamp);
```

Where:
- `user`: The wallet address initiating the withdrawal
- `to`: The recipient address for the withdrawal
- `amount`: Amount in USDC (6 decimals, stored as U256)
- `timestamp`: Block timestamp when withdrawal was processed

## Architecture

### Core Components

1. **WithdrawalMonitor**: Main service orchestrator
2. **WithdrawalEventHandler**: Event processing and database storage
3. **Database Functions**: CRUD operations for withdrawal tracking
4. **Background Tasks**: Confirmation checking, retry processing, balance sync

### Database Schema

The service creates and manages three main tables:

#### `usdc_withdrawals`
- Tracks individual withdrawal events from smart contract
- States: `pending` → `confirmed` → `processed`
- Includes confirmation tracking and retry logic

#### `user_withdrawal_balances` 
- Aggregated view of user withdrawal history
- Auto-updated via database triggers
- Tracks total withdrawn, counts, timestamps

#### `withdrawal_processing_log`
- Audit trail for all processing events
- Error tracking and retry management
- Debugging and monitoring support

### Key Features

#### Confirmation Threshold
- Configurable block confirmations required (default: 12)
- Prevents double-spending and chain reorganization issues
- Validates withdrawal finality before balance updates

#### Idempotency
- Prevents duplicate debiting of same withdrawal transaction
- Uses `(event_transaction_hash, log_index)` as unique constraint
- Safe retry processing for failed operations

#### Balance Consistency  
- Atomic transactions ensure database consistency
- User balance debited only after withdrawal confirmation
- Database triggers maintain aggregate balances

#### Error Handling & Retry Logic
- Exponential backoff for failed operations
- Configurable retry attempts and timeouts
- Comprehensive error logging and monitoring

## Configuration

```rust
pub struct WithdrawalMonitorConfig {
    /// Required block confirmations before processing withdrawal
    pub required_confirmations: u64,        // Default: 12
    /// How often to check for confirmation updates (seconds)
    pub confirmation_check_interval: u64,   // Default: 30
    /// Maximum retry attempts for failed operations  
    pub max_retry_attempts: u32,            // Default: 3
    /// Retry backoff multiplier (seconds)
    pub retry_backoff_seconds: u64,         // Default: 60
    /// Batch size for processing withdrawals
    pub batch_size: usize,                  // Default: 100
}
```

## Usage

### Basic Usage

```rust
use ethereum_client::{EthereumClient, WithdrawalMonitor, WithdrawalMonitorConfig};
use sqlx::PgPool;
use std::sync::Arc;

// Create Ethereum client
let ethereum_client = Arc::new(EthereumClient::new(config).await?);

// Create database connection
let db_pool = PgPool::connect(&database_url).await?;

// Configure monitoring
let monitor_config = WithdrawalMonitorConfig {
    required_confirmations: 12,
    confirmation_check_interval: 30,
    max_retry_attempts: 3,
    retry_backoff_seconds: 60,
    batch_size: 100,
};

// Create and start withdrawal monitor
let mut withdrawal_monitor = WithdrawalMonitor::new(
    ethereum_client,
    db_pool,
    Some(monitor_config)
);

withdrawal_monitor.start().await?;
```

### Example Application

Run the example application:

```bash
cargo run --example withdrawal_monitor_example
```

### Environment Variables

```bash
# Required
DATABASE_URL=postgresql://user:pass@0.0.0.0:5432/arda_credit_db
ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/your-key
LOAN_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890

# Optional  
RUST_LOG=info
WITHDRAWAL_REQUIRED_CONFIRMATIONS=12
WITHDRAWAL_CHECK_INTERVAL=30
```

## Database Migrations

The service requires database migration `031_create_usdc_withdrawal_tracking.sql`:

```bash
# Apply migration
sqlx migrate run

# Or use make command if available
make initDB
```

## Monitoring & Statistics

The service provides withdrawal statistics:

```rust
let stats = withdrawal_monitor.get_withdrawal_stats().await?;

// Returns:
// {
//   "total_withdrawals": 150,
//   "pending_withdrawals": 5,
//   "confirmed_withdrawals": 140,
//   "processed_withdrawals": 140,
//   "failed_withdrawals": 5,
//   "unique_withdrawers": 75,
//   "total_amount_wei": "50000000000",
//   "total_amount_usdc": "50000.000000"
// }
```

## Integration Points

### Balance Update Service (ARDGBL-223)
- Updates user balances via `update_user_balance_from_withdrawal_tx()`
- Atomic transactions ensure consistency
- Integrates with existing user balance management

### Existing Deposit Monitor (ARDGBL-212)
- Shares common event handling patterns
- Reuses database connection pools and error handling
- Compatible monitoring and retry infrastructure

### Smart Contract Validation
- Validates withdrawal against user's available balance
- Ensures sufficient funds before processing
- Prevents overdraft conditions

## Error Handling

### Common Error Scenarios

1. **Insufficient Balance**: User attempts withdrawal exceeding available balance
2. **Network Issues**: Ethereum RPC connection failures
3. **Database Errors**: Connection or constraint violations
4. **Confirmation Delays**: Transactions taking longer than expected

### Retry Logic

- Failed withdrawals entered into retry queue
- Exponential backoff prevents overwhelming systems
- Maximum retry attempts prevent infinite loops
- Manual intervention possible for persistent failures

### Monitoring Alerts

Monitor these key metrics:

- **Failed Withdrawals**: Withdrawals stuck in retry queue
- **Processing Delays**: Confirmations taking longer than expected  
- **Balance Mismatches**: Database vs on-chain balance discrepancies
- **Error Rates**: High failure rates indicating system issues

## Security Considerations

### Withdrawal Validation
- Confirm user has sufficient available balance
- Validate withdrawal amount and recipient address
- Check for suspicious withdrawal patterns

### Database Security  
- Atomic transactions prevent partial updates
- Constraints prevent invalid data states
- Audit logs track all withdrawal processing events

### Access Control
- Database connections use principle of least privilege
- API keys and credentials secured via environment variables
- Read-only database access for monitoring dashboards

## Performance Considerations

### Batch Processing
- Process withdrawals in configurable batches
- Balance database load and processing latency
- Scale batch sizes based on withdrawal volume

### Connection Pooling
- Shared database connections across components
- Configurable pool sizes for optimal performance
- Connection health monitoring and recovery

### Caching Strategy
- Cache frequently accessed user balances
- Minimize database queries for confirmation checks
- Invalidate cache on balance updates

## Testing

### Unit Tests
```bash
cargo test withdrawal_monitor
```

### Integration Tests  
```bash
# Requires running PostgreSQL database
cd db && cargo test withdrawal
```

### Manual Testing
```bash
# Start local blockchain (Anvil)
anvil

# Deploy contracts
forge create src/Loan.sol:Loan --broadcast

# Run withdrawal monitor example
cargo run --example withdrawal_monitor_example
```

## Troubleshooting

### Common Issues

**Service Won't Start**
- Check database connectivity
- Verify Ethereum RPC endpoint 
- Confirm contract address is correct

**Withdrawals Not Processing**
- Check required confirmation count
- Verify block progression
- Review error logs for specific failures

**Balance Inconsistencies**
- Run database consistency checks
- Compare on-chain vs database balances
- Check for failed atomic transactions

### Debug Commands

```bash
# Check withdrawal processing status
psql $DATABASE_URL -c "SELECT status, COUNT(*) FROM usdc_withdrawals GROUP BY status;"

# View recent withdrawal processing logs  
psql $DATABASE_URL -c "SELECT * FROM withdrawal_processing_log ORDER BY created_at DESC LIMIT 10;"

# Check user withdrawal balances
psql $DATABASE_URL -c "SELECT user_address, total_withdrawn_usdc, withdrawal_count FROM user_withdrawal_balances ORDER BY total_withdrawn DESC LIMIT 10;"
```

## Future Enhancements

### Planned Features

1. **Real-time Notifications**: WebSocket notifications for withdrawal status changes
2. **Advanced Monitoring**: Grafana dashboards and Prometheus metrics
3. **Multi-chain Support**: Support for additional blockchain networks
4. **Withdrawal Limits**: Configurable daily/monthly withdrawal limits
5. **Compliance Integration**: AML/KYC checks for large withdrawals

### Performance Optimizations

1. **Event Streaming**: WebSocket event listening for faster processing
2. **Database Sharding**: Partition large withdrawal tables by date/user
3. **Caching Layer**: Redis caching for frequently accessed balances
4. **Background Jobs**: Queue-based processing for high-volume scenarios

## Support

For questions, issues, or feature requests:

1. Check existing documentation in `/docs`
2. Review GitHub issues for similar problems
3. Create new issue with reproduction steps
4. Include relevant logs and configuration