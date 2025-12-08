# Quick Start: Self Double Pledge Feature

## What is Self Double Pledge?

Self double pledge allows the same collateral to be intentionally used across multiple deals. For example:
- Multiple tranches of the same loan
- Senior and junior liens on the same property
- Partial pledges of the same asset to different deals

## Quick Setup

### 1. Run the Migration

```bash
make initDB  # Applies migration 062_add_intentional_double_pledge.sql
```

### 2. Test the Endpoints

All new endpoints are under `/api/v1/collateral/`:

```bash
# Get all deals using a collateral
GET /api/v1/collateral/{id}/deals

# Get collateral with deal count
GET /api/v1/collateral/{id}/with-deals

# Mark as intentionally double-pledged
POST /api/v1/collateral/{id}/mark-intentional-double-pledge
{
  "intentional": true,
  "notes": "Multiple tranches"
}

# List all multi-deal collateral
GET /api/v1/collateral/multi-deal?limit=20
```

## Example Workflow

```bash
# 1. Create collateral
curl -X POST http://0.0.0.0:8080/api/v1/collateral \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name": "Property A", "collateral_type": "real_estate"}'
# Returns: {"id": "col-123", ...}

# 2. Create multiple deals with same collateral
curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "originator_id": "company-1",
    "investor_id": "company-2",
    "principal_amount": "100000",
    "collateral_id": "col-123"
  }'

curl -X POST http://0.0.0.0:8080/api/v1/deal \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "originator_id": "company-1",
    "investor_id": "company-3",
    "principal_amount": "50000",
    "collateral_id": "col-123"
  }'

# 3. Mark as intentional
curl -X POST http://0.0.0.0:8080/api/v1/collateral/col-123/mark-intentional-double-pledge \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "intentional": true,
    "notes": "Senior and junior tranches"
  }'

# 4. Query multi-deal collateral
curl http://0.0.0.0:8080/api/v1/collateral/multi-deal \
  -H "Authorization: Bearer $TOKEN"
```

## Database Functions

```rust
// Get deals for a collateral
let deal_ids = get_deals_by_collateral_id(&pool, collateral_id).await?;

// Mark as intentional
let updated = mark_intentional_double_pledge(&pool, id, true, Some("notes".into())).await?;

// Get with count
let with_count = get_collateral_with_deal_count(&pool, id).await?;

// List multi-deal
let list = list_multi_deal_collateral(&pool, 20, 0).await?;
```

## Response Fields

The `CollateralResponse` now includes:

```json
{
  "id": "col-123",
  "name": "Property A",
  "intentional_double_pledge": true,
  "double_pledge_notes": "Multiple tranches",
  "deal_count": 2,
  "deal_ids": ["deal-1", "deal-2"],
  ...
}
```

## Files Changed

1. **Migration**: `db/migrations/062_add_intentional_double_pledge.sql`
2. **Database**: `db/src/taxonomy/collateral.rs`
3. **API Handlers**: `api/src/handlers/taxonomy/collateral.rs`
4. **Routes**: `api/src/api/router.rs`
5. **Paths**: `api/src/config/paths.rs`
6. **Tests**: `api/tests/self_double_pledge_test.rs`

## Documentation

Full documentation: `docs/self-double-pledge.md`

## Key Points

- ✅ Multiple deals can share the same collateral
- ✅ Users can mark this as intentional
- ✅ System tracks deal count per collateral
- ✅ Separate from duplicate detection (detected vs intentional)
- ✅ Backward compatible (all changes are additive)

## Swagger UI

View all new endpoints at:
```
http://0.0.0.0:8080/swagger-ui/
```

Look for the "Collateral" section.
