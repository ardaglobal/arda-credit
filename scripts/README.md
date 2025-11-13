# Test Data Population Scripts

This directory contains scripts to populate the Arda Credit database with fake test data for development and testing purposes.

## Available Scripts

1. **`populate_test_data`** - Legacy script for basic user/loan data
2. **`populate_deal_data`** - Advanced script for realistic deal data with companies

## Prerequisites

1. PostgreSQL database running
2. Database migrations applied
3. Environment variables set

## Setup

### 1. Start the Database

```bash
# Start PostgreSQL via Docker
docker-compose up postgres -d

# Or start locally if you have PostgreSQL installed
# Make sure it's running on 0.0.0.0:5432
```

### 2. Set Environment Variables

```bash
export DATABASE_URL="postgresql://postgres:password@127.0.0.1:5432/arda_credit_db"
```

## Deal Data Population Script (Recommended)

### Usage

```bash
# Random mode - Generate random deals with random companies
cargo run --bin populate_deal_data -- --count 30

# User mode - Generate deals for a specific user (creates user if doesn't exist)
cargo run --bin populate_deal_data -- --email matt@arda.xyz --count 20

# Clear existing deal data first, then generate new data
cargo run --bin populate_deal_data -- --clear --count 25

# From root directory
cargo run -p scripts --bin populate_deal_data -- --email user@example.com --count 15
```

### Features

- **Email-based mode detection**: No email = random mode, email provided = user mode
- **Automatic user creation**: Creates users that don't exist
- **Company management**: Creates investor and originator companies for users
- **Realistic data**: Proper principal amounts ($10K-$500K), interest rates (4-12% APR)
- **Market-ready fields**: Industries, regions, asset classes, risk levels, descriptions
- **Configurable**: All parameters available as constants at the top of the script

### What It Creates

**Random Mode:**
- Random companies with realistic names
- Deals between different companies
- Varied deal statuses, amounts, and terms

**User Mode:**
- Two companies for the user: "Name Investor Corp" and "Name Originator Corp" 
- Company memberships with admin roles
- 50/50 split: half deals as investor, half as originator
- Realistic counterparty companies

### Sample Output

```
🚀 Starting deal data population...
✅ Connected to database
👤 User mode: Generating deals for matt@arda.xyz
👤 User matt@arda.xyz not found, creating new user...
✅ Created new user: matt@arda.xyz (079c3d55-4604-457a-b53f-940a26de8e90)
🏢 Created companies: matt Investor Corp and matt Originator Corp
💰 Creating 2 deals as investor
  💰 Created deal: $84841 at 10.6% APR (active)
  💰 Created deal: $461525 at 11.63% APR (active)
🏢 Creating 2 deals as originator
  💰 Created deal: $342131 at 5.07% APR (active)
  💰 Created deal: $247633 at 6.96% APR (active)
✅ Created 4 deals for user matt@arda.xyz
🎉 Deal data population completed successfully!
```

## Legacy Test Data Script

### Usage

```bash
# From the root directory
cargo run -p scripts --bin populate_test_data

# Or from the scripts directory
cd scripts
cargo run --bin populate_test_data
```

## What the Script Creates

### Test Identities (10 users)
- **Investors**: Users who can borrow money
- **Lenders**: Users who can lend money  
- **KYC Status**: Mix of approved, pending, rejected, and not_submitted

### Test Loans (8 loans)
- Various loan amounts from $250 to $2000
- Different interest rates (4% - 8% APR)
- Mixed loan statuses: active, originated, paid_off, closed
- Realistic creation dates (5-120 days ago)

### Test Payments (15-30 payments)
- Multiple payments per active loan
- Different payment types: principal, interest, mixed  
- Spread over time with realistic dates

### Initial Balances
- Each user gets $10,000 available balance
- $0 locked balance (for future features)

## Sample Output

```
🚀 Starting database population with test data...
✅ Connected to database
🧹 Clearing existing test data...
👥 Creating test identities...
  👤 Created Alice Johnson (investor) with KYC: approved
  👤 Created Bob Smith (lender) with KYC: approved
  ...
💰 Creating test loans...
  💰 Created $500.00 loan: alice -> bob (active)
  💰 Created $1000.00 loan: carol -> david (active)
  ...
💳 Creating test payments...
  💳 Created 3 payments for loan 12345678 ($83.33 each)
  ...

📊 Test Data Summary:
=====================
👥 Identities: 10
   - investor: 5
   - lender: 5
🔐 KYC Status:
   - approved: 6
   - pending: 2
   - rejected: 1
   - not_submitted: 1
💰 Loans: 8
   - Total Principal: $4450.00
   - Total Outstanding: $3560.00
📈 Loan Status:
   - active: 5
   - originated: 1
   - paid_off: 1
   - closed: 1
💳 Payments: 18
   - Total Payment Amount: $890.00
📊 Payment Types:
   - principal: 6
   - interest: 6
   - mixed: 6

🌐 API Testing URLs:
GET http://0.0.0.0:8080/api/v1/ccr/identities
GET http://0.0.0.0:8080/api/v1/ccr/loans
GET http://0.0.0.0:8080/api/v1/ccr/loans/stats
POST http://0.0.0.0:8080/api/v1/ccr/identities
```

## Configuration

### Deal Data Script Configuration

The `populate_deal_data.rs` script is highly configurable via constants at the top of the file:

```rust
// Default values
const DEFAULT_DEAL_COUNT_STR: &str = "25";
const DEFAULT_DATABASE_URL: &str = "postgresql://postgres:password@127.0.0.1:5432/arda_credit_db";

// Deal generation parameters
const MIN_PRINCIPAL_AMOUNT: u32 = 10_000;        // $10K minimum
const MAX_PRINCIPAL_AMOUNT: u32 = 500_000;       // $500K maximum
const MIN_INTEREST_RATE_BPS: i32 = 400;          // 4% APR
const MAX_INTEREST_RATE_BPS: i32 = 1200;         // 12% APR

// Distribution weights (must sum to 100)
const DEAL_STATUS_ACTIVE_WEIGHT: u32 = 60;       // 60% active deals
const DEAL_STATUS_ORIGINATED_WEIGHT: u32 = 20;   // 20% originated
const DEAL_STATUS_PAID_OFF_WEIGHT: u32 = 15;     // 15% paid off
const DEAL_STATUS_CLOSED_WEIGHT: u32 = 5;        // 5% closed

const RISK_LEVEL_LOW_WEIGHT: u32 = 30;           // 30% low risk
const RISK_LEVEL_MEDIUM_WEIGHT: u32 = 50;        // 50% medium risk  
const RISK_LEVEL_HIGH_WEIGHT: u32 = 20;          // 20% high risk
```

## Testing the API

After running the population scripts, you can test the API endpoints:

```bash
# Start the API server
cargo run -p api --bin server

# Test deal endpoints (new)
curl http://localhost:8080/api/v1/deals
curl http://localhost:8080/api/v1/deals?limit=10
curl "http://localhost:8080/api/v1/deals?status=active&limit=5"

# Test legacy endpoints
curl http://0.0.0.0:8080/api/v1/ccr/identities
curl http://0.0.0.0:8080/api/v1/ccr/loans
curl http://0.0.0.0:8080/api/v1/ccr/loans/stats

# Register a new user
curl -X POST http://0.0.0.0:8080/api/v1/user \
  -H "Content-Type: application/json" \
  -d '{
    "user_type": "investor",
    "email": "test@example.com"
  }'
```

## Customization

### Deal Data Script

Easily customize by modifying constants in `populate_deal_data.rs`:
- **Deal amounts**: Change `MIN_PRINCIPAL_AMOUNT` and `MAX_PRINCIPAL_AMOUNT`
- **Interest rates**: Adjust `MIN_INTEREST_RATE_BPS` and `MAX_INTEREST_RATE_BPS` 
- **Status distribution**: Modify the `DEAL_STATUS_*_WEIGHT` constants
- **Risk distribution**: Change `RISK_LEVEL_*_WEIGHT` constants
- **Data variety**: Add to `INDUSTRIES`, `REGIONS`, `ASSET_CLASSES` arrays
- **Company names**: Expand `COMPANY_TYPES` and `COMPANY_PREFIXES` arrays

### Legacy Test Data Script

You can modify the `populate_test_data.rs` script to:
- Change the number of users/loans/payments
- Adjust loan amounts and interest rates
- Modify KYC status distributions
- Add different loan statuses
- Create specific test scenarios

## Cleanup

Both scripts provide cleanup options:

### Deal Data Script
```bash
# Clear existing deal data before generating new data
cargo run --bin populate_deal_data -- --clear --count 10
```

### Manual Database Cleanup
```sql
-- Connect to PostgreSQL and run:

-- For deal data (new)
DELETE FROM deal_payments;
DELETE FROM deals;
DELETE FROM company_memberships WHERE company_id IN 
  (SELECT id FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%');
DELETE FROM companies WHERE name LIKE '%Test Corp%' OR name LIKE '%Random Corp%';

-- For legacy data
DELETE FROM payments;
DELETE FROM loans; 
DELETE FROM balances;
DELETE FROM identities;
```

### Complete Reset
```sql
-- WARNING: This will delete ALL data
DELETE FROM deal_payments;
DELETE FROM deals;
DELETE FROM company_memberships;
DELETE FROM companies;
DELETE FROM payments;
DELETE FROM loans;
DELETE FROM balances;
DELETE FROM users;
```

## Troubleshooting

### Common Issues

- **Database connection errors**: 
  - Check that PostgreSQL is running on the correct port
  - Verify DATABASE_URL environment variable is correct
  - Test connection: `psql $DATABASE_URL`

- **Migration errors**: 
  - Make sure you've run `sqlx migrate run` from the db directory
  - Check that all migrations completed successfully

- **Permission errors**: 
  - Ensure the database user has CREATE/INSERT/UPDATE/DELETE permissions
  - Check that the database exists and is accessible

- **Compilation errors**:
  - Run `cargo clean` and try again
  - Make sure you're in the correct directory
  - Verify all dependencies are available with `cargo check`

- **Deal creation fails**:
  - Check that companies exist before creating deals
  - Verify foreign key constraints are satisfied
  - Ensure deal data meets database validation requirements

### Debug Mode

For more detailed output, set the RUST_LOG environment variable:

```bash
RUST_LOG=debug cargo run --bin populate_deal_data -- --count 5
```

This will show all database queries and operations for troubleshooting.

## Quick Start Examples

### For Frontend Development
```bash
# Generate 50 realistic deals for frontend testing
cargo run --bin populate_deal_data -- --clear --count 50

# Test the API
curl http://localhost:8080/api/v1/deals?limit=10
```

### For User-Specific Testing  
```bash
# Create deals for a specific user
cargo run --bin populate_deal_data -- --email alice@company.com --count 15

# Test user-specific endpoints
curl "http://localhost:8080/api/v1/deals?originator_id=USER_COMPANY_ID"
```

### For Demo/Presentation
```bash
# Create a small set of high-quality demo data
cargo run --bin populate_deal_data -- --clear --count 8
```

## Which Script to Use?

- **Use `populate_deal_data`** (recommended) for:
  - Modern deal-based development
  - Frontend integration testing
  - Demo and presentation data
  - Realistic marketplace data
  - Company-based workflows

- **Use `populate_test_data`** (legacy) for:
  - Legacy loan system testing
  - Backward compatibility testing
  - Simple user/loan scenarios