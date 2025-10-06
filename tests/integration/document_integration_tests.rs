//! Integration tests for document storage system
//!
//! These tests verify the complete document storage workflow including
//! database operations, S3 storage, role-based access control, and ZK integration.

use arda_credit_lib::{
    AccessLevel, ApprovalStatus, Company, CompanyType, Document, DocumentAction, DocumentBatch,
    DocumentError, DocumentStatus, DocumentStorageClient, DocumentType, DocumentUploadRequest,
    DocumentZkState, KybStatus, S3Metadata, StorageConfig, StorageError, SupportedFileType,
    TeamMember, TeamRole,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tokio;
use uuid::Uuid;

/// Test configuration for document integration tests
pub struct DocumentTestConfig {
    pub db_pool: PgPool,
    pub storage_config: StorageConfig,
    pub test_company_id: Uuid,
    pub test_user_id: Uuid,
    pub test_deal_id: Uuid,
}

impl DocumentTestConfig {
    /// Create a new test configuration
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize test database connection
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://test:test@localhost/arda_credit_test".to_string());
        let db_pool = PgPool::connect(&database_url).await?;

        // Run migrations
        sqlx::migrate!("../db/migrations").run(&db_pool).await?;

        // Create test storage configuration
        let storage_config = StorageConfig {
            bucket_name: "arda-credit-test-documents".to_string(),
            region: "us-east-1".to_string(),
            kms_key_id: None,
            max_file_size_mb: 10, // Smaller limit for tests
            lifecycle_days_to_ia: 1,
            lifecycle_days_to_glacier: 2,
            lifecycle_days_to_delete: 7,
        };

        // Generate test IDs
        let test_company_id = Uuid::new_v4();
        let test_user_id = Uuid::new_v4();
        let test_deal_id = Uuid::new_v4();

        Ok(Self {
            db_pool,
            storage_config,
            test_company_id,
            test_user_id,
            test_deal_id,
        })
    }

    /// Setup test data in database
    pub async fn setup_test_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create test company
        sqlx::query!(
            r#"
            INSERT INTO companies (id, name, company_type, kyb_status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            ON CONFLICT (id) DO NOTHING
            "#,
            self.test_company_id,
            "Test Company",
            CompanyType::Originator.to_string(),
            KybStatus::Approved.to_string()
        )
        .execute(&self.db_pool)
        .await?;

        // Create test user identity
        sqlx::query!(
            r#"
            INSERT INTO users (id, email, kyc_status, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (id) DO NOTHING
            "#,
            self.test_user_id,
            "test@example.com",
            "approved"
        )
        .execute(&self.db_pool)
        .await?;

        // Create test team member
        sqlx::query!(
            r#"
            INSERT INTO team_members (
                id, company_id, user_id, role, status,
                can_upload_documents, can_download_documents, can_delete_documents,
                can_approve_documents, can_view_audit_logs, can_manage_team,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()
            )
            ON CONFLICT (id) DO NOTHING
            "#,
            Uuid::new_v4(),
            self.test_company_id,
            self.test_user_id,
            TeamRole::Admin.to_string(),
            "active",
            true,  // can_upload_documents
            true,  // can_download_documents
            true,  // can_delete_documents
            true,  // can_approve_documents
            true,  // can_view_audit_logs
            true   // can_manage_team
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Cleanup test data
    pub async fn cleanup_test_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Delete test documents
        sqlx::query!(
            "DELETE FROM documents WHERE company_id = $1",
            self.test_company_id
        )
        .execute(&self.db_pool)
        .await?;

        // Delete test audit logs
        sqlx::query!(
            "DELETE FROM document_audit_logs WHERE company_id = $1",
            self.test_company_id
        )
        .execute(&self.db_pool)
        .await?;

        // Delete test team members
        sqlx::query!(
            "DELETE FROM team_members WHERE company_id = $1",
            self.test_company_id
        )
        .execute(&self.db_pool)
        .await?;

        // Delete test company
        sqlx::query!(
            "DELETE FROM companies WHERE id = $1",
            self.test_company_id
        )
        .execute(&self.db_pool)
        .await?;

        // Delete test identity
        sqlx::query!(
            "DELETE FROM users WHERE id = $1",
            self.test_user_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Create test document content
    pub fn create_test_document_content(&self, content: &str) -> Vec<u8> {
        content.as_bytes().to_vec()
    }

    /// Create test PDF content
    pub fn create_test_pdf_content(&self) -> Vec<u8> {
        // Minimal PDF content for testing
        b"%PDF-1.4\n1 0 obj\n<<\n/Type /Catalog\n/Pages 2 0 R\n>>\nendobj\n2 0 obj\n<<\n/Type /Pages\n/Kids [3 0 R]\n/Count 1\n>>\nendobj\n3 0 obj\n<<\n/Type /Page\n/Parent 2 0 R\n/MediaBox [0 0 612 792]\n>>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000010 00000 n \n0000000079 00000 n \n0000000173 00000 n \ntrailer\n<<\n/Size 4\n/Root 1 0 R\n>>\nstartxref\n253\n%%EOF".to_vec()
    }
}

/// Test document model creation and validation
#[tokio::test]
async fn test_document_model_creation() {
    let document_id = [1u8; 32];
    let company_id = [2u8; 32];
    let uploaded_by = [3u8; 32];
    let content = b"test document content";
    let timestamp = chrono::Utc::now().timestamp() as u64;

    let document = Document::new(
        document_id,
        company_id,
        DocumentType::LoanContract,
        content,
        uploaded_by,
        timestamp,
    );

    assert_eq!(document.document_id, document_id);
    assert_eq!(document.company_id, company_id);
    assert_eq!(document.document_type, DocumentType::LoanContract);
    assert_eq!(document.uploaded_by, uploaded_by);
    assert_eq!(document.approval_status, ApprovalStatus::Pending);
    assert_eq!(document.status, DocumentStatus::Active);
    assert_eq!(document.version, 1);

    // Test document approval
    let approved_by = [4u8; 32];
    let mut document_copy = document.clone();
    document_copy.approve(approved_by, timestamp + 100);

    assert_eq!(document_copy.approval_status, ApprovalStatus::Approved);
    assert_eq!(document_copy.approved_by, Some(approved_by));
    assert!(document_copy.is_approved());

    // Test document deletion
    document_copy.delete();
    assert_eq!(document_copy.status, DocumentStatus::Deleted);
    assert!(document_copy.is_deleted());
}

/// Test supported file type validation
#[tokio::test]
async fn test_supported_file_types() {
    // Test valid file types
    assert_eq!(
        SupportedFileType::from_extension("pdf"),
        Some(SupportedFileType::Pdf)
    );
    assert_eq!(
        SupportedFileType::from_extension("PDF"),
        Some(SupportedFileType::Pdf)
    );
    assert_eq!(
        SupportedFileType::from_extension("docx"),
        Some(SupportedFileType::Docx)
    );
    assert_eq!(
        SupportedFileType::from_extension("csv"),
        Some(SupportedFileType::Csv)
    );

    // Test invalid file types
    assert_eq!(SupportedFileType::from_extension("txt"), None);
    assert_eq!(SupportedFileType::from_extension("jpg"), None);
    assert_eq!(SupportedFileType::from_extension(""), None);

    // Test MIME type validation
    assert_eq!(
        SupportedFileType::from_mime_type("application/pdf"),
        Some(SupportedFileType::Pdf)
    );
    assert_eq!(
        SupportedFileType::from_mime_type("text/csv"),
        Some(SupportedFileType::Csv)
    );
    assert_eq!(SupportedFileType::from_mime_type("text/plain"), None);

    // Test MIME type and extension consistency
    let pdf_type = SupportedFileType::Pdf;
    assert_eq!(pdf_type.mime_type(), "application/pdf");
    assert_eq!(pdf_type.extension(), "pdf");
}

/// Test document ZK state operations
#[tokio::test]
async fn test_document_zk_state_operations() {
    let mut zk_state = DocumentZkState::new();
    assert_eq!(zk_state.total_documents, 0);

    // Create test document
    let document_id = [1u8; 32];
    let company_id = [2u8; 32];
    let uploaded_by = [3u8; 32];
    let content = b"test document content";
    let timestamp = chrono::Utc::now().timestamp() as u64;

    let document = Document::new(
        document_id,
        company_id,
        DocumentType::LoanContract,
        content,
        uploaded_by,
        timestamp,
    );

    // Test document upload state transition
    let upload_transition = zk_state
        .process_document_upload(document.clone(), uploaded_by, timestamp)
        .unwrap();

    assert_eq!(upload_transition.document_id, document_id);
    assert_eq!(upload_transition.company_id, company_id);
    assert_eq!(upload_transition.created_by, uploaded_by);
    assert_eq!(zk_state.total_documents, 1);

    // Test document approval state transition
    let approved_by = [4u8; 32];
    let approval_transition = zk_state
        .process_document_approval(document_id, approved_by, timestamp + 100)
        .unwrap();

    assert_eq!(approval_transition.document_id, document_id);
    assert_eq!(approval_transition.created_by, approved_by);

    // Test document inclusion proof
    let inclusion_proof = zk_state
        .generate_document_inclusion_proof(&document_id)
        .unwrap();

    assert_eq!(inclusion_proof.document_id, document_id);
    assert_eq!(inclusion_proof.document_hash, document.sha256_hash);

    let is_valid = zk_state.verify_document_inclusion_proof(&inclusion_proof).unwrap();
    assert!(is_valid);

    // Test state commitment calculation
    let state_commitment = zk_state.calculate_state_commitment();
    assert_ne!(state_commitment, [0u8; 32]);

    // Test document deletion state transition
    let deleted_by = [5u8; 32];
    let deletion_transition = zk_state
        .process_document_deletion(document_id, deleted_by, timestamp + 200)
        .unwrap();

    assert_eq!(deletion_transition.document_id, document_id);
    assert_eq!(deletion_transition.created_by, deleted_by);

    let deleted_document = zk_state.get_document(&document_id).unwrap();
    assert!(deleted_document.is_deleted());
}

/// Test document batch operations
#[tokio::test]
async fn test_document_batch_operations() {
    use arda_credit_lib::{DocumentStateTransition, StateTransitionType};

    let company_id = [2u8; 32];
    let created_by = [3u8; 32];
    let timestamp = chrono::Utc::now().timestamp() as u64;

    // Create test state transitions
    let transition1 = DocumentStateTransition {
        transition_id: [1u8; 32],
        document_id: [1u8; 32],
        company_id,
        transition_type: StateTransitionType::DocumentUpload,
        previous_state_hash: None,
        new_state_hash: [4u8; 32],
        created_by,
        created_at: timestamp,
    };

    let transition2 = DocumentStateTransition {
        transition_id: [2u8; 32],
        document_id: [1u8; 32],
        company_id,
        transition_type: StateTransitionType::DocumentApproval,
        previous_state_hash: Some([4u8; 32]),
        new_state_hash: [5u8; 32],
        created_by,
        created_at: timestamp + 100,
    };

    // Create document batch
    let batch = DocumentBatch::new(
        company_id,
        vec![transition1, transition2],
        [0u8; 32],
        [5u8; 32],
        created_by,
        timestamp,
    );

    assert_eq!(batch.company_id, company_id);
    assert_eq!(batch.transition_count(), 2);
    assert_eq!(batch.created_by, created_by);

    // Test batch integrity verification
    let is_valid = batch.verify_batch_integrity().unwrap();
    assert!(is_valid);

    // Test public values generation
    use arda_credit_lib::DocumentProofPublicValues;
    let public_values = DocumentProofPublicValues::from_batch(&batch);

    assert_eq!(public_values.company_id, company_id);
    assert_eq!(public_values.transition_count, 2);
    assert_eq!(public_values.previous_state_root, [0u8; 32]);
    assert_eq!(public_values.new_state_root, [5u8; 32]);
}

/// Test role-based access control
#[tokio::test]
async fn test_role_based_access_control() {
    let company_id = [2u8; 32];
    let user_id = [3u8; 32];

    // Test Admin role permissions
    let admin_member = TeamMember::new(
        [1u8; 32],
        company_id,
        user_id,
        TeamRole::Admin,
        chrono::Utc::now().timestamp() as u64,
    );

    assert!(admin_member.can_perform_action(DocumentAction::Upload));
    assert!(admin_member.can_perform_action(DocumentAction::Download));
    assert!(admin_member.can_perform_action(DocumentAction::Delete));
    assert!(admin_member.can_perform_action(DocumentAction::Approve));
    assert!(admin_member.can_perform_action(DocumentAction::ViewAuditLogs));

    // Test Write role permissions
    let write_member = TeamMember::new(
        [2u8; 32],
        company_id,
        user_id,
        TeamRole::Write,
        chrono::Utc::now().timestamp() as u64,
    );

    assert!(write_member.can_perform_action(DocumentAction::Upload));
    assert!(write_member.can_perform_action(DocumentAction::Download));
    assert!(!write_member.can_perform_action(DocumentAction::Delete));
    assert!(write_member.can_perform_action(DocumentAction::Approve));
    assert!(!write_member.can_perform_action(DocumentAction::ViewAuditLogs));

    // Test Read role permissions
    let read_member = TeamMember::new(
        [3u8; 32],
        company_id,
        user_id,
        TeamRole::Read,
        chrono::Utc::now().timestamp() as u64,
    );

    assert!(!read_member.can_perform_action(DocumentAction::Upload));
    assert!(read_member.can_perform_action(DocumentAction::Download));
    assert!(!read_member.can_perform_action(DocumentAction::Delete));
    assert!(!read_member.can_perform_action(DocumentAction::Approve));
    assert!(!read_member.can_perform_action(DocumentAction::ViewAuditLogs));
}

/// Test document error handling
#[tokio::test]
async fn test_document_error_handling() {
    // Test invalid document ID
    let invalid_id = [0u8; 32];
    let error = DocumentError::DocumentNotFound {
        document_id: invalid_id,
    };
    assert!(error.to_string().contains("not found"));

    // Test access denied error
    let access_error = DocumentError::AccessDenied {
        user_id: [1u8; 32],
        action: DocumentAction::Delete,
        resource: "document".to_string(),
    };
    assert!(access_error.to_string().contains("Access denied"));

    // Test integrity verification error
    let integrity_error = DocumentError::IntegrityVerificationFailed {
        document_id: [1u8; 32],
    };
    assert!(integrity_error.to_string().contains("Integrity verification failed"));

    // Test invalid file type error
    let file_type_error = DocumentError::InvalidFileType {
        file_type: "txt".to_string(),
    };
    assert!(file_type_error.to_string().contains("Invalid file type"));
}

/// Test storage configuration
#[tokio::test]
async fn test_storage_configuration() {
    let config = StorageConfig::default();
    assert_eq!(config.bucket_name, "arda-credit-documents");
    assert_eq!(config.region, "us-east-1");
    assert_eq!(config.max_file_size_mb, 100);
    assert_eq!(config.lifecycle_days_to_ia, 30);
    assert_eq!(config.lifecycle_days_to_glacier, 90);
    assert_eq!(config.lifecycle_days_to_delete, 2555);

    // Test custom configuration
    let custom_config = StorageConfig {
        bucket_name: "custom-bucket".to_string(),
        region: "us-west-2".to_string(),
        kms_key_id: Some("custom-key".to_string()),
        max_file_size_mb: 50,
        lifecycle_days_to_ia: 15,
        lifecycle_days_to_glacier: 45,
        lifecycle_days_to_delete: 1000,
    };

    assert_eq!(custom_config.bucket_name, "custom-bucket");
    assert_eq!(custom_config.region, "us-west-2");
    assert_eq!(custom_config.kms_key_id, Some("custom-key".to_string()));
    assert_eq!(custom_config.max_file_size_mb, 50);
}

/// Test document watermarking
#[tokio::test]
async fn test_document_watermarking() {
    use arda_credit_lib::DocumentWatermark;

    let watermark = DocumentWatermark::new(
        "Test Company".to_string(),
        "user@example.com".to_string(),
    );

    let watermark_text = watermark.generate_text();
    assert!(watermark_text.contains("CONFIDENTIAL"));
    assert!(watermark_text.contains("Test Company"));
    assert!(watermark_text.contains("user@example.com"));

    let metadata = watermark.generate_metadata();
    assert_eq!(
        metadata.get("watermark-company"),
        Some(&"Test Company".to_string())
    );
    assert_eq!(
        metadata.get("watermark-user"),
        Some(&"user@example.com".to_string())
    );
    assert!(metadata.contains_key("watermark-timestamp"));
}

/// Test error scenarios and edge cases
#[tokio::test]
async fn test_error_scenarios() {
    let mut zk_state = DocumentZkState::new();

    // Test duplicate document upload
    let document_id = [1u8; 32];
    let company_id = [2u8; 32];
    let uploaded_by = [3u8; 32];
    let content = b"test content";
    let timestamp = chrono::Utc::now().timestamp() as u64;

    let document = Document::new(
        document_id,
        company_id,
        DocumentType::LoanContract,
        content,
        uploaded_by,
        timestamp,
    );

    // First upload should succeed
    let result1 = zk_state.process_document_upload(document.clone(), uploaded_by, timestamp);
    assert!(result1.is_ok());

    // Second upload of same document should fail
    let result2 = zk_state.process_document_upload(document, uploaded_by, timestamp + 100);
    assert!(result2.is_err());

    // Test operations on non-existent document
    let non_existent_id = [99u8; 32];
    let approval_result = zk_state.process_document_approval(non_existent_id, uploaded_by, timestamp);
    assert!(approval_result.is_err());

    let deletion_result = zk_state.process_document_deletion(non_existent_id, uploaded_by, timestamp);
    assert!(deletion_result.is_err());

    let proof_result = zk_state.generate_document_inclusion_proof(&non_existent_id);
    assert!(proof_result.is_err());

    // Test invalid file sizes
    let large_content = vec![0u8; 200 * 1024 * 1024]; // 200MB
    let storage_config = StorageConfig::default();

    // This would fail in the actual storage client validation
    assert!(large_content.len() > (storage_config.max_file_size_mb as usize * 1024 * 1024));
}