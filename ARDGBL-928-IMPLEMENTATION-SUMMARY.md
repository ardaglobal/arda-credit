# ARDGBL-928: Self Double Pledge Implementation Summary

## Issue Description

Allow for self double pledging - the ability to handle scenarios where the same exact collateral is referenced in 2 or more deals. This requires support for intentional self-assignment of double pledge status.

## Solution Overview

Implemented a comprehensive self double-pledge system that:
1. Allows multiple deals to reference the same collateral (already supported by schema)
2. Adds ability to mark collateral as intentionally double-pledged
3. Provides APIs to query and manage double-pledged collateral
4. Distinguishes between intentional double-pledge (user action) and detected duplicates (automated)

## Changes Made

### 1. Database Migration

**File**: `db/migrations/062_add_intentional_double_pledge.sql`

Added two new columns to the `collateral` table:
- `intentional_double_pledge` (BOOLEAN, default FALSE)
- `double_pledge_notes` (TEXT, nullable for explanatory notes)

Created view `collateral_with_deal_count` for efficient querying of collateral with deal counts.

### 2. Database Layer (`db/src/taxonomy/collateral.rs`)

**Updated Structures:**
- `Collateral` struct: Added `intentional_double_pledge` and `double_pledge_notes` fields
- `UpdateCollateralRequest`: Added support for updating double-pledge fields
- All SQL queries updated to include new fields

**New Functions:**
```rust
// Get all deal IDs that reference a collateral
pub async fn get_deals_by_collateral_id(pool: &PgPool, collateral_id: Uuid) -> Result<Vec<Uuid>, DbError>

// Mark collateral as intentionally double-pledged
pub async fn mark_intentional_double_pledge(
    pool: &PgPool,
    collateral_id: Uuid,
    intentional: bool,
    notes: Option<String>,
) -> Result<Collateral, DbError>

// Get collateral with deal count and deal IDs
pub async fn get_collateral_with_deal_count(
    pool: &PgPool,
    collateral_id: Uuid,
) -> Result<CollateralWithDealCount, DbError>

// List collateral with 2+ deals (potential self double-pledge)
pub async fn list_multi_deal_collateral(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<CollateralWithDealCount>, DbError>
```

**Exports** (`db/src/lib.rs`):
Updated to export all new functions and types.

### 3. API Layer (`api/src/handlers/taxonomy/collateral.rs`)

**Updated Response Type:**
- `CollateralResponse`: Added `intentional_double_pledge`, `double_pledge_notes`, `deal_count`, and `deal_ids` fields
- `UpdateCollateralApiRequest`: Added double-pledge field support

**New Request Type:**
- `MarkIntentionalDoublePledgeRequest`: For marking intentional double-pledge

**New API Endpoints:**

1. **GET `/api/v1/collateral/{id}/deals`**
   - Get all deal IDs that share a collateral
   - Returns: `Vec<String>` (list of deal UUIDs)

2. **GET `/api/v1/collateral/{id}/with-deals`**
   - Get collateral with deal count and IDs
   - Returns: `CollateralResponse` with `deal_count` and `deal_ids`

3. **POST `/api/v1/collateral/{id}/mark-intentional-double-pledge`**
   - Mark collateral as intentionally double-pledged
   - Request body: `{ "intentional": bool, "notes": string? }`
   - Returns: `CollateralResponse`

4. **GET `/api/v1/collateral/multi-deal`**
   - List all collateral with 2+ deals
   - Query params: `limit`, `offset`
   - Returns: `Vec<CollateralResponse>` with deal counts

### 4. Path Configuration (`api/src/config/paths.rs`)

Added new path constants:
- `COLLATERAL_DEALS`
- `COLLATERAL_WITH_DEALS`
- `COLLATERAL_MARK_INTENTIONAL_DOUBLE_PLEDGE`
- `COLLATERAL_MULTI_DEAL`

### 5. Router Configuration (`api/src/api/router.rs`)

Registered all new endpoints with appropriate HTTP methods.

### 6. OpenAPI Documentation (`api/src/api/openapi.rs`)

- Added new handlers to OpenAPI paths
- Added `MarkIntentionalDoublePledgeRequest` to schemas
- All endpoints properly documented with Swagger/OpenAPI annotations

### 7. Tests (`api/tests/self_double_pledge_test.rs`)

Comprehensive test suite including:
- Test multiple deals sharing same collateral
- Test marking as intentional double-pledge
- Test listing multi-deal collateral
- Test intentional vs detected double-pledge distinction
- API-level test stubs for integration testing

### 8. Documentation (`docs/self-double-pledge.md`)

Comprehensive documentation covering:
- Use cases and concepts
- Database schema changes
- API endpoint details with examples
- Workflow examples
- Frontend integration guidelines
- Security considerations
- Performance notes
- Future enhancements

## Key Design Decisions

### 1. Intentional vs Detected Double-Pledge

Two independent systems:
- **Intentional** (`intentional_double_pledge`): User explicitly marks collateral as shared
- **Detected** (`has_potential_duplicates`, `double_pledged`): Automated detection of duplicate collateral records

This separation allows:
- Clear distinction between legitimate sharing and potential fraud
- Both flags can coexist (e.g., same collateral intentionally shared AND detected as duplicate)
- Different workflows for each scenario

### 2. Database Design

The existing schema already supported multiple deals referencing the same collateral through the `collateral_id` foreign key in the `deals` table. We simply added:
- User-controlled flags to mark intent
- Helper functions to query this relationship
- API endpoints to expose the functionality

### 3. API Design

RESTful endpoints following existing patterns:
- Use of path parameters for resource IDs
- Consistent authentication requirements
- Paginated list endpoints
- Clear request/response types

## Usage Examples

### Creating Self Double-Pledge

```bash
# 1. Create collateral
POST /api/v1/collateral
{
  "name": "123 Main Street Property",
  "collateral_type": "real_estate"
}

# 2. Create deals with same collateral_id
POST /api/v1/deal
{
  "collateral_id": "collateral-uuid",
  "principal_amount": "100000.00",
  ...
}

# 3. Mark as intentional
POST /api/v1/collateral/{id}/mark-intentional-double-pledge
{
  "intentional": true,
  "notes": "Multiple tranches of senior secured debt"
}
```

### Querying Multi-Deal Collateral

```bash
# List all collateral with multiple deals
GET /api/v1/collateral/multi-deal?limit=20

# Get specific collateral with deal details
GET /api/v1/collateral/{id}/with-deals
```

## Testing

Run tests with:
```bash
cargo test --test self_double_pledge_test -- --ignored
```

Note: Tests require database connection and are marked as `#[ignore]` by default.

## Migration

To apply the database changes:
```bash
make initDB
```

This will:
1. Apply migration `062_add_intentional_double_pledge.sql`
2. Regenerate SQLx offline query cache
3. Update database with new columns and view

## Verification Steps

1. ✅ Database migration created
2. ✅ Database functions implemented and exported
3. ✅ API endpoints created and registered
4. ✅ Request/response types updated
5. ✅ OpenAPI documentation updated
6. ✅ Path constants defined
7. ✅ Routes registered
8. ✅ Tests created
9. ✅ Documentation written

## Future Improvements

1. **Collateral Allocation**: Track percentage of collateral value allocated to each deal
2. **Lien Priority**: Track which deal has first lien position
3. **Automatic Alerts**: Notify when collateral is used in multiple deals without being marked intentional
4. **Historical Tracking**: Audit trail for changes to intentional_double_pledge flag
5. **Dashboard**: Visual representation of all multi-deal collateral
6. **Validation**: Ensure total allocated value doesn't exceed collateral value

## Security Considerations

1. Authorization should be added to ensure users can only mark their own company's collateral
2. Consider audit logging for double-pledge marking actions
3. Add validation to prevent marking as intentional without associated deals
4. Consider workflow approvals for marking as intentional in production

## Impact on Existing Features

- **Backward Compatible**: All changes are additive
- **Existing Collateral**: Will have `intentional_double_pledge = FALSE` by default
- **Existing Deals**: Continue to work as before
- **API Changes**: Only adds new endpoints, existing endpoints gain optional fields
- **Database**: Migration adds columns with defaults, no data loss

## Resolves

Linear Issue: ARDGBL-928 - "Allow for self double pledging"

The system now fully supports:
- Multiple deals referencing the same collateral
- User-controlled marking of intentional double-pledge
- Querying and listing of multi-deal collateral
- Clear distinction between intentional and detected double-pledge
