//! ZK proof integration for document operations
//!
//! This module provides zero-knowledge proof integration for document state transitions,
//! ensuring privacy-preserving document operations while maintaining verifiable integrity.

use crate::{
    document::{Document, DocumentAction, DocumentStateTransition, StateTransitionType},
    transactions::{LoanTransaction, TransactionType},
    zkvm_ads::StateTransition,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

/// Document ZK proof errors
#[derive(Error, Debug, Clone)]
pub enum DocumentZkError {
    #[error("Invalid document state transition: {message}")]
    InvalidStateTransition { message: String },

    #[error("Document hash mismatch: expected {expected:?}, got {actual:?}")]
    HashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("Invalid proof generation: {message}")]
    InvalidProofGeneration { message: String },

    #[error("State commitment error: {message}")]
    StateCommitmentError { message: String },

    #[error("Merkle tree error: {message}")]
    MerkleTreeError { message: String },
}

/// Document state for ZK proofs (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentZkState {
    pub documents: HashMap<[u8; 32], Document>,
    pub state_transitions: Vec<DocumentStateTransition>,
    pub merkle_root: [u8; 32],
    pub total_documents: u64,
    pub last_transition_timestamp: u64,
}

impl DocumentZkState {
    /// Create a new empty document ZK state
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            state_transitions: Vec::new(),
            merkle_root: [0u8; 32],
            total_documents: 0,
            last_transition_timestamp: 0,
        }
    }

    /// Calculate state commitment hash
    #[must_use]
    pub fn calculate_state_commitment(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Hash merkle root
        hasher.update(&self.merkle_root);

        // Hash total documents count
        hasher.update(&self.total_documents.to_le_bytes());

        // Hash last transition timestamp
        hasher.update(&self.last_transition_timestamp.to_le_bytes());

        // Hash all document IDs (sorted for determinism)
        let mut document_ids: Vec<[u8; 32]> = self.documents.keys().cloned().collect();
        document_ids.sort();
        for doc_id in document_ids {
            hasher.update(&doc_id);
        }

        hasher.finalize().into()
    }

    /// Process document upload state transition
    pub fn process_document_upload(
        &mut self,
        document: Document,
        uploaded_by: [u8; 32],
        timestamp: u64,
    ) -> Result<DocumentStateTransition, DocumentZkError> {
        // Check if document already exists
        if self.documents.contains_key(&document.document_id) {
            return Err(DocumentZkError::InvalidStateTransition {
                message: "Document already exists".to_string(),
            });
        }

        // Calculate previous state hash
        let previous_state_hash = if self.state_transitions.is_empty() {
            None
        } else {
            Some(self.calculate_state_commitment())
        };

        // Add document to state
        self.documents
            .insert(document.document_id, document.clone());
        self.total_documents += 1;
        self.last_transition_timestamp = timestamp;

        // Calculate new state hash
        let new_state_hash = self.calculate_state_commitment();

        // Create state transition
        let transition_id = self.generate_transition_id(&document.document_id, timestamp);
        let state_transition = DocumentStateTransition {
            transition_id,
            document_id: document.document_id,
            company_id: document.company_id,
            transition_type: StateTransitionType::DocumentUpload,
            previous_state_hash,
            new_state_hash,
            created_by: uploaded_by,
            created_at: timestamp,
        };

        // Add transition to history
        self.state_transitions.push(state_transition.clone());

        // Update merkle root (simplified - in real implementation would use proper Merkle tree)
        self.merkle_root = new_state_hash;

        Ok(state_transition)
    }

    /// Process document approval state transition
    pub fn process_document_approval(
        &mut self,
        document_id: [u8; 32],
        approved_by: [u8; 32],
        timestamp: u64,
    ) -> Result<DocumentStateTransition, DocumentZkError> {
        // Check that document exists first
        if !self.documents.contains_key(&document_id) {
            return Err(DocumentZkError::InvalidStateTransition {
                message: "Document not found".to_string(),
            });
        }

        // Get company_id first (before mutable borrow)
        let company_id = self.documents.get(&document_id).unwrap().company_id;

        // Calculate previous state hash
        let previous_state_hash = Some(self.calculate_state_commitment());

        // Get document and update approval
        let document = self.documents.get_mut(&document_id).unwrap();
        document.approve(approved_by, timestamp);
        self.last_transition_timestamp = timestamp;

        // Calculate new state hash
        let new_state_hash = self.calculate_state_commitment();

        // Create state transition
        let transition_id = self.generate_transition_id(&document_id, timestamp);
        let state_transition = DocumentStateTransition {
            transition_id,
            document_id,
            company_id,
            transition_type: StateTransitionType::DocumentApproval,
            previous_state_hash,
            new_state_hash,
            created_by: approved_by,
            created_at: timestamp,
        };

        // Add transition to history
        self.state_transitions.push(state_transition.clone());

        // Update merkle root
        self.merkle_root = new_state_hash;

        Ok(state_transition)
    }

    /// Process document deletion state transition
    pub fn process_document_deletion(
        &mut self,
        document_id: [u8; 32],
        deleted_by: [u8; 32],
        timestamp: u64,
    ) -> Result<DocumentStateTransition, DocumentZkError> {
        // Check that document exists first
        if !self.documents.contains_key(&document_id) {
            return Err(DocumentZkError::InvalidStateTransition {
                message: "Document not found".to_string(),
            });
        }

        // Get company_id first (before mutable borrow)
        let company_id = self.documents.get(&document_id).unwrap().company_id;

        // Calculate previous state hash
        let previous_state_hash = Some(self.calculate_state_commitment());

        // Get document and update status
        let document = self.documents.get_mut(&document_id).unwrap();
        document.delete();
        self.last_transition_timestamp = timestamp;

        // Calculate new state hash
        let new_state_hash = self.calculate_state_commitment();

        // Create state transition
        let transition_id = self.generate_transition_id(&document_id, timestamp);
        let state_transition = DocumentStateTransition {
            transition_id,
            document_id,
            company_id,
            transition_type: StateTransitionType::DocumentDeletion,
            previous_state_hash,
            new_state_hash,
            created_by: deleted_by,
            created_at: timestamp,
        };

        // Add transition to history
        self.state_transitions.push(state_transition.clone());

        // Update merkle root
        self.merkle_root = new_state_hash;

        Ok(state_transition)
    }

    /// Generate inclusion proof for document
    pub fn generate_document_inclusion_proof(
        &self,
        document_id: &[u8; 32],
    ) -> Result<DocumentInclusionProof, DocumentZkError> {
        let document = self.documents.get(document_id).ok_or_else(|| {
            DocumentZkError::InvalidStateTransition {
                message: "Document not found".to_string(),
            }
        })?;

        // In a real implementation, this would generate a proper Merkle proof
        // For now, we'll create a simplified proof structure
        let proof_path = self.generate_merkle_proof_path(document_id)?;

        Ok(DocumentInclusionProof {
            document_id: *document_id,
            document_hash: document.sha256_hash,
            merkle_root: self.merkle_root,
            proof_path,
            state_commitment: self.calculate_state_commitment(),
        })
    }

    /// Verify document inclusion proof
    pub fn verify_document_inclusion_proof(
        &self,
        proof: &DocumentInclusionProof,
    ) -> Result<bool, DocumentZkError> {
        // Verify merkle root matches
        if proof.merkle_root != self.merkle_root {
            return Ok(false);
        }

        // Verify state commitment
        if proof.state_commitment != self.calculate_state_commitment() {
            return Ok(false);
        }

        // Verify document exists with correct hash
        if let Some(document) = self.documents.get(&proof.document_id) {
            Ok(document.sha256_hash == proof.document_hash)
        } else {
            Ok(false)
        }
    }

    /// Get document by ID
    #[must_use]
    pub fn get_document(&self, document_id: &[u8; 32]) -> Option<&Document> {
        self.documents.get(document_id)
    }

    /// Get all state transitions for a document
    #[must_use]
    pub fn get_document_transitions(
        &self,
        document_id: &[u8; 32],
    ) -> Vec<&DocumentStateTransition> {
        self.state_transitions
            .iter()
            .filter(|t| t.document_id == *document_id)
            .collect()
    }

    /// Get state transitions by type
    #[must_use]
    pub fn get_transitions_by_type(
        &self,
        transition_type: StateTransitionType,
    ) -> Vec<&DocumentStateTransition> {
        self.state_transitions
            .iter()
            .filter(|t| t.transition_type == transition_type)
            .collect()
    }

    // Helper methods

    /// Generate transition ID
    fn generate_transition_id(&self, document_id: &[u8; 32], timestamp: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(document_id);
        hasher.update(&timestamp.to_le_bytes());
        hasher.update(&self.state_transitions.len().to_le_bytes());
        hasher.update(b"document_transition");
        hasher.finalize().into()
    }

    /// Generate Merkle proof path (simplified)
    fn generate_merkle_proof_path(
        &self,
        _document_id: &[u8; 32],
    ) -> Result<Vec<[u8; 32]>, DocumentZkError> {
        // In a real implementation, this would generate the actual Merkle proof path
        // For now, return a placeholder path
        Ok(vec![self.merkle_root])
    }
}

impl Default for DocumentZkState {
    fn default() -> Self {
        Self::new()
    }
}

/// Document inclusion proof for ZK verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentInclusionProof {
    pub document_id: [u8; 32],
    pub document_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub proof_path: Vec<[u8; 32]>,
    pub state_commitment: [u8; 32],
}

/// Document batch for ZK proof generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentBatch {
    pub batch_id: [u8; 32],
    pub company_id: [u8; 32],
    pub state_transitions: Vec<DocumentStateTransition>,
    pub previous_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub batch_timestamp: u64,
    pub created_by: [u8; 32],
}

impl DocumentBatch {
    /// Create a new document batch
    #[must_use]
    pub fn new(
        company_id: [u8; 32],
        state_transitions: Vec<DocumentStateTransition>,
        previous_state_root: [u8; 32],
        new_state_root: [u8; 32],
        created_by: [u8; 32],
        timestamp: u64,
    ) -> Self {
        let batch_id = Self::calculate_batch_id(&state_transitions, timestamp);

        Self {
            batch_id,
            company_id,
            state_transitions,
            previous_state_root,
            new_state_root,
            batch_timestamp: timestamp,
            created_by,
        }
    }

    /// Calculate batch ID
    fn calculate_batch_id(
        state_transitions: &[DocumentStateTransition],
        timestamp: u64,
    ) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Hash all transition IDs
        for transition in state_transitions {
            hasher.update(&transition.transition_id);
        }

        hasher.update(&timestamp.to_le_bytes());
        hasher.update(b"document_batch");
        hasher.finalize().into()
    }

    /// Verify batch integrity
    pub fn verify_batch_integrity(&self) -> Result<bool, DocumentZkError> {
        // Verify batch ID
        let calculated_batch_id =
            Self::calculate_batch_id(&self.state_transitions, self.batch_timestamp);
        if calculated_batch_id != self.batch_id {
            return Ok(false);
        }

        // Verify state transition chain
        let mut current_state = self.previous_state_root;
        for transition in &self.state_transitions {
            if let Some(prev_hash) = transition.previous_state_hash {
                if prev_hash != current_state {
                    return Ok(false);
                }
            }
            current_state = transition.new_state_hash;
        }

        // Verify final state matches
        Ok(current_state == self.new_state_root)
    }

    /// Get transition count
    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.state_transitions.len()
    }

    /// Get transitions by type
    #[must_use]
    pub fn get_transitions_by_type(
        &self,
        transition_type: StateTransitionType,
    ) -> Vec<&DocumentStateTransition> {
        self.state_transitions
            .iter()
            .filter(|t| t.transition_type == transition_type)
            .collect()
    }
}

/// Document ZK proof public values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProofPublicValues {
    pub previous_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub company_id: [u8; 32],
    pub batch_id: [u8; 32],
    pub transition_count: u32,
    pub batch_timestamp: u64,
    pub transition_types_hash: [u8; 32], // Hash of all transition types for transparency
}

impl DocumentProofPublicValues {
    /// Create public values from document batch
    #[must_use]
    pub fn from_batch(batch: &DocumentBatch) -> Self {
        let transition_types_hash = Self::calculate_transition_types_hash(&batch.state_transitions);

        Self {
            previous_state_root: batch.previous_state_root,
            new_state_root: batch.new_state_root,
            company_id: batch.company_id,
            batch_id: batch.batch_id,
            transition_count: batch.state_transitions.len() as u32,
            batch_timestamp: batch.batch_timestamp,
            transition_types_hash,
        }
    }

    /// Calculate hash of transition types
    fn calculate_transition_types_hash(transitions: &[DocumentStateTransition]) -> [u8; 32] {
        let mut hasher = Sha256::new();

        for transition in transitions {
            hasher.update(&[transition.transition_type as u8]);
        }

        hasher.finalize().into()
    }
}

/// Integration with existing ZkVMAds system
impl From<DocumentStateTransition> for StateTransition {
    fn from(doc_transition: DocumentStateTransition) -> Self {
        StateTransition {
            old_root: doc_transition.previous_state_hash.unwrap_or([0u8; 32]),
            new_root: doc_transition.new_state_hash,
            nullifiers_inserted: vec![doc_transition
                .transition_id
                .iter()
                .take(8)
                .enumerate()
                .map(|(i, &b)| (b as i64) << (i * 8))
                .sum()],
        }
    }
}

/// Convert document operations to loan transactions for ZK processing
pub fn document_operation_to_loan_transaction(
    document_id: [u8; 32],
    company_id: [u8; 32],
    user_id: [u8; 32],
    operation: DocumentAction,
    timestamp: u64,
) -> LoanTransaction {
    let transaction_type = match operation {
        DocumentAction::Upload => TransactionType::UserRegistration, // Reuse existing type
        DocumentAction::Approve => TransactionType::KycVerification,
        DocumentAction::Delete => TransactionType::DealCloseout,  // was LoanCloseout
        _ => TransactionType::UserRegistration,
    };

    LoanTransaction {
        transaction_id: document_id,
        user_id,
        deal_id: company_id, // Use company_id as deal_id for document operations
        transaction_type,
        amount: 0, // Documents don't have monetary amounts
        created_at: timestamp,
        metadata: crate::transactions::MetadataArray::from_bytes(&[operation as u8]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{AccessLevel, ApprovalStatus, DocumentStatus, DocumentType};

    fn create_test_document() -> Document {
        let document_id = [1u8; 32];
        let company_id = [2u8; 32];
        let uploaded_by = [3u8; 32];
        let content = b"test document content";

        Document::new(
            document_id,
            company_id,
            DocumentType::LoanContract,
            content,
            uploaded_by,
            1_234_567_890,
        )
    }

    #[test]
    fn test_document_zk_state_creation() {
        let state = DocumentZkState::new();
        assert_eq!(state.total_documents, 0);
        assert_eq!(state.documents.len(), 0);
        assert_eq!(state.state_transitions.len(), 0);
    }

    #[test]
    fn test_document_upload_state_transition() {
        let mut state = DocumentZkState::new();
        let document = create_test_document();
        let uploaded_by = [3u8; 32];
        let timestamp = 1_234_567_890;

        let transition = state
            .process_document_upload(document.clone(), uploaded_by, timestamp)
            .unwrap();

        assert_eq!(
            transition.transition_type,
            StateTransitionType::DocumentUpload
        );
        assert_eq!(transition.document_id, document.document_id);
        assert_eq!(transition.created_by, uploaded_by);
        assert_eq!(state.total_documents, 1);
        assert!(state.documents.contains_key(&document.document_id));
    }

    #[test]
    fn test_document_approval_state_transition() {
        let mut state = DocumentZkState::new();
        let document = create_test_document();
        let uploaded_by = [3u8; 32];
        let approved_by = [4u8; 32];
        let timestamp = 1_234_567_890;

        // First upload the document
        state
            .process_document_upload(document.clone(), uploaded_by, timestamp)
            .unwrap();

        // Then approve it
        let transition = state
            .process_document_approval(document.document_id, approved_by, timestamp + 100)
            .unwrap();

        assert_eq!(
            transition.transition_type,
            StateTransitionType::DocumentApproval
        );
        assert_eq!(transition.created_by, approved_by);

        let stored_document = state.get_document(&document.document_id).unwrap();
        assert!(stored_document.is_approved());
    }

    #[test]
    fn test_state_commitment_calculation() {
        let mut state = DocumentZkState::new();
        let initial_commitment = state.calculate_state_commitment();

        let document = create_test_document();
        state
            .process_document_upload(document, [3u8; 32], 1_234_567_890)
            .unwrap();

        let new_commitment = state.calculate_state_commitment();
        assert_ne!(initial_commitment, new_commitment);
    }

    #[test]
    fn test_document_inclusion_proof() {
        let mut state = DocumentZkState::new();
        let document = create_test_document();

        state
            .process_document_upload(document.clone(), [3u8; 32], 1_234_567_890)
            .unwrap();

        let proof = state
            .generate_document_inclusion_proof(&document.document_id)
            .unwrap();

        assert_eq!(proof.document_id, document.document_id);
        assert_eq!(proof.document_hash, document.sha256_hash);

        let is_valid = state.verify_document_inclusion_proof(&proof).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_document_batch_creation() {
        let company_id = [2u8; 32];
        let created_by = [3u8; 32];
        let timestamp = 1_234_567_890;

        let transition = DocumentStateTransition {
            transition_id: [1u8; 32],
            document_id: [1u8; 32],
            company_id,
            transition_type: StateTransitionType::DocumentUpload,
            previous_state_hash: None,
            new_state_hash: [4u8; 32],
            created_by,
            created_at: timestamp,
        };

        let batch = DocumentBatch::new(
            company_id,
            vec![transition],
            [0u8; 32],
            [4u8; 32],
            created_by,
            timestamp,
        );

        assert_eq!(batch.company_id, company_id);
        assert_eq!(batch.state_transitions.len(), 1);
        assert_eq!(batch.transition_count(), 1);

        let is_valid = batch.verify_batch_integrity().unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_document_proof_public_values() {
        let company_id = [2u8; 32];
        let created_by = [3u8; 32];
        let timestamp = 1_234_567_890;

        let transition = DocumentStateTransition {
            transition_id: [1u8; 32],
            document_id: [1u8; 32],
            company_id,
            transition_type: StateTransitionType::DocumentUpload,
            previous_state_hash: None,
            new_state_hash: [4u8; 32],
            created_by,
            created_at: timestamp,
        };

        let batch = DocumentBatch::new(
            company_id,
            vec![transition],
            [0u8; 32],
            [4u8; 32],
            created_by,
            timestamp,
        );

        let public_values = DocumentProofPublicValues::from_batch(&batch);

        assert_eq!(public_values.company_id, company_id);
        assert_eq!(public_values.transition_count, 1);
        assert_eq!(public_values.batch_timestamp, timestamp);
        assert_eq!(public_values.previous_state_root, [0u8; 32]);
        assert_eq!(public_values.new_state_root, [4u8; 32]);
    }
}
