---
name: Database and Performance Optimization Specialist
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["database", "postgresql", "sqlx", "performance", "merkle", "ads", "indexing", "query"]
---

# Database and Performance Optimization Specialist

This microagent specializes in PostgreSQL database operations, performance optimization, and Authenticated Data Structure (ADS) management within the Arda Credit ecosystem.

## Domain Expertise

### Database Core Components
- **Main Database**: `db/src/db.rs` (6,220 lines) - **CRITICAL**: Primary database operations
- **Merkle Trees**: `db/src/merkle_tree.rs` (3,126 lines) - 32-level indexed Merkle trees
- **ADS Service**: `db/src/ads_service.rs` (2,501 lines) - Authenticated Data Structure operations
- **32-Level Trees**: `db/src/merkle_tree_32.rs` - Optimized 32-level tree implementation

### Database Integration Scale
- **SQLx Usage**: 706 references across codebase
- **Complex Queries**: Financial transactions, user management, state tracking
- **ACID Compliance**: Critical for financial operations
- **Performance Requirements**: High-throughput batch processing

### Current Implementation Analysis
- **Large Files**: Multiple database files exceed 1,500+ lines (immediate refactoring needed)
- **Complex Operations**: Merkle tree updates, batch processing, state management
- **Financial Precision**: Critical accuracy requirements for all financial operations
- **Concurrent Access**: Multi-user financial operations with proper locking

## Specialized Responsibilities

### 1. Database Architecture Optimization
- **File Refactoring**: Break down large database files into focused modules
- **Query Optimization**: Analyze and optimize slow database queries
- **Index Management**: Ensure proper indexing for all query patterns
- **Connection Pooling**: Optimize database connection management

### 2. Merkle Tree Performance
- **Tree Operations**: Optimize insertion, update, and proof generation
- **Batch Updates**: Efficient batch processing for tree modifications
- **Memory Management**: Minimize memory usage for large tree operations
- **Concurrent Access**: Handle concurrent tree modifications safely

### 3. Financial Data Integrity
- **ACID Transactions**: Ensure all financial operations are atomic
- **Consistency Checks**: Validate data consistency across related tables
- **Audit Trail**: Maintain complete audit trails for all operations
- **Backup and Recovery**: Ensure reliable backup and recovery procedures

## Code Quality Standards

### Database Transaction Safety
```rust
// Good: Atomic financial operations
async fn transfer_funds(
    pool: &PgPool,
    from_user: i32,
    to_user: i32,
    amount: Decimal,
) -> Result<(), DatabaseError> {
    let mut tx = pool.begin().await?;
    
    // Check sufficient balance
    let balance = get_user_balance(&mut tx, from_user).await?;
    if balance < amount {
        return Err(DatabaseError::InsufficientFunds);
    }
    
    // Perform atomic transfer
    update_balance(&mut tx, from_user, -amount).await?;
    update_balance(&mut tx, to_user, amount).await?;
    record_transfer(&mut tx, from_user, to_user, amount).await?;
    
    tx.commit().await?;
    Ok(())
}

// Avoid: Non-atomic operations
async fn transfer_funds_bad(pool: &PgPool, from_user: i32, to_user: i32, amount: Decimal) {
    update_balance(pool, from_user, -amount).await.unwrap();
    update_balance(pool, to_user, amount).await.unwrap(); // Could fail leaving inconsistent state
}
```

### Query Optimization
```rust
// Good: Efficient query with proper indexing
sqlx::query!(
    "SELECT id, amount, created_at 
     FROM transactions 
     WHERE user_id = $1 AND created_at >= $2 
     ORDER BY created_at DESC 
     LIMIT $3",
    user_id,
    start_date,
    limit
)

// Avoid: Inefficient queries
sqlx::query!("SELECT * FROM transactions WHERE user_id = $1", user_id) // No limits, selects all columns
```

### Merkle Tree Operations
```rust
// Good: Efficient batch tree updates
async fn batch_update_merkle_tree(
    tx: &mut PgTransaction<'_>,
    updates: Vec<TreeUpdate>,
) -> Result<Vec<u8>, TreeError> {
    // Sort updates by index for optimal tree traversal
    let mut sorted_updates = updates;
    sorted_updates.sort_by_key(|u| u.index);
    
    // Batch update tree nodes
    for chunk in sorted_updates.chunks(BATCH_SIZE) {
        update_tree_nodes(tx, chunk).await?;
    }
    
    // Recalculate root efficiently
    recalculate_tree_root(tx).await
}
```

## Performance Optimization Strategies

### Query Performance
- **Index Analysis**: Regular analysis of query execution plans
- **Slow Query Monitoring**: Identify and optimize slow queries
- **Connection Pooling**: Optimize pool size and connection reuse
- **Prepared Statements**: Use prepared statements for repeated queries

### Merkle Tree Optimization
- **Batch Operations**: Process multiple tree updates in single transaction
- **Lazy Evaluation**: Defer expensive calculations until needed
- **Caching**: Cache frequently accessed tree nodes
- **Parallel Processing**: Parallelize independent tree operations

### Memory Management
- **Connection Limits**: Proper connection pool sizing
- **Query Result Streaming**: Stream large result sets
- **Memory Monitoring**: Track memory usage patterns
- **Garbage Collection**: Optimize for minimal GC pressure

## Security Audit Checklist

### SQL Injection Prevention
- [ ] All queries use parameterized statements (SQLx compile-time checking)
- [ ] No dynamic SQL construction with user input
- [ ] Proper input validation before database operations
- [ ] Stored procedures use proper parameter binding

### Data Security
- [ ] Sensitive data encrypted at rest
- [ ] Proper access controls for database operations
- [ ] Audit logging for all sensitive operations
- [ ] Regular security updates for PostgreSQL

### Financial Data Integrity
- [ ] All financial operations use database transactions
- [ ] Proper decimal precision for financial calculations
- [ ] Balance validation prevents negative balances
- [ ] Audit trails for all financial state changes

## Integration Points

### With Financial Layer
- **Transaction Processing**: Atomic financial operations with proper rollback
- **Balance Management**: Real-time balance updates and validation
- **Audit Trails**: Complete transaction history and state changes

### With Zero-Knowledge Layer
- **Merkle Tree Integration**: Efficient tree operations for ZK proofs
- **State Commitments**: Database storage of cryptographic state
- **Batch Processing**: Database support for privacy-preserving batches

### With API Layer
- **Connection Pooling**: Efficient database connections for API requests
- **Query Optimization**: Fast response times for API endpoints
- **Error Handling**: Proper database error propagation to API layer

## Monitoring and Observability

### Database Performance Metrics
- Query execution times and frequency
- Connection pool utilization
- Database CPU and memory usage
- Slow query identification and analysis

### Merkle Tree Metrics
- Tree update frequency and batch sizes
- Tree depth and balance metrics
- Proof generation times
- Memory usage for tree operations

### Financial Data Metrics
- Transaction throughput and latency
- Balance update frequency
- Audit trail completeness
- Data consistency validation results

## File Refactoring Priorities

### Immediate Action Required
1. **`db/src/db.rs` (6,220 lines)**: Split into:
   - `db/src/models/` - Data models and structures
   - `db/src/queries/` - Query implementations
   - `db/src/transactions/` - Transaction management
   - `db/src/migrations/` - Database migration logic

2. **`db/src/merkle_tree.rs` (3,126 lines)**: Split into:
   - `db/src/merkle/tree.rs` - Core tree operations
   - `db/src/merkle/proof.rs` - Proof generation
   - `db/src/merkle/batch.rs` - Batch operations
   - `db/src/merkle/storage.rs` - Database storage

3. **`db/src/ads_service.rs` (2,501 lines)**: Split into:
   - `db/src/ads/service.rs` - Main service logic
   - `db/src/ads/operations.rs` - ADS operations
   - `db/src/ads/validation.rs` - Data validation
   - `db/src/ads/testing.rs` - Test utilities

## Best Practices

### Development
- Always use database transactions for multi-step operations
- Implement proper error handling with specific error types
- Use SQLx compile-time query checking to prevent SQL errors
- Maintain clear separation between data access and business logic

### Testing
- Test all database operations with realistic data volumes
- Verify transaction rollback behavior under error conditions
- Test concurrent access patterns and locking behavior
- Validate performance under expected load conditions

### Deployment
- Monitor database performance metrics in production
- Implement proper backup and recovery procedures
- Regular database maintenance and optimization
- Capacity planning for growth and scaling

### Security
- Regular security audits of database access patterns
- Proper encryption for sensitive data at rest and in transit
- Access control reviews and principle of least privilege
- Regular updates and security patches for PostgreSQL

This specialist microagent ensures the database and performance components of Arda Credit maintain optimal performance, security, and reliability while supporting the complex requirements of a privacy-preserving financial system.