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
//! and remain private. Only collateral IDs and association counts are in state,
//! which is sufficient for double-pledge prevention.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::transactions::LoanTransaction;

/// Errors that can occur during collateral processing
#[derive(Error, Debug, Clone)]
pub enum CollateralError {
    #[error("Collateral already exists: {collateral_id:?}")]
    CollateralAlreadyExists { collateral_id: Vec<u8> },

    #[error("Collateral not found: {collateral_id:?}")]
    CollateralNotFound { collateral_id: Vec<u8> },

    #[error("Collateral already pledged to deal: {collateral_id:?} → {existing_deal_id:?}")]
    CollateralAlreadyPledged {
        collateral_id: Vec<u8>,
        existing_deal_id: Vec<u8>,
    },

    #[error("Collateral not associated with deal: {collateral_id:?} + {deal_id:?}")]
    CollateralNotAssociated {
        collateral_id: Vec<u8>,
        deal_id: Vec<u8>,
    },

    #[error("Deal not found: {deal_id:?}")]
    DealNotFound { deal_id: Vec<u8> },

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
    pub collateral_id: Vec<u8>,

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
    pub collateral_id: Vec<u8>,

    /// Hashed deal ID
    pub deal_id: Vec<u8>,

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
    pub collateral: HashMap<Vec<u8>, Collateral>,

    /// Active associations (collateral_id → Vec<deal_id>)
    /// Only includes currently active associations
    pub active_associations: HashMap<Vec<u8>, Vec<Vec<u8>>>,

    /// Complete association history (for audit trail)
    /// Key: (collateral_id, deal_id) as concatenated bytes
    pub association_history: HashMap<Vec<u8>, CollateralAssociation>,

    /// Detection requests (collateral_id → request_timestamp)
    pub detection_requests: HashMap<Vec<u8>, u64>,
}

impl CollateralState {
    /// Create a new empty collateral state
    #[must_use]
    pub fn new() -> Self {
        Self {
            collateral: HashMap::new(),
            active_associations: HashMap::new(),
            association_history: HashMap::new(),
            detection_requests: HashMap::new(),
        }
    }

    /// Extract collateral type from transaction metadata
    ///
    /// The collateral type is stored in the first 32 bytes of metadata.
    pub fn extract_collateral_type_from_metadata(
        transaction: &LoanTransaction,
    ) -> Result<String, CollateralError> {
        let metadata = transaction.metadata.as_bytes();
        if metadata.len() < 32 {
            return Err(CollateralError::InvalidMetadata);
        }

        // Find null terminator or take first 32 bytes
        let type_bytes = &metadata[0..32];
        let null_pos = type_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let type_str = std::str::from_utf8(&type_bytes[0..null_pos])
            .map_err(|_| CollateralError::InvalidMetadata)?;

        Ok(type_str.to_string())
    }

    /// Create association key from collateral_id and deal_id
    ///
    /// The key is collateral_id + deal_id (64 bytes total)
    pub fn make_association_key(collateral_id: &[u8], deal_id: &[u8]) -> Vec<u8> {
        let mut key = Vec::with_capacity(64);
        key.extend_from_slice(collateral_id);
        key.extend_from_slice(deal_id);
        key
    }
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
        let collateral_id = transaction.deal_id.to_vec();

        // Validation: Collateral must NOT already exist
        if self.collateral.contains_key(&collateral_id) {
            return Err(CollateralError::CollateralAlreadyExists { collateral_id });
        }

        // Extract collateral_type from metadata
        let collateral_type = Self::extract_collateral_type_from_metadata(transaction)?;

        // Create collateral in state
        let collateral = Collateral {
            collateral_id: collateral_id.clone(),
            created_at: transaction.created_at,
            updated_at: transaction.created_at,
            collateral_type,
        };

        self.collateral.insert(collateral_id, collateral);
        Ok(())
    }

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
        let collateral_id = transaction.deal_id.to_vec(); // Use deal_id field

        // Validation: Collateral must exist
        let collateral = self
            .collateral
            .get_mut(&collateral_id)
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

        let collateral_id = metadata[0..32].to_vec();
        let deal_id = metadata[32..64].to_vec();

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
                    existing_deal_id: active_deals[0].clone(), // First active deal
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
            collateral_id: collateral_id.clone(),
            deal_id: deal_id.clone(),
            associated_at: transaction.created_at,
            disassociated_at: None,
            is_active: true,
        };

        // Add to active associations
        self.active_associations
            .entry(collateral_id.clone())
            .or_insert_with(Vec::new)
            .push(deal_id.clone());

        // Add to history
        self.association_history.insert(assoc_key, association);

        Ok(())
    }

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

        let collateral_id = metadata[0..32].to_vec();
        let deal_id = metadata[32..64].to_vec();

        // Validation: Association must exist and be active
        let assoc_key = Self::make_association_key(&collateral_id, &deal_id);
        let association = self.association_history.get_mut(&assoc_key).ok_or(
            CollateralError::CollateralNotAssociated {
                collateral_id: collateral_id.clone(),
                deal_id: deal_id.clone(),
            },
        )?;

        if !association.is_active {
            // Already disassociated
            return Ok(()); // Idempotent
        }

        // Mark association as inactive
        association.is_active = false;
        association.disassociated_at = Some(transaction.created_at);

        // Remove from active associations
        if let Some(deals) = self.active_associations.get_mut(&collateral_id) {
            deals.retain(|d| d != &deal_id);

            // Remove entry if no more active associations
            if deals.is_empty() {
                self.active_associations.remove(&collateral_id);
            }
        }

        Ok(())
    }

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
        let collateral_id = transaction.deal_id.to_vec(); // Use deal_id field

        // Validation: Collateral should exist (but not required - can check non-existent)
        // Record the detection request
        self.detection_requests
            .insert(collateral_id, transaction.created_at);

        Ok(())
    }

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
        let collateral_id = transaction.deal_id.to_vec(); // Use deal_id field

        // Update detection request timestamp to reflect completion
        self.detection_requests
            .insert(collateral_id, transaction.created_at);

        // Detection results stay in metadata (private)
        // zkVM just proves the update was recorded

        Ok(())
    }
}

impl Default for CollateralState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::LoanTransaction;
    use crate::transactions::{MetadataArray, TransactionType};

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
            collateral_id: vec![1u8; 32],
            created_at: 1000,
            updated_at: 1000,
            collateral_type: "real_estate".to_string(),
        };

        assert_eq!(collateral.collateral_id, vec![1u8; 32]);
        assert_eq!(collateral.created_at, 1000);
    }

    #[test]
    fn test_collateral_creation_success() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];

        let tx = create_collateral_creation_tx(collateral_id, "real_estate");

        let result = state.process_collateral_creation(&tx);
        assert!(result.is_ok());

        // Verify collateral exists
        let collateral_id_vec = collateral_id.to_vec();
        assert!(state.collateral.contains_key(&collateral_id_vec));

        let collateral = state.collateral.get(&collateral_id_vec).unwrap();
        assert_eq!(collateral.collateral_id, collateral_id_vec);
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
                assert_eq!(id, collateral_id.to_vec());
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
        let collateral_id_vec = collateral_id.to_vec();
        let collateral = state.collateral.get(&collateral_id_vec).unwrap();
        assert_eq!(collateral.collateral_type, "commercial_property");
        assert!(collateral.updated_at > collateral.created_at);
    }

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
        let collateral_id_vec = collateral_id.to_vec();
        let deal_id_vec = deal_id.to_vec();
        let active = state
            .active_associations
            .get(&collateral_id_vec)
            .unwrap()
            .clone();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], deal_id_vec);

        // Verify history
        let assoc_key = CollateralState::make_association_key(&collateral_id, &deal_id);
        let history = state.association_history.get(&assoc_key).unwrap();
        assert!(history.is_active);
        assert!(history.disassociated_at.is_none());
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
                assert_eq!(coll_id, collateral_id.to_vec());
                assert_eq!(existing_deal_id, deal1_id.to_vec());
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

        assert!(
            result.is_ok(),
            "Should allow reassociation after disassociation"
        );

        // Verify second deal is now associated
        let collateral_id_vec = collateral_id.to_vec();
        let deal2_id_vec = deal2_id.to_vec();
        let active = state
            .active_associations
            .get(&collateral_id_vec)
            .unwrap()
            .clone();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], deal2_id_vec);
    }

    #[test]
    fn test_detection_request() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];

        let tx = create_detection_request_tx(collateral_id);
        let result = state.process_collateral_detection_request(&tx);

        assert!(result.is_ok());
        let collateral_id_vec = collateral_id.to_vec();
        assert!(state.detection_requests.contains_key(&collateral_id_vec));
    }

    #[test]
    fn test_detection_update() {
        let mut state = CollateralState::new();
        let collateral_id = [1u8; 32];

        let update_tx = create_detection_update_tx(collateral_id);
        let result = state.process_collateral_detection_update(&update_tx);

        assert!(result.is_ok());
        let collateral_id_vec = collateral_id.to_vec();
        assert!(state.detection_requests.contains_key(&collateral_id_vec));
    }

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
            user_id: [0u8; 32], // No user for collateral creation
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id: collateral_id, // Use deal_id field for collateral_id
            transaction_type: TransactionType::CollateralCreation,
            amount: 0,
            created_at: 1_640_995_200,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_collateral_update_tx(collateral_id: [u8; 32], new_type: &str) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        let type_bytes = new_type.as_bytes();
        let len = type_bytes.len().min(32);
        metadata_bytes[0..len].copy_from_slice(&type_bytes[0..len]);

        LoanTransaction {
            transaction_id: [98u8; 32],
            user_id: [0u8; 32],
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralUpdate,
            amount: 0,
            created_at: 1_640_995_300, // Later than creation
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_association_tx(collateral_id: [u8; 32], deal_id: [u8; 32]) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal_id);

        LoanTransaction {
            transaction_id: [97u8; 32],
            user_id: [0u8; 32],
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id: [0u8; 32], // Zero for association transactions
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
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
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

    fn create_detection_request_tx(collateral_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [95u8; 32],
            user_id: [0u8; 32],
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id: collateral_id, // Collateral ID in deal_id field
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
            originator_company_id: [0u8; 32],
            investor_company_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralDetectionUpdate,
            amount: 0,
            created_at: 1_640_995_700,
            metadata: MetadataArray::empty(),
        }
    }
}
