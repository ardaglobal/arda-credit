# Test Data Population Scripts

This directory contains scripts to populate the Arda Credit database with fake test data for development and testing purposes.

## Prerequisites

1. PostgreSQL database running
2. Database migrations applied
3. Environment variables set

## Usage

### 1. Start the Database

```bash
# Start PostgreSQL via Docker
docker-compose up postgres -d

# Or start locally if you have PostgreSQL installed
# Make sure it's running on localhost:5432
```

### 2. Set Environment Variables

```bash
export DATABASE_URL="postgresql://postgres:password@127.0.0.1:5432/arda_credit_db"
```

### 3. Run the Population Script

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
GET http://localhost:8080/api/v2/ccr/identities
GET http://localhost:8080/api/v2/ccr/loans
GET http://localhost:8080/api/v2/ccr/loans/stats
POST http://localhost:8080/api/v2/ccr/identities
```

## Testing the API

After running the population script, you can test the API endpoints:

```bash
# Start the API server
cargo run -p api --bin server

# Test endpoints
curl http://localhost:8080/api/v2/ccr/identities
curl http://localhost:8080/api/v2/ccr/loans
curl http://localhost:8080/api/v2/ccr/loans/stats

# Register a new user
curl -X POST http://localhost:8080/api/v2/ccr/identities \
  -H "Content-Type: application/json" \
  -d '{
    "user_type": "investor",
    "email": "test@example.com"
  }'
```

## Customization

You can modify the `populate_test_data.rs` script to:
- Change the number of users/loans/payments
- Adjust loan amounts and interest rates
- Modify KYC status distributions
- Add different loan statuses
- Create specific test scenarios

## Cleanup

The script automatically clears existing test data before creating new data. If you want to manually clear the database:

```sql
-- Connect to PostgreSQL and run:
DELETE FROM payments;
DELETE FROM loans; 
DELETE FROM balances;
DELETE FROM identities;
```

## Troubleshooting

- **Database connection errors**: Check that PostgreSQL is running and DATABASE_URL is correct
- **Migration errors**: Make sure you've run `sqlx migrate run` from the db directory
- **Permission errors**: Ensure the database user has CREATE/INSERT/UPDATE/DELETE permissions