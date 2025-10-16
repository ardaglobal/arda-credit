use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, instrument};
use uuid::Uuid;

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type ApiResult<T> = Result<T, ApiError>;

// ============================================================================
// STORAGE SERVICE TRAITS AND TYPES
// ============================================================================

/// Configuration for document storage
#[derive(Debug, Clone)]
pub struct DocumentStorageConfig {
    pub storage_type: StorageBackend,
    pub filesystem_base_path: Option<PathBuf>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub max_file_size_bytes: usize,
    pub allowed_mime_types: Vec<String>,
}

impl Default for DocumentStorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageBackend::Filesystem,
            filesystem_base_path: Some(PathBuf::from("./data/documents")),
            s3_bucket: None,
            s3_region: None,
            max_file_size_bytes: 50 * 1024 * 1024, // 50 MB default
            allowed_mime_types: vec![
                "application/pdf".to_string(),
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(), // .docx
                "application/msword".to_string(), // .doc
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(), // .xlsx
                "application/vnd.ms-excel".to_string(), // .xls
                "text/plain".to_string(),
                "text/csv".to_string(),
                "image/jpeg".to_string(),
                "image/png".to_string(),
            ],
        }
    }
}

/// Storage backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    Filesystem,
    S3,
}

/// Document storage metadata returned after upload
#[derive(Debug, Clone)]
pub struct StorageMetadata {
    pub storage_path: String,
    pub storage_type: StorageBackend,
    pub file_size_bytes: usize,
    pub checksum: String,
    pub mime_type: String,
}

/// Uploaded file data
#[derive(Debug)]
pub struct UploadedFile {
    pub original_filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

// ============================================================================
// DOCUMENT STORAGE SERVICE
// ============================================================================

pub struct DocumentStorageService {
    config: DocumentStorageConfig,
}

impl DocumentStorageService {
    pub fn new(config: DocumentStorageConfig) -> Self {
        Self { config }
    }

    /// Validate file before storage
    #[instrument(skip(self, file_data))]
    pub fn validate_file(
        &self,
        file_data: &[u8],
        mime_type: &str,
        original_filename: &str,
    ) -> ApiResult<()> {
        // Check file size
        if file_data.len() > self.config.max_file_size_bytes {
            return Err(ApiError::ValidationError(format!(
                "File size ({} bytes) exceeds maximum allowed size ({} bytes)",
                file_data.len(),
                self.config.max_file_size_bytes
            )));
        }

        // Check MIME type
        if !self
            .config
            .allowed_mime_types
            .contains(&mime_type.to_string())
        {
            return Err(ApiError::ValidationError(format!(
                "File type '{}' is not allowed. Supported types: {}",
                mime_type,
                self.config.allowed_mime_types.join(", ")
            )));
        }

        // Basic filename validation
        if original_filename.is_empty() {
            return Err(ApiError::ValidationError(
                "Filename cannot be empty".to_string(),
            ));
        }

        debug!(
            "File validation passed: {} ({} bytes, {})",
            original_filename,
            file_data.len(),
            mime_type
        );

        Ok(())
    }

    /// Calculate SHA256 checksum of file data
    pub fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Get file extension from MIME type
    pub fn get_extension_from_mime(mime_type: &str) -> &str {
        match mime_type {
            "application/pdf" => ".pdf",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => ".docx",
            "application/msword" => ".doc",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => ".xlsx",
            "application/vnd.ms-excel" => ".xls",
            "text/plain" => ".txt",
            "text/csv" => ".csv",
            "image/jpeg" => ".jpg",
            "image/png" => ".png",
            _ => "",
        }
    }

    /// Store a document and return storage metadata
    #[instrument(skip(self, file))]
    pub async fn store_document(
        &self,
        deal_id: Uuid,
        file: UploadedFile,
    ) -> ApiResult<StorageMetadata> {
        // Validate the file
        self.validate_file(&file.data, &file.mime_type, &file.original_filename)?;

        // Calculate checksum
        let checksum = Self::calculate_checksum(&file.data);

        // Store based on backend type
        let (storage_path, storage_type) = match self.config.storage_type {
            StorageBackend::Filesystem => {
                let path = self.store_to_filesystem(deal_id, &file, &checksum).await?;
                (path, StorageBackend::Filesystem)
            }
            StorageBackend::S3 => {
                let path = self.store_to_s3(deal_id, &file, &checksum).await?;
                (path, StorageBackend::S3)
            }
        };

        Ok(StorageMetadata {
            storage_path,
            storage_type,
            file_size_bytes: file.data.len(),
            checksum,
            mime_type: file.mime_type,
        })
    }

    /// Store document to filesystem
    #[instrument(skip(self, file))]
    async fn store_to_filesystem(
        &self,
        deal_id: Uuid,
        file: &UploadedFile,
        checksum: &str,
    ) -> ApiResult<String> {
        let base_path =
            self.config.filesystem_base_path.as_ref().ok_or_else(|| {
                ApiError::Configuration("Filesystem base path not configured".into())
            })?;

        // Create directory structure: base_path/deal_id/
        let deal_dir = base_path.join(deal_id.to_string());
        fs::create_dir_all(&deal_dir).await.map_err(|e| {
            error!("Failed to create directory {}: {}", deal_dir.display(), e);
            ApiError::InternalError(format!("Failed to create storage directory: {}", e))
        })?;

        // Generate unique filename: checksum + extension
        let extension = Self::get_extension_from_mime(&file.mime_type);
        let filename = format!("{}{}", checksum, extension);
        let file_path = deal_dir.join(&filename);

        // Write file to disk
        let mut file_handle = fs::File::create(&file_path).await.map_err(|e| {
            error!("Failed to create file {}: {}", file_path.display(), e);
            ApiError::InternalError(format!("Failed to create file: {}", e))
        })?;

        file_handle.write_all(&file.data).await.map_err(|e| {
            error!("Failed to write file {}: {}", file_path.display(), e);
            ApiError::InternalError(format!("Failed to write file: {}", e))
        })?;

        file_handle.sync_all().await.map_err(|e| {
            error!("Failed to sync file {}: {}", file_path.display(), e);
            ApiError::InternalError(format!("Failed to sync file: {}", e))
        })?;

        debug!("File stored successfully: {}", file_path.display());

        // Return relative path from base
        Ok(format!("{}/{}", deal_id, filename))
    }

    /// Store document to S3 (placeholder implementation)
    #[instrument(skip(self, _file))]
    async fn store_to_s3(
        &self,
        _deal_id: Uuid,
        _file: &UploadedFile,
        _checksum: &str,
    ) -> ApiResult<String> {
        // TODO: Implement S3 storage using AWS SDK
        // For now, return an error indicating S3 is not yet implemented
        error!("S3 storage is not yet implemented");
        Err(ApiError::Configuration(
            "S3 storage backend is not yet implemented. Please use filesystem storage.".into(),
        ))

        // Future implementation will look like:
        // 1. Get S3 client from config
        // 2. Generate S3 key: deals/{deal_id}/{checksum}{extension}
        // 3. Upload to S3 with proper content-type
        // 4. Return S3 key as storage_path
    }

    /// Retrieve document from storage
    #[instrument(skip(self))]
    pub async fn retrieve_document(
        &self,
        storage_path: &str,
        storage_type: StorageBackend,
    ) -> ApiResult<Vec<u8>> {
        match storage_type {
            StorageBackend::Filesystem => self.retrieve_from_filesystem(storage_path).await,
            StorageBackend::S3 => self.retrieve_from_s3(storage_path).await,
        }
    }

    /// Retrieve document from filesystem
    #[instrument(skip(self))]
    async fn retrieve_from_filesystem(&self, storage_path: &str) -> ApiResult<Vec<u8>> {
        let base_path =
            self.config.filesystem_base_path.as_ref().ok_or_else(|| {
                ApiError::Configuration("Filesystem base path not configured".into())
            })?;

        let file_path = base_path.join(storage_path);

        // Security check: ensure the path is within the base directory
        let canonical_base = fs::canonicalize(base_path).await.map_err(|e| {
            error!("Failed to canonicalize base path: {}", e);
            ApiError::InternalError("Storage path configuration error".into())
        })?;

        let canonical_file = fs::canonicalize(&file_path).await.map_err(|e| {
            error!(
                "Failed to canonicalize file path {}: {}",
                file_path.display(),
                e
            );
            ApiError::NotFound("Document not found".into())
        })?;

        if !canonical_file.starts_with(&canonical_base) {
            error!("Path traversal attempt detected: {}", storage_path);
            return Err(ApiError::ValidationError(
                "Invalid storage path".to_string(),
            ));
        }

        // Read file
        let data = fs::read(&file_path).await.map_err(|e| {
            error!("Failed to read file {}: {}", file_path.display(), e);
            ApiError::NotFound("Document not found".into())
        })?;

        debug!("File retrieved successfully: {}", file_path.display());
        Ok(data)
    }

    /// Retrieve document from S3 (placeholder)
    #[instrument(skip(self))]
    async fn retrieve_from_s3(&self, _storage_path: &str) -> ApiResult<Vec<u8>> {
        error!("S3 retrieval is not yet implemented");
        Err(ApiError::Configuration(
            "S3 storage backend is not yet implemented".into(),
        ))
    }

    /// Delete document from storage
    #[instrument(skip(self))]
    pub async fn delete_document(
        &self,
        storage_path: &str,
        storage_type: StorageBackend,
    ) -> ApiResult<()> {
        match storage_type {
            StorageBackend::Filesystem => self.delete_from_filesystem(storage_path).await,
            StorageBackend::S3 => self.delete_from_s3(storage_path).await,
        }
    }

    /// Delete document from filesystem
    #[instrument(skip(self))]
    async fn delete_from_filesystem(&self, storage_path: &str) -> ApiResult<()> {
        let base_path =
            self.config.filesystem_base_path.as_ref().ok_or_else(|| {
                ApiError::Configuration("Filesystem base path not configured".into())
            })?;

        let file_path = base_path.join(storage_path);

        // Security check: ensure the path is within the base directory
        let canonical_base = fs::canonicalize(base_path).await.map_err(|e| {
            error!("Failed to canonicalize base path: {}", e);
            ApiError::InternalError("Storage path configuration error".into())
        })?;

        let canonical_file = fs::canonicalize(&file_path).await.map_err(|e| {
            error!("File not found for deletion {}: {}", file_path.display(), e);
            return ApiError::NotFound("Document not found".into());
        })?;

        if !canonical_file.starts_with(&canonical_base) {
            error!(
                "Path traversal attempt detected during deletion: {}",
                storage_path
            );
            return Err(ApiError::ValidationError(
                "Invalid storage path".to_string(),
            ));
        }

        // Delete file
        fs::remove_file(&file_path).await.map_err(|e| {
            error!("Failed to delete file {}: {}", file_path.display(), e);
            ApiError::InternalError(format!("Failed to delete file: {}", e))
        })?;

        debug!("File deleted successfully: {}", file_path.display());
        Ok(())
    }

    /// Delete document from S3 (placeholder)
    #[instrument(skip(self))]
    async fn delete_from_s3(&self, _storage_path: &str) -> ApiResult<()> {
        error!("S3 deletion is not yet implemented");
        Err(ApiError::Configuration(
            "S3 storage backend is not yet implemented".into(),
        ))
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Parse MIME type from file extension
pub fn mime_from_extension(filename: &str) -> Option<String> {
    let path = Path::new(filename);
    let extension = path.extension()?.to_str()?.to_lowercase();

    match extension.as_str() {
        "pdf" => Some("application/pdf".to_string()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ),
        "doc" => Some("application/msword".to_string()),
        "xlsx" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        }
        "xls" => Some("application/vnd.ms-excel".to_string()),
        "txt" => Some("text/plain".to_string()),
        "csv" => Some("text/csv".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_from_extension() {
        assert_eq!(
            mime_from_extension("document.pdf"),
            Some("application/pdf".to_string())
        );
        assert_eq!(
            mime_from_extension("report.docx"),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string()
            )
        );
        assert_eq!(
            mime_from_extension("data.xlsx"),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        );
        assert_eq!(mime_from_extension("unknown.xyz"), None);
    }

    #[test]
    fn test_calculate_checksum() {
        let data = b"Hello, World!";
        let checksum = DocumentStorageService::calculate_checksum(data);
        assert_eq!(
            checksum,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_get_extension_from_mime() {
        assert_eq!(
            DocumentStorageService::get_extension_from_mime("application/pdf"),
            ".pdf"
        );
        assert_eq!(
            DocumentStorageService::get_extension_from_mime(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            ),
            ".docx"
        );
        assert_eq!(
            DocumentStorageService::get_extension_from_mime("text/plain"),
            ".txt"
        );
        assert_eq!(
            DocumentStorageService::get_extension_from_mime("unknown/type"),
            ""
        );
    }
}
