# Arda Credit Loan vApp Architecture Plan

This document provides detailed technical specifications for the Arda Credit loan system. For a high-level overview of the three-component architecture (smart contracts, ZKVM program, and web server), see [`docs/architecture/three-component-overview.md`](../architecture/three-component-overview.md).

## Overview

### User Management
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum UserType {
    Investor,
    Lender,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KycStatus {
    NotSubmitted,
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    pub user_type: UserType,
    pub kyc_status: KycStatus,
    pub kyc_submitted_at: Option<DateTime<Utc>>,
    pub kyc_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Loan Management
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum LoanStatus {
    Originated,  // Loan created, not yet funded
    Active,      // Loan funded and active
    PaidOff,     // Loan fully repaid
    Closed,      // Loan closed/settled
    Defaulted,   // Loan in default
}

#[derive(Debug, Clone)]
pub struct Loan {
    pub loan_id: Uuid,
    pub borrower_id: Uuid,       // Must be KYC'd
    pub lender_id: Uuid,         // Must be KYC'd
    pub principal_amount: Decimal,
    pub outstanding_balance: Decimal,
    pub interest_rate: Decimal,  // Annual percentage rate
    pub term_months: i32,
    pub loan_status: LoanStatus,
    pub originated_at: DateTime<Utc>,
    pub funded_at: Option<DateTime<Utc>>,
    pub maturity_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Transaction Processing
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionType {
    LoanOrigination,
    LoanPayment,
    LoanCloseout,
    KycSubmission,
    KycVerification,
}

#[derive(Debug, Clone)]
pub struct LoanTransaction {
    pub transaction_id: Uuid,
    pub user_id: Uuid,          // Transaction initiator
    pub loan_id: Option<Uuid>,  // None for KYC transactions
    pub transaction_type: TransactionType,
    pub amount: Option<Decimal>, // None for non-monetary transactions
    pub metadata: serde_json::Value, // Additional transaction data
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub batch_id: Option<Uuid>,
}
```

## Database Schema

### Core Tables
```sql
-- Users table
CREATE TABLE users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) NOT NULL UNIQUE,
    user_type VARCHAR(50) NOT NULL CHECK (user_type IN ('Investor', 'Lender')),
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'NotSubmitted' CHECK (kyc_status IN ('NotSubmitted', 'Pending', 'Approved', 'Rejected')),
    kyc_submitted_at TIMESTAMPTZ,
    kyc_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Loans table
CREATE TABLE loans (
    loan_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    borrower_id UUID NOT NULL REFERENCES users(user_id),
    lender_id UUID NOT NULL REFERENCES users(user_id),
    principal_amount DECIMAL(15,2) NOT NULL,
    outstanding_balance DECIMAL(15,2) NOT NULL,
    interest_rate DECIMAL(5,4) NOT NULL, -- e.g., 0.0750 for 7.5%
    term_months INTEGER NOT NULL,
    loan_status VARCHAR(50) NOT NULL DEFAULT 'Originated' CHECK (loan_status IN ('Originated', 'Active', 'PaidOff', 'Closed', 'Defaulted')),
    originated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    funded_at TIMESTAMPTZ,
    maturity_date TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Loan transactions table (replaces existing arithmetic_transactions)
CREATE TABLE loan_transactions (
    transaction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    loan_id UUID REFERENCES loans(loan_id), -- NULL for KYC transactions
    transaction_type VARCHAR(50) NOT NULL CHECK (transaction_type IN ('LoanOrigination', 'LoanPayment', 'LoanCloseout', 'KycSubmission', 'KycVerification')),
    amount DECIMAL(15,2), -- NULL for non-monetary transactions
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    batch_id UUID REFERENCES proof_batches(batch_id)
);
```

## API Endpoints

### User Management
- `POST /api/v1/users` - Register new user
- `GET /api/v1/users/{user_id}` - Get user details
- `POST /api/v1/users/{user_id}/kyc` - Submit KYC
- `PUT /api/v1/users/{user_id}/kyc/verify` - Verify KYC (admin)

### Loan Operations
- `POST /api/v1/loans` - Originate new loan
- `GET /api/v1/loans/{loan_id}` - Get loan details
- `POST /api/v1/loans/{loan_id}/payments` - Make loan payment
- `POST /api/v1/loans/{loan_id}/closeout` - Close out loan

### Transaction Processing (existing)
- `GET /api/v1/transactions/pending` - View pending transactions
- `POST /api/v1/batches` - Process batch of transactions
- `GET /api/v1/batches/{batch_id}` - Get batch details

## API Testing Examples

For complete API documentation, see the Swagger UI at `http://0.0.0.0:8080/swagger-ui/`

### User Operations
```bash
# Register users
curl -X POST http://0.0.0.0:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{"email":"alice@example.com","user_type":"investor"}'

# KYC operations
curl -X POST http://0.0.0.0:8080/api/v1/user/{uuid}/kyc \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get user details
curl http://0.0.0.0:8080/api/v1/user/{uuid} \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### Loan/Deal Operations
```bash
# Loan/Deal origination (requires KYC'd users)
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"originator_id":"<uuid>","investor_id":"<uuid>","amount":50000,"rate":0.075,"term_months":36}'

# Loan payments
curl -X POST http://0.0.0.0:8080/api/v1/deal/{uuid}/payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"amount":2500}'

# Loan queries
curl http://0.0.0.0:8080/api/v1/deal/{uuid} \
  -H "Authorization: Bearer YOUR_TOKEN"

curl "http://0.0.0.0:8080/api/v1/deals?user_id={uuid}" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**For complete API documentation, see the Swagger UI at http://0.0.0.0:8080/swagger-ui/**

## Zero-Knowledge Properties

### Public Values (On-Chain)
```rust
pub struct LoanBatchPublicValues {
    pub previous_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub transaction_count: u32,
    pub batch_timestamp: u64,
    pub transaction_types_hash: [u8; 32],
    pub previous_total_balance: u128,
    pub current_total_balance: u128,
}
```

### Private Data (Off-Chain)
- Individual loan amounts
- User identities and KYC status  
- Payment amounts and frequencies
- Interest rates and terms
- Loan origination details

### Privacy Guarantees
1. **Transaction Privacy**: Individual loan transactions remain private
2. **KYC Privacy**: KYC status can be proven without revealing identity
3. **Amount Privacy**: Loan amounts and payments are hidden in batches
4. **Inclusion Proofs**: Users can prove their transaction was included without revealing others

## Transaction Inclusion Proofs

### Proof Structure
```rust
pub struct TransactionInclusionProof {
    pub batch_proof: SP1Proof,
    pub merkle_proof: MerkleProof,
    pub transaction_hash: [u8; 32],
    pub transaction_type: TransactionType,
    pub user_id: Uuid,
}
```

### Verification Process
1. User downloads batch proof containing their transaction
2. Local verification checks:
   - SP1 proof validates state transition
   - Merkle proof validates transaction inclusion
   - Transaction hash matches user's transaction
3. Cryptographic guarantee without revealing other transactions

## Implementation Priority

### Phase 1: Core Infrastructure
1. Update database schema with loan tables
2. Implement basic user registration and KYC
3. Add loan data structures to `lib/`
4. Update SP1 program for loan transaction processing

### Phase 2: Loan Operations
1. Implement loan origination
2. Add payment processing
3. Implement loan closeout
4. Add batch processing for loan transactions

### Phase 3: Privacy Features
1. Transaction inclusion proofs
2. KYC status proofs
3. Enhanced privacy features
4. Production hardening

## Security Considerations

### KYC Requirements
- Loan origination requires both parties to be KYC'd
- KYC verification is tracked in transaction history
- KYC status can be proven without revealing identity

### Transaction Validation
- All monetary transactions require valid user and loan IDs
- Loan status must allow the requested operation
- Outstanding balance validation for payments

### Privacy Protection
- Individual transaction amounts never revealed on-chain
- User identities protected through ZK proofs
- KYC status provable but not directly exposed