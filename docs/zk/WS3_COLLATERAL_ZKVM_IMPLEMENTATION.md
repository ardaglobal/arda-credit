# Work Stream 3: Collateral zkVM Implementation

**Epic:** EPIC-3  
**Priority:** HIGH  
**Effort:** 5.5 days (44 hours)  
**Owner:** zkVM/Cryptography Engineer  
**Dependencies:** EPIC-1 complete, can run parallel with EPIC-2  

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Requirements](#technical-requirements)
3. [zkVM Constraints & Design Principles](#zkvm-constraints--design-principles)
4. [Sub-Issue Implementation Guides](#sub-issue-implementation-guides)
5. [Double-Pledge Prevention Algorithm](#double-pledge-prevention-algorithm)
6. [Testing Strategy](#testing-strategy)
7. [Performance Considerations](#performance-considerations)
8. [Validation Checklist](#validation-checklist)

---

## Overview

### Purpose

Implement collateral state management and processing logic within the zkVM to enable **cryptographic proof** of collateral operations. This is critical for proving that collateral is not double-pledged, providing regulatory compliance, and maintaining a verifiable audit trail.

### Why This Matters

**Collateral Double-Pledging** is a serious regulatory concern. By proving collateral state transitions in the zkVM, we can provide **cryptographic evidence** that:
- A piece of collateral is only associated with one active deal at a time
- Collateral state transitions follow business rules
- The history of collateral usage is tamper-proof
- Regulators can verify claims without accessing private data

### Context

**Current State:**
- Collateral operations are NO-OPs in zkVM (lines 272-280 of `lib/src/deal_processor.rs`)
- Collateral tracked in database but not proven
- Double-pledge detection happens in collateral-intel service (database check)

```rust
// Current implementation (TO BE REPLACED)
TransactionType::CollateralCreation | ... => {
    // No-op: Collateral transactions are recorded for audit trail
    // but don't affect zkVM state
}
```

**Target State:**
- Collateral is full first-class citizen in zkVM state
- `GlobalLoanState` contains collateral HashMap
- All 6 collateral transaction types processed
- Double-pledge prevention proven cryptographically
- Collateral state roots included in proofs

### Scope (6 Transaction Types)

1. **CollateralCreation (#20)** - Add collateral to zkVM state
2. **CollateralUpdate (#21)** - Modify collateral properties
3. **CollateralDealAssociation (#22)** - Link collateral to deal, **prove no double-pledge**
4. **CollateralDealDisassociation (#23)** - Unlink collateral from deal
5. **CollateralDetectionRequest (#24)** - Record detection request
6. **CollateralDetectionUpdate (#25)** - Record detection results

**Out of Scope:** API integration (that's WS-4)

---

## Technical Requirements

### Functional Requirements

**FR-1: Collateral State Management**
- GlobalLoanState must contain collateral HashMap
- Each collateral identified by hashed ID (32 bytes)
- State persists across transaction processing
- State serializable for testing

**FR-2: Association Tracking**
- Track which collateral is associated with which deal(s)
- Support 1:N relationship (one collateral can be associated with multiple deals over time)
- Support checking current active associations (for double-pledge prevention)

**FR-3: Double-Pledge Prevention**
- CRITICAL: Must prove collateral not associated with another active deal
- Check happens in zkVM, not database
- Error if collateral already pledged to active deal
- Historical associations allowed (collateral reused after deal closes)

**FR-4: Collateral Properties**
- Store collateral type, name (in metadata for privacy)
- Track creation and update timestamps
- Support arbitrary metadata

**FR-5: Detection Event Tracking**
- Record when detection requested
- Record detection results
- Link to collateral and deal

### Non-Functional Requirements

**NFR-1: zkVM Compatibility**
- No database access
- No file I/O
- No network calls
- Deterministic execution
- Must compile for SP1 zkVM target

**NFR-2: Performance**
- Collateral operations should not significantly increase proof generation time
- Target: <10% increase in proof time
- Efficient HashMap operations

**NFR-3: Privacy**
- Collateral details (name, address) in metadata, not public values
- Only collateral count or commitment in public values (if needed)
- Individual collateral associations remain private

**NFR-4: Correctness**
- State transitions must be provably correct
- Double-pledge detection must be cryptographically sound
- No edge cases that allow invalid states

---

## zkVM Constraints & Design Principles

### What You CANNOT Do in lib/

❌ **Database Access**
```rust
// WRONG - This will not compile in zkVM
let collateral = sqlx::query!("SELECT * FROM collateral WHERE id = $1", id)
    .fetch_one(&pool)
    .await?;
```

❌ **File I/O**
```rust
// WRONG - No file system in zkVM
let data = std::fs::read_to_string("collateral.json")?;
```

❌ **Network Calls**
```rust
// WRONG - No network in zkVM
let response = reqwest::get("https://api.example.com").await?;
```

❌ **System Time**
```rust
// WRONG - Non-deterministic
let now = SystemTime::now();
```

❌ **Random Numbers**
```rust
// WRONG - Non-deterministic
let random_value = rand::random::<u64>();
```

### What You MUST Do

✅ **Pure Functions**
```rust
// CORRECT - Pure state transition
pub fn process_collateral_creation(
    &mut self,
    transaction: &LoanTransaction,
) -> Result<(), CollateralError> {
    // All data comes from transaction (input)
    // All state is in-memory HashMap
    // Deterministic execution
}
```

✅ **In-Memory State**
```rust
// CORRECT - HashMap is zkVM-compatible
pub struct CollateralState {
    pub collateral: HashMap<[u8; 32], Collateral>,
    pub associations: HashMap<[u8; 32], Vec<[u8; 32]>>,
}
```

✅ **Timestamps from Inputs**
```rust
// CORRECT - Timestamp comes from transaction
let created_at = transaction.created_at;  // Not SystemTime::now()
```

✅ **Deterministic Logic**
```rust
// CORRECT - Same inputs always produce same outputs
if self.collateral.contains_key(&collateral_id) {
    return Err(CollateralError::AlreadyExists);
}
self.collateral.insert(collateral_id, collateral);
```

### Design Principles

1. **State is Truth** - The HashMap contains all collateral state
2. **Transactions are Facts** - Each transaction is an immutable fact about what happened
3. **Processing is Proof** - Successfully processing proves the transaction was valid
4. **Errors are Proofs Too** - If we error, we prove the transaction was invalid

---

## Sub-Issue Implementation Guides

## EPIC-3.1: Create Collateral Module Structure

**Effort:** 4 hours  
**Dependencies:** None - can start after EPIC-1  
**Priority:** CRITICAL (blocks all other collateral work)  

### Implementation Steps

#### Step 1: Create Module File

**File:** `lib/src/collateral.rs`

**Initial structure:**

```rust
//! Collateral State Management for zkVM
//!
//! This module provides collateral state management that runs inside the SP1 zkVM.
//! It handles collateral creation, updates, association with deals, and critical
//! double-pledge prevention.
//!
//! ## Double-Pledge Prevention
//!
//! The key regulatory requirement is proving that collateral is not pledged to
//! multiple active deals simultaneously. This module provides cryptographic proof
//! by tracking associations in zkVM state.
//!
//! ## Privacy
//!
//! Collateral details (names, addresses, values) are stored in transaction metadata
//! and remain private. Only collateral IDs and association counts are in state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::transactions::LoanTransaction;

/// Errors that can occur during collateral processing
#[derive(Error, Debug, Clone)]
pub enum CollateralError {
    #[error("Collateral already exists: {collateral_id:?}")]
    CollateralAlreadyExists { collateral_id: [u8; 32] },
    
    #[error("Collateral not found: {collateral_id:?}")]
    CollateralNotFound { collateral_id: [u8; 32] },
    
    #[error("Collateral already pledged to deal: {collateral_id:?} → {existing_deal_id:?}")]
    CollateralAlreadyPledged {
        collateral_id: [u8; 32],
        existing_deal_id: [u8; 32],
    },
    
    #[error("Collateral not associated with deal: {collateral_id:?} + {deal_id:?}")]
    CollateralNotAssociated {
        collateral_id: [u8; 32],
        deal_id: [u8; 32],
    },
    
    #[error("Deal not found: {deal_id:?}")]
    DealNotFound { deal_id: [u8; 32] },
    
    #[error("Invalid collateral metadata")]
    InvalidMetadata,
    
    #[error("Invalid transaction data")]
    InvalidTransactionData,
}

/// Collateral asset in zkVM state
///
/// Privacy note: Sensitive details (name, address, value) are NOT stored here.
/// They remain in transaction metadata. Only the collateral_id and timestamps
/// are in state, which is sufficient for double-pledge prevention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collateral {
    /// Hashed collateral ID (32 bytes)
    pub collateral_id: [u8; 32],
    
    /// When collateral was created (Unix timestamp)
    pub created_at: u64,
    
    /// When collateral was last updated (Unix timestamp)
    pub updated_at: u64,
    
    /// Collateral type (stored for categorization, details in metadata)
    pub collateral_type: String,
}

/// Association between collateral and deal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralAssociation {
    /// Hashed collateral ID
    pub collateral_id: [u8; 32],
    
    /// Hashed deal ID
    pub deal_id: [u8; 32],
    
    /// When association was created
    pub associated_at: u64,
    
    /// When association was removed (None if still active)
    pub disassociated_at: Option<u64>,
    
    /// Whether this association is currently active
    pub is_active: bool,
}

/// Collateral state management for zkVM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralState {
    /// All collateral in the system (collateral_id → Collateral)
    pub collateral: HashMap<[u8; 32], Collateral>,
    
    /// Active associations (collateral_id → Vec<deal_id>)
    /// Only includes currently active associations
    pub active_associations: HashMap<[u8; 32], Vec<[u8; 32]>>,
    
    /// Complete association history (for audit trail)
    /// Key: (collateral_id, deal_id) as concatenated bytes
    pub association_history: HashMap<[u8; 64], CollateralAssociation>,
    
    /// Detection requests (collateral_id → request_timestamp)
    pub detection_requests: HashMap<[u8; 32], u64>,
}

impl CollateralState {
    /// Create new empty collateral state
    #[must_use]
    pub fn new() -> Self {
        Self {
            collateral: HashMap::new(),
            active_associations: HashMap::new(),
            association_history: HashMap::new(),
            detection_requests: HashMap::new(),
        }
    }
    
    // Methods will be added in subsequent steps
}

impl Default for CollateralState {
    fn default() -> Self {
        Self::new()
    }
}
```

**Validation:**
- Compiles: `cargo check --package arda-credit-lib`
- No database dependencies
- Serde-compatible for serialization
- Error types cover all cases

**Effort:** 2 hours

#### Step 2: Add to lib.rs Module Exports

**File:** `lib/src/lib.rs`

**Add module declaration:**

```rust
// After line 35 (other modules)
pub mod balance;
pub mod deal;
pub mod deal_processor;
pub mod transactions;
pub mod user;
pub mod utils;
pub mod zkvm_ads;
pub mod collateral;  // NEW

#[cfg(test)]
mod zkvm_ads_consistency_tests;
```

**Add public exports:**

```rust
// After line 73 (other exports)
pub use user::{KycStatus, User, UserError, UserState, UserType};
pub use utils::{cents_to_decimal, decimal_to_cents, hash_uuid};
pub use zkvm_ads::{StateTransition, ZkVMAds, ZkVMNullifier};
// NEW: Export collateral types
pub use collateral::{
    Collateral, CollateralAssociation, CollateralError, CollateralState,
};
```

**Validation:**
- Compiles: `cargo check --package arda-credit-lib`
- Can import from other modules: `use arda_credit_lib::Collateral;`

**Effort:** 30 minutes

#### Step 3: Add Basic Tests

**Add to `lib/src/collateral.rs`:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collateral_state_creation() {
        let state = CollateralState::new();
        
        assert_eq!(state.collateral.len(), 0);
        assert_eq!(state.active_associations.len(), 0);
        assert_eq!(state.association_history.len(), 0);
    }
    
    #[test]
    fn test_collateral_struct() {
        let collateral = Collateral {
            collateral_id: [1u8; 32],
            created_at: 1000,
            updated_at: 1000,
            collateral_type: "real_estate".to_string(),
        };
        
        assert_eq!(collateral.collateral_id, [1u8; 32]);
        assert_eq!(collateral.created_at, 1000);
    }
    
    #[test]
    fn test_association_struct() {
        let assoc = CollateralAssociation {
            collateral_id: [1u8; 32],
            deal_id: [2u8; 32],
            associated_at: 1000,
            disassociated_at: None,
            is_active: true,
        };
        
        assert!(assoc.is_active);
        assert!(assoc.disassociated_at.is_none());
    }
}
```

**Run tests:**
```bash
cargo test --package arda-credit-lib collateral
```

**Effort:** 1.5 hours total for EPIC-3.1

---

## EPIC-3.2: Update GlobalLoanState

**Effort:** 4 hours  
**Dependencies:** EPIC-3.1 complete  
**Priority:** CRITICAL (blocks all processor implementation)  

### Implementation Steps

#### Step 1: Update GlobalLoanState Struct

**File:** `lib/src/deal_processor.rs`

**Modify struct (around line 56):**

```rust
/// Global state for loan processing (maintains user, loan, and collateral state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,

    /// All loans in the system
    pub loans: HashMap<[u8; 32], Deal>,
    
    /// NEW: All collateral in the system
    pub collateral: HashMap<[u8; 32], Collateral>,
    
    /// NEW: Active collateral-deal associations
    pub active_associations: HashMap<[u8; 32], Vec<[u8; 32]>>,
    
    /// NEW: Complete association history
    pub association_history: HashMap<[u8; 64], CollateralAssociation>,

    /// User balance tracking state
    pub balance_state: BalanceState,

    /// Configurable loan terms
    pub loan_terms: LoanTerms,

    /// Transaction counter for ordering
    pub transaction_count: u64,

    /// Last update timestamp
    pub last_updated: u64,
}
```

#### Step 2: Update Default Implementation

```rust
impl Default for GlobalLoanState {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            collateral: HashMap::new(),  // NEW
            active_associations: HashMap::new(),  // NEW
            association_history: HashMap::new(),  // NEW
            balance_state: BalanceState::new(),
            loan_terms: LoanTerms::default(),
            transaction_count: 0,
            last_updated: 0,
        }
    }
}
```

#### Step 3: Update Helper Methods

**Add collateral-specific helper methods:**

```rust
impl GlobalLoanState {
    // ... existing methods ...
    
    /// Get collateral by ID (read-only)
    #[must_use]
    pub fn get_collateral(&self, collateral_id: &[u8; 32]) -> Option<&Collateral> {
        self.collateral.get(collateral_id)
    }
    
    /// Get active associations for a collateral
    #[must_use]
    pub fn get_active_associations(&self, collateral_id: &[u8; 32]) -> Vec<[u8; 32]> {
        self.active_associations
            .get(collateral_id)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Check if collateral is actively pledged to any deal
    #[must_use]
    pub fn is_collateral_pledged(&self, collateral_id: &[u8; 32]) -> bool {
        self.active_associations
            .get(collateral_id)
            .map_or(false, |deals| !deals.is_empty())
    }
    
    /// Get all collateral associated with a deal
    #[must_use]
    pub fn get_deal_collateral(&self, deal_id: &[u8; 32]) -> Vec<[u8; 32]> {
        self.active_associations
            .iter()
            .filter_map(|(coll_id, deal_ids)| {
                if deal_ids.contains(deal_id) {
                    Some(*coll_id)
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Compute total collateral count
    #[must_use]
    pub fn total_collateral_count(&self) -> usize {
        self.collateral.len()
    }
    
    /// Compute total active associations
    #[must_use]
    pub fn total_active_associations(&self) -> usize {
        self.active_associations
            .values()
            .map(|deals| deals.len())
            .sum()
    }
}
```

#### Step 4: Update Tests

**Add test for updated structure:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_global_state_with_collateral() {
        let state = GlobalLoanState::new();
        
        assert_eq!(state.collateral.len(), 0);
        assert_eq!(state.active_associations.len(), 0);
        assert_eq!(state.total_collateral_count(), 0);
    }
    
    #[test]
    fn test_collateral_helper_methods() {
        let mut state = GlobalLoanState::new();
        
        let collateral_id = [1u8; 32];
        let deal_id = [2u8; 32];
        
        // Add collateral
        state.collateral.insert(
            collateral_id,
            Collateral {
                collateral_id,
                created_at: 1000,
                updated_at: 1000,
                collateral_type: "real_estate".to_string(),
            }
        );
        
        // Check helpers
        assert!(state.get_collateral(&collateral_id).is_some());
        assert!(!state.is_collateral_pledged(&collateral_id));
        assert_eq!(state.total_collateral_count(), 1);
        
        // Add association
        state.active_associations.insert(collateral_id, vec![deal_id]);
        
        assert!(state.is_collateral_pledged(&collateral_id));
        assert_eq!(state.get_active_associations(&collateral_id), vec![deal_id]);
        assert_eq!(state.total_active_associations(), 1);
    }
}
```

**Validation:**
- Compiles: `cargo check --package arda-credit-lib`
- Tests pass: `cargo test --package arda-credit-lib global_state`
- Serialization works (for proof inputs)

**Effort:** 4 hours total for EPIC-3.2

---

## EPIC-3.3: Implement CollateralCreation Processor

**Effort:** 4 hours  
**Dependencies:** EPIC-3.2 complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Understand Metadata Format

**From API layer (WS-4), metadata will contain:**
```json
{
  "collateral_type": "real_estate",
  "name": "123 Main St Property",
  "address": "123 Main St, City, State",
  "custom_id": "COLL-2024-001",
  "estimated_value": "500000.00"
}
```

**For zkVM, we only need:**
- collateral_id (from transaction)
- collateral_type (for categorization)
- created_at (from transaction timestamp)

**Sensitive details stay in metadata (private).**

#### Step 2: Implement process_collateral_creation

**File:** `lib/src/collateral.rs`

**Add to CollateralState impl:**

```rust
impl CollateralState {
    /// Process collateral creation transaction
    ///
    /// Creates a new collateral in the zkVM state. Validates that the
    /// collateral does not already exist.
    ///
    /// # Arguments
    /// * `transaction` - The collateral creation transaction
    ///
    /// # Returns
    /// * `Ok(())` - Collateral created successfully
    /// * `Err(CollateralError)` - Collateral already exists or invalid data
    ///
    /// # Metadata Format
    /// The transaction metadata should contain:
    /// ```json
    /// {
    ///   "collateral_type": "real_estate" | "equipment" | "inventory" | ...,
    ///   "name": "Human readable name (private)",
    ///   "address": "Location (private)",
    /// }
    /// ```
    pub fn process_collateral_creation(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        // Extract collateral_id from transaction
        // For collateral creation, we use deal_id field to store collateral_id
        // (deal_id would be zero for collateral operations not yet associated)
        let collateral_id = transaction.deal_id;
        
        // Validation: Collateral must NOT already exist
        if self.collateral.contains_key(&collateral_id) {
            return Err(CollateralError::CollateralAlreadyExists { collateral_id });
        }
        
        // Extract collateral_type from metadata
        let collateral_type = Self::extract_collateral_type_from_metadata(transaction)?;
        
        // Create collateral in state
        let collateral = Collateral {
            collateral_id,
            created_at: transaction.created_at,
            updated_at: transaction.created_at,
            collateral_type,
        };
        
        self.collateral.insert(collateral_id, collateral);
        
        Ok(())
    }
    
    /// Helper: Extract collateral type from transaction metadata
    fn extract_collateral_type_from_metadata(
        transaction: &LoanTransaction,
    ) -> Result<String, CollateralError> {
        // Metadata encoding for collateral:
        // First 32 bytes: collateral_type as UTF-8 string
        // Remaining bytes: reserved for future use
        
        let metadata = transaction.metadata.as_bytes();
        
        // Find null terminator or use all 32 bytes
        let type_bytes = &metadata[0..32];
        let null_pos = type_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let type_str = std::str::from_utf8(&type_bytes[0..null_pos])
            .map_err(|_| CollateralError::InvalidMetadata)?;
        
        if type_str.is_empty() {
            return Err(CollateralError::InvalidMetadata);
        }
        
        Ok(type_str.to_string())
    }
}
```

#### Step 3: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::{MetadataArray, TransactionType};
    
    fn create_collateral_creation_tx(
        collateral_id: [u8; 32],
        collateral_type: &str,
    ) -> LoanTransaction {
        // Encode collateral_type in metadata
        let mut metadata_bytes = [0u8; 64];
        let type_bytes = collateral_type.as_bytes();
        let len = type_bytes.len().min(32);
        metadata_bytes[0..len].copy_from_slice(&type_bytes[0..len]);
        
        LoanTransaction {
            transaction_id: [99u8; 32],
            user_id: [0u8; 32],  // No user for collateral creation
            deal_id: collateral_id,  // Use deal_id field for collateral_id
            transaction_type: TransactionType::CollateralCreation,
            amount: 0,
            created_at: 1_640_995_200,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }
    
    #[test]
    fn test_collateral_creation_success() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        let tx = create_collateral_creation_tx(collateral_id, "real_estate");
        
        let result = state.process_collateral_creation(&tx);
        assert!(result.is_ok());
        
        // Verify collateral exists
        assert!(state.collateral.contains_key(&collateral_id));
        
        let collateral = state.collateral.get(&collateral_id).unwrap();
        assert_eq!(collateral.collateral_id, collateral_id);
        assert_eq!(collateral.collateral_type, "real_estate");
        assert_eq!(collateral.created_at, 1_640_995_200);
    }
    
    #[test]
    fn test_collateral_creation_duplicate_fails() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        let tx = create_collateral_creation_tx(collateral_id, "real_estate");
        
        // First creation should succeed
        assert!(state.process_collateral_creation(&tx).is_ok());
        
        // Second creation should fail
        let result = state.process_collateral_creation(&tx);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CollateralError::CollateralAlreadyExists { collateral_id: id } => {
                assert_eq!(id, collateral_id);
            }
            _ => panic!("Expected CollateralAlreadyExists error"),
        }
    }
    
    #[test]
    fn test_extract_collateral_type() {
        let tx = create_collateral_creation_tx([1u8; 32], "equipment");
        let collateral_type = CollateralState::extract_collateral_type_from_metadata(&tx)
            .expect("Should extract type");
        
        assert_eq!(collateral_type, "equipment");
    }
}
```

**Validation:**
- All tests pass
- Edge cases covered (duplicates, invalid metadata)

**Effort:** 4 hours total for EPIC-3.3

---

## EPIC-3.4: Implement CollateralUpdate Processor

**Effort:** 4 hours  
**Dependencies:** EPIC-3.2 complete (can be parallel with EPIC-3.3)  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Implement process_collateral_update

**File:** `lib/src/collateral.rs`

```rust
impl CollateralState {
    /// Process collateral update transaction
    ///
    /// Updates properties of an existing collateral. The collateral must
    /// already exist in state.
    ///
    /// # Arguments
    /// * `transaction` - The collateral update transaction
    ///
    /// # Returns
    /// * `Ok(())` - Collateral updated successfully
    /// * `Err(CollateralError)` - Collateral not found or invalid data
    ///
    /// # Metadata Format
    /// ```json
    /// {
    ///   "collateral_type": "updated_type",  // Optional
    ///   "name": "Updated name",              // Private
    ///   "value": "600000.00"                 // Private
    /// }
    /// ```
    pub fn process_collateral_update(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        let collateral_id = transaction.deal_id;  // Use deal_id field
        
        // Validation: Collateral must exist
        let collateral = self.collateral.get_mut(&collateral_id)
            .ok_or(CollateralError::CollateralNotFound { collateral_id })?;
        
        // Extract updated collateral_type from metadata (if provided)
        if let Ok(new_type) = Self::extract_collateral_type_from_metadata(transaction) {
            if !new_type.is_empty() {
                collateral.collateral_type = new_type;
            }
        }
        
        // Update timestamp
        collateral.updated_at = transaction.created_at;
        
        Ok(())
    }
}
```

#### Step 2: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collateral_update_success() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        // Create collateral first
        let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
        state.process_collateral_creation(&create_tx).unwrap();
        
        // Update it
        let update_tx = create_collateral_update_tx(collateral_id, "commercial_property");
        let result = state.process_collateral_update(&update_tx);
        
        assert!(result.is_ok());
        
        // Verify update
        let collateral = state.collateral.get(&collateral_id).unwrap();
        assert_eq!(collateral.collateral_type, "commercial_property");
        assert!(collateral.updated_at > collateral.created_at);
    }
    
    #[test]
    fn test_collateral_update_not_found_fails() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        // Try to update non-existent collateral
        let update_tx = create_collateral_update_tx(collateral_id, "real_estate");
        let result = state.process_collateral_update(&update_tx);
        
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CollateralError::CollateralNotFound { .. }
        ));
    }
    
    fn create_collateral_update_tx(
        collateral_id: [u8; 32],
        new_type: &str,
    ) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        let type_bytes = new_type.as_bytes();
        let len = type_bytes.len().min(32);
        metadata_bytes[0..len].copy_from_slice(&type_bytes[0..len]);
        
        LoanTransaction {
            transaction_id: [98u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralUpdate,
            amount: 0,
            created_at: 1_640_995_300,  // Later than creation
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }
}
```

**Effort:** 4 hours total for EPIC-3.4

---

## EPIC-3.5: Implement Association/Disassociation Processors

**Effort:** 8 hours  
**Dependencies:** EPIC-3.2 complete (can be parallel with EPIC-3.3/3.4)  
**Priority:** CRITICAL (double-pledge prevention)  

### Implementation Steps

#### Step 1: Implement process_collateral_association

**This is the MOST CRITICAL function - it proves no double-pledging!**

**File:** `lib/src/collateral.rs`

```rust
impl CollateralState {
    /// Process collateral-deal association transaction
    ///
    /// Associates a collateral with a deal. This is the CRITICAL function for
    /// double-pledge prevention - it validates cryptographically that the
    /// collateral is not currently pledged to another active deal.
    ///
    /// # Arguments
    /// * `transaction` - The association transaction
    ///
    /// # Returns
    /// * `Ok(())` - Association created, proof of no double-pledge
    /// * `Err(CollateralError::CollateralAlreadyPledged)` - CRITICAL: Proves double-pledge attempt!
    ///
    /// # Metadata Format
    /// Metadata bytes encoding:
    /// - Bytes 0-31: collateral_id (32 bytes)
    /// - Bytes 32-63: deal_id (32 bytes)
    ///
    /// This allows the zkVM to extract both IDs for association.
    ///
    /// # Double-Pledge Prevention
    ///
    /// The zkVM PROVES that when this transaction processed, the collateral
    /// was not associated with any other active deal. This is cryptographically
    /// verifiable proof for regulators.
    pub fn process_collateral_association(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        // Extract IDs from metadata
        let metadata = transaction.metadata.as_bytes();
        
        let mut collateral_id = [0u8; 32];
        let mut deal_id = [0u8; 32];
        collateral_id.copy_from_slice(&metadata[0..32]);
        deal_id.copy_from_slice(&metadata[32..64]);
        
        // Validation 1: Collateral must exist
        if !self.collateral.contains_key(&collateral_id) {
            return Err(CollateralError::CollateralNotFound { collateral_id });
        }
        
        // Validation 2: CRITICAL - Check for double-pledge
        // If collateral is already associated with another active deal, REJECT
        if let Some(active_deals) = self.active_associations.get(&collateral_id) {
            if !active_deals.is_empty() {
                // CRITICAL: Collateral is already pledged!
                // Return error with proof of which deal it's pledged to
                return Err(CollateralError::CollateralAlreadyPledged {
                    collateral_id,
                    existing_deal_id: active_deals[0],  // First active deal
                });
            }
        }
        
        // Validation 3: Check if this exact association already exists
        let assoc_key = Self::make_association_key(&collateral_id, &deal_id);
        if let Some(existing) = self.association_history.get(&assoc_key) {
            if existing.is_active {
                // Already associated and active
                return Ok(()); // Idempotent
            }
        }
        
        // Create association
        let association = CollateralAssociation {
            collateral_id,
            deal_id,
            associated_at: transaction.created_at,
            disassociated_at: None,
            is_active: true,
        };
        
        // Add to active associations
        self.active_associations
            .entry(collateral_id)
            .or_insert_with(Vec::new)
            .push(deal_id);
        
        // Add to history
        self.association_history.insert(assoc_key, association);
        
        Ok(())
    }
    
    /// Create association key from collateral_id and deal_id
    fn make_association_key(collateral_id: &[u8; 32], deal_id: &[u8; 32]) -> [u8; 64] {
        let mut key = [0u8; 64];
        key[0..32].copy_from_slice(collateral_id);
        key[32..64].copy_from_slice(deal_id);
        key
    }
}
```

#### Step 2: Implement process_collateral_disassociation

```rust
impl CollateralState {
    /// Process collateral-deal disassociation transaction
    ///
    /// Removes the association between a collateral and a deal.
    /// This typically happens when a deal is paid off and closed,
    /// or when collateral is being replaced.
    ///
    /// After disassociation, the collateral can be associated with
    /// a new deal (no longer considered double-pledged).
    ///
    /// # Arguments
    /// * `transaction` - The disassociation transaction
    ///
    /// # Returns
    /// * `Ok(())` - Association removed successfully
    /// * `Err(CollateralError)` - Association not found or invalid
    pub fn process_collateral_disassociation(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        // Extract IDs from metadata (same format as association)
        let metadata = transaction.metadata.as_bytes();
        
        let mut collateral_id = [0u8; 32];
        let mut deal_id = [0u8; 32];
        collateral_id.copy_from_slice(&metadata[0..32]);
        deal_id.copy_from_slice(&metadata[32..64]);
        
        // Validation: Association must exist and be active
        let assoc_key = Self::make_association_key(&collateral_id, &deal_id);
        let association = self.association_history.get_mut(&assoc_key)
            .ok_or(CollateralError::CollateralNotAssociated { collateral_id, deal_id })?;
        
        if !association.is_active {
            // Already disassociated
            return Ok(()); // Idempotent
        }
        
        // Mark association as inactive
        association.is_active = false;
        association.disassociated_at = Some(transaction.created_at);
        
        // Remove from active associations
        if let Some(deals) = self.active_associations.get_mut(&collateral_id) {
            deals.retain(|&d| d != deal_id);
            
            // Remove entry if no more active associations
            if deals.is_empty() {
                self.active_associations.remove(&collateral_id);
            }
        }
        
        Ok(())
    }
}
```

#### Step 3: Add Comprehensive Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_association_success() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        let deal_id = [2u8; 32];
        
        // Create collateral first
        let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
        state.process_collateral_creation(&create_tx).unwrap();
        
        // Associate with deal
        let assoc_tx = create_association_tx(collateral_id, deal_id);
        let result = state.process_collateral_association(&assoc_tx);
        
        assert!(result.is_ok());
        
        // Verify association
        let active = state.get_active_associations(&collateral_id);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], deal_id);
        assert!(state.is_collateral_pledged(&collateral_id));
    }
    
    #[test]
    fn test_double_pledge_prevention() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        let deal1_id = [2u8; 32];
        let deal2_id = [3u8; 32];
        
        // Create collateral
        let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
        state.process_collateral_creation(&create_tx).unwrap();
        
        // Associate with first deal
        let assoc1_tx = create_association_tx(collateral_id, deal1_id);
        state.process_collateral_association(&assoc1_tx).unwrap();
        
        // Try to associate with second deal - SHOULD FAIL
        let assoc2_tx = create_association_tx(collateral_id, deal2_id);
        let result = state.process_collateral_association(&assoc2_tx);
        
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CollateralError::CollateralAlreadyPledged {
                collateral_id: coll_id,
                existing_deal_id,
            } => {
                assert_eq!(coll_id, collateral_id);
                assert_eq!(existing_deal_id, deal1_id);
            }
            _ => panic!("Expected CollateralAlreadyPledged error"),
        }
    }
    
    #[test]
    fn test_disassociation_allows_reassociation() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        let deal1_id = [2u8; 32];
        let deal2_id = [3u8; 32];
        
        // Create collateral
        let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
        state.process_collateral_creation(&create_tx).unwrap();
        
        // Associate with first deal
        let assoc1_tx = create_association_tx(collateral_id, deal1_id);
        state.process_collateral_association(&assoc1_tx).unwrap();
        
        // Disassociate from first deal
        let disassoc_tx = create_disassociation_tx(collateral_id, deal1_id);
        let result = state.process_collateral_disassociation(&disassoc_tx);
        assert!(result.is_ok());
        
        // Now should be able to associate with second deal
        let assoc2_tx = create_association_tx(collateral_id, deal2_id);
        let result = state.process_collateral_association(&assoc2_tx);
        
        assert!(result.is_ok(), "Should allow reassociation after disassociation");
        
        // Verify second deal is now associated
        let active = state.get_active_associations(&collateral_id);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], deal2_id);
    }
    
    #[test]
    fn test_disassociation_success() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        let deal_id = [2u8; 32];
        
        // Setup: Create and associate
        setup_associated_collateral(&mut state, collateral_id, deal_id);
        
        // Disassociate
        let disassoc_tx = create_disassociation_tx(collateral_id, deal_id);
        let result = state.process_collateral_disassociation(&disassoc_tx);
        
        assert!(result.is_ok());
        
        // Verify disassociation
        let active = state.get_active_associations(&collateral_id);
        assert_eq!(active.len(), 0);
        assert!(!state.is_collateral_pledged(&collateral_id));
        
        // Verify history updated
        let assoc_key = CollateralState::make_association_key(&collateral_id, &deal_id);
        let history = state.association_history.get(&assoc_key).unwrap();
        assert!(!history.is_active);
        assert!(history.disassociated_at.is_some());
    }
    
    // Helper functions
    fn create_association_tx(collateral_id: [u8; 32], deal_id: [u8; 32]) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal_id);
        
        LoanTransaction {
            transaction_id: [97u8; 32],
            user_id: [0u8; 32],
            deal_id: [0u8; 32],  // Zero for association transactions
            transaction_type: TransactionType::CollateralDealAssociation,
            amount: 0,
            created_at: 1_640_995_400,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }
    
    fn create_disassociation_tx(collateral_id: [u8; 32], deal_id: [u8; 32]) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal_id);
        
        LoanTransaction {
            transaction_id: [96u8; 32],
            user_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CollateralDealDisassociation,
            amount: 0,
            created_at: 1_640_995_500,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }
    
    fn setup_associated_collateral(
        state: &mut CollateralState,
        collateral_id: [u8; 32],
        deal_id: [u8; 32],
    ) {
        let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
        state.process_collateral_creation(&create_tx).unwrap();
        
        let assoc_tx = create_association_tx(collateral_id, deal_id);
        state.process_collateral_association(&assoc_tx).unwrap();
    }
}
```

**Effort:** 8 hours total for EPIC-3.5

---

## EPIC-3.6: Implement Detection Processors

**Effort:** 8 hours  
**Dependencies:** EPIC-3.2 complete (can be parallel with EPIC-3.3/3.4/3.5)  
**Priority:** MEDIUM  

### Implementation Steps

#### Step 1: Implement process_collateral_detection_request

```rust
impl CollateralState {
    /// Process collateral detection request transaction
    ///
    /// Records that a duplicate detection check was requested for this collateral.
    /// This is initiated by the collateral-intel service.
    ///
    /// # Arguments
    /// * `transaction` - The detection request transaction
    ///
    /// # Metadata Format
    /// - Bytes 0-31: collateral_id
    pub fn process_collateral_detection_request(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        let collateral_id = transaction.deal_id;  // Use deal_id field
        
        // Validation: Collateral should exist (but not required - can check non-existent)
        // Record the detection request
        self.detection_requests.insert(collateral_id, transaction.created_at);
        
        Ok(())
    }
}
```

#### Step 2: Implement process_collateral_detection_update

```rust
impl CollateralState {
    /// Process collateral detection update transaction
    ///
    /// Records the results of a duplicate detection check.
    /// The actual detection results are in metadata (private).
    ///
    /// # Arguments
    /// * `transaction` - The detection update transaction
    ///
    /// # Metadata Format
    /// ```json
    /// {
    ///   "matches_found": 3,
    ///   "confidence_score": 0.95,
    ///   "duplicate_ids": ["uuid1", "uuid2"],  // Private
    /// }
    /// ```
    ///
    /// # Note
    /// Detection results are for audit trail. The zkVM doesn't need to
    /// process them beyond recording that an update occurred.
    pub fn process_collateral_detection_update(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), CollateralError> {
        let collateral_id = transaction.deal_id;  // Use deal_id field
        
        // Update detection request timestamp to reflect completion
        self.detection_requests.insert(collateral_id, transaction.created_at);
        
        // Detection results stay in metadata (private)
        // zkVM just proves the update was recorded
        
        Ok(())
    }
}
```

#### Step 3: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detection_request() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        let tx = create_detection_request_tx(collateral_id);
        let result = state.process_collateral_detection_request(&tx);
        
        assert!(result.is_ok());
        assert!(state.detection_requests.contains_key(&collateral_id));
    }
    
    #[test]
    fn test_detection_update() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];
        
        let update_tx = create_detection_update_tx(collateral_id);
        let result = state.process_collateral_detection_update(&update_tx);
        
        assert!(result.is_ok());
        assert!(state.detection_requests.contains_key(&collateral_id));
    }
    
    fn create_detection_request_tx(collateral_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [95u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id,  // Collateral ID in deal_id field
            transaction_type: TransactionType::CollateralDetectionRequest,
            amount: 0,
            created_at: 1_640_995_600,
            metadata: MetadataArray::empty(),
        }
    }
    
    fn create_detection_update_tx(collateral_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [94u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralDetectionUpdate,
            amount: 0,
            created_at: 1_640_995_700,
            metadata: MetadataArray::empty(),
        }
    }
}
```

**Effort:** 8 hours total for EPIC-3.6

---

## EPIC-3.7: Integrate with deal_processor.rs

**Effort:** 4 hours  
**Dependencies:** EPIC-3.3, 3.4, 3.5, 3.6 all complete  
**Priority:** HIGH  

### Implementation Steps

#### Step 1: Update Imports

**File:** `lib/src/deal_processor.rs`

**Add to imports (top of file):**

```rust
use crate::collateral::{CollateralError, CollateralState};
```

#### Step 2: Replace NO-OP Code

**Find and replace (lines 272-280):**

**OLD CODE (DELETE):**
```rust
// Collateral transactions are for audit trail only, not processed in zkVM
TransactionType::CollateralCreation
| TransactionType::CollateralUpdate
| TransactionType::CollateralDealAssociation
| TransactionType::CollateralDealDisassociation
| TransactionType::CollateralDetectionRequest
| TransactionType::CollateralDetectionUpdate => {
    // No-op: Collateral transactions are recorded for audit trail
    // but don't affect zkVM state (deals, balances, users)
}
```

**NEW CODE (INSERT):**
```rust
TransactionType::CollateralCreation => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: self.active_associations.clone(),
        association_history: self.association_history.clone(),
        detection_requests: HashMap::new(),  // Not needed for creation
    };
    
    collateral_state.process_collateral_creation(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    self.collateral = collateral_state.collateral;
}

TransactionType::CollateralUpdate => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: self.active_associations.clone(),
        association_history: self.association_history.clone(),
        detection_requests: HashMap::new(),
    };
    
    collateral_state.process_collateral_update(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    self.collateral = collateral_state.collateral;
}

TransactionType::CollateralDealAssociation => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: self.active_associations.clone(),
        association_history: self.association_history.clone(),
        detection_requests: HashMap::new(),
    };
    
    // CRITICAL: This proves no double-pledging!
    collateral_state.process_collateral_association(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    self.active_associations = collateral_state.active_associations;
    self.association_history = collateral_state.association_history;
}

TransactionType::CollateralDealDisassociation => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: self.active_associations.clone(),
        association_history: self.association_history.clone(),
        detection_requests: HashMap::new(),
    };
    
    collateral_state.process_collateral_disassociation(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    self.active_associations = collateral_state.active_associations;
    self.association_history = collateral_state.association_history;
}

TransactionType::CollateralDetectionRequest => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: HashMap::new(),  // Not needed
        association_history: HashMap::new(),  // Not needed
        detection_requests: HashMap::new(),
    };
    
    collateral_state.process_collateral_detection_request(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    // Detection requests don't update state (just audit trail)
}

TransactionType::CollateralDetectionUpdate => {
    let mut collateral_state = CollateralState {
        collateral: self.collateral.clone(),
        active_associations: HashMap::new(),
        association_history: HashMap::new(),
        detection_requests: HashMap::new(),
    };
    
    collateral_state.process_collateral_detection_update(transaction)
        .map_err(|e| ProcessingError::CollateralError(e))?;
    
    // Detection updates don't modify state (just audit trail)
}
```

#### Step 3: Add CollateralError to ProcessingError

**File:** `lib/src/deal_processor.rs`

**Update ProcessingError enum (around line 19-52):**

```rust
/// Errors that can occur during loan processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Loan error: {0}")]
    LoanError(#[from] DealError),

    #[error("Balance error: {0}")]
    BalanceError(#[from] BalanceError),
    
    // NEW: Collateral errors
    #[error("Collateral error: {0}")]
    CollateralError(#[from] CollateralError),

    #[error("Invalid transaction type for operation")]
    InvalidTransactionType,
    
    // ... rest of errors
}
```

**Validation:**
- Compiles without errors
- Error conversion works automatically (From trait)
- Can match on CollateralError variants

**Effort:** 4 hours total for EPIC-3.7

---

## EPIC-3.8: zkVM Testing for Collateral

**Effort:** 8 hours  
**Dependencies:** EPIC-3.7 complete  
**Priority:** HIGH  

### Test Strategy

**Create:** `lib/src/collateral.rs` - expand test module

### Comprehensive Test Suite

#### Test 1: Complete Collateral Lifecycle

```rust
#[test]
fn test_complete_collateral_lifecycle() {
    let mut state = GlobalLoanState::new();
    let collateral_id = [1u8; 32];
    let deal_id = [2u8; 32];
    
    // 1. Create collateral
    let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
    state.process_transaction(&create_tx).unwrap();
    
    assert_eq!(state.collateral.len(), 1);
    assert!(!state.is_collateral_pledged(&collateral_id));
    
    // 2. Update collateral
    let update_tx = create_collateral_update_tx(collateral_id, "commercial_property");
    state.process_transaction(&update_tx).unwrap();
    
    let collateral = state.get_collateral(&collateral_id).unwrap();
    assert_eq!(collateral.collateral_type, "commercial_property");
    
    // 3. Associate with deal
    let assoc_tx = create_association_tx(collateral_id, deal_id);
    state.process_transaction(&assoc_tx).unwrap();
    
    assert!(state.is_collateral_pledged(&collateral_id));
    assert_eq!(state.get_active_associations(&collateral_id).len(), 1);
    
    // 4. Request detection
    let detect_req_tx = create_detection_request_tx(collateral_id);
    state.process_transaction(&detect_req_tx).unwrap();
    
    // 5. Update detection results
    let detect_upd_tx = create_detection_update_tx(collateral_id);
    state.process_transaction(&detect_upd_tx).unwrap();
    
    // 6. Disassociate from deal
    let disassoc_tx = create_disassociation_tx(collateral_id, deal_id);
    state.process_transaction(&disassoc_tx).unwrap();
    
    assert!(!state.is_collateral_pledged(&collateral_id));
    assert_eq!(state.get_active_associations(&collateral_id).len(), 0);
    
    // Verify transaction count
    assert_eq!(state.transaction_count, 6);
}
```

#### Test 2: Double-Pledge Prevention in Full State

```rust
#[test]
fn test_double_pledge_prevention_in_global_state() {
    let mut state = GlobalLoanState::new();
    
    let collateral_id = [1u8; 32];
    let deal1_id = [2u8; 32];
    let deal2_id = [3u8; 32];
    
    // Setup users and deals for context
    setup_test_users(&mut state);
    setup_test_deal(&mut state, deal1_id);
    setup_test_deal(&mut state, deal2_id);
    
    // Create collateral
    let create_tx = create_collateral_creation_tx(collateral_id, "real_estate");
    state.process_transaction(&create_tx).unwrap();
    
    // Associate with deal 1
    let assoc1_tx = create_association_tx(collateral_id, deal1_id);
    state.process_transaction(&assoc1_tx).unwrap();
    
    // Try to associate with deal 2 - MUST FAIL
    let assoc2_tx = create_association_tx(collateral_id, deal2_id);
    let result = state.process_transaction(&assoc2_tx);
    
    assert!(result.is_err());
    
    // Verify error is CollateralAlreadyPledged
    match result.unwrap_err() {
        ProcessingError::CollateralError(CollateralError::CollateralAlreadyPledged {
            collateral_id: cid,
            existing_deal_id: did,
        }) => {
            assert_eq!(cid, collateral_id);
            assert_eq!(did, deal1_id);
        }
        e => panic!("Expected CollateralAlreadyPledged, got: {:?}", e),
    }
    
    // Verify deal2 was NOT associated
    let active = state.get_active_associations(&collateral_id);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0], deal1_id);
}
```

#### Test 3: Batch Processing with Collateral

```rust
#[test]
fn test_batch_with_collateral_transactions() {
    use crate::transactions::DealBatch;
    
    let mut state = GlobalLoanState::new();
    
    let collateral1_id = [10u8; 32];
    let collateral2_id = [11u8; 32];
    let deal1_id = [20u8; 32];
    let deal2_id = [21u8; 32];
    
    // Create batch with mixed transaction types
    let batch = DealBatch {
        batch_id: [100u8; 32],
        transactions: vec![
            // Create two pieces of collateral
            create_collateral_creation_tx(collateral1_id, "real_estate"),
            create_collateral_creation_tx(collateral2_id, "equipment"),
            
            // Associate each with different deals
            create_association_tx(collateral1_id, deal1_id),
            create_association_tx(collateral2_id, deal2_id),
            
            // Request detection for both
            create_detection_request_tx(collateral1_id),
            create_detection_request_tx(collateral2_id),
        ],
        previous_state_root: [0u8; 32],
        created_at: 1_640_995_200,
    };
    
    // Process batch
    let result = state.process_batch(&batch);
    assert!(result.is_ok());
    
    // Verify final state
    assert_eq!(state.collateral.len(), 2);
    assert_eq!(state.total_active_associations(), 2);
    assert!(state.is_collateral_pledged(&collateral1_id));
    assert!(state.is_collateral_pledged(&collateral2_id));
}
```

#### Test 4: Collateral with Deal Lifecycle

```rust
#[test]
fn test_collateral_through_deal_lifecycle() {
    let mut state = GlobalLoanState::new();
    
    let originator_id = [1u8; 32];
    let investor_id = [2u8; 32];
    let deal_id = [10u8; 32];
    let collateral_id = [20u8; 32];
    
    // Setup: Create users with KYC
    setup_kycd_users(&mut state, originator_id, investor_id);
    
    // Create collateral
    let coll_create = create_collateral_creation_tx(collateral_id, "real_estate");
    state.process_transaction(&coll_create).unwrap();
    
    // Create deal
    let deal_tx = create_deal_origination(deal_id, originator_id, investor_id, 100000);
    state.process_transaction(&deal_tx).unwrap();
    
    // Associate collateral with deal
    let assoc_tx = create_association_tx(collateral_id, deal_id);
    state.process_transaction(&assoc_tx).unwrap();
    
    assert!(state.is_collateral_pledged(&collateral_id));
    
    // Make payments and close deal
    let payment = create_deal_payment(deal_id, originator_id, 100000);
    state.process_transaction(&payment).unwrap();
    
    let closeout = create_deal_closeout(deal_id, investor_id);
    state.process_transaction(&closeout).unwrap();
    
    // After deal is closed, can disassociate collateral
    let disassoc_tx = create_disassociation_tx(collateral_id, deal_id);
    state.process_transaction(&disassoc_tx).unwrap();
    
    assert!(!state.is_collateral_pledged(&collateral_id));
    
    // Now collateral can be reused for another deal
    let deal2_id = [11u8; 32];
    setup_test_deal(&mut state, deal2_id);
    
    let assoc2_tx = create_association_tx(collateral_id, deal2_id);
    let result = state.process_transaction(&assoc2_tx);
    
    assert!(result.is_ok(), "Should allow reassociation after disassociation");
}
```

#### Test 5: Error Propagation

```rust
#[test]
fn test_collateral_errors_propagate_correctly() {
    let mut state = GlobalLoanState::new();
    let collateral_id = [1u8; 32];
    
    // Try to update non-existent collateral
    let update_tx = create_collateral_update_tx(collateral_id, "real_estate");
    let result = state.process_transaction(&update_tx);
    
    assert!(result.is_err());
    
    // Verify it's a CollateralError wrapped in ProcessingError
    match result.unwrap_err() {
        ProcessingError::CollateralError(CollateralError::CollateralNotFound { .. }) => {
            // Expected
        }
        e => panic!("Expected CollateralNotFound, got: {:?}", e),
    }
}
```

**Validation:**
- Compiles: `cargo check --package arda-credit-lib`
- All tests pass: `cargo test --package arda-credit-lib`
- No NO-OPs remain
- All 6 collateral types processed

**Effort:** 4 hours total for EPIC-3.7

---

## Double-Pledge Prevention Algorithm

### The Critical Requirement

**Regulatory Concern:** A piece of collateral cannot be pledged to multiple active loans simultaneously. This would allow:
- Over-leverage (using same asset as security for multiple debts)
- Fraud (claiming sole ownership of collateral while it secures other loans)
- Risk exposure (if collateral is seized, which lender has priority?)

### How We Prove It in ZK

**The Algorithm:**

```rust
pub fn process_collateral_association(
    &mut self,
    transaction: &LoanTransaction,
) -> Result<(), CollateralError> {
    // Extract IDs
    let collateral_id = /* from metadata */;
    let deal_id = /* from metadata */;
    
    // CRITICAL CHECK: Is collateral already pledged?
    if let Some(active_deals) = self.active_associations.get(&collateral_id) {
        if !active_deals.is_empty() {
            // Collateral is already pledged to active_deals[0]
            // REJECT the transaction
            // This rejection is PROVEN in the ZK proof!
            return Err(CollateralError::CollateralAlreadyPledged {
                collateral_id,
                existing_deal_id: active_deals[0],
            });
        }
    }
    
    // If we reach here, we've PROVEN collateral is not double-pledged
    // Add the association
    self.active_associations
        .entry(collateral_id)
        .or_insert_with(Vec::new)
        .push(deal_id);
    
    Ok(())
}
```

### Why This is Powerful

**Database Check (Old Way):**
```sql
-- Can check, but can't prove to third party
SELECT COUNT(*) FROM collateral_associations 
WHERE collateral_id = $1 AND is_active = true;
```
- ❌ Regulator must trust our database
- ❌ Database can be modified
- ❌ No cryptographic proof

**zkVM Proof (Our Way):**
```
The zero-knowledge proof cryptographically guarantees:
"When transaction T was processed at time X, collateral C was 
not associated with any other active deal. This can be verified 
by anyone with the proof, without revealing which deals exist 
or any other private information."
```
- ✅ Mathematically provable
- ✅ Cryptographically verifiable by third parties
- ✅ Immutable once proven on-chain
- ✅ Doesn't reveal private deal details

### State Transitions

**Valid Transitions:**
```
1. Collateral created (not pledged)
   active_associations[collateral_id] = []

2. Associated with Deal A (now pledged to A)
   active_associations[collateral_id] = [deal_A]

3. Try to associate with Deal B
   Check: active_associations[collateral_id] = [deal_A] ← NOT EMPTY
   REJECT! ← This rejection is proven in ZK

4. Disassociate from Deal A (not pledged anymore)
   active_associations[collateral_id] = []

5. Now can associate with Deal B (valid)
   active_associations[collateral_id] = [deal_B]
```

**Invalid Transitions (Prevented):**
```
1. Create collateral
2. Associate with Deal A
3. Associate with Deal B ← REJECTED by zkVM
   - Proof shows this was attempted and rejected
   - Cryptographic evidence of prevention
```

---

## Performance Considerations

### Proof Generation Impact

**Current state without collateral:**
- GlobalLoanState: ~1KB per 100 users/deals
- Proof time: ~30-60 seconds (Sindri)

**With collateral added:**
- GlobalLoanState: ~1KB + ~500 bytes per 100 collateral items
- Estimated proof time: ~35-70 seconds (< 20% increase)
- Still well within acceptable range

### HashMap Performance

**Operations:**
- Insert: O(1) average
- Lookup: O(1) average
- Double-pledge check: O(1) lookup in active_associations

**Worst case:**
- 1000 collateral items
- 1000 associations
- HashMap still O(1) for all operations
- Negligible impact on proof time

### Memory Considerations

**State size estimates:**

```rust
Collateral struct: 32 + 8 + 8 + ~20 = ~68 bytes
CollateralAssociation: 32 + 32 + 8 + 9 + 1 = ~82 bytes

For 1000 collateral items:
- collateral HashMap: 1000 * 68 = ~68KB
- active_associations: 1000 * 32 = ~32KB (assuming 1:1 with deals)
- association_history: 1000 * 82 = ~82KB

Total additional: ~182KB for 1000 items
```

**Conclusion:** Memory impact is minimal, no concern for zkVM execution.

---

## Integration Checklist

### Code Quality

- [ ] All code in `lib/src/collateral.rs`
- [ ] No database dependencies
- [ ] No file I/O
- [ ] No network calls
- [ ] No non-deterministic operations
- [ ] Compiles for zkVM target
- [ ] All clippy warnings resolved
- [ ] Formatted with rustfmt

### Functionality

- [ ] CollateralCreation processor implemented
- [ ] CollateralUpdate processor implemented
- [ ] CollateralDealAssociation processor implemented
- [ ] CollateralDealDisassociation processor implemented
- [ ] CollateralDetectionRequest processor implemented
- [ ] CollateralDetectionUpdate processor implemented
- [ ] All integrated in deal_processor.rs
- [ ] ProcessingError updated

### Testing

- [ ] Unit tests for each processor (>15 tests)
- [ ] Double-pledge prevention tested
- [ ] Complete lifecycle tested
- [ ] Error cases tested
- [ ] Batch processing tested
- [ ] Integration with deals tested
- [ ] Code coverage >85%

### State Management

- [ ] GlobalLoanState updated
- [ ] Collateral HashMap added
- [ ] Active associations tracking added
- [ ] Association history added
- [ ] Helper methods implemented
- [ ] Serialization/deserialization works

### Documentation

- [ ] Module-level docs explain purpose
- [ ] Each function has doc comments
- [ ] Double-pledge algorithm documented
- [ ] Metadata formats documented
- [ ] Examples provided

---

## Validation Checklist

### EPIC-3 Complete When:

**Functional:**
- [ ] Can create collateral in zkVM state
- [ ] Can update collateral properties
- [ ] Can associate collateral with deal
- [ ] Can disassociate collateral from deal
- [ ] Double-pledge attempts are rejected
- [ ] Detection requests recorded
- [ ] All 6 transaction types process correctly

**Quality:**
- [ ] >85% code coverage
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] No compiler warnings
- [ ] No clippy warnings
- [ ] Code reviewed and approved

**Performance:**
- [ ] Proof time increase <20%
- [ ] Memory usage acceptable
- [ ] No performance regressions

**Integration:**
- [ ] Works with GlobalLoanState
- [ ] Works with process_transaction()
- [ ] Works in batch processing
- [ ] Errors propagate correctly

**Documentation:**
- [ ] Code documentation complete
- [ ] Architecture docs updated
- [ ] Audit report updated
- [ ] Handoff guide for WS-4

**Deployment:**
- [ ] Code merged to feature branch
- [ ] CI/CD passing
- [ ] Ready for WS-4 (API integration)

---

## Handoff to WS-4 (Collateral API Integration)

### What WS-4 Needs to Know

**1. Metadata Encoding Format**

For CollateralCreation:
```
Bytes 0-31: collateral_type as UTF-8 string (null-padded)
Bytes 32-63: Reserved
```

For CollateralAssociation/Disassociation:
```
Bytes 0-31: collateral_id (32 bytes)
Bytes 32-63: deal_id (32 bytes)
```

**2. Transaction ID Field Usage**

- `deal_id` field in LoanTransaction is used for collateral_id
- This is because collateral operations aren't initially associated with deals
- Association transactions use metadata for both IDs

**3. Error Cases to Handle in API**

When API receives CollateralAlreadyPledged error from zkVM:
- This means double-pledge was attempted
- API should reject association request
- Return clear error to user
- Log for security monitoring

**4. Testing Collateral Operations**

Use these test cases from lib/:
- Double-pledge prevention
- Association after disassociation
- Update non-existent collateral
- Complete lifecycle

---

## Common Issues & Solutions

### Issue 1: "CollateralError not in scope"

**Cause:** Missing import in deal_processor.rs

**Solution:**
```rust
use crate::collateral::{CollateralError, CollateralState};
```

### Issue 2: "HashMap operations too slow"

**Cause:** Unlikely, but if many collateral items

**Solution:**
- HashMap is O(1) for all operations
- If concerned, benchmark with `cargo bench`
- Consider Vec if needed (unlikely)

### Issue 3: "Metadata format mismatch"

**Cause:** API encodes metadata differently than zkVM expects

**Solution:**
- Document format clearly in both layers
- Add validation tests
- Coordinate with WS-4 team

### Issue 4: "Tests fail: 'user not found'"

**Cause:** Collateral tests need user context

**Solution:**
```rust
// Setup users before testing collateral with deals
setup_kycd_users(&mut state, user1, user2);
```

### Issue 5: "Double-pledge test passes when it should fail"

**Cause:** Logic error in association check

**Debug:**
```rust
// Add debug logging
println!("Active associations: {:?}", self.active_associations.get(&collateral_id));
println!("Is pledged: {}", self.is_collateral_pledged(&collateral_id));
```

---

## Timeline

### Day-by-Day Breakdown

**Day 1 (8 hours):**
- EPIC-3.1: Module structure (4 hours)
- EPIC-3.2: Update GlobalLoanState (4 hours)
- **Deliverable:** Foundation ready, can parallelize rest

**Day 2 (8 hours):**
- EPIC-3.3: CollateralCreation (4 hours)
- EPIC-3.4: CollateralUpdate (4 hours)
- **Deliverable:** Basic CRUD operations working

**Day 3 (8 hours):**
- EPIC-3.5: Association/Disassociation (8 hours - most complex)
- **Deliverable:** Double-pledge prevention implemented

**Day 4 (8 hours):**
- EPIC-3.6: Detection processors (4 hours)
- EPIC-3.7: Integration with deal_processor (4 hours)
- **Deliverable:** All processors integrated

**Day 5-6 (12 hours):**
- EPIC-3.8: Comprehensive testing (8 hours)
- Buffer/polish (4 hours)
- **Deliverable:** EPIC-3 complete, tested, documented

**Total: 5.5 days (44 hours)**

---

## Success Metrics

### Definition of Done

**EPIC-3 is complete when:**

1. ✅ All 6 collateral processors implemented
2. ✅ Double-pledge prevention works and is tested
3. ✅ GlobalLoanState includes collateral
4. ✅ All unit tests passing (>15 tests)
5. ✅ Integration tests passing (>5 tests)
6. ✅ Code coverage >85%
7. ✅ No NO-OPs remain for collateral
8. ✅ Documentation complete
9. ✅ Code reviewed and merged
10. ✅ Ready for WS-4 to start API integration

### Measurement

```bash
# Verify all processors implemented
grep -A 5 "TransactionType::Collateral" lib/src/deal_processor.rs
# Should show actual processing, not "No-op"

# Run all tests
cargo test --package arda-credit-lib collateral

# Check coverage
cargo tarpaulin --package arda-credit-lib -- collateral

# Verify no NO-OPs
grep -n "No-op.*collateral" lib/src/deal_processor.rs
# Should return no results
```

---

**End of Work Stream 3 Implementation Plan**

**Next Work Stream:** After EPIC-3 is complete, WS-4 (Collateral API Integration) can begin implementing API endpoints that use these zkVM processors.
