---
name: Financial Operations and Compliance Specialist
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["loan", "kyc", "financial", "compliance", "usdc", "deposit", "withdrawal", "credit", "liquidity"]
---

# Financial Operations and Compliance Specialist

This microagent specializes in financial operations, loan processing, KYC compliance, and regulatory requirements within the Arda Credit ecosystem.

## Domain Expertise

### Financial Core Components
- **Loan Processing**: `lib/src/loan_processor.rs` (2,345 lines) - Core loan business logic
- **Loan Models**: `lib/src/loan.rs` (1,000 lines) - Loan data structures and operations
- **Balance Management**: `lib/src/balance.rs` (988 lines) - User balance tracking and validation
- **Financial Handlers**: `api/src/financial_transaction_handlers.rs` (1,599 lines) - API endpoints

### KYC and Compliance
- **KYC Service**: `api/src/kyc_service.rs` - Know Your Customer verification
- **Authentication**: `api/src/auth.rs` - User authentication with Argon2 + JWT
- **User Management**: `api/src/user_handlers.rs` - User registration and management
- **Compliance Tracking**: Database triggers and audit trails

### Liquidity Pool Management
- **Pool Handlers**: `api/src/pool_handlers.rs` (1,336 lines) - Liquidity pool operations
- **Pool Borough**: `api/src/pool_borough_handlers.rs` - Geographic risk distribution
- **Credit Lines**: `api/src/credit_line_handlers.rs` (1,184 lines) - Credit line management
- **Pool API**: `api/src/pool_api.rs` - Pool-specific API operations

## Current Implementation Analysis

### Financial Operations Scale
- **Database Queries**: 706 SQLx references across codebase
- **USDC Integration**: Direct smart contract deposits/withdrawals
- **Transaction Types**: Deposits, withdrawals, loan origination, payments, closeouts
- **User Types**: Investors, lenders, merchants with KYC status tracking

### Compliance Features
- **KYC Verification**: Multi-level KYC status (verified, pending, rejected)
- **Geographic Limits**: Borough-based percentage limits for risk diversification
- **Audit Trails**: Complete transaction history and state changes
- **Regulatory Reporting**: Structured data for compliance reporting

## Specialized Responsibilities

### 1. Loan Lifecycle Management
- **Origination**: Verify loan terms, collateral, and participant eligibility
- **Payment Processing**: Handle loan payments with proper accounting
- **Interest Calculations**: Accurate interest accrual and compounding
- **Closeout Procedures**: Final settlement and loan completion

### 2. KYC and Compliance
- **Identity Verification**: Multi-tier KYC verification processes
- **Risk Assessment**: Geographic and financial risk evaluation
- **Regulatory Compliance**: AML, BSA, and other financial regulations
- **Audit Trail Maintenance**: Complete transaction and state change logging

### 3. Liquidity Pool Operations
- **Pool Creation**: Investor-driven liquidity pool establishment
- **Risk Management**: LTV ratios, interest rates, repayment periods
- **Geographic Distribution**: Borough-based risk diversification
- **Yield Management**: Interest rate optimization and pool performance

## Code Quality Standards

### Financial Precision
```rust
// Good: Use rust_decimal for financial calculations
use rust_decimal::Decimal;
let interest = principal * rate / Decimal::from(100);

// Avoid: Floating point for financial calculations
let interest = principal * 0.05; // Precision loss risk
```

### Error Handling for Financial Operations
```rust
// Good: Comprehensive error handling
match process_loan_payment(payment) {
    Ok(result) => result,
    Err(LoanError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
    Err(LoanError::InvalidLoan) => return Err(ApiError::LoanNotFound),
    Err(e) => return Err(ApiError::InternalError(e.to_string())),
}

// Avoid: Generic error handling
let result = process_loan_payment(payment).unwrap();
```

### Database Transaction Safety
```rust
// Good: Atomic financial operations
let mut tx = pool.begin().await?;
update_user_balance(&mut tx, user_id, amount).await?;
record_transaction(&mut tx, transaction).await?;
tx.commit().await?;

// Avoid: Non-atomic financial operations
update_user_balance(pool, user_id, amount).await?;
record_transaction(pool, transaction).await?; // Could fail leaving inconsistent state
```

## Security Audit Checklist

### Financial Security
- [ ] All financial calculations use `rust_decimal` for precision
- [ ] Database transactions are atomic for financial operations
- [ ] Input validation prevents negative amounts and overflow
- [ ] Balance checks prevent overdrafts and double-spending

### KYC and Authentication
- [ ] Password hashing uses Argon2 with proper parameters
- [ ] JWT tokens have appropriate expiration times
- [ ] KYC data is properly encrypted at rest
- [ ] Access controls prevent unauthorized financial operations

### Regulatory Compliance
- [ ] All financial transactions are logged with audit trails
- [ ] KYC verification follows regulatory requirements
- [ ] Geographic restrictions are properly enforced
- [ ] Suspicious activity monitoring is implemented

## Integration Points

### With Zero-Knowledge Layer
- **Privacy-Preserving Transactions**: Financial operations maintain privacy through ZK proofs
- **Batch Processing**: Financial transactions processed in privacy-preserving batches
- **State Verification**: Financial state changes verified cryptographically

### With Smart Contracts
- **USDC Operations**: Direct integration with USDC smart contracts
- **State Synchronization**: On-chain state matches off-chain financial records
- **Event Processing**: Smart contract events trigger financial state updates

### With Database Layer
- **ACID Compliance**: All financial operations maintain database consistency
- **Audit Logging**: Complete audit trail for all financial operations
- **Performance Optimization**: Efficient queries for financial data retrieval

## Performance Optimization

### Financial Calculations
- **Batch Processing**: Process multiple financial operations efficiently
- **Caching**: Cache frequently accessed financial data
- **Database Indexing**: Optimize database queries for financial operations
- **Connection Pooling**: Efficient database connection management

### Compliance Operations
- **KYC Caching**: Cache KYC verification results appropriately
- **Risk Calculation**: Optimize risk assessment algorithms
- **Reporting**: Efficient generation of compliance reports
- **Data Archival**: Proper archival of historical financial data

## Monitoring and Observability

### Financial Metrics
- Transaction volume and value
- Loan origination and payment rates
- Pool utilization and performance
- User balance and activity patterns

### Compliance Metrics
- KYC verification success rates
- Geographic distribution compliance
- Audit trail completeness
- Regulatory reporting accuracy

### Risk Metrics
- Default rates and risk exposure
- Geographic concentration limits
- Liquidity pool health
- Credit line utilization

## Best Practices

### Development
- Always use `rust_decimal::Decimal` for financial calculations
- Implement comprehensive input validation for all financial operations
- Use database transactions for all multi-step financial operations
- Maintain clear audit trails for all financial state changes

### Testing
- Test financial calculations with edge cases and boundary conditions
- Verify compliance rules are properly enforced
- Test error handling for all financial failure scenarios
- Validate audit trail completeness and accuracy

### Deployment
- Monitor financial operation performance and accuracy
- Implement proper backup and recovery procedures for financial data
- Maintain compliance with regulatory requirements
- Regular security audits of financial operations

This specialist microagent ensures the financial operations and compliance components of Arda Credit maintain the highest standards of accuracy, security, and regulatory compliance.