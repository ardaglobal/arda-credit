use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::{DbError, DbResult};

// ============================================================================
// DOCUMENT TYPES
// ============================================================================

/// Document associated with a deal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DealDocument {
    pub id: Uuid,
    pub deal_id: Uuid,

    // File metadata
    pub file_name: String,
    pub original_file_name: String,
    pub file_type: String,
    pub file_extension: String,
    pub file_size_bytes: i64,

    // Storage location
    pub storage_path: String,
    pub storage_type: StorageType,

    // Document categorization
    pub document_type: Option<String>,
    pub document_category: Option<String>,
    pub description: Option<String>,

    // Upload information
    pub uploaded_by: Uuid,
    pub uploaded_at: DateTime<Utc>,

    // Ingestion status
    pub ingestion_status: IngestionStatus,
    pub ingested_at: Option<DateTime<Utc>>,
    pub ingestion_error: Option<String>,

    // Extracted metadata
    pub page_count: Option<i32>,
    pub word_count: Option<i32>,
    pub sheet_count: Option<i32>,
    pub extracted_text_preview: Option<String>,

    // Security and validation
    pub checksum: String,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    // Version control
    pub version: i32,
    pub parent_document_id: Option<Uuid>,

    // Audit timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Document ingestion job for async processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentIngestionJob {
    pub id: Uuid,
    pub document_id: Uuid,

    // Job status
    pub status: JobStatus,

    // Job timing
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    // Processing details
    pub processing_attempts: i32,
    pub max_attempts: i32,
    pub error_message: Option<String>,

    // Extracted data
    pub extracted_metadata: Option<serde_json::Value>,
    pub extracted_text: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Storage type for documents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum StorageType {
    Filesystem,
    S3,
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageType::Filesystem => write!(f, "filesystem"),
            StorageType::S3 => write!(f, "s3"),
        }
    }
}

impl std::str::FromStr for StorageType {
    type Err = DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "filesystem" => Ok(StorageType::Filesystem),
            "s3" => Ok(StorageType::S3),
            _ => Err(DbError::ValidationError(format!(
                "Invalid storage type: {}",
                s
            ))),
        }
    }
}

/// Ingestion status for documents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum IngestionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for IngestionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestionStatus::Pending => write!(f, "pending"),
            IngestionStatus::Processing => write!(f, "processing"),
            IngestionStatus::Completed => write!(f, "completed"),
            IngestionStatus::Failed => write!(f, "failed"),
            IngestionStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for IngestionStatus {
    type Err = DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(IngestionStatus::Pending),
            "processing" => Ok(IngestionStatus::Processing),
            "completed" => Ok(IngestionStatus::Completed),
            "failed" => Ok(IngestionStatus::Failed),
            "skipped" => Ok(IngestionStatus::Skipped),
            _ => Err(DbError::ValidationError(format!(
                "Invalid ingestion status: {}",
                s
            ))),
        }
    }
}

/// Job status for ingestion jobs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum JobStatus {
    #[serde(rename = "queued")]
    #[sqlx(rename = "queued")]
    Queued,
    #[serde(rename = "processing")]
    #[sqlx(rename = "processing")]
    Processing,
    #[serde(rename = "completed")]
    #[sqlx(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    #[sqlx(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    #[sqlx(rename = "cancelled")]
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "queued"),
            JobStatus::Processing => write!(f, "processing"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "queued" => Ok(JobStatus::Queued),
            "processing" => Ok(JobStatus::Processing),
            "completed" => Ok(JobStatus::Completed),
            "failed" => Ok(JobStatus::Failed),
            "cancelled" => Ok(JobStatus::Cancelled),
            _ => Err(DbError::ValidationError(format!(
                "Invalid job status: {}",
                s
            ))),
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to create a new document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub deal_id: Uuid,
    pub file_name: String,
    pub original_file_name: String,
    pub file_type: String,
    pub file_extension: String,
    pub file_size_bytes: i64,
    pub storage_path: String,
    pub storage_type: StorageType,
    pub document_type: Option<String>,
    pub document_category: Option<String>,
    pub description: Option<String>,
    pub uploaded_by: Uuid,
    pub checksum: String,
}

/// Request to update document metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentRequest {
    pub document_type: Option<String>,
    pub document_category: Option<String>,
    pub description: Option<String>,
}

/// Document statistics for a deal
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentStats {
    pub total_documents: i64,
    pub total_size_bytes: i64,
    pub pending_ingestion: i64,
    pub completed_ingestion: i64,
    pub failed_ingestion: i64,
    pub document_types: Option<serde_json::Value>,
}

// ============================================================================
// DATABASE OPERATIONS
// ============================================================================

/// Create a new document record
#[instrument(skip(pool))]
pub async fn create_document(pool: &PgPool, req: CreateDocumentRequest) -> DbResult<DealDocument> {
    debug!(
        "Creating document for deal_id: {}, filename: {}",
        req.deal_id, req.file_name
    );

    let doc = sqlx::query_as::<_, DealDocument>(
        r#"
        INSERT INTO deal_documents (
            deal_id, file_name, original_file_name, file_type, file_extension,
            file_size_bytes, storage_path, storage_type, document_type,
            document_category, description, uploaded_by, checksum, ingestion_status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'pending')
        RETURNING *
        "#,
    )
    .bind(req.deal_id)
    .bind(&req.file_name)
    .bind(&req.original_file_name)
    .bind(&req.file_type)
    .bind(&req.file_extension)
    .bind(req.file_size_bytes)
    .bind(&req.storage_path)
    .bind(req.storage_type.to_string())
    .bind(req.document_type)
    .bind(req.document_category)
    .bind(req.description)
    .bind(req.uploaded_by)
    .bind(&req.checksum)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create document: {}", e);
        DbError::Database(e)
    })?;

    debug!("Document created with id: {}", doc.id);
    Ok(doc)
}

/// Get document by ID
#[instrument(skip(pool))]
pub async fn get_document_by_id(pool: &PgPool, document_id: Uuid) -> DbResult<DealDocument> {
    let doc = sqlx::query_as::<_, DealDocument>(
        r#"
        SELECT * FROM deal_documents
        WHERE id = $1 AND is_deleted = false
        "#,
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch document: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Document {} not found", document_id)))?;

    Ok(doc)
}

/// Get all documents for a deal
#[instrument(skip(pool))]
pub async fn get_documents_by_deal_id(pool: &PgPool, deal_id: Uuid) -> DbResult<Vec<DealDocument>> {
    let docs = sqlx::query_as::<_, DealDocument>(
        r#"
        SELECT * FROM deal_documents
        WHERE deal_id = $1 AND is_deleted = false
        ORDER BY uploaded_at DESC
        "#,
    )
    .bind(deal_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch documents for deal: {}", e);
        DbError::Database(e)
    })?;

    Ok(docs)
}

/// Update document metadata
#[instrument(skip(pool))]
pub async fn update_document(
    pool: &PgPool,
    document_id: Uuid,
    req: UpdateDocumentRequest,
) -> DbResult<DealDocument> {
    let doc = sqlx::query_as::<_, DealDocument>(
        r#"
        UPDATE deal_documents
        SET document_type = COALESCE($2, document_type),
            document_category = COALESCE($3, document_category),
            description = COALESCE($4, description),
            updated_at = NOW()
        WHERE id = $1 AND is_deleted = false
        RETURNING *
        "#,
    )
    .bind(document_id)
    .bind(req.document_type)
    .bind(req.document_category)
    .bind(req.description)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update document: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Document {} not found", document_id)))?;

    Ok(doc)
}

/// Soft delete a document
#[instrument(skip(pool))]
pub async fn delete_document(
    pool: &PgPool,
    document_id: Uuid,
    deleted_by: Uuid,
) -> DbResult<DealDocument> {
    let doc = sqlx::query_as::<_, DealDocument>(
        r#"
        UPDATE deal_documents
        SET is_deleted = true,
            deleted_at = NOW(),
            deleted_by = $2,
            updated_at = NOW()
        WHERE id = $1 AND is_deleted = false
        RETURNING *
        "#,
    )
    .bind(document_id)
    .bind(deleted_by)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete document: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Document {} not found", document_id)))?;

    Ok(doc)
}

/// Get document statistics for a deal
#[instrument(skip(pool))]
pub async fn get_deal_document_stats(pool: &PgPool, deal_id: Uuid) -> DbResult<DocumentStats> {
    let stats = sqlx::query_as::<_, DocumentStats>(
        r#"
        SELECT * FROM get_deal_document_stats($1)
        "#,
    )
    .bind(deal_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch document stats: {}", e);
        DbError::Database(e)
    })?;

    Ok(stats)
}

/// Update document ingestion status
#[instrument(skip(pool))]
pub async fn update_document_ingestion_status(
    pool: &PgPool,
    document_id: Uuid,
    status: IngestionStatus,
    error_message: Option<String>,
) -> DbResult<DealDocument> {
    let doc = sqlx::query_as::<_, DealDocument>(
        r#"
        UPDATE deal_documents
        SET ingestion_status = $2,
            ingested_at = CASE WHEN $2 IN ('completed', 'failed', 'skipped') THEN NOW() ELSE ingested_at END,
            ingestion_error = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(document_id)
    .bind(status.to_string())
    .bind(error_message)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update document ingestion status: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Document {} not found", document_id)))?;

    Ok(doc)
}

/// Create an ingestion job for a document
#[instrument(skip(pool))]
pub async fn create_ingestion_job(
    pool: &PgPool,
    document_id: Uuid,
) -> DbResult<DocumentIngestionJob> {
    let job = sqlx::query_as::<_, DocumentIngestionJob>(
        r#"
        INSERT INTO document_ingestion_jobs (document_id, status)
        VALUES ($1, 'queued')
        RETURNING *
        "#,
    )
    .bind(document_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create ingestion job: {}", e);
        DbError::Database(e)
    })?;

    debug!("Ingestion job created with id: {}", job.id);
    Ok(job)
}

/// Get pending ingestion jobs
#[instrument(skip(pool))]
pub async fn get_pending_ingestion_jobs(
    pool: &PgPool,
    limit: i64,
) -> DbResult<Vec<DocumentIngestionJob>> {
    let jobs = sqlx::query_as::<_, DocumentIngestionJob>(
        r#"
        SELECT * FROM document_ingestion_jobs
        WHERE status = 'queued'
            AND processing_attempts < max_attempts
        ORDER BY queued_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch pending ingestion jobs: {}", e);
        DbError::Database(e)
    })?;

    Ok(jobs)
}

/// Update ingestion job status
#[instrument(skip(pool))]
pub async fn update_ingestion_job_status(
    pool: &PgPool,
    job_id: Uuid,
    status: JobStatus,
    error_message: Option<String>,
) -> DbResult<DocumentIngestionJob> {
    let job = sqlx::query_as::<_, DocumentIngestionJob>(
        r#"
        UPDATE document_ingestion_jobs
        SET status = $2,
            started_at = CASE WHEN $2 = 'processing' THEN NOW() ELSE started_at END,
            completed_at = CASE WHEN $2 IN ('completed', 'failed', 'cancelled') THEN NOW() ELSE completed_at END,
            error_message = $3,
            processing_attempts = processing_attempts + 1,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(status.to_string())
    .bind(error_message)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update ingestion job status: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Ingestion job {} not found", job_id)))?;

    Ok(job)
}

/// Save extracted document data to ingestion job
#[instrument(skip(pool, extracted_text, extracted_metadata))]
pub async fn save_extracted_document_data(
    pool: &PgPool,
    job_id: Uuid,
    extracted_text: Option<String>,
    extracted_metadata: Option<serde_json::Value>,
) -> DbResult<DocumentIngestionJob> {
    let job = sqlx::query_as::<_, DocumentIngestionJob>(
        r#"
        UPDATE document_ingestion_jobs
        SET extracted_text = $2,
            extracted_metadata = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(extracted_text)
    .bind(extracted_metadata)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save extracted document data: {}", e);
        DbError::Database(e)
    })?
    .ok_or_else(|| DbError::NotFound(format!("Ingestion job {} not found", job_id)))?;

    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_type_parsing() {
        assert_eq!(
            "filesystem".parse::<StorageType>().unwrap(),
            StorageType::Filesystem
        );
        assert_eq!("s3".parse::<StorageType>().unwrap(), StorageType::S3);
        assert!("invalid".parse::<StorageType>().is_err());
    }

    #[test]
    fn test_ingestion_status_parsing() {
        assert_eq!(
            "pending".parse::<IngestionStatus>().unwrap(),
            IngestionStatus::Pending
        );
        assert_eq!(
            "processing".parse::<IngestionStatus>().unwrap(),
            IngestionStatus::Processing
        );
        assert_eq!(
            "completed".parse::<IngestionStatus>().unwrap(),
            IngestionStatus::Completed
        );
        assert_eq!(
            "failed".parse::<IngestionStatus>().unwrap(),
            IngestionStatus::Failed
        );
        assert_eq!(
            "skipped".parse::<IngestionStatus>().unwrap(),
            IngestionStatus::Skipped
        );
        assert!("invalid".parse::<IngestionStatus>().is_err());
    }

    #[test]
    fn test_job_status_parsing() {
        assert_eq!("queued".parse::<JobStatus>().unwrap(), JobStatus::Queued);
        assert_eq!(
            "processing".parse::<JobStatus>().unwrap(),
            JobStatus::Processing
        );
        assert_eq!(
            "completed".parse::<JobStatus>().unwrap(),
            JobStatus::Completed
        );
        assert_eq!("failed".parse::<JobStatus>().unwrap(), JobStatus::Failed);
        assert_eq!(
            "cancelled".parse::<JobStatus>().unwrap(),
            JobStatus::Cancelled
        );
        assert!("invalid".parse::<JobStatus>().is_err());
    }
}
