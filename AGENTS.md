# CLAUDE.md

This file provides guidance to AI agents when working with code in this repository.

## Overview

**Arda Credit** is a credit deal management platform with document ingestion, AI analysis, and blockchain integration. The system enables deal origination, payments, marketplace trading, and comprehensive document processing.

### Core Features

**Company & User Management:**
- **Companies** - Organizations that create/invest in deals (with KYB verification)
- **Users** - Individual team members within companies (with optional KYC)
- **Deals** - Agreements between originator companies and investor companies
- **Authentication** - Magic link email authentication with session management

**Deal Operations:**
- **Deal CRUD** - Create, read, update, delete deals between companies
- **Deal Payments** - Make payments toward outstanding deals
- **Deal Marketplace** - List deals for sale, browse, and purchase
- **Deal Transfers** - Transfer ownership between investors with validation

**Financial Operations:**
- **USDC Deposits** - Direct deposits to smart contract with auto-balance tracking
- **USDC Withdrawals** - Balance verification and blockchain execution
- **Transaction History** - Complete audit trail of all financial operations

**Blockchain & Privacy:**
- **Smart Contracts** - ARDA contract for deposits/withdrawals/proof verification
- **ZK Proofs** - SP1 zkVM batch processing via Sindri service
- **Merkle Tree ADS** - Database-backed authenticated data structure

### Core Architecture

**Three-Component Architecture:**
1. **Smart Contracts** (`contracts/`) - On-chain proof verification and state management
2. **ZKVM Program** (`program/`) - Zero-knowledge transaction processing
3. **Web Server** (Main repo) - API server and orchestration

**Component Structure:**
- **`api/`** - REST API server with Swagger docs
- **`db/`** - PostgreSQL with deal and transaction data
- **`contracts/`** - Solidity contracts (ARDA, MockUSDC, ARDAFaucet)
- **`program/`** - SP1 zkVM program for batch processing
- **`lib/`** - Shared computation logic (zkVM compatible)
- **`ethereum-client/`** - Ethereum integration utilities

## Essential Development Commands

### Quick Setup
```bash
make setup          # Install deps + copy .env + init database
make env            # Copy .env.example to .env (edit SINDRI_API_KEY)
make initDB         # Initialize database with migrations + SQLx cache
make deploy         # Deploy circuit to Sindri (for sandbox/production)
```

### Development Workflow
```bash
# Local development
make up             # Start Postgres + Anvil (local) or Postgres only (sandbox/production)
make server         # Run server (auto-deploys contracts if needed)

# Testing
make test           # All tests with database
cargo test          # Unit tests only
make forge-test     # Smart contract tests

# Code quality
make fmt            # Format code
make lint           # Lint code

# Cleanup
make down           # Stop all services
make clean-anvil    # Reset Anvil blockchain state
```

### Local Anvil Development

For local/staging environments, use Anvil devnet:

```bash
# Faucet (get test ETH + USDC)
make faucet-request ADDRESS=0x...   # 0.5 ETH + 1M USDC (1 hour cooldown)
make faucet-status ADDRESS=0x...    # Check balance and cooldown
make faucet-stats                   # View statistics

# Contract deployment (environment-aware)
SERVER_ENVIRONMENT=local make deploy-contracts      # → Anvil with mocks
SERVER_ENVIRONMENT=sandbox make deploy-contracts    # → Sepolia testnet
```

**What gets deployed:**
- **Local/Staging**: MockUSDC + ARDAFaucet + ARDA + SP1MockVerifier
- **Sandbox/Production**: ARDA only (requires RPC URL, verifier, program key)

### API Server Usage

**Swagger UI**: `http://0.0.0.0:8080/swagger-ui/` - Interactive API documentation

**Basic API Examples:**
```bash
# Health check
curl http://0.0.0.0:8080/api/v1/health

# Create user
curl -X POST http://0.0.0.0:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{"email":"user@example.com","user_type":"investor"}'

# List deals
curl "http://0.0.0.0:8080/api/v1/deals?status=active&limit=20" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**For all endpoints, see Swagger UI at http://0.0.0.0:8080/swagger-ui/**

### Email Debug Mode

For development without AWS SES:
```bash
echo "EMAIL_DEBUG_MODE=true" >> .env
# Magic links will be logged to console instead of sent via email
```

## Environment Configuration

**Key variables:**
- `DATABASE_URL` - PostgreSQL connection string
- `SERVER_ENVIRONMENT` - `local`, `staging`, `sandbox`, or `production`
  - `local`/`staging` use local SP1 prover
  - `sandbox`/`production` use Sindri service
- `SINDRI_API_KEY` - Required for sandbox/production proof generation
- `SINDRI_CIRCUIT_TAG` - Circuit version for ZK proofs
- `ETHEREUM_RPC_URL` - Ethereum RPC endpoint
- `ETHEREUM_ARDA_CONTRACT_ADDRESS` - Deployed ARDA contract address
- `DEAL_PROGRAM_VKEY` - Deal program verification key
- `EMAIL_DEBUG_MODE` - Set to `true` to log magic links to console
- `EMAIL_ENABLED` - Set to `true` to enable email

**AI provider (choose one):**
- `OPENAI_API_KEY` - OpenAI GPT models
- `ANTHROPIC_API_KEY` - Anthropic Claude models
- `AZURE_OPENAI_KEY` + `AZURE_OPENAI_ENDPOINT` - Azure OpenAI
- `OPENROUTER_API_KEY` - OpenRouter multi-model access

## Core Data Structures

**Users & Companies:**
```rust
struct User {
    user_id: uuid::Uuid,
    email: String,
    kyc_status: KycStatus,  // NotSubmitted | Pending | Approved | Rejected
    created_at: DateTime<Utc>,
}

struct Company {
    company_id: uuid::Uuid,
    legal_name: String,
    kyb_status: KybStatus,  // NotSubmitted | Pending | Approved | Rejected
    created_at: DateTime<Utc>,
}
```

**Deals:**
```rust
struct Deal {
    deal_id: uuid::Uuid,
    originator_id: uuid::Uuid,  // Company ID
    investor_id: uuid::Uuid,    // Company ID
    principal_amount: Decimal,
    outstanding_balance: Decimal,
    interest_rate_bps: i32,
    loan_status: String,  // originated | active | paid_off | closed | defaulted
    originated_at: DateTime<Utc>,
    maturity_date: Option<DateTime<Utc>>,
}

struct DealPayment {
    payment_id: uuid::Uuid,
    deal_id: uuid::Uuid,
    payer_id: uuid::Uuid,
    amount: Decimal,
    payment_type: String, // regular | principal | interest | penalty
    processed_at: DateTime<Utc>,
}
```

**Marketplace:**
```rust
struct DealListing {
    listing_id: uuid::Uuid,
    deal_id: uuid::Uuid,
    seller_id: uuid::Uuid,
    asking_price: Decimal,
    status: String, // active | sold | cancelled | expired
    listed_at: DateTime<Utc>,
}

struct DealTransfer {
    transfer_id: uuid::Uuid,
    deal_id: uuid::Uuid,
    from_investor_id: uuid::Uuid,
    to_investor_id: uuid::Uuid,
    transfer_price: Decimal,
    status: String, // pending | completed | cancelled
}
```

**Financial:**
```rust
struct UsdcDeposit {
    id: i32,
    user_address: String,
    amount: Decimal,
    transaction_hash: String,
    status: String, // pending | confirmed | processed | failed
    block_number: i64,
}

struct UsdcWithdrawal {
    id: i32,
    user_address: String,
    recipient_address: String,
    amount: Decimal,
    status: String, // pending | processing | completed | failed | cancelled
}
```

## Common Development Patterns

### Adding New Deal Operations
1. Add transaction type to `TransactionType` enum in `lib/src/transactions.rs`
2. Implement handler in `api/src/deal_handlers.rs`
3. Add API endpoint route in `api/src/routes.rs`
4. Test with Swagger UI at `http://0.0.0.0:8080/swagger-ui/`

### Database Changes
1. Create migration in `db/migrations/`
2. Run `make initDB` to apply and regenerate SQLx cache
3. Update relevant structs in `db/src/`

### Smart Contract Updates
1. Modify contracts in `contracts/src/`
2. Test: `make forge-test`
3. Deploy: `make deploy-contracts`
4. Update `ETHEREUM_ARDA_CONTRACT_ADDRESS` in .env

### Circuit Changes
1. Modify SP1 program in `program/src/main.rs`
2. Test locally: `make run` (~8s Core proofs)
3. Deploy to Sindri: `make deploy`
4. Update `SINDRI_CIRCUIT_TAG` in .env

## Deal Workflow Examples

### Complete Deal Lifecycle
```bash
# 1. Register company and users
curl -X POST http://0.0.0.0:8080/api/v1/company \
  -H "Content-Type: application/json" \
  -d '{"legal_name":"Acme Corp","email":"contact@acme.com"}'

# 2. Create deal between companies
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"originator_id":"company-uuid","investor_id":"investor-company-uuid","amount":50000}'

# 3. Make payment
curl -X POST http://0.0.0.0:8080/api/v1/deal/{deal-uuid}/payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"amount":5000}'

# 4. Trigger batch processing (ZK proof generation)
curl -X POST http://0.0.0.0:8080/api/v1/batch/trigger \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### Document-Based Deal Creation (OCR Workflow)
**NEW**: This workflow supports document upload where OCR extraction is done by a separate service,
and the backend creates companies and associates the user with their selected company.

**Flow**: Frontend → OCR Service → Frontend → Backend

```bash
# Process OCR-extracted company data
curl -X POST http://0.0.0.0:8080/api/v1/deals/process-document-extraction \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "document_reference": "https://ocr-service.com/documents/abc123",
    "originator": {
      "legal_name": "Acme Corp",
      "industry": "Technology",
      "jurisdiction": "Delaware"
    },
    "investor": {
      "legal_name": "Investment Fund LLC",
      "industry": "Finance"
    },
    "borrower": {
      "legal_name": "BorrowerCo Inc"
    },
    "user_association": {
      "company_role": "originator",
      "profile_type": "originator"
    },
    "deal_terms": {
      "principal_amount": "100000.00",
      "interest_rate_bps": 500,
      "term_days": 365
    }
  }'
```

**Response includes:**
- Created/looked-up companies with IDs
- User membership details (admin role, active status)
- Pre-filled deal data for frontend to display

**Features:**
- Auto-creates companies if they don't exist
- Links user to selected company as admin
- Idempotent (re-running with same data won't duplicate)
- Supports optional borrower entity
- Returns pre-filled deal data for user confirmation

**Then create the deal:**
```bash
# Use pre-filled data to create deal (user confirms on frontend)
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "originator_id": "uuid-from-response",
    "investor_id": "uuid-from-response",
    "borrower_id": "uuid-from-response",
    "principal_amount": "100000.00",
    "interest_rate_bps": 500,
    "term_days": 365
  }'
```

### Deal Marketplace
```bash
# List deal for sale
curl -X POST http://0.0.0.0:8080/api/v1/deal/{deal-uuid}/list-for-sale \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"asking_price":47500,"description":"Investment opportunity"}'

# Browse marketplace
curl "http://0.0.0.0:8080/api/v1/deals/for-sale?limit=10" \
  -H "Authorization: Bearer YOUR_TOKEN"

# Transfer ownership
curl -X POST http://0.0.0.0:8080/api/v1/deal/{deal-uuid}/transfer \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"from_investor_id":"current-uuid","to_investor_id":"new-uuid","transfer_price":47500}'
```

### USDC Operations
```bash
# Check balance
curl "http://0.0.0.0:8080/api/v1/balance/{wallet-address}" \
  -H "Authorization: Bearer YOUR_TOKEN"

# Initiate withdrawal
curl -X POST http://0.0.0.0:8080/api/v1/withdrawal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{"user_address":"0x...","recipient_address":"0x...","amount":"100.50"}'

# Check withdrawal status
curl "http://0.0.0.0:8080/api/v1/withdrawal/{id}" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

## Debugging Tips

### SQLx Offline Mode
```bash
export SQLX_OFFLINE=true
cargo check --workspace     # Works without PostgreSQL
```

### Common Issues
- **Proof generation fails**: Check `SINDRI_API_KEY` and circuit deployment
- **Database connection errors**: Ensure PostgreSQL running via `make up`
- **API authentication errors**: Verify Bearer token is valid
- **Contract deployment fails**: Check all Ethereum environment variables

### Service URLs
- **API Server**: `http://0.0.0.0:8080`
- **Health Check**: `http://0.0.0.0:8080/api/v1/health`
- **Swagger Docs**: `http://0.0.0.0:8080/swagger-ui/`
- **Database**: `0.0.0.0:5432`

### Key API Endpoints
- **Authentication**: `POST /api/v1/auth/magic-link`, `POST /api/v1/auth/verify`
- **Users**: `POST /api/v1/user`, `GET /api/v1/user/{id}`
- **Companies**: `POST /api/v1/company`, `GET /api/v1/companies`
- **Deals**: `POST /api/v1/deal`, `GET /api/v1/deals`, `GET /api/v1/deal/{id}`
- **Document Extraction**: `POST /api/v1/deals/process-document-extraction` (NEW - OCR workflow)
- **Marketplace**: `GET /api/v1/deals/for-sale`, `POST /api/v1/deal/{id}/list-for-sale`
- **Transfers**: `POST /api/v1/deal/{id}/transfer`, `GET /api/v1/deal/{id}/transfers`
- **Financial**: `GET /api/v1/balance/{address}`, `POST /api/v1/withdrawal`

## Documentation References

See the [`docs/`](docs/) directory for detailed information:

### Development & Setup
- **Local Development**: `docs/deployment/development-setup.md`
- **Local Testing**: `docs/development/local-testing.md`
- **Local Anvil Workflow**: `docs/development/local-anvil-workflow.md`

### Architecture & Design
- **Three-Component Architecture**: `docs/architecture/three-component-overview.md`
- **Deal System**: `docs/architecture/deal-system.md`
- **Performance Targets**: `docs/architecture/performance-targets.md`

### Integration & APIs
- **API Documentation**: `docs/integrations/api-swagger.md`
- **Authentication**: `docs/integrations/authentication.md`
- **Frontend Examples**: `docs/integrations/frontend-examples.md`

### Deployment & Operations
- **Production Security**: `docs/deployment/security-production.md`
- **Monitoring**: `docs/deployment/monitoring.md`

## System Notes

### Company-Based Model
- **Deals are between companies**, not individual users
- Users are team members within companies
- Users have KYC status, companies have KYB (Know Your Business) status
- Deal originators and investors are company IDs

### Terminology Migration
- System converted from "loan" to "deal" terminology
- Database field `loan_status` kept for internal compatibility
- All APIs use "deal" terminology
- Smart contract renamed from "Loan" to "ARDA"

### ZK Privacy Model
- **Deal amounts and payments** - Private via batch processing
- **Company identities** - Visible (deals are company-to-company)
- **Batch processing** - Groups transactions for privacy
- **Proof verification** - On-chain via ARDA contract

See `docs/README.md` for complete documentation hub.
