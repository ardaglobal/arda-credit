//! Document storage and management types for zkVM execution
//!
//! This module contains document-related types and operations for the company-based
//! document storage system with role-based access controls and ZK proof integration.

use crate::transactions::{LoanTransaction, TransactionType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Company types in the document system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompanyType {
    Originator = 1,
    Investor = 2,
}

/// KYB (Know Your Business) status for company compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KybStatus {
    NotSubmitted = 0,
    Pending = 1,
    Approved = 2,
    Rejected = 3,
}

/// Team member roles within companies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TeamRole {
    Admin = 1,  // Full document access, upload, download, delete
    Write = 2,  // Upload, download, approve documents, no deletion
    Read = 3,   // View and download only, no upload capabilities
}

/// Document types supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DocumentType {
    LoanContract = 1,
    FinancialStatement = 2,
    KycDocument = 3,
    LegalDocument = 4,
    Other = 5,
}

/// Document access levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AccessLevel {
    Public = 1,
    Company = 2,
    Deal = 3,
    Restricted = 4,
}

/// Document approval status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ApprovalStatus {
    Pending = 1,
    Approved = 2,
    Rejected = 3,
}

/// Document status in lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DocumentStatus {
    Active = 1,
    Archived = 2,
    Deleted = 3,
}

/// Company entity (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub company_id: [u8; 32],        // Hashed UUID for privacy
    pub company_type: CompanyType,
    pub kyb_status: KybStatus,
    pub kyb_submitted_at: u64,       // Unix timestamp, 0 if not submitted
    pub kyb_verified_at: u64,        // Unix timestamp, 0 if not verified
    pub document_retention_days: u32, // Document retention policy
    pub created_at: u64,             // Unix timestamp
}

impl Company {
    /// Check if company is KYB approved
    #[must_use]
    pub const fn is_kyb_approved(&self) -> bool {
        matches!(self.kyb_status, KybStatus::Approved)
    }

    /// Check if company can store documents (KYB approved)
    #[must_use]
    pub const fn can_store_documents(&self) -> bool {
        self.is_kyb_approved()
    }
}

/// Team member within a company (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    pub member_id: [u8; 32],         // Hashed UUID for privacy
    pub company_id: [u8; 32],        // Hashed company UUID
    pub user_id: [u8; 32],           // Hashed user UUID
    pub role: TeamRole,
    pub can_upload_documents: bool,
    pub can_download_documents: bool,
    pub can_delete_documents: bool,
    pub can_approve_documents: bool,
    pub can_manage_team: bool,
    pub can_view_audit_logs: bool,
    pub joined_at: u64,              // Unix timestamp
    pub last_active_at: u64,         // Unix timestamp
}

impl TeamMember {
    /// Create a new team member with role-based permissions
    #[must_use]
    pub fn new(
        member_id: [u8; 32],
        company_id: [u8; 32],
        user_id: [u8; 32],
        role: TeamRole,
        joined_at: u64,
    ) -> Self {
        let (
            can_upload_documents,
            can_download_documents,
            can_delete_documents,
            can_approve_documents,
            can_manage_team,
            can_view_audit_logs,
        ) = match role {
            TeamRole::Admin => (true, true, true, true, true, true),
            TeamRole::Write => (true, true, false, true, false, false),
            TeamRole::Read => (false, true, false, false, false, false),
        };

        Self {
            member_id,
            company_id,
            user_id,
            role,
            can_upload_documents,
            can_download_documents,
            can_delete_documents,
            can_approve_documents,
            can_manage_team,
            can_view_audit_logs,
            joined_at,
            last_active_at: joined_at,
        }
    }

    /// Check if member can perform a specific action
    #[must_use]
    pub const fn can_perform_action(&self, action: &DocumentAction) -> bool {
        match action {
            DocumentAction::Upload => self.can_upload_documents,
            DocumentAction::Download | DocumentAction::View => self.can_download_documents,
            DocumentAction::Delete => self.can_delete_documents,
            DocumentAction::Approve | DocumentAction::Reject => self.can_approve_documents,
            DocumentAction::ManageTeam => self.can_manage_team,
            DocumentAction::ViewAuditLogs => self.can_view_audit_logs,
        }
    }
}

/// Document actions for permission checking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentAction {
    Upload,
    Download,
    View,
    Delete,
    Approve,
    Reject,
    ManageTeam,
    ViewAuditLogs,
}

/// Document metadata (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub document_id: [u8; 32],       // Hashed UUID for privacy
    pub company_id: [u8; 32],        // Hashed company UUID
    pub deal_id: Option<[u8; 32]>,   // Optional hashed deal UUID
    pub document_type: DocumentType,
    pub file_size_bytes: u64,
    pub sha256_hash: [u8; 32],       // SHA-256 hash for integrity
    pub document_fingerprint: [u8; 32], // Additional fingerprint for duplicate detection
    pub access_level: AccessLevel,
    pub approval_status: ApprovalStatus,
    pub status: DocumentStatus,
    pub version: u32,
    pub parent_document_id: Option<[u8; 32]>, // For document revisions
    pub uploaded_by: [u8; 32],       // Hashed user UUID
    pub approved_by: Option<[u8; 32]>, // Hashed user UUID
    pub created_at: u64,             // Unix timestamp
    pub approved_at: u64,            // Unix timestamp, 0 if not approved
    pub expires_at: u64,             // Unix timestamp, 0 if no expiration
}

impl Document {
    /// Create a new document
    #[must_use]
    pub fn new(
        document_id: [u8; 32],
        company_id: [u8; 32],
        document_type: DocumentType,
        file_content: &[u8],
        uploaded_by: [u8; 32],
        created_at: u64,
    ) -> Self {
        let sha256_hash = Self::calculate_sha256_hash(file_content);
        let document_fingerprint = Self::calculate_document_fingerprint(file_content, &document_id);

        Self {
            document_id,
            company_id,
            deal_id: None,
            document_type,
            file_size_bytes: file_content.len() as u64,
            sha256_hash,
            document_fingerprint,
            access_level: AccessLevel::Company,
            approval_status: ApprovalStatus::Pending,
            status: DocumentStatus::Active,
            version: 1,
            parent_document_id: None,
            uploaded_by,
            approved_by: None,
            created_at,
            approved_at: 0,
            expires_at: 0,
        }
    }

    /// Calculate SHA-256 hash of document content
    #[must_use]
    pub fn calculate_sha256_hash(content: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.finalize().into()
    }

    /// Calculate document fingerprint for duplicate detection
    #[must_use]
    pub fn calculate_document_fingerprint(content: &[u8], document_id: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.update(document_id);
        hasher.update(b"arda_document_fingerprint");
        hasher.finalize().into()
    }

    /// Verify document integrity against stored hash
    #[must_use]
    pub fn verify_integrity(&self, content: &[u8]) -> bool {
        let calculated_hash = Self::calculate_sha256_hash(content);
        calculated_hash == self.sha256_hash
    }

    /// Check if document is approved
    #[must_use]
    pub const fn is_approved(&self) -> bool {
        matches!(self.approval_status, ApprovalStatus::Approved)
    }

    /// Check if document is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.status, DocumentStatus::Active)
    }

    /// Check if document has expired
    #[must_use]
    pub fn is_expired(&self, current_time: u64) -> bool {
        self.expires_at > 0 && current_time > self.expires_at
    }

    /// Approve document
    pub fn approve(&mut self, approved_by: [u8; 32], approved_at: u64) {
        self.approval_status = ApprovalStatus::Approved;
        self.approved_by = Some(approved_by);
        self.approved_at = approved_at;
    }

    /// Reject document
    pub fn reject(&mut self) {
        self.approval_status = ApprovalStatus::Rejected;
    }

    /// Archive document
    pub fn archive(&mut self) {
        self.status = DocumentStatus::Archived;
    }

    /// Delete document (soft delete)
    pub fn delete(&mut self) {
        self.status = DocumentStatus::Deleted;
    }
}

/// Document audit log entry (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentAuditLog {
    pub log_id: [u8; 32],            // Hashed UUID for privacy
    pub document_id: [u8; 32],       // Hashed document UUID
    pub company_id: [u8; 32],        // Hashed company UUID
    pub user_id: [u8; 32],           // Hashed user UUID
    pub action: DocumentAction,
    pub result: AuditResult,
    pub created_at: u64,             // Unix timestamp
}

/// Audit result types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AuditResult {
    Success = 1,
    Failure = 2,
    Denied = 3,
}

/// Document state transition for ZK proofs (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentStateTransition {
    pub transition_id: [u8; 32],     // Hashed UUID for privacy
    pub document_id: [u8; 32],       // Hashed document UUID
    pub company_id: [u8; 32],        // Hashed company UUID
    pub transition_type: StateTransitionType,
    pub previous_state_hash: Option<[u8; 32]>,
    pub new_state_hash: [u8; 32],
    pub created_by: [u8; 32],        // Hashed user UUID
    pub created_at: u64,             // Unix timestamp
}

/// State transition types for ZK proofs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum StateTransitionType {
    DocumentUpload = 1,
    DocumentApproval = 2,
    DocumentDeletion = 3,
    AccessGrant = 4,
    AccessRevoke = 5,
}

/// Errors specific to document operations
#[derive(Error, Debug, Clone)]
pub enum DocumentError {
    #[error("Document not found: {document_id:?}")]
    DocumentNotFound { document_id: [u8; 32] },

    #[error("Company not found: {company_id:?}")]
    CompanyNotFound { company_id: [u8; 32] },

    #[error("Team member not found: {user_id:?} in company {company_id:?}")]
    TeamMemberNotFound {
        user_id: [u8; 32],
        company_id: [u8; 32],
    },

    #[error("Access denied: user {user_id:?} cannot perform {action:?} on document {document_id:?}")]
    AccessDenied {
        user_id: [u8; 32],
        document_id: [u8; 32],
        action: DocumentAction,
    },

    #[error("KYB not approved for company: {company_id:?}")]
    KybNotApproved { company_id: [u8; 32] },

    #[error("Document integrity verification failed: {document_id:?}")]
    IntegrityVerificationFailed { document_id: [u8; 32] },

    #[error("Document already exists: {sha256_hash:?} in company {company_id:?}")]
    DuplicateDocument {
        sha256_hash: [u8; 32],
        company_id: [u8; 32],
    },

    #[error("Invalid document type for operation")]
    InvalidDocumentType,

    #[error("Document has expired: {document_id:?}")]
    DocumentExpired { document_id: [u8; 32] },

    #[error("Invalid file type: {mime_type}")]
    InvalidFileType { mime_type: String },

    #[error("File size exceeds limit: {size} bytes")]
    FileSizeExceeded { size: u64 },
}

/// Document storage state management
pub struct DocumentState {
    pub companies: HashMap<[u8; 32], Company>,
    pub team_members: HashMap<[u8; 32], TeamMember>,
    pub documents: HashMap<[u8; 32], Document>,
    pub audit_logs: Vec<DocumentAuditLog>,
    pub state_transitions: Vec<DocumentStateTransition>,
}

impl DocumentState {
    /// Create a new empty document state
    #[must_use]
    pub fn new() -> Self {
        Self {
            companies: HashMap::new(),
            team_members: HashMap::new(),
            documents: HashMap::new(),
            audit_logs: Vec::new(),
            state_transitions: Vec::new(),
        }
    }

    /// Register a new company
    pub fn register_company(
        &mut self,
        company_id: [u8; 32],
        company_type: CompanyType,
        created_at: u64,
    ) -> Result<(), DocumentError> {
        if self.companies.contains_key(&company_id) {
            return Ok(()); // Company already exists
        }

        let company = Company {
            company_id,
            company_type,
            kyb_status: KybStatus::NotSubmitted,
            kyb_submitted_at: 0,
            kyb_verified_at: 0,
            document_retention_days: 2555, // ~7 years default
            created_at,
        };

        self.companies.insert(company_id, company);
        Ok(())
    }

    /// Add team member to company
    pub fn add_team_member(
        &mut self,
        member_id: [u8; 32],
        company_id: [u8; 32],
        user_id: [u8; 32],
        role: TeamRole,
        joined_at: u64,
    ) -> Result<(), DocumentError> {
        // Verify company exists
        if !self.companies.contains_key(&company_id) {
            return Err(DocumentError::CompanyNotFound { company_id });
        }

        let team_member = TeamMember::new(member_id, company_id, user_id, role, joined_at);
        self.team_members.insert(member_id, team_member);
        Ok(())
    }

    /// Upload a new document
    pub fn upload_document(
        &mut self,
        document_id: [u8; 32],
        company_id: [u8; 32],
        document_type: DocumentType,
        file_content: &[u8],
        uploaded_by: [u8; 32],
        created_at: u64,
    ) -> Result<(), DocumentError> {
        // Verify company exists and is KYB approved
        let company = self
            .companies
            .get(&company_id)
            .ok_or(DocumentError::CompanyNotFound { company_id })?;

        if !company.can_store_documents() {
            return Err(DocumentError::KybNotApproved { company_id });
        }

        // Verify user has upload permissions
        let team_member = self
            .team_members
            .values()
            .find(|tm| tm.company_id == company_id && tm.user_id == uploaded_by)
            .ok_or(DocumentError::TeamMemberNotFound {
                user_id: uploaded_by,
                company_id,
            })?;

        if !team_member.can_perform_action(&DocumentAction::Upload) {
            return Err(DocumentError::AccessDenied {
                user_id: uploaded_by,
                document_id,
                action: DocumentAction::Upload,
            });
        }

        // Check for duplicate documents
        let sha256_hash = Document::calculate_sha256_hash(file_content);
        if self
            .documents
            .values()
            .any(|doc| doc.company_id == company_id && doc.sha256_hash == sha256_hash)
        {
            return Err(DocumentError::DuplicateDocument {
                sha256_hash,
                company_id,
            });
        }

        // Create and store document
        let document = Document::new(
            document_id,
            company_id,
            document_type,
            file_content,
            uploaded_by,
            created_at,
        );

        self.documents.insert(document_id, document);

        // Log the upload action
        self.log_document_action(
            document_id,
            company_id,
            uploaded_by,
            DocumentAction::Upload,
            AuditResult::Success,
            created_at,
        );

        Ok(())
    }

    /// Approve a document
    pub fn approve_document(
        &mut self,
        document_id: [u8; 32],
        approved_by: [u8; 32],
        approved_at: u64,
    ) -> Result<(), DocumentError> {
        let document = self
            .documents
            .get_mut(&document_id)
            .ok_or(DocumentError::DocumentNotFound { document_id })?;

        let company_id = document.company_id;

        // Verify user has approval permissions
        let team_member = self
            .team_members
            .values()
            .find(|tm| tm.company_id == company_id && tm.user_id == approved_by)
            .ok_or(DocumentError::TeamMemberNotFound {
                user_id: approved_by,
                company_id,
            })?;

        if !team_member.can_perform_action(&DocumentAction::Approve) {
            return Err(DocumentError::AccessDenied {
                user_id: approved_by,
                document_id,
                action: DocumentAction::Approve,
            });
        }

        document.approve(approved_by, approved_at);

        // Log the approval action
        self.log_document_action(
            document_id,
            company_id,
            approved_by,
            DocumentAction::Approve,
            AuditResult::Success,
            approved_at,
        );

        Ok(())
    }

    /// Check if user can access document
    pub fn can_access_document(
        &self,
        user_id: [u8; 32],
        document_id: [u8; 32],
        action: DocumentAction,
    ) -> Result<bool, DocumentError> {
        let document = self
            .documents
            .get(&document_id)
            .ok_or(DocumentError::DocumentNotFound { document_id })?;

        let company_id = document.company_id;

        // Find team member
        let team_member = self
            .team_members
            .values()
            .find(|tm| tm.company_id == company_id && tm.user_id == user_id)
            .ok_or(DocumentError::TeamMemberNotFound {
                user_id,
                company_id,
            })?;

        Ok(team_member.can_perform_action(&action))
    }

    /// Log document action for audit trail
    fn log_document_action(
        &mut self,
        document_id: [u8; 32],
        company_id: [u8; 32],
        user_id: [u8; 32],
        action: DocumentAction,
        result: AuditResult,
        created_at: u64,
    ) {
        let log_id = {
            let mut hasher = Sha256::new();
            hasher.update(&document_id);
            hasher.update(&user_id);
            hasher.update(&created_at.to_le_bytes());
            hasher.update(b"audit_log");
            hasher.finalize().into()
        };

        let audit_log = DocumentAuditLog {
            log_id,
            document_id,
            company_id,
            user_id,
            action,
            result,
            created_at,
        };

        self.audit_logs.push(audit_log);
    }

    /// Get company by ID
    #[must_use]
    pub fn get_company(&self, company_id: &[u8; 32]) -> Option<&Company> {
        self.companies.get(company_id)
    }

    /// Get document by ID
    #[must_use]
    pub fn get_document(&self, document_id: &[u8; 32]) -> Option<&Document> {
        self.documents.get(document_id)
    }

    /// Get team member by user and company
    #[must_use]
    pub fn get_team_member(
        &self,
        user_id: &[u8; 32],
        company_id: &[u8; 32],
    ) -> Option<&TeamMember> {
        self.team_members
            .values()
            .find(|tm| tm.user_id == *user_id && tm.company_id == *company_id)
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_company_id() -> [u8; 32] {
        [1u8; 32]
    }

    fn create_test_user_id() -> [u8; 32] {
        [2u8; 32]
    }

    fn create_test_document_id() -> [u8; 32] {
        [3u8; 32]
    }

    fn create_test_member_id() -> [u8; 32] {
        [4u8; 32]
    }

    #[test]
    fn test_company_registration() {
        let mut state = DocumentState::new();
        let company_id = create_test_company_id();

        let result = state.register_company(company_id, CompanyType::Originator, 1_234_567_890);
        assert!(result.is_ok());

        let company = state.get_company(&company_id).unwrap();
        assert_eq!(company.company_type, CompanyType::Originator);
        assert_eq!(company.kyb_status, KybStatus::NotSubmitted);
    }

    #[test]
    fn test_team_member_permissions() {
        let admin = TeamMember::new(
            create_test_member_id(),
            create_test_company_id(),
            create_test_user_id(),
            TeamRole::Admin,
            1_234_567_890,
        );

        assert!(admin.can_perform_action(&DocumentAction::Upload));
        assert!(admin.can_perform_action(&DocumentAction::Download));
        assert!(admin.can_perform_action(&DocumentAction::Delete));
        assert!(admin.can_perform_action(&DocumentAction::Approve));

        let read_only = TeamMember::new(
            create_test_member_id(),
            create_test_company_id(),
            create_test_user_id(),
            TeamRole::Read,
            1_234_567_890,
        );

        assert!(!read_only.can_perform_action(&DocumentAction::Upload));
        assert!(read_only.can_perform_action(&DocumentAction::Download));
        assert!(!read_only.can_perform_action(&DocumentAction::Delete));
        assert!(!read_only.can_perform_action(&DocumentAction::Approve));
    }

    #[test]
    fn test_document_creation_and_integrity() {
        let document_id = create_test_document_id();
        let company_id = create_test_company_id();
        let user_id = create_test_user_id();
        let content = b"test document content";

        let document = Document::new(
            document_id,
            company_id,
            DocumentType::LoanContract,
            content,
            user_id,
            1_234_567_890,
        );

        assert_eq!(document.document_type, DocumentType::LoanContract);
        assert_eq!(document.file_size_bytes, content.len() as u64);
        assert!(document.verify_integrity(content));
        assert!(!document.verify_integrity(b"different content"));
    }

    #[test]
    fn test_document_upload_with_permissions() {
        let mut state = DocumentState::new();
        let company_id = create_test_company_id();
        let user_id = create_test_user_id();
        let document_id = create_test_document_id();
        let member_id = create_test_member_id();

        // Register company
        state
            .register_company(company_id, CompanyType::Originator, 1_234_567_890)
            .unwrap();

        // Approve company KYB
        let company = state.companies.get_mut(&company_id).unwrap();
        company.kyb_status = KybStatus::Approved;
        company.kyb_verified_at = 1_234_567_890;

        // Add team member with write permissions
        state
            .add_team_member(member_id, company_id, user_id, TeamRole::Write, 1_234_567_890)
            .unwrap();

        // Upload document
        let content = b"test loan contract content";
        let result = state.upload_document(
            document_id,
            company_id,
            DocumentType::LoanContract,
            content,
            user_id,
            1_234_567_900,
        );

        assert!(result.is_ok());

        let document = state.get_document(&document_id).unwrap();
        assert_eq!(document.document_type, DocumentType::LoanContract);
        assert_eq!(document.approval_status, ApprovalStatus::Pending);
    }

    #[test]
    fn test_document_approval() {
        let mut state = DocumentState::new();
        let company_id = create_test_company_id();
        let user_id = create_test_user_id();
        let document_id = create_test_document_id();
        let member_id = create_test_member_id();

        // Setup company and team member
        state
            .register_company(company_id, CompanyType::Originator, 1_234_567_890)
            .unwrap();

        let company = state.companies.get_mut(&company_id).unwrap();
        company.kyb_status = KybStatus::Approved;

        state
            .add_team_member(member_id, company_id, user_id, TeamRole::Admin, 1_234_567_890)
            .unwrap();

        // Upload document
        let content = b"test document";
        state
            .upload_document(
                document_id,
                company_id,
                DocumentType::LoanContract,
                content,
                user_id,
                1_234_567_900,
            )
            .unwrap();

        // Approve document
        let result = state.approve_document(document_id, user_id, 1_234_567_910);
        assert!(result.is_ok());

        let document = state.get_document(&document_id).unwrap();
        assert!(document.is_approved());
        assert_eq!(document.approved_at, 1_234_567_910);
    }

    #[test]
    fn test_access_control() {
        let mut state = DocumentState::new();
        let company_id = create_test_company_id();
        let admin_user_id = create_test_user_id();
        let read_user_id = [5u8; 32];
        let document_id = create_test_document_id();

        // Setup company
        state
            .register_company(company_id, CompanyType::Originator, 1_234_567_890)
            .unwrap();

        let company = state.companies.get_mut(&company_id).unwrap();
        company.kyb_status = KybStatus::Approved;

        // Add admin and read-only users
        state
            .add_team_member([4u8; 32], company_id, admin_user_id, TeamRole::Admin, 1_234_567_890)
            .unwrap();

        state
            .add_team_member([5u8; 32], company_id, read_user_id, TeamRole::Read, 1_234_567_890)
            .unwrap();

        // Upload document as admin
        let content = b"test document";
        state
            .upload_document(
                document_id,
                company_id,
                DocumentType::LoanContract,
                content,
                admin_user_id,
                1_234_567_900,
            )
            .unwrap();

        // Test access permissions
        assert!(state
            .can_access_document(admin_user_id, document_id, DocumentAction::Delete)
            .unwrap());

        assert!(!state
            .can_access_document(read_user_id, document_id, DocumentAction::Delete)
            .unwrap());

        assert!(state
            .can_access_document(read_user_id, document_id, DocumentAction::Download)
            .unwrap());
    }
}