# AI Agent Prompt: Fix SurrealDB Serialization Error in Arda-Collateral-Intel

## Context

The arda-collateral-intel service is experiencing a serialization error when attempting to store detection matches in its SurrealDB database. The error occurs during the analysis worker's attempt to persist match data after successfully detecting duplicate collateral.

## Error Details

```
ERROR arda_collateral_intel::workers::analysis_worker: Failed to analyze new collateral a66b6d08-3e89-4bd7-a16b-85fb3cee36c7: Database error: Failed to create detection match: Serialization error: invalid type: enum, expected any valid JSON value
```

### Error Location
- **Component**: `arda_collateral_intel::workers::analysis_worker`
- **Operation**: Creating detection matches in SurrealDB
- **Stage**: After successful exact match detection (Stage 1 completes successfully)
- **Point of Failure**: When calling `store_detection_for_collateral` → `store_detection_matches`

### Observable Symptoms

From the logs, we can see:
1. ✅ Detection analysis completes successfully with 5 exact matches found
2. ✅ Results are successfully published back to arda-credit API (HTTP 200)
3. ✅ Detection records are created in SurrealDB (detection table)
4. ❌ Serialization fails when attempting to store individual matches in the database
5. ⚠️ Detection history endpoint returns 503 error with count=2 but can't retrieve records

## Root Cause Analysis

The error message "invalid type: enum, expected any valid JSON value" indicates that a Rust enum is being serialized without proper JSON conversion. Based on the SurrealDB integration pattern, the most likely culprits are:

### Candidate Enums

Look for enums in these areas:

1. **DetectionStatus** - Used when storing detections
2. **MatchType** - Describes the type of match (exact_name, custom_id, address, fuzzy_name)
3. **ConfidenceLevel** - Match confidence (high, medium, low)
4. **CollateralLifecycleStatus** - Lifecycle state of collateral
5. **Any other enum fields** in detection match structures

### Expected Pattern

SurrealDB expects string values, but Rust enums by default serialize as structured data unless properly annotated.

## Files to Investigate

Based on the error trace and typical patterns:

1. **`src/services/knowledge_graph.rs`** (or similar)
   - Look for the `store_detection_matches` function
   - Check `store_detection_for_collateral` function
   - Review how enums are being serialized before database insertion

2. **`src/models/detection.rs`** (or similar model file)
   - Find enum definitions for DetectionStatus, MatchType, ConfidenceLevel
   - Check Serialize/Deserialize implementations

3. **`src/database/*.rs`** or **`src/services/*.rs`**
   - Look for SurrealDB query construction
   - Check struct definitions used in database operations

4. **`src/workers/analysis_worker.rs`**
   - Review the error handling path that logs the failure
   - Trace back to the exact database call that's failing

## Fix Strategy

### Option 1: Add Serde String Serialization (Recommended)

For each problematic enum, add serde annotations to serialize as strings:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")] // or "lowercase", "PascalCase", etc.
pub enum DetectionStatus {
    NoDuplicates,
    LowRisk,
    MediumRisk,
    HighRisk,
}

// If the enum needs to be stored as a simple string:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    #[serde(rename = "exact_name")]
    ExactName,
    #[serde(rename = "custom_id")]
    CustomId,
    #[serde(rename = "address")]
    Address,
    #[serde(rename = "fuzzy_name")]
    FuzzyName,
}
```

### Option 2: Convert Enums to Strings Before Serialization

If you can't modify the enum definitions, convert them to strings before database operations:

```rust
// Before insertion
let match_type_str = match match_type {
    MatchType::ExactName => "exact_name",
    MatchType::CustomId => "custom_id",
    MatchType::Address => "address",
    MatchType::FuzzyName => "fuzzy_name",
}.to_string();

// Store the string instead of the enum
```

### Option 3: Use serde_json for Manual Serialization

If the enum needs complex serialization logic:

```rust
impl DetectionStatus {
    pub fn to_surreal_value(&self) -> String {
        match self {
            DetectionStatus::NoDuplicates => "no_duplicates".to_string(),
            DetectionStatus::LowRisk => "low_risk".to_string(),
            DetectionStatus::MediumRisk => "medium_risk".to_string(),
            DetectionStatus::HighRisk => "high_risk".to_string(),
        }
    }
}
```

## Implementation Steps

### Step 1: Locate the Failing Query

Search for the database operation that's failing:

```bash
# Find the detection match storage code
grep -r "create detection match" src/
grep -r "store.*detection.*match" src/
grep -r "INSERT.*detection_match" src/
```

### Step 2: Identify Enum Fields

Look at the struct being inserted:

```rust
// Example - find the actual struct in your code
struct DetectionMatch {
    detection_id: Thing,
    matched_collateral_id: Thing,
    match_type: MatchType,  // <-- Likely culprit
    confidence: ConfidenceLevel,  // <-- Another candidate
    // ... other fields
}
```

### Step 3: Fix Enum Serialization

Add proper serde annotations or convert to strings:

```rust
// Option A: Fix the enum definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    ExactName,
    CustomId,
    Address,
    FuzzyName,
}

// Option B: Convert before database call
let match_data = DetectionMatchData {
    detection_id: detection_id.clone(),
    matched_collateral_id: matched_id.clone(),
    match_type: match_obj.match_type.to_string(), // Convert enum to string
    confidence: match_obj.confidence.to_string(), // Convert enum to string
    // ... other fields
};
```

### Step 4: Test the Fix

1. Run the arda-collateral-intel service
2. Trigger a detection by creating deals with the same collateral
3. Check the logs for successful match storage:
   ```
   ✅ Successfully stored detection with N matches
   ```
4. Query the detection history endpoint and verify it returns results (not 503)

### Step 5: Verify Database State

Check SurrealDB directly:

```surreal
-- Query to verify matches are stored
SELECT * FROM detection_match LIMIT 10;

-- Check that enum values are strings
SELECT match_type, confidence FROM detection_match;
```

## Testing Checklist

- [ ] Service starts without errors
- [ ] Detection analysis completes (exact match stage)
- [ ] Detection record is created in database
- [ ] Detection matches are stored without serialization errors
- [ ] Detection history endpoint returns 200 OK (not 503)
- [ ] Database contains match records with proper string values

## Additional Resources

### SurrealDB Serde Patterns

SurrealDB expects these data types:
- Strings: Use `String` or annotated enums
- Numbers: Use `i64`, `f64`, etc.
- Booleans: Use `bool`
- Arrays: Use `Vec<T>`
- Objects: Use structs with `#[derive(Serialize, Deserialize)]`

### Debugging Tips

1. Add temporary debug logging before the database call:
   ```rust
   let json_value = serde_json::to_value(&detection_match)?;
   tracing::debug!("Serialized match data: {}", json_value);
   ```

2. This will show you exactly what's being sent to SurrealDB and help identify the problematic field.

## Success Criteria

When complete, the logs should show:

```
✅ Stage 1: Found 5 exact match(es)
✅ Successfully stored detection with 5 matches
✅ Detection history: [list of detections with matches]
```

And the test script should show:
```
✅ Database matches: Found 5 match(es) (expected at least 2)
```

## Questions to Ask Yourself

1. What enums are in the detection match data structure?
2. Do they have `#[serde(rename_all = "...")]` or `#[serde(rename = "...")]` annotations?
3. Are they being converted to strings before database insertion?
4. Can I see the serialized JSON value before it's sent to SurrealDB?
5. Does the SurrealDB schema expect string fields for these values?

## Expected Result

After implementing the fix:
- Detection matches will be stored successfully in SurrealDB
- The detection history endpoint will return properly formatted results
- The arda-credit test script will show matches in the database
- No serialization errors in the logs

