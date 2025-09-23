//! Document service layer with role-based access control and KYB verification
//!
//! This module provides the business logic layer for document operations,
//! integrating database operations, S3 storage, and access control.

use crate::auth::Claims;
use arda_credit_lib::{
    AccessLevel, ApprovalStatus, Company, CompanyType, Document, DocumentAction, DocumentError,
    DocumentStatus, DocumentStorageClient, DocumentType, DocumentUploadRequest, KybStatus,
    S3Metadata, StorageConfig, StorageError, SupportedFileType, TeamMember, TeamRole,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

/// Document service errors
#[derive(Error, Debug)]
pub enum DocumentServiceError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    #[error("Document error: {0}")]
    DocumentError(#[from] DocumentError),

    #[error("Validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Access denied: {message}")]
    AccessDenied { message: String },

    #[error("Company not found: {company_id}")]
    CompanyNotFound { company_id: Uuid },

    #[error("Document not found: {document_id}")]
    DocumentNotFound { document_id: Uuid },

    #[error("User not found: {user_id}")]
    UserNotFound { user_id: Uuid },

    #[error("Team member not found: user {user_id} in company {company_id}")]
    TeamMemberNotFound { user_id: Uuid, company_id: Uuid },

    #[error("KYB not approved for company: {company_id}")]
    KybNotApproved { company_id: Uuid },

    #[error("KYC not approved for user: {user_id}")]
    KycNotApproved { user_id: Uuid },

    #[error("Invalid file type: {file_type}")]
    InvalidFileType { file_type: String },

    #[error("File size exceeds limit: {size_mb}MB")]
    FileSizeExceeded { size_mb: u64 },

    #[error("Document already exists: {filename} in company {company_id}")]
    DuplicateDocument { filename: String, company_id: Uuid },

    #[error("Document approval required before {operation}")]
    ApprovalRequired { operation: String },
}

/// Document upload request with validation
#[derive(Debug, Clone, Validate, Deserialize)]
pub struct DocumentUploadRequestDto {
    #[validate(length(min = 1, max = 500, message = "Filename must be 1-500 characters"))]
    pub filename: String,

    #[validate(length(min = 1, max = 50, message = "Document type must be 1-50 characters"))]
    pub document_type: String,

    pub deal_id: Option<Uuid>,

    #[validate(length(max = 100, message = "Category must be less than 100 characters"))]
    pub category: Option<String>,

    pub tags: Option<Vec<String>>,

    #[validate(length(min = 1, max = 20, message = "Access level must be specified"))]
    pub access_level: String,

    pub metadata: Option<HashMap<String, String>>,
}

/// Document response DTO
#[derive(Debug, Clone, Serialize)]
pub struct DocumentResponseDto {
    pub id: Uuid,
    pub company_id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub file_extension: String,
    pub mime_type: String,
    pub file_size_bytes: i64,
    pub document_type: String,
    pub deal_id: Option<Uuid>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub sha256_hash: String,
    pub access_level: String,
    pub approval_status: String,
    pub status: String,
    pub version: i32,
    pub parent_document_id: Option<Uuid>,
    pub uploaded_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub s3_bucket: String,
    pub s3_key: String,
}

/// Document list request with filtering
#[derive(Debug, Clone, Validate, Deserialize)]
pub struct DocumentListRequest {
    pub company_id: Uuid,
    pub document_type: Option<String>,
    pub deal_id: Option<Uuid>,
    pub status: Option<String>,
    pub approval_status: Option<String>,

    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: Option<i64>,

    #[validate(range(min = 0, message = "Offset must be non-negative"))]
    pub offset: Option<i64>,
}

/// Document audit log entry
#[derive(Debug, Clone, Serialize)]
pub struct DocumentAuditLogDto {
    pub id: Uuid,
    pub document_id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub result: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Document service with role-based access control
pub struct DocumentService {
    db_pool: PgPool,
    storage_client: DocumentStorageClient,
}

impl DocumentService {
    /// Create a new document service
    pub async fn new(db_pool: PgPool, storage_config: StorageConfig) -> Result<Self, DocumentServiceError> {
        let storage_client = DocumentStorageClient::new(storage_config).await?;

        Ok(Self {
            db_pool,
            storage_client,
        })
    }

    /// Upload a new document with role-based access control
    pub async fn upload_document(
        &self,
        claims: &Claims,
        company_id: Uuid,
        request: DocumentUploadRequestDto,
        file_content: Vec<u8>,
    ) -> Result<DocumentResponseDto, DocumentServiceError> {
        // Validate request
        request.validate()?;

        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Check permissions
        self.check_document_permission(user_id, company_id, DocumentAction::Upload).await?;

        // Validate file type
        let file_type = self.validate_file_type(&request.filename, &file_content)?;

        // Check for duplicate documents
        self.check_duplicate_document(company_id, &file_content).await?;

        // Generate document ID
        let document_id = Uuid::new_v4();

        // Calculate SHA-256 hash
        let sha256_hash = self.calculate_sha256_hash(&file_content);

        // Determine MIME type
        let mime_type = file_type.mime_type().to_string();

        // Create S3 upload request
        let s3_request = DocumentUploadRequest {
            company_id,
            deal_id: request.deal_id,
            document_type: request.document_type.clone(),
            filename: request.filename.clone(),
            content: file_content.clone(),
            content_type: mime_type.clone(),
            metadata: request.metadata.clone().unwrap_or_default(),
        };

        // Upload to S3
        let s3_metadata = self.storage_client.upload_document(document_id, s3_request).await?;

        // Parse access level
        let access_level = self.parse_access_level(&request.access_level)?;

        // Store document metadata in database
        let document_row = sqlx::query!(
            r#"
            INSERT INTO documents (
                id, company_id, filename, original_filename, file_extension, mime_type,
                file_size_bytes, document_type, deal_id, category, tags, sha256_hash,
                document_fingerprint, access_level, approval_status, status, version,
                uploaded_by, s3_bucket, s3_key, s3_version_id, encryption_status
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            ) RETURNING *
            "#,
            document_id,
            company_id,
            request.filename,
            request.filename,
            file_type.extension(),
            mime_type,
            file_content.len() as i64,
            request.document_type,
            request.deal_id,
            request.category,
            request.tags.as_deref(),
            hex::encode(&sha256_hash),
            hex::encode(self.calculate_document_fingerprint(&file_content, &document_id)),
            access_level.to_string(),
            ApprovalStatus::Pending.to_string(),
            DocumentStatus::Active.to_string(),
            1i32,
            user_id,
            s3_metadata.bucket,
            s3_metadata.key,
            s3_metadata.version_id,
            "encrypted"
        )
        .fetch_one(&self.db_pool)
        .await?;

        // Log the upload action
        self.log_document_action(
            document_id,
            company_id,
            user_id,
            "upload",
            "success",
            None,
            None,
            None,
            None,
            None,
        ).await?;

        info!(
            document_id = %document_id,
            company_id = %company_id,
            user_id = %user_id,
            filename = %request.filename,
            "Document uploaded successfully"
        );

        // Convert to response DTO
        Ok(self.row_to_document_dto(document_row))
    }

    /// Download a document with access control
    pub async fn download_document(
        &self,
        claims: &Claims,
        document_id: Uuid,
    ) -> Result<(Vec<u8>, String, String), DocumentServiceError> {
        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Get document metadata
        let document = self.get_document_by_id(document_id).await?;

        // Check permissions
        self.check_document_permission(user_id, document.company_id, DocumentAction::Download).await?;

        // Check if document is active
        if document.status != DocumentStatus::Active.to_string() {
            return Err(DocumentServiceError::DocumentNotFound { document_id });
        }

        // Download from S3
        let download_response = self
            .storage_client
            .download_document(&document.s3_bucket, &document.s3_key, None)
            .await?;

        // Verify integrity
        let calculated_hash = self.calculate_sha256_hash(&download_response.content);
        let stored_hash = hex::decode(&document.sha256_hash)
            .map_err(|_| DocumentServiceError::DocumentError(DocumentError::IntegrityVerificationFailed { 
                document_id: [0u8; 32] // This would need proper conversion in real implementation
            }))?;

        if calculated_hash != stored_hash.as_slice() {
            error!(
                document_id = %document_id,
                "Document integrity verification failed"
            );
            return Err(DocumentServiceError::DocumentError(DocumentError::IntegrityVerificationFailed { 
                document_id: [0u8; 32] // This would need proper conversion in real implementation
            }));
        }

        // Log the download action
        self.log_document_action(
            document_id,
            document.company_id,
            user_id,
            "download",
            "success",
            None,
            None,
            None,
            None,
            None,
        ).await?;

        info!(
            document_id = %document_id,
            user_id = %user_id,
            filename = %document.filename,
            "Document downloaded successfully"
        );

        Ok((
            download_response.content,
            download_response.content_type,
            download_response.filename,
        ))
    }

    /// Approve a document
    pub async fn approve_document(
        &self,
        claims: &Claims,
        document_id: Uuid,
    ) -> Result<DocumentResponseDto, DocumentServiceError> {
        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Get document metadata
        let document = self.get_document_by_id(document_id).await?;

        // Check permissions
        self.check_document_permission(user_id, document.company_id, DocumentAction::Approve).await?;

        // Update document approval status
        let updated_document = sqlx::query!(
            r#"
            UPDATE documents 
            SET approval_status = $1, approved_by = $2, approved_at = NOW(), updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
            ApprovalStatus::Approved.to_string(),
            user_id,
            document_id
        )
        .fetch_one(&self.db_pool)
        .await?;

        // Log the approval action
        self.log_document_action(
            document_id,
            document.company_id,
            user_id,
            "approve",
            "success",
            None,
            None,
            None,
            None,
            None,
        ).await?;

        info!(
            document_id = %document_id,
            user_id = %user_id,
            "Document approved successfully"
        );

        Ok(self.row_to_document_dto(updated_document))
    }

    /// Delete a document (soft delete)
    pub async fn delete_document(
        &self,
        claims: &Claims,
        document_id: Uuid,
    ) -> Result<(), DocumentServiceError> {
        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Get document metadata
        let document = self.get_document_by_id(document_id).await?;

        // Check permissions
        self.check_document_permission(user_id, document.company_id, DocumentAction::Delete).await?;

        // Soft delete document
        sqlx::query!(
            r#"
            UPDATE documents 
            SET status = $1, deleted_at = NOW(), updated_at = NOW()
            WHERE id = $2
            "#,
            DocumentStatus::Deleted.to_string(),
            document_id
        )
        .execute(&self.db_pool)
        .await?;

        // Log the deletion action
        self.log_document_action(
            document_id,
            document.company_id,
            user_id,
            "delete",
            "success",
            None,
            None,
            None,
            None,
            None,
        ).await?;

        info!(
            document_id = %document_id,
            user_id = %user_id,
            "Document deleted successfully"
        );

        Ok(())
    }

    /// List documents for a company with filtering
    pub async fn list_documents(
        &self,
        claims: &Claims,
        request: DocumentListRequest,
    ) -> Result<Vec<DocumentResponseDto>, DocumentServiceError> {
        // Validate request
        request.validate()?;

        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Check permissions
        self.check_document_permission(user_id, request.company_id, DocumentAction::View).await?;

        // Build query with filters
        let mut query = String::from(
            r#"
            SELECT * FROM documents 
            WHERE company_id = $1 AND status != 'deleted'
            "#,
        );

        let mut param_count = 1;
        let mut params: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = vec![Box::new(request.company_id)];

        if let Some(document_type) = &request.document_type {
            param_count += 1;
            query.push_str(&format!(" AND document_type = ${}", param_count));
            params.push(Box::new(document_type.clone()));
        }

        if let Some(deal_id) = request.deal_id {
            param_count += 1;
            query.push_str(&format!(" AND deal_id = ${}", param_count));
            params.push(Box::new(deal_id));
        }

        if let Some(status) = &request.status {
            param_count += 1;
            query.push_str(&format!(" AND status = ${}", param_count));
            params.push(Box::new(status.clone()));
        }

        if let Some(approval_status) = &request.approval_status {
            param_count += 1;
            query.push_str(&format!(" AND approval_status = ${}", param_count));
            params.push(Box::new(approval_status.clone()));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = request.limit {
            param_count += 1;
            query.push_str(&format!(" LIMIT ${}", param_count));
            params.push(Box::new(limit));
        }

        if let Some(offset) = request.offset {
            param_count += 1;
            query.push_str(&format!(" OFFSET ${}", param_count));
            params.push(Box::new(offset));
        }

        // Execute query (simplified - in real implementation would use proper parameter binding)
        let documents = sqlx::query(&query)
            .bind(request.company_id)
            .fetch_all(&self.db_pool)
            .await?;

        let document_dtos = documents
            .into_iter()
            .map(|row| self.row_to_document_dto(row))
            .collect();

        Ok(document_dtos)
    }

    /// Get document audit logs
    pub async fn get_document_audit_logs(
        &self,
        claims: &Claims,
        company_id: Uuid,
        document_id: Option<Uuid>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<DocumentAuditLogDto>, DocumentServiceError> {
        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Check permissions
        self.check_document_permission(user_id, company_id, DocumentAction::ViewAuditLogs).await?;

        // Build query
        let mut query = String::from(
            r#"
            SELECT * FROM document_audit_logs 
            WHERE company_id = $1
            "#,
        );

        if document_id.is_some() {
            query.push_str(" AND document_id = $2");
        }

        query.push_str(" ORDER BY created_at DESC");

        if limit.is_some() {
            query.push_str(" LIMIT $3");
        }

        if offset.is_some() {
            query.push_str(" OFFSET $4");
        }

        // Execute query (simplified)
        let logs = if let Some(doc_id) = document_id {
            sqlx::query(&query)
                .bind(company_id)
                .bind(doc_id)
                .fetch_all(&self.db_pool)
                .await?
        } else {
            sqlx::query(&query)
                .bind(company_id)
                .fetch_all(&self.db_pool)
                .await?
        };

        let audit_dtos = logs
            .into_iter()
            .map(|row| DocumentAuditLogDto {
                id: row.get("id"),
                document_id: row.get("document_id"),
                company_id: row.get("company_id"),
                user_id: row.get("user_id"),
                action: row.get("action"),
                result: row.get("result"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                request_id: row.get("request_id"),
                details: row.get("details"),
                error_message: row.get("error_message"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(audit_dtos)
    }

    /// Verify document integrity
    pub async fn verify_document_integrity(
        &self,
        claims: &Claims,
        document_id: Uuid,
    ) -> Result<bool, DocumentServiceError> {
        // Parse user ID from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| DocumentServiceError::UserNotFound { user_id: Uuid::nil() })?;

        // Get document metadata
        let document = self.get_document_by_id(document_id).await?;

        // Check permissions
        self.check_document_permission(user_id, document.company_id, DocumentAction::View).await?;

        // Download document content
        let download_response = self
            .storage_client
            .download_document(&document.s3_bucket, &document.s3_key, None)
            .await?;

        // Verify integrity
        let calculated_hash = self.calculate_sha256_hash(&download_response.content);
        let stored_hash = hex::decode(&document.sha256_hash)
            .map_err(|_| DocumentServiceError::DocumentError(DocumentError::IntegrityVerificationFailed { 
                document_id: [0u8; 32] // This would need proper conversion in real implementation
            }))?;

        let is_valid = calculated_hash == stored_hash.as_slice();

        // Log the verification action
        let result = if is_valid { "success" } else { "failure" };
        self.log_document_action(
            document_id,
            document.company_id,
            user_id,
            "verify",
            result,
            None,
            None,
            None,
            None,
            if is_valid { None } else { Some("Integrity verification failed".to_string()) },
        ).await?;

        Ok(is_valid)
    }

    // Helper methods

    /// Check if user has permission for document operation
    async fn check_document_permission(
        &self,
        user_id: Uuid,
        company_id: Uuid,
        action: DocumentAction,
    ) -> Result<(), DocumentServiceError> {
        // Check if user is a team member of the company
        let team_member = sqlx::query!(
            r#"
            SELECT tm.*, c.kyb_status, i.kyc_status
            FROM team_members tm
            JOIN companies c ON tm.company_id = c.id
            JOIN identities i ON tm.user_id = i.id
            WHERE tm.user_id = $1 AND tm.company_id = $2 AND tm.status = 'active'
            "#,
            user_id,
            company_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        let team_member = team_member.ok_or(DocumentServiceError::TeamMemberNotFound {
            user_id,
            company_id,
        })?;

        // Check KYC status for user
        if team_member.kyc_status != Some("approved".to_string()) {
            return Err(DocumentServiceError::KycNotApproved { user_id });
        }

        // Check KYB status for company (for sensitive operations)
        if matches!(action, DocumentAction::Upload | DocumentAction::Delete | DocumentAction::Approve) {
            if team_member.kyb_status != Some("approved".to_string()) {
                return Err(DocumentServiceError::KybNotApproved { company_id });
            }
        }

        // Check specific permissions based on action
        let has_permission = match action {
            DocumentAction::Upload => team_member.can_upload_documents.unwrap_or(false),
            DocumentAction::Download | DocumentAction::View => team_member.can_download_documents.unwrap_or(false),
            DocumentAction::Delete => team_member.can_delete_documents.unwrap_or(false),
            DocumentAction::Approve | DocumentAction::Reject => team_member.can_approve_documents.unwrap_or(false),
            DocumentAction::ViewAuditLogs => team_member.can_view_audit_logs.unwrap_or(false),
            DocumentAction::ManageTeam => team_member.can_manage_team.unwrap_or(false),
        };

        if !has_permission {
            return Err(DocumentServiceError::AccessDenied {
                message: format!("User {} cannot perform {:?} in company {}", user_id, action, company_id),
            });
        }

        Ok(())
    }

    /// Get document by ID
    async fn get_document_by_id(&self, document_id: Uuid) -> Result<DocumentResponseDto, DocumentServiceError> {
        let document_row = sqlx::query!(
            "SELECT * FROM documents WHERE id = $1",
            document_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        let document_row = document_row.ok_or(DocumentServiceError::DocumentNotFound { document_id })?;

        Ok(self.row_to_document_dto(document_row))
    }

    /// Validate file type
    fn validate_file_type(&self, filename: &str, content: &[u8]) -> Result<SupportedFileType, DocumentServiceError> {
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let file_type = SupportedFileType::from_extension(extension)
            .ok_or_else(|| DocumentServiceError::InvalidFileType {
                file_type: extension.to_string(),
            })?;

        // Check file size (100MB limit)
        let size_mb = content.len() as u64 / (1024 * 1024);
        if size_mb > 100 {
            return Err(DocumentServiceError::FileSizeExceeded { size_mb });
        }

        Ok(file_type)
    }

    /// Check for duplicate documents
    async fn check_duplicate_document(
        &self,
        company_id: Uuid,
        content: &[u8],
    ) -> Result<(), DocumentServiceError> {
        let sha256_hash = hex::encode(self.calculate_sha256_hash(content));

        let existing = sqlx::query!(
            "SELECT id FROM documents WHERE company_id = $1 AND sha256_hash = $2 AND status != 'deleted'",
            company_id,
            sha256_hash
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if existing.is_some() {
            return Err(DocumentServiceError::DuplicateDocument {
                filename: "duplicate".to_string(),
                company_id,
            });
        }

        Ok(())
    }

    /// Calculate SHA-256 hash
    fn calculate_sha256_hash(&self, content: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.finalize().to_vec()
    }

    /// Calculate document fingerprint
    fn calculate_document_fingerprint(&self, content: &[u8], document_id: &Uuid) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hasher.update(document_id.as_bytes());
        hasher.update(b"arda_document_fingerprint");
        hasher.finalize().to_vec()
    }

    /// Parse access level from string
    fn parse_access_level(&self, access_level: &str) -> Result<AccessLevel, DocumentServiceError> {
        match access_level.to_lowercase().as_str() {
            "public" => Ok(AccessLevel::Public),
            "company" => Ok(AccessLevel::Company),
            "deal" => Ok(AccessLevel::Deal),
            "restricted" => Ok(AccessLevel::Restricted),
            _ => Err(DocumentServiceError::ValidationError(
                validator::ValidationErrors::new()
            )),
        }
    }

    /// Log document action for audit trail
    async fn log_document_action(
        &self,
        document_id: Uuid,
        company_id: Uuid,
        user_id: Uuid,
        action: &str,
        result: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
        request_id: Option<String>,
        details: Option<serde_json::Value>,
        error_message: Option<String>,
    ) -> Result<(), DocumentServiceError> {
        sqlx::query!(
            r#"
            INSERT INTO document_audit_logs (
                document_id, company_id, user_id, action, result,
                ip_address, user_agent, request_id, details, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            document_id,
            company_id,
            user_id,
            action,
            result,
            ip_address,
            user_agent,
            request_id,
            details,
            error_message
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Convert database row to document DTO
    fn row_to_document_dto(&self, row: sqlx::postgres::PgRow) -> DocumentResponseDto {
        DocumentResponseDto {
            id: row.get("id"),
            company_id: row.get("company_id"),
            filename: row.get("filename"),
            original_filename: row.get("original_filename"),
            file_extension: row.get("file_extension"),
            mime_type: row.get("mime_type"),
            file_size_bytes: row.get("file_size_bytes"),
            document_type: row.get("document_type"),
            deal_id: row.get("deal_id"),
            category: row.get("category"),
            tags: row.get("tags"),
            sha256_hash: row.get("sha256_hash"),
            access_level: row.get("access_level"),
            approval_status: row.get("approval_status"),
            status: row.get("status"),
            version: row.get("version"),
            parent_document_id: row.get("parent_document_id"),
            uploaded_by: row.get("uploaded_by"),
            approved_by: row.get("approved_by"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            approved_at: row.get("approved_at"),
            expires_at: row.get("expires_at"),
            s3_bucket: row.get("s3_bucket"),
            s3_key: row.get("s3_key"),
        }
    }
}

impl ToString for AccessLevel {
    fn to_string(&self) -> String {
        match self {
            AccessLevel::Public => "public".to_string(),
            AccessLevel::Company => "company".to_string(),
            AccessLevel::Deal => "deal".to_string(),
            AccessLevel::Restricted => "restricted".to_string(),
        }
    }
}

impl ToString for ApprovalStatus {
    fn to_string(&self) -> String {
        match self {
            ApprovalStatus::Pending => "pending".to_string(),
            ApprovalStatus::Approved => "approved".to_string(),
            ApprovalStatus::Rejected => "rejected".to_string(),
        }
    }
}

impl ToString for DocumentStatus {
    fn to_string(&self) -> String {
        match self {
            DocumentStatus::Active => "active".to_string(),
            DocumentStatus::Archived => "archived".to_string(),
            DocumentStatus::Deleted => "deleted".to_string(),
        }
    }
}