# Self Double Pledge Feature

## Overview

The self double-pledge feature allows users to intentionally mark and track when the **same collateral** is used across multiple deals. This is distinct from duplicate detection, which identifies when **different collateral records** might represent the same physical asset.

## Use Cases

1. **Multiple Tranches**: A single property used as collateral for multiple tranches of the same loan
2. **Layered Financing**: Primary and secondary liens on the same asset
3. **Partial Pledges**: Different portions of the same collateral pledged to different deals
4. **Syndicated Loans**: Same collateral shared across multiple investors in a syndication

## Key Concepts

### Intentional vs Detected Double Pledge

- **Intentional Double Pledge (`intentional_double_pledge`)**: User explicitly marks that a collateral is knowingly used across multiple deals
- **Detected Double Pledge (`has_potential_duplicates` + `double_pledged`)**: System detects that different collateral records might represent the same asset

These are independent flags that can coexist:
- Intentional = User action
- Detected = Automated detection service (arda-collateral-intel)

## Database Schema

### New Columns in `collateral` table

```sql
ALTER TABLE collateral
    ADD COLUMN intentional_double_pledge BOOLEAN DEFAULT FALSE,
    ADD COLUMN double_pledge_notes TEXT;
```

**Fields:**
- `intentional_double_pledge`: Boolean flag indicating user intent
- `double_pledge_notes`: Optional text explaining why collateral is intentionally double-pledged

### Existing Structure

The `deals` table already has a `collateral_id` foreign key that references `collateral(id)`. This is a one-to-many relationship:
- One collateral can be referenced by multiple deals
- Each deal references at most one collateral

## API Endpoints

### 1. Get Deals by Collateral

Get all deals that reference a specific collateral.

```bash
GET /api/v1/collateral/{id}/deals
```

**Response:**
```json
["deal-uuid-1", "deal-uuid-2", "deal-uuid-3"]
```

### 2. Get Collateral with Deal Count

Get collateral details along with the count and list of associated deals.

```bash
GET /api/v1/collateral/{id}/with-deals
```

**Response:**
```json
{
  "id": "collateral-uuid",
  "name": "123 Main Street Property",
  "collateral_type": "real_estate",
  "intentional_double_pledge": true,
  "double_pledge_notes": "Used for multiple tranches",
  "deal_count": 3,
  "deal_ids": ["deal-uuid-1", "deal-uuid-2", "deal-uuid-3"],
  ...
}
```

### 3. Update Collateral (including intentional double pledge flags)

Use the standard collateral update endpoint to set intentional double pledge flags.

```bash
PUT /api/v1/collateral/{id}
Content-Type: application/json

{
  "intentional_double_pledge": true,
  "double_pledge_notes": "This property is used for multiple tranches of the same loan"
}
```

**Response:**
```json
{
  "id": "collateral-uuid",
  "name": "123 Main Street Property",
  "intentional_double_pledge": true,
  "double_pledge_notes": "This property is used for multiple tranches of the same loan",
  ...
}
```

**Note:** The dedicated `POST /api/v1/collateral/{id}/mark-intentional-double-pledge` endpoint has been removed. Use the standard `PUT /api/v1/collateral/{id}` endpoint instead.

## Workflow Examples

### Scenario 1: Creating Intentional Self Double-Pledge

```bash
# 1. Create collateral
curl -X POST http://0.0.0.0:8080/api/v1/collateral \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "123 Main Street Property",
    "collateral_type": "real_estate",
    "custom_id": "PROP-2025-001"
  }'

# Response: {"id": "collateral-uuid", ...}

# 2. Create first deal with this collateral
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "originator_id": "company-uuid-1",
    "investor_id": "company-uuid-2",
    "principal_amount": "100000.00",
    "interest_rate_bps": 500,
    "collateral_id": "collateral-uuid"
  }'

# 3. Create second deal with same collateral
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "originator_id": "company-uuid-1",
    "investor_id": "company-uuid-3",
    "principal_amount": "50000.00",
    "interest_rate_bps": 450,
    "collateral_id": "collateral-uuid"
  }'

# 4. Mark as intentionally double-pledged
curl -X PUT http://0.0.0.0:8080/api/v1/collateral/collateral-uuid \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "intentional_double_pledge": true,
    "double_pledge_notes": "Multiple tranches of senior secured debt"
  }'
```

### Scenario 2: Monitoring Double-Pledge Status

```bash
# Get collateral details (includes deal_count and deal_ids when available)
curl "http://0.0.0.0:8080/api/v1/collateral/collateral-uuid" \
  -H "Authorization: Bearer $TOKEN"

# Returns: ["deal-uuid-1", "deal-uuid-2", "deal-uuid-3"]
```

## Database Functions

### Core Functions

```rust
// Get all deal IDs that reference a collateral
pub async fn get_deals_by_collateral_id(
    pool: &PgPool, 
    collateral_id: Uuid
) -> Result<Vec<Uuid>, DbError>

// Get collateral with deal count and IDs
pub async fn get_collateral_with_deal_count(
    pool: &PgPool,
    collateral_id: Uuid,
) -> Result<CollateralWithDealCount, DbError>

// List collateral with 2+ deals
pub async fn list_multi_deal_collateral(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<CollateralWithDealCount>, DbError>

// Update collateral (including intentional_double_pledge flags)
pub async fn update_collateral(
    pool: &PgPool,
    collateral_id: Uuid,
    request: UpdateCollateralRequest,
) -> Result<Collateral, DbError>
```

**Note:** The dedicated `mark_intentional_double_pledge` function has been removed. Use the standard `update_collateral` function to update `intentional_double_pledge` and `double_pledge_notes` fields.

## Frontend Integration

### Displaying Self Double-Pledge Status

```typescript
interface CollateralResponse {
  id: string;
  name: string;
  intentional_double_pledge?: boolean;
  double_pledge_notes?: string;
  deal_count?: number;
  deal_ids?: string[];
  // ... other fields
}

// Example: Show warning for multi-deal collateral
if (collateral.deal_count && collateral.deal_count > 1) {
  if (collateral.intentional_double_pledge) {
    showInfo(`This collateral is intentionally used across ${collateral.deal_count} deals`);
    if (collateral.double_pledge_notes) {
      showInfo(`Reason: ${collateral.double_pledge_notes}`);
    }
  } else {
    showWarning(`WARNING: This collateral is used in ${collateral.deal_count} deals but not marked as intentional`);
  }
}
```

### Marking as Intentional

```typescript
async function markIntentionalDoublePledge(
  collateralId: string,
  intentional: boolean,
  notes?: string
) {
  const response = await fetch(
    `/api/v1/collateral/${collateralId}`,
    {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify({ 
        intentional_double_pledge: intentional, 
        double_pledge_notes: notes 
      }),
    }
  );
  return response.json();
}
```

## Testing

Run the integration tests:

```bash
# Run all self double-pledge tests
cargo test --test self_double_pledge_test -- --ignored

# Run specific test
cargo test --test self_double_pledge_test test_multiple_deals_share_same_collateral -- --ignored
```

## Migration

The migration `062_add_intentional_double_pledge.sql` adds:
1. `intentional_double_pledge` column (BOOLEAN, default FALSE)
2. `double_pledge_notes` column (TEXT, nullable)
3. Index on `intentional_double_pledge` for efficient querying
4. View `collateral_with_deal_count` for easy querying

To apply:

```bash
make initDB  # Runs all migrations including 062
```

## Security Considerations

1. **Authorization**: Users should only be able to mark their own company's collateral as intentionally double-pledged
2. **Audit Trail**: Consider logging when collateral is marked as intentionally double-pledged
3. **Validation**: Ensure deals referencing the collateral actually exist before marking
4. **Notifications**: Consider alerting relevant parties when collateral is double-pledged

## Performance

- Indexes on `collateral_id` in `deals` table ensure efficient lookups
- `collateral_with_deal_count` view provides pre-computed counts
- Use pagination when listing multi-deal collateral

## Future Enhancements

1. **Collateral Allocation**: Track how much of the collateral value is allocated to each deal
2. **Priority/Lien Position**: Track which deal has first priority on the collateral
3. **Alerts**: Automatic notifications when collateral is used in multiple deals
4. **Reporting**: Dashboard showing all multi-deal collateral and utilization
5. **Historical Tracking**: Track changes to intentional_double_pledge flag over time

## Related Features

- **Duplicate Detection** (`has_potential_duplicates`, `double_pledged`): Automated detection of similar collateral records
- **Collateral Intel Service**: External service for advanced duplicate detection
- **Deal Marketplace**: Collateral status affects deal saleability
