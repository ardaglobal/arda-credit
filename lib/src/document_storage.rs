//! AWS S3 document storage integration with encryption and security
//!
//! This module provides secure document storage using AWS S3 with company-specific
//! encryption, bucket organization, and lifecycle policies.

use aws_config::{BehaviorVersion, Region};
use aws_sdk_kms::Client as KmsClient;
use aws_sdk_s3::{
    operation::get_object::GetObjectOutput,
    presigning::PresigningConfigError,
    primitives::ByteStream,
    types::{
        BucketLifecycleConfiguration, Delete, ExpirationStatus, LifecycleRule, ObjectIdentifier,
        ServerSideEncryption, Transition, TransitionStorageClass,
    },
    Client as S3Client,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Supported document file types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportedFileType {
    Pdf,
    Doc,
    Docx,
    Csv,
}

impl SupportedFileType {
    /// Get MIME type for the file type
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Doc => "application/msword",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Csv => "text/csv",
        }
    }

    /// Get file extension
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Csv => "csv",
        }
    }

    /// Parse from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "doc" => Some(Self::Doc),
            "docx" => Some(Self::Docx),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }

    /// Parse from MIME type
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        match mime_type {
            "application/pdf" => Some(Self::Pdf),
            "application/msword" => Some(Self::Doc),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(Self::Docx)
            }
            "text/csv" => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Document storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub bucket_name: String,
    pub region: String,
    pub kms_key_id: Option<String>,
    pub max_file_size_mb: u64,
    pub lifecycle_days_to_ia: i32, // Days to move to Infrequent Access
    pub lifecycle_days_to_glacier: i32, // Days to move to Glacier
    pub lifecycle_days_to_delete: i32, // Days to delete (for non-permanent docs)
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            bucket_name: "arda-credit-documents".to_string(),
            region: "us-east-1".to_string(),
            kms_key_id: None,
            max_file_size_mb: 100, // 100MB max file size
            lifecycle_days_to_ia: 30,
            lifecycle_days_to_glacier: 90,
            lifecycle_days_to_delete: 2555, // ~7 years
        }
    }
}

/// S3 storage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Metadata {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<String>,
    pub etag: String,
    pub content_type: String,
    pub content_length: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub server_side_encryption: Option<String>,
    pub kms_key_id: Option<String>,
}

/// Document upload request
#[derive(Debug, Clone)]
pub struct DocumentUploadRequest {
    pub company_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub document_type: String,
    pub filename: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub metadata: HashMap<String, String>,
}

/// Document download response
#[derive(Debug)]
pub struct DocumentDownloadResponse {
    pub content: Vec<u8>,
    pub content_type: String,
    pub filename: String,
    pub metadata: S3Metadata,
}

/// Errors for document storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("AWS S3 error: {0}")]
    S3Error(#[from] aws_sdk_s3::Error),

    #[error("AWS KMS error: {0}")]
    KmsError(#[from] aws_sdk_kms::Error),
    #[error("AWS KMS build error: {0}")]
    KmsBuildError(#[from] aws_sdk_kms::error::BuildError),
    #[error("AWS S3 presigning config error: {0}")]
    PresigningError(#[from] PresigningConfigError),
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Invalid file type: {file_type}. Supported types: PDF, DOC, DOCX, CSV")]
    InvalidFileType { file_type: String },

    #[error("File size exceeds limit: {size_mb}MB (max: {max_size_mb}MB)")]
    FileSizeExceeded { size_mb: u64, max_size_mb: u64 },

    #[error("Document not found: bucket={bucket}, key={key}")]
    DocumentNotFound { bucket: String, key: String },
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Access denied: {operation} on {resource}")]
    AccessDenied { operation: String, resource: String },

    #[error("Encryption error: {message}")]
    EncryptionError { message: String },

    #[error("Invalid bucket configuration: {message}")]
    BucketConfigError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// AWS S3 document storage client
pub struct DocumentStorageClient {
    s3_client: S3Client,
    kms_client: Option<KmsClient>,
    config: StorageConfig,
}

impl DocumentStorageClient {
    /// Create a new document storage client
    pub async fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .load()
            .await;

        let s3_client = S3Client::new(&aws_config);
        let kms_client = if config.kms_key_id.is_some() {
            Some(KmsClient::new(&aws_config))
        } else {
            None
        };

        Ok(Self {
            s3_client,
            kms_client,
            config,
        })
    }

    /// Generate S3 key for document storage
    /// Format: {company_id}/{deal_id}/{document_type}/{document_id}.{extension}
    fn generate_s3_key(
        &self,
        company_id: &Uuid,
        deal_id: Option<&Uuid>,
        document_type: &str,
        document_id: &Uuid,
        file_extension: &str,
    ) -> String {
        match deal_id {
            Some(deal_id) => format!(
                "{}/{}/{}/{}.{}",
                company_id, deal_id, document_type, document_id, file_extension
            ),
            None => format!(
                "{}/general/{}/{}.{}",
                company_id, document_type, document_id, file_extension
            ),
        }
    }

    /// Validate file type and size
    fn validate_file(
        &self,
        filename: &str,
        content: &[u8],
    ) -> Result<SupportedFileType, StorageError> {
        // Extract file extension
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Validate file type
        let file_type = SupportedFileType::from_extension(extension).ok_or_else(|| {
            StorageError::InvalidFileType {
                file_type: extension.to_string(),
            }
        })?;

        // Validate file size
        let size_mb = content.len() as u64 / (1024 * 1024);
        if size_mb > self.config.max_file_size_mb {
            return Err(StorageError::FileSizeExceeded {
                size_mb,
                max_size_mb: self.config.max_file_size_mb,
            });
        }

        Ok(file_type)
    }

    /// Upload document to S3 with encryption
    pub async fn upload_document(
        &self,
        document_id: Uuid,
        request: DocumentUploadRequest,
    ) -> Result<S3Metadata, StorageError> {
        // Validate file
        let file_type = self.validate_file(&request.filename, &request.content)?;

        // Generate S3 key
        let s3_key = self.generate_s3_key(
            &request.company_id,
            request.deal_id.as_ref(),
            &request.document_type,
            &document_id,
            file_type.extension(),
        );

        // Prepare metadata
        let mut s3_metadata = HashMap::new();
        s3_metadata.insert("company-id".to_string(), request.company_id.to_string());
        s3_metadata.insert("document-type".to_string(), request.document_type.clone());
        s3_metadata.insert("original-filename".to_string(), request.filename.clone());
        s3_metadata.insert("document-id".to_string(), document_id.to_string());

        if let Some(deal_id) = request.deal_id {
            s3_metadata.insert("deal-id".to_string(), deal_id.to_string());
        }

        // Add custom metadata
        for (key, value) in request.metadata {
            s3_metadata.insert(format!("custom-{}", key), value);
        }

        // Store values before move
        let content_length = request.content.len() as u64;
        let content_type = request.content_type.clone();

        // Create put object request
        let mut put_request = self
            .s3_client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(&s3_key)
            .body(ByteStream::from(request.content))
            .content_type(&request.content_type)
            .set_metadata(Some(s3_metadata));

        // Add encryption if KMS key is configured
        if let Some(kms_key_id) = &self.config.kms_key_id {
            put_request = put_request
                .server_side_encryption(ServerSideEncryption::AwsKms)
                .ssekms_key_id(kms_key_id);
        } else {
            put_request = put_request.server_side_encryption(ServerSideEncryption::Aes256);
        }

        // Execute upload
        let response = put_request
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        // Return metadata
        Ok(S3Metadata {
            bucket: self.config.bucket_name.clone(),
            key: s3_key,
            version_id: response.version_id,
            etag: response.e_tag.unwrap_or_default(),
            content_type,
            content_length,
            last_modified: Some(chrono::Utc::now()),
            server_side_encryption: response.server_side_encryption.map(|sse| sse.to_string()),
            kms_key_id: self.config.kms_key_id.clone(),
        })
    }

    /// Download document from S3
    pub async fn download_document(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<DocumentDownloadResponse, StorageError> {
        // Create get object request
        let mut get_request = self.s3_client.get_object().bucket(bucket).key(key);

        if let Some(version_id) = version_id {
            get_request = get_request.version_id(version_id);
        }

        // Execute download
        let response: GetObjectOutput = get_request
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        // Read content
        let content = response
            .body
            .collect()
            .await
            .map_err(|e| StorageError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            .into_bytes()
            .to_vec();

        // Extract filename from metadata or key
        let filename = response
            .metadata
            .as_ref()
            .and_then(|m| m.get("original-filename"))
            .cloned()
            .unwrap_or_else(|| {
                std::path::Path::new(key)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("document")
                    .to_string()
            });

        // Store content_type before first use
        let content_type = response.content_type.unwrap_or_default();

        Ok(DocumentDownloadResponse {
            content,
            content_type: content_type.clone(),
            filename,
            metadata: S3Metadata {
                bucket: bucket.to_string(),
                key: key.to_string(),
                version_id: response.version_id,
                etag: response.e_tag.unwrap_or_default(),
                content_type,
                content_length: response.content_length.unwrap_or(0) as u64,
                last_modified: response.last_modified.map(|dt| {
                    chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())
                        .unwrap_or_default()
                }),
                server_side_encryption: response.server_side_encryption.map(|sse| sse.to_string()),
                kms_key_id: response.ssekms_key_id,
            },
        })
    }

    /// Delete document from S3
    pub async fn delete_document(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<(), StorageError> {
        let mut delete_request = self.s3_client.delete_object().bucket(bucket).key(key);

        if let Some(version_id) = version_id {
            delete_request = delete_request.version_id(version_id);
        }

        delete_request
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        Ok(())
    }

    /// Delete multiple documents (batch delete)
    pub async fn delete_documents(
        &self,
        bucket: &str,
        keys: Vec<(String, Option<String>)>, // (key, version_id)
    ) -> Result<(), StorageError> {
        if keys.is_empty() {
            return Ok(());
        }

        let objects: Vec<ObjectIdentifier> = keys
            .into_iter()
            .map(|(key, version_id)| {
                let mut obj = ObjectIdentifier::builder().key(key);
                if let Some(version_id) = version_id {
                    obj = obj.version_id(version_id);
                }
                obj.build().unwrap()
            })
            .collect();

        let delete = Delete::builder().set_objects(Some(objects)).build()?;

        self.s3_client
            .delete_objects()
            .bucket(bucket)
            .delete(delete)
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        Ok(())
    }

    /// Check if document exists
    pub async fn document_exists(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<&str>,
    ) -> Result<bool, StorageError> {
        let mut head_request = self.s3_client.head_object().bucket(bucket).key(key);

        if let Some(version_id) = version_id {
            head_request = head_request.version_id(version_id);
        }

        match head_request.send().await {
            Ok(_) => Ok(true),
            Err(err) => {
                if err.to_string().contains("NotFound") {
                    Ok(false)
                } else {
                    Err(StorageError::S3Error(err.into()))
                }
            }
        }
    }

    /// Generate presigned URL for secure document access
    pub async fn generate_presigned_url(
        &self,
        bucket: &str,
        key: &str,
        expires_in_seconds: u64,
        operation: PresignedOperation,
    ) -> Result<String, StorageError> {
        let expires_in = std::time::Duration::from_secs(expires_in_seconds);

        match operation {
            PresignedOperation::GetObject => {
                let presigned_request = self
                    .s3_client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                        expires_in,
                    )?)
                    .await
                    .map_err(|e| StorageError::S3Error(e.into()))?;

                Ok(presigned_request.uri().to_string())
            }
            PresignedOperation::PutObject => {
                let presigned_request = self
                    .s3_client
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                        expires_in,
                    )?)
                    .await
                    .map_err(|e| StorageError::S3Error(e.into()))?;

                Ok(presigned_request.uri().to_string())
            }
        }
    }

    /// Setup bucket lifecycle configuration
    pub async fn setup_bucket_lifecycle(&self) -> Result<(), StorageError> {
        let lifecycle_rule = LifecycleRule::builder()
            .id("arda-document-lifecycle")
            .status(ExpirationStatus::Enabled)
            .transitions(
                Transition::builder()
                    .days(self.config.lifecycle_days_to_ia)
                    .storage_class(TransitionStorageClass::StandardIa)
                    .build(),
            )
            .transitions(
                Transition::builder()
                    .days(self.config.lifecycle_days_to_glacier)
                    .storage_class(TransitionStorageClass::Glacier)
                    .build(),
            )
            .build()?;

        let lifecycle_config = BucketLifecycleConfiguration::builder()
            .rules(lifecycle_rule)
            .build()
            .map_err(|_| {
                StorageError::ConfigurationError(
                    "Failed to build lifecycle configuration".to_string(),
                )
            })?;

        self.s3_client
            .put_bucket_lifecycle_configuration()
            .bucket(&self.config.bucket_name)
            .lifecycle_configuration(lifecycle_config)
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        Ok(())
    }

    /// List documents for a company
    pub async fn list_company_documents(
        &self,
        company_id: &Uuid,
        prefix: Option<&str>,
        max_keys: Option<i32>,
    ) -> Result<Vec<String>, StorageError> {
        let company_prefix = match prefix {
            Some(p) => format!("{}/{}", company_id, p),
            None => company_id.to_string(),
        };

        let mut list_request = self
            .s3_client
            .list_objects_v2()
            .bucket(&self.config.bucket_name)
            .prefix(&company_prefix);

        if let Some(max_keys) = max_keys {
            list_request = list_request.max_keys(max_keys);
        }

        let response = list_request
            .send()
            .await
            .map_err(|e| StorageError::S3Error(e.into()))?;

        let keys = response
            .contents
            .unwrap_or_default()
            .into_iter()
            .filter_map(|obj| obj.key)
            .collect();

        Ok(keys)
    }

    /// Get storage configuration
    #[must_use]
    pub const fn config(&self) -> &StorageConfig {
        &self.config
    }
}

/// Presigned URL operation types
#[derive(Debug, Clone, Copy)]
pub enum PresignedOperation {
    GetObject,
    PutObject,
}

/// Document watermarking utility
pub struct DocumentWatermark {
    company_name: String,
    user_email: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl DocumentWatermark {
    /// Create a new document watermark
    #[must_use]
    pub fn new(company_name: String, user_email: String) -> Self {
        Self {
            company_name,
            user_email,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Generate watermark text
    #[must_use]
    pub fn generate_text(&self) -> String {
        format!(
            "CONFIDENTIAL - {} - {} - {}",
            self.company_name,
            self.user_email,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    /// Generate watermark metadata
    #[must_use]
    pub fn generate_metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("watermark-company".to_string(), self.company_name.clone());
        metadata.insert("watermark-user".to_string(), self.user_email.clone());
        metadata.insert(
            "watermark-timestamp".to_string(),
            self.timestamp.to_rfc3339(),
        );
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_file_type_validation() {
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
        assert_eq!(SupportedFileType::from_extension("txt"), None);

        assert_eq!(
            SupportedFileType::from_mime_type("application/pdf"),
            Some(SupportedFileType::Pdf)
        );
        assert_eq!(SupportedFileType::from_mime_type("text/plain"), None);
    }

    #[test]
    fn test_storage_config_defaults() {
        let config = StorageConfig::default();
        assert_eq!(config.bucket_name, "arda-credit-documents");
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.max_file_size_mb, 100);
        assert_eq!(config.lifecycle_days_to_ia, 30);
        assert_eq!(config.lifecycle_days_to_glacier, 90);
        assert_eq!(config.lifecycle_days_to_delete, 2555);
    }

    #[test]
    fn test_document_watermark() {
        let watermark =
            DocumentWatermark::new("Test Company".to_string(), "user@example.com".to_string());

        let text = watermark.generate_text();
        assert!(text.contains("CONFIDENTIAL"));
        assert!(text.contains("Test Company"));
        assert!(text.contains("user@example.com"));

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

    #[tokio::test]
    async fn test_s3_key_generation() {
        let config = StorageConfig::default();
        let client = DocumentStorageClient {
            s3_client: S3Client::from_conf(
                aws_sdk_s3::Config::builder()
                    .region(Region::new("us-east-1"))
                    .build(),
            ),
            kms_client: None,
            config,
        };

        let company_id = Uuid::new_v4();
        let deal_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();

        // Test with deal ID
        let key_with_deal = client.generate_s3_key(
            &company_id,
            Some(&deal_id),
            "loan_contract",
            &document_id,
            "pdf",
        );
        assert_eq!(
            key_with_deal,
            format!(
                "{}/{}/loan_contract/{}.pdf",
                company_id, deal_id, document_id
            )
        );

        // Test without deal ID
        let key_without_deal =
            client.generate_s3_key(&company_id, None, "kyc_document", &document_id, "pdf");
        assert_eq!(
            key_without_deal,
            format!("{}/general/kyc_document/{}.pdf", company_id, document_id)
        );
    }
}
