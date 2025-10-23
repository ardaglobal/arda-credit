use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Custom serde module for serializing UUID as string
mod uuid_as_string {
    use super::*;
    use serde::de::{self, Visitor};
    use std::fmt;

    pub fn serialize<S>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&uuid.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UuidStringVisitor;

        impl<'de> Visitor<'de> for UuidStringVisitor {
            type Value = Uuid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a UUID string or SurrealDB Id")
            }

            fn visit_str<E>(self, value: &str) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                Uuid::parse_str(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                Uuid::parse_str(&value).map_err(E::custom)
            }

            // Handle SurrealDB's native types by converting them to strings first
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Uuid, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                // Try to deserialize as a string
                String::deserialize(deserializer)
                    .and_then(|s| Uuid::parse_str(&s).map_err(de::Error::custom))
            }

            // Handle other types by trying to convert to string
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                let s = std::str::from_utf8(v).map_err(E::custom)?;
                Uuid::parse_str(s).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(UuidStringVisitor)
    }
}

// Custom serde module for Option<UUID> as Option<String>
mod option_uuid_as_string {
    use super::*;
    use serde::de::{self, Visitor};
    use std::fmt;

    pub fn serialize<S>(opt_uuid: &Option<Uuid>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_uuid {
            Some(uuid) => serializer.serialize_some(&uuid.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptUuidStringVisitor;

        impl<'de> Visitor<'de> for OptUuidStringVisitor {
            type Value = Option<Uuid>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an optional UUID string or null")
            }

            fn visit_none<E>(self) -> Result<Option<Uuid>, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_unit<E>(self) -> Result<Option<Uuid>, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Option<Uuid>, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct InnerVisitor;

                impl<'de> Visitor<'de> for InnerVisitor {
                    type Value = Uuid;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a UUID string or SurrealDB Id")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Uuid, E>
                    where
                        E: de::Error,
                    {
                        Uuid::parse_str(value).map_err(E::custom)
                    }

                    fn visit_string<E>(self, value: String) -> Result<Uuid, E>
                    where
                        E: de::Error,
                    {
                        Uuid::parse_str(&value).map_err(E::custom)
                    }

                    // Handle SurrealDB's native types
                    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Uuid, D::Error>
                    where
                        D: serde::Deserializer<'de>,
                    {
                        String::deserialize(deserializer)
                            .and_then(|s| Uuid::parse_str(&s).map_err(de::Error::custom))
                    }

                    fn visit_bytes<E>(self, v: &[u8]) -> Result<Uuid, E>
                    where
                        E: de::Error,
                    {
                        let s = std::str::from_utf8(v).map_err(E::custom)?;
                        Uuid::parse_str(s).map_err(E::custom)
                    }
                }

                deserializer.deserialize_any(InnerVisitor).map(Some)
            }

            fn visit_str<E>(self, value: &str) -> Result<Option<Uuid>, E>
            where
                E: de::Error,
            {
                Uuid::parse_str(value).map(Some).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Option<Uuid>, E>
            where
                E: de::Error,
            {
                Uuid::parse_str(&value).map(Some).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(OptUuidStringVisitor)
    }
}

// Custom serde module for extracting UUID from SurrealDB record ID
// Handles both direct UUID strings and record IDs like "document_embeddings:⟨uuid⟩"
mod uuid_from_record_id {
    use super::*;
    use serde::de::{self, Visitor};
    use std::fmt;

    pub fn serialize<S>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&uuid.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UuidVisitor;

        impl<'de> Visitor<'de> for UuidVisitor {
            type Value = Uuid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a UUID string or SurrealDB record ID")
            }

            fn visit_str<E>(self, value: &str) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                self.parse_uuid_from_string(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                self.parse_uuid_from_string(&value)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Uuid, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut id_value: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    if key == "id" {
                        id_value = Some(map.next_value()?);
                        break;
                    } else {
                        // Skip other fields
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }

                if let Some(id_str) = id_value {
                    self.parse_uuid_from_string(&id_str)
                } else {
                    Err(de::Error::custom("Record object missing 'id' field"))
                }
            }

            // Handle SurrealDB's native types
            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Uuid, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                String::deserialize(deserializer).and_then(|s| self.parse_uuid_from_string(&s))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                let s = std::str::from_utf8(v).map_err(E::custom)?;
                self.parse_uuid_from_string(s)
            }
        }

        impl UuidVisitor {
            fn parse_uuid_from_string<E>(&self, s: &str) -> Result<Uuid, E>
            where
                E: de::Error,
            {
                // Try to parse as UUID first
                if let Ok(uuid) = Uuid::parse_str(s) {
                    return Ok(uuid);
                }

                // If that fails, try to extract UUID from record ID format
                // Format: "table_name:⟨uuid⟩" or "table_name:uuid"
                if let Some(uuid_part) = s.split(':').nth(1) {
                    let uuid_str = uuid_part.trim_start_matches('⟨').trim_end_matches('⟩');
                    Uuid::parse_str(uuid_str).map_err(E::custom)
                } else {
                    Err(E::custom(format!("Invalid record ID format: {}", s)))
                }
            }
        }

        deserializer.deserialize_any(UuidVisitor)
    }
}

/// Document metadata stored in SurrealDB
/// Note: UUID fields are serialized as strings for SurrealDB compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(with = "uuid_as_string")]
    pub document_id: Uuid,
    #[serde(with = "uuid_as_string")]
    pub company_id: Uuid,
    #[serde(with = "option_uuid_as_string")]
    pub deal_id: Option<Uuid>,
    pub filename: String,
    pub document_type: String,
    pub file_size: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>, // Path to file in storage (relative to base path) - optional for backward compatibility
    #[serde(with = "option_uuid_as_string")]
    pub user_id: Option<Uuid>,
    pub upload_timestamp: DateTime<Utc>,
    pub status: String, // 'uploaded', 'queued', 'processing', 'completed', 'failed'
    pub content: String,
    pub page_count: Option<i32>,
    pub processing_progress: Option<f32>, // 0.0 to 1.0
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub processing_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Document embedding chunk with vector data
/// Note: UUID fields are serialized as strings for SurrealDB compatibility
/// The embedding_id is extracted from the SurrealDB record ID (aliased as "id")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEmbedding {
    #[serde(with = "uuid_from_record_id", alias = "id")]
    pub embedding_id: Uuid,
    #[serde(with = "uuid_as_string")]
    pub document_id: Uuid,
    #[serde(with = "uuid_as_string")]
    pub company_id: Uuid,
    #[serde(with = "option_uuid_as_string")]
    pub deal_id: Option<Uuid>,
    #[serde(with = "option_uuid_as_string")]
    pub user_id: Option<Uuid>,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub chunk_position: usize,
    pub page_number: Option<i32>,
    pub embedding: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

/// Search result from SurrealDB vector search
/// Note: UUID fields are deserialized from strings
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    #[serde(with = "uuid_as_string")]
    pub embedding_id: Uuid,
    #[serde(with = "uuid_as_string")]
    pub document_id: Uuid,
    #[serde(with = "uuid_as_string")]
    pub company_id: Uuid,
    #[serde(with = "option_uuid_as_string")]
    pub deal_id: Option<Uuid>,
    pub score: f32,
    pub chunk_text: String,
    pub chunk_index: usize,
    pub chunk_position: usize,
    pub page_number: Option<i32>,
}

/// SurrealDB Service for document storage and vector search
///
/// Provides:
/// - Document management (CRUD)
/// - Document embedding storage
/// - Vector similarity search with MTREE index
/// - Multi-tenant support via company_id
pub struct SurrealDBService {
    db: Surreal<Any>,
    namespace: String,
    database: String,
}

impl SurrealDBService {
    /// Create a new SurrealDB service from environment variables
    ///
    /// Required env vars:
    /// - SURREALDB_URL (e.g., wss://cloud.surrealdb.com or ws://localhost:8000/rpc)
    /// - SURREALDB_USERNAME (default: root)
    /// - SURREALDB_PASSWORD
    /// - SURREALDB_NAMESPACE (default: arda)
    /// - SURREALDB_DATABASE (default: credit)
    pub async fn from_env() -> Result<Self> {
        let url = std::env::var("SURREALDB_URL")
            .unwrap_or_else(|_| "wss://cloud.surrealdb.com".to_string());
        let username = std::env::var("SURREALDB_USERNAME").unwrap_or_else(|_| "root".to_string());
        let password = std::env::var("SURREALDB_PASSWORD")
            .context("SURREALDB_PASSWORD environment variable is required")?;
        let namespace = std::env::var("SURREALDB_NAMESPACE").unwrap_or_else(|_| "arda".to_string());
        let database = std::env::var("SURREALDB_DATABASE").unwrap_or_else(|_| "credit".to_string());

        info!(
            "Connecting to SurrealDB at {} (namespace: {}, database: {})",
            url, namespace, database
        );

        Self::new(&url, &username, &password, &namespace, &database).await
    }

    /// Create a new SurrealDB service with explicit configuration
    pub async fn new(
        url: &str,
        username: &str,
        password: &str,
        namespace: &str,
        database: &str,
    ) -> Result<Self> {
        // Install default crypto provider for rustls (required for WSS connections)
        // This is safe to call multiple times - it will only install once
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        // Connect to SurrealDB using the `any` engine (supports ws:// and wss://)
        let db = surrealdb::engine::any::connect(url)
            .await
            .context("Failed to connect to SurrealDB")?;

        // Select namespace and database
        db.use_ns(namespace)
            .use_db(database)
            .await
            .context("Failed to select namespace and database")?;

        // Sign in with credentials
        db.signin(Root { username, password })
            .await
            .context("Failed to sign in to SurrealDB")?;

        info!("Successfully connected to SurrealDB");

        Ok(Self {
            db,
            namespace: namespace.to_string(),
            database: database.to_string(),
        })
    }

    /// Initialize the schema (create tables and indexes)
    ///
    /// This should be called during application startup.
    pub async fn init_schema(&self) -> Result<()> {
        info!("Initializing SurrealDB schema");

        // Load and execute the schema file
        let schema = include_str!("surrealdb_schema.surql");

        // Execute schema queries (split by semicolon and execute each)
        for query in schema.split(';').filter(|s| !s.trim().is_empty()) {
            self.db
                .query(query.trim())
                .await
                .context(format!("Failed to execute schema query: {}", query))?;
        }

        info!("SurrealDB schema initialized successfully");
        Ok(())
    }

    /// Execute a raw SurrealQL query
    ///
    /// This is useful for admin operations, schema migrations, and debugging.
    /// Returns the raw response which can be parsed as needed.
    pub async fn execute_query(&self, query: &str) -> Result<surrealdb::Response> {
        self.db
            .query(query)
            .await
            .context("Failed to execute query")
    }

    // ====================
    // Document CRUD Operations
    // ====================

    /// Create a new document
    pub async fn create_document(&self, document: Document) -> Result<Document> {
        debug!("Creating document: {}", document.document_id);

        // Use raw query with bindings to properly handle DateTime types
        // Use angle brackets for UUIDs with hyphens in SurrealDB
        // Use SurrealDB's time functions for datetime fields to avoid serialization issues
        let record_id = format!("documents:⟨{}⟩", document.document_id);

        // Convert DateTime values to RFC3339 strings for SurrealDB's time::parse()
        let upload_timestamp_str = document.upload_timestamp.to_rfc3339();
        let processing_started_str = document.processing_started_at.map(|dt| dt.to_rfc3339());
        let processing_completed_str = document.processing_completed_at.map(|dt| dt.to_rfc3339());

        let query = format!(
            r#"
            CREATE {} SET
                document_id = $document_id,
                company_id = $company_id,
                deal_id = $deal_id,
                filename = $filename,
                document_type = $document_type,
                file_size = $file_size,
                storage_path = $storage_path,
                user_id = $user_id,
                upload_timestamp = <datetime> $upload_timestamp,
                status = $status,
                content = $content,
                page_count = $page_count,
                processing_progress = $processing_progress,
                error_message = $error_message,
                retry_count = $retry_count,
                processing_started_at = IF $processing_started_at != NONE THEN <datetime> $processing_started_at ELSE NONE END,
                processing_completed_at = IF $processing_completed_at != NONE THEN <datetime> $processing_completed_at ELSE NONE END,
                created_at = time::now(),
                updated_at = time::now()
        "#,
            record_id
        );

        let mut result = self
            .db
            .query(query)
            .bind(("document_id", document.document_id.to_string()))
            .bind(("company_id", document.company_id.to_string()))
            .bind(("deal_id", document.deal_id.map(|id| id.to_string())))
            .bind(("filename", document.filename))
            .bind(("document_type", document.document_type))
            .bind(("file_size", document.file_size))
            .bind((
                "storage_path",
                document.storage_path.clone().unwrap_or_default(),
            ))
            .bind(("user_id", document.user_id.map(|id| id.to_string())))
            .bind(("upload_timestamp", upload_timestamp_str))
            .bind(("status", document.status))
            .bind(("content", document.content))
            .bind(("page_count", document.page_count))
            .bind(("processing_progress", document.processing_progress))
            .bind(("error_message", document.error_message))
            .bind(("retry_count", document.retry_count))
            .bind(("processing_started_at", processing_started_str))
            .bind(("processing_completed_at", processing_completed_str))
            .await
            .map_err(|e| {
                tracing::error!("SurrealDB create error: {:?}", e);
                anyhow::anyhow!("Failed to create document: {}", e)
            })?;

        let created: Vec<Document> = result.take(0).map_err(|e| {
            tracing::error!(
                "Failed to parse SurrealDB response for document {}: {:?}",
                document.document_id,
                e
            );
            anyhow::anyhow!("Failed to parse created document: {}", e)
        })?;

        debug!("SurrealDB returned {} documents", created.len());
        created
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to create document: no result returned"))
    }

    /// Get a document by ID
    pub async fn get_document(&self, document_id: Uuid) -> Result<Option<Document>> {
        debug!("Fetching document: {}", document_id);

        let document: Option<Document> = self
            .db
            .select(("documents", document_id.to_string()))
            .await
            .context("Failed to get document")?;

        Ok(document)
    }

    /// Update a document
    pub async fn update_document(&self, document: Document) -> Result<Document> {
        debug!("Updating document: {}", document.document_id);

        // Convert DateTime values to RFC3339 strings for SurrealDB's datetime casting
        let upload_timestamp_str = document.upload_timestamp.to_rfc3339();
        let processing_started_str = document.processing_started_at.map(|dt| dt.to_rfc3339());
        let processing_completed_str = document.processing_completed_at.map(|dt| dt.to_rfc3339());
        let created_at_str = document.created_at.to_rfc3339();

        // Build query with record ID directly in the query string
        let record_id = format!("documents:⟨{}⟩", document.document_id);
        let query = format!(
            r#"
            UPDATE {} SET
                company_id = $company_id,
                deal_id = $deal_id,
                filename = $filename,
                document_type = $document_type,
                file_size = $file_size,
                storage_path = $storage_path,
                user_id = $user_id,
                upload_timestamp = <datetime> $upload_timestamp,
                status = $status,
                content = $content,
                page_count = $page_count,
                processing_progress = $processing_progress,
                error_message = $error_message,
                retry_count = $retry_count,
                processing_started_at = IF $processing_started_at != NONE THEN <datetime> $processing_started_at ELSE NONE END,
                processing_completed_at = IF $processing_completed_at != NONE THEN <datetime> $processing_completed_at ELSE NONE END,
                created_at = <datetime> $created_at,
                updated_at = time::now()
            RETURN *
        "#,
            record_id
        );

        let mut result = self
            .db
            .query(query)
            .bind(("company_id", document.company_id.to_string()))
            .bind(("deal_id", document.deal_id.map(|id| id.to_string())))
            .bind(("filename", document.filename))
            .bind(("document_type", document.document_type))
            .bind(("file_size", document.file_size))
            .bind((
                "storage_path",
                document.storage_path.clone().unwrap_or_default(),
            ))
            .bind(("user_id", document.user_id.map(|id| id.to_string())))
            .bind(("upload_timestamp", upload_timestamp_str))
            .bind(("status", document.status))
            .bind(("content", document.content))
            .bind(("page_count", document.page_count))
            .bind(("processing_progress", document.processing_progress))
            .bind(("error_message", document.error_message))
            .bind(("retry_count", document.retry_count))
            .bind(("processing_started_at", processing_started_str))
            .bind(("processing_completed_at", processing_completed_str))
            .bind(("created_at", created_at_str))
            .await
            .context("Failed to update document")?;

        let updated: Vec<Document> = result.take(0).context("Failed to parse updated document")?;
        updated
            .into_iter()
            .next()
            .context("Failed to update document: no result returned")
    }

    /// Delete a document and all its embeddings
    pub async fn delete_document(&self, document_id: Uuid) -> Result<()> {
        info!("Deleting document and embeddings: {}", document_id);

        // Delete document
        let _: Option<Document> = self
            .db
            .delete(("documents", document_id.to_string()))
            .await
            .context("Failed to delete document")?;

        // Delete all embeddings for this document
        self.db
            .query("DELETE document_embeddings WHERE document_id = $document_id")
            .bind(("document_id", document_id.to_string()))
            .await
            .context("Failed to delete document embeddings")?;

        Ok(())
    }

    /// Get all documents for a company
    pub async fn get_documents_by_company(&self, company_id: Uuid) -> Result<Vec<Document>> {
        debug!("Fetching documents for company: {}", company_id);

        let mut result = self
            .db
            .query("SELECT * FROM documents WHERE company_id = $company_id")
            .bind(("company_id", company_id.to_string()))
            .await
            .context("Failed to query documents")?;

        let documents: Vec<Document> = result.take(0).context("Failed to parse documents")?;

        Ok(documents)
    }

    /// Get all documents for a deal
    pub async fn get_documents_by_deal(&self, deal_id: Uuid) -> Result<Vec<Document>> {
        debug!("Fetching documents for deal: {}", deal_id);

        let mut result = self
            .db
            .query("SELECT * FROM documents WHERE deal_id = $deal_id")
            .bind(("deal_id", deal_id.to_string()))
            .await
            .context("Failed to query documents")?;

        let documents: Vec<Document> = result.take(0).context("Failed to parse documents")?;

        Ok(documents)
    }

    /// Get all documents for a user
    pub async fn get_documents_by_user(&self, user_id: Uuid) -> Result<Vec<Document>> {
        debug!("Fetching documents for user: {}", user_id);

        let mut result = self
            .db
            .query("SELECT * FROM documents WHERE user_id = $user_id")
            .bind(("user_id", user_id.to_string()))
            .await
            .context("Failed to query documents")?;

        let documents: Vec<Document> = result.take(0).context("Failed to parse documents")?;

        Ok(documents)
    }

    // ====================
    // Job Queue Operations
    // ====================

    /// Get documents by status (for job queue polling)
    ///
    /// # Arguments
    /// * `status` - Status to filter by (e.g., "uploaded", "queued")
    /// * `limit` - Maximum number of documents to return
    ///
    /// # Returns
    /// * Vector of documents with the specified status, ordered by upload_timestamp
    pub async fn get_documents_by_status(
        &self,
        status: &str,
        limit: usize,
    ) -> Result<Vec<Document>> {
        debug!(
            "Fetching documents with status '{}' (limit: {})",
            status, limit
        );

        let status_owned = status.to_string();
        let query = format!(
            "SELECT * FROM documents WHERE status = $status ORDER BY upload_timestamp ASC LIMIT {}",
            limit
        );

        let mut result = self
            .db
            .query(query)
            .bind(("status", status_owned))
            .await
            .context("Failed to query documents by status")?;

        let documents: Vec<Document> = result.take(0).map_err(|e| {
            tracing::error!(
                "Failed to parse documents from SurrealDB (status='{}'): {:?}",
                status,
                e
            );
            tracing::error!(
                "This may indicate documents in SurrealDB don't have all required fields"
            );
            anyhow::anyhow!("Failed to parse documents: {}", e)
        })?;

        debug!(
            "Found {} documents with status '{}'",
            documents.len(),
            status
        );
        Ok(documents)
    }

    /// Update document status with timestamps
    ///
    /// # Arguments
    /// * `document_id` - Document ID
    /// * `status` - New status
    /// * `progress` - Optional progress percentage (0.0 to 1.0)
    /// * `error_message` - Optional error message
    pub async fn update_document_status(
        &self,
        document_id: Uuid,
        status: &str,
        progress: Option<f32>,
        error_message: Option<String>,
    ) -> Result<Document> {
        debug!("Updating document {} status to '{}'", document_id, status);

        let status_owned = status.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let record_id = format!("documents:⟨{}⟩", document_id);

        // Build query with record ID directly in the query string
        let mut query = format!(
            "UPDATE {} SET status = $status, updated_at = <datetime> $now",
            record_id
        );

        // Add processing timestamps based on status
        if status == "processing" {
            query.push_str(", processing_started_at = <datetime> $now");
        } else if status == "completed" || status == "failed" {
            query.push_str(", processing_completed_at = <datetime> $now");
        }

        // Add optional fields
        if let Some(prog) = progress {
            query.push_str(&format!(", processing_progress = {}", prog));
        }
        if error_message.is_some() {
            query.push_str(", error_message = $error_message");
        }

        // Add RETURN clause to get the updated document back
        query.push_str(" RETURN *");

        let mut db_query = self
            .db
            .query(query)
            .bind(("status", status_owned))
            .bind(("now", now_str));

        if let Some(err) = error_message {
            db_query = db_query.bind(("error_message", err));
        }

        let mut result = db_query.await.context("Failed to update document status")?;

        let updated: Vec<Document> = result.take(0).context("Failed to parse updated document")?;
        updated
            .into_iter()
            .next()
            .context("No document returned after update")
    }

    /// Increment retry count for a document
    pub async fn increment_retry_count(&self, document_id: Uuid) -> Result<i32> {
        debug!("Incrementing retry count for document {}", document_id);

        let record_id = format!("documents:⟨{}⟩", document_id);
        let query = format!(
            "UPDATE {} SET retry_count += 1, updated_at = time::now() RETURN retry_count",
            record_id
        );

        let mut result = self
            .db
            .query(query)
            .await
            .context("Failed to increment retry count")?;

        #[derive(Deserialize)]
        struct RetryResult {
            retry_count: i32,
        }

        let retry_result: Vec<RetryResult> =
            result.take(0).context("Failed to parse retry count")?;
        let retry_count = retry_result
            .into_iter()
            .next()
            .context("No retry count returned")?
            .retry_count;

        Ok(retry_count)
    }

    /// Mark document as processing (atomically check and update status)
    ///
    /// Returns true if the document was successfully claimed for processing
    pub async fn claim_document_for_processing(&self, document_id: Uuid) -> Result<bool> {
        // First, check current status for debugging using proper record ID syntax
        let check_query = "SELECT status FROM type::thing('documents', $document_id)";
        let mut check_result = self
            .db
            .query(check_query)
            .bind(("document_id", document_id.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct StatusCheck {
            status: String,
        }

        let current_status: Vec<StatusCheck> = check_result.take(0).unwrap_or_default();
        if let Some(status_obj) = current_status.first() {
            info!(
                "📊 Current status in SurrealDB for document {}: '{}'",
                document_id, status_obj.status
            );
        } else {
            warn!("⚠️  Document {} not found in SurrealDB!", document_id);
        }

        // Atomic update: only update if status is 'uploaded' or 'queued'
        // Use record ID format that works with UPDATE
        let record_id = format!("documents:⟨{}⟩", document_id);
        let query = format!(
            r#"
            UPDATE {} SET
                status = 'processing',
                processing_started_at = time::now(),
                updated_at = time::now()
            WHERE status IN ['uploaded', 'queued']
        "#,
            record_id
        );

        let mut result = self
            .db
            .query(query)
            .await
            .context("Failed to claim document for processing")?;

        let updated: Vec<Document> = result.take(0).unwrap_or_default();
        let claimed = !updated.is_empty();

        if claimed {
            info!(
                "✅ Successfully claimed document {} for processing (status changed to 'processing')",
                document_id
            );
        } else {
            info!(
                "❌ Failed to claim document {} - current status is NOT 'uploaded' or 'queued'",
                document_id
            );
            debug!(
                "Document {} already being processed or not available",
                document_id
            );
        }

        Ok(claimed)
    }

    // ====================
    // Document Embedding CRUD Operations
    // ====================

    /// Create a new document embedding
    pub async fn create_embedding(
        &self,
        embedding: DocumentEmbedding,
    ) -> Result<DocumentEmbedding> {
        debug!(
            "Creating embedding for document: {} (chunk {})",
            embedding.document_id, embedding.chunk_index
        );

        // Convert DateTime to RFC3339 string for SurrealDB compatibility
        let created_at_str = embedding.created_at.to_rfc3339();

        // Use raw query with bindings to properly handle DateTime types
        // Use angle brackets for UUIDs with hyphens in SurrealDB
        // Note: embedding_id is encoded in the record ID, so we don't set it as a field
        // RETURN AFTER to get the created record's ID as a simple string
        let record_id = format!("document_embeddings:⟨{}⟩", embedding.embedding_id);
        let query = format!(
            r#"
            CREATE {} SET
                document_id = $document_id,
                company_id = $company_id,
                deal_id = $deal_id,
                user_id = $user_id,
                chunk_index = $chunk_index,
                chunk_text = $chunk_text,
                chunk_position = $chunk_position,
                page_number = $page_number,
                embedding = $embedding,
                created_at = <datetime> $created_at
            RETURN {{
                id: string::split(meta::id(id), ':')[1],
                document_id: document_id,
                company_id: company_id,
                deal_id: deal_id,
                user_id: user_id,
                chunk_index: chunk_index,
                chunk_text: chunk_text,
                chunk_position: chunk_position,
                page_number: page_number,
                embedding: embedding,
                created_at: created_at
            }}
        "#,
            record_id
        );

        let mut result = self
            .db
            .query(query)
            .bind(("document_id", embedding.document_id.to_string()))
            .bind(("company_id", embedding.company_id.to_string()))
            .bind(("deal_id", embedding.deal_id.map(|id| id.to_string())))
            .bind(("user_id", embedding.user_id.map(|id| id.to_string())))
            .bind(("chunk_index", embedding.chunk_index))
            .bind(("chunk_text", embedding.chunk_text.clone()))
            .bind(("chunk_position", embedding.chunk_position))
            .bind(("page_number", embedding.page_number))
            .bind(("embedding", embedding.embedding.clone()))
            .bind(("created_at", created_at_str))
            .await
            .map_err(|e| {
                error!("Failed to create embedding in SurrealDB: {:#}", e);
                anyhow::anyhow!("Failed to create embedding: {:#}", e)
            })?;

        // Parse as JSON to handle SurrealDB's native types
        let raw_results: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!("Failed to parse created embedding: {:#}", e);
            anyhow::anyhow!("Failed to parse created embedding: {:#}", e)
        })?;

        debug!("Raw results from SurrealDB: {:?}", raw_results);

        let raw_json = raw_results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to create embedding: no result returned"))?;

        debug!(
            "Raw JSON: {}",
            serde_json::to_string_pretty(&raw_json).unwrap_or_default()
        );

        let obj = raw_json.as_object().ok_or_else(|| {
            error!("Response is not an object: {:?}", raw_json);
            anyhow::anyhow!("Response is not an object")
        })?;

        // Parse fields from JSON - handle id field being None or missing
        let embedding_id = if let Some(id_value) = obj.get("id") {
            debug!("Found id field: {:?}", id_value);
            if id_value.is_null() {
                debug!("id field is null, using original embedding_id");
                embedding.embedding_id
            } else if let Some(id_str) = id_value.as_str() {
                Uuid::parse_str(id_str).unwrap_or_else(|e| {
                    warn!("Failed to parse id '{}': {}, using original", id_str, e);
                    embedding.embedding_id
                })
            } else {
                debug!("id field is not a string: {:?}, using original", id_value);
                embedding.embedding_id
            }
        } else {
            debug!("id field not present, using original embedding_id");
            embedding.embedding_id
        };

        // If response is missing fields, it means RETURN clause didn't work
        // Fall back to using the original embedding data
        let document_id = obj
            .get("document_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| {
                debug!("document_id not in response, using original");
                embedding.document_id
            });

        let company_id = obj
            .get("company_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| {
                debug!("company_id not in response, using original");
                embedding.company_id
            });

        let deal_id = obj
            .get("deal_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .or(embedding.deal_id);

        let user_id = obj
            .get("user_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .or(embedding.user_id);

        let chunk_index = obj
            .get("chunk_index")
            .and_then(|v| v.as_i64())
            .map(|n| n as usize)
            .unwrap_or_else(|| {
                debug!("chunk_index not in response, using original");
                embedding.chunk_index
            });

        let chunk_text = obj
            .get("chunk_text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                debug!("chunk_text not in response, using original");
                embedding.chunk_text.clone()
            });

        let chunk_position = obj
            .get("chunk_position")
            .and_then(|v| v.as_i64())
            .map(|n| n as usize)
            .unwrap_or_else(|| {
                debug!("chunk_position not in response, using original");
                embedding.chunk_position
            });

        let page_number = obj
            .get("page_number")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32)
            .or(embedding.page_number);

        let embedding_vec = obj
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Vec<f32>>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                debug!("embedding not in response, using original");
                embedding.embedding.clone()
            });

        let created_at =
            if let Some(created_at_str) = obj.get("created_at").and_then(|v| v.as_str()) {
                DateTime::parse_from_rfc3339(created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|e| {
                        warn!(
                            "Failed to parse created_at '{}': {}, using original",
                            created_at_str, e
                        );
                        embedding.created_at
                    })
            } else {
                debug!("created_at not in response, using original");
                embedding.created_at
            };

        // Convert to DocumentEmbedding
        Ok(DocumentEmbedding {
            embedding_id,
            document_id,
            company_id,
            deal_id,
            user_id,
            chunk_index,
            chunk_text,
            chunk_position,
            page_number,
            embedding: embedding_vec,
            created_at,
        })
    }

    /// Create multiple embeddings in batch
    pub async fn create_embeddings_batch(
        &self,
        embeddings: Vec<DocumentEmbedding>,
    ) -> Result<Vec<DocumentEmbedding>> {
        info!("Creating {} embeddings in batch", embeddings.len());

        let mut created_embeddings = Vec::new();
        let total = embeddings.len();

        for (i, embedding) in embeddings.into_iter().enumerate() {
            let created = self.create_embedding(embedding).await.map_err(|e| {
                error!("Failed to create embedding {}/{}: {:#}", i + 1, total, e);
                e
            })?;
            created_embeddings.push(created);

            // Log progress every 100 embeddings
            if (i + 1) % 100 == 0 || i + 1 == total {
                debug!("Created {}/{} embeddings", i + 1, total);
            }
        }

        info!(
            "Successfully created {} embeddings",
            created_embeddings.len()
        );
        Ok(created_embeddings)
    }

    /// Get all embeddings for a document
    pub async fn get_embeddings_by_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentEmbedding>> {
        debug!("Fetching embeddings for document: {}", document_id);

        let mut result = self
            .db
            .query("SELECT * FROM document_embeddings WHERE document_id = $document_id ORDER BY chunk_index")
            .bind(("document_id", document_id.to_string()))
            .await
            .context("Failed to query embeddings")?;

        let embeddings: Vec<DocumentEmbedding> =
            result.take(0).context("Failed to parse embeddings")?;

        Ok(embeddings)
    }

    /// Delete all embeddings for a document
    pub async fn delete_embeddings_by_document(&self, document_id: Uuid) -> Result<()> {
        info!("Deleting embeddings for document: {}", document_id);

        self.db
            .query("DELETE document_embeddings WHERE document_id = $document_id")
            .bind(("document_id", document_id.to_string()))
            .await
            .context("Failed to delete embeddings")?;

        Ok(())
    }

    // ====================
    // Vector Search Operations
    // ====================

    /// Semantic search across all documents using vector similarity
    ///
    /// # Arguments
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// * Vector of search results sorted by relevance (highest score first)
    pub async fn search(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>> {
        self.search_by_company(None, query_vector, limit).await
    }

    /// Semantic search within a specific company's documents
    ///
    /// # Arguments
    /// * `company_id` - Optional company ID to scope the search to
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    pub async fn search_by_company(
        &self,
        company_id: Option<Uuid>,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        debug!(
            "Searching with vector (company: {:?}, limit: {})",
            company_id, limit
        );

        // Build query with optional company filter
        // Don't use VALUE - need to ORDER BY score, which requires score in the result set
        // Explicitly cast UUID fields to strings using <string> syntax
        // Filter out records with null required fields
        let query = if company_id.is_some() {
            format!(
                "SELECT
                 string::split(meta::id(id), ':')[1] AS embedding_id,
                 document_id,
                 company_id,
                 deal_id,
                 chunk_text,
                 chunk_index,
                 chunk_position,
                 page_number,
                 vector::similarity::cosine(embedding, $query_vector) AS score
                 FROM document_embeddings
                 WHERE company_id = $company_id
                 AND document_id != NONE
                 AND company_id != NONE
                 AND chunk_text != NONE
                 ORDER BY score DESC
                 LIMIT {}",
                limit
            )
        } else {
            format!(
                "SELECT
                 string::split(meta::id(id), ':')[1] AS embedding_id,
                 document_id,
                 company_id,
                 deal_id,
                 chunk_text,
                 chunk_index,
                 chunk_position,
                 page_number,
                 vector::similarity::cosine(embedding, $query_vector) AS score
                 FROM document_embeddings
                 WHERE document_id != NONE
                 AND company_id != NONE
                 AND chunk_text != NONE
                 ORDER BY score DESC
                 LIMIT {}",
                limit
            )
        };

        debug!("Executing search query: {}", query);
        debug!("Query vector dimensions: {}", query_vector.len());

        let mut result = if let Some(cid) = company_id {
            self.db
                .query(&query)
                .bind(("query_vector", query_vector))
                .bind(("company_id", cid.to_string()))
                .await
                .map_err(|e| {
                    error!("Search query execution failed: {:#}", e);
                    error!("Query was: {}", query);
                    anyhow::anyhow!("Failed to execute search query: {:#}", e)
                })?
        } else {
            self.db
                .query(&query)
                .bind(("query_vector", query_vector))
                .await
                .map_err(|e| {
                    error!("Search query execution failed: {:#}", e);
                    error!("Query was: {}", query);
                    anyhow::anyhow!("Failed to execute search query: {:#}", e)
                })?
        };

        // Parse results directly as JSON values
        let raw_results: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!("Failed to parse search results: {:#}", e);
            anyhow::anyhow!("Failed to parse search results: {:#}", e)
        })?;

        debug!("Parsed {} raw search results", raw_results.len());

        let search_results: Vec<SearchResult> = raw_results
            .into_iter()
            .filter_map(|json_val| {
                let obj = json_val.as_object()?;

                // embedding_id might be null, use a random UUID as placeholder
                let embedding_id = obj
                    .get("embedding_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or_else(Uuid::new_v4);

                // Parse required fields - skip record if any are missing
                let document_id = Uuid::parse_str(obj.get("document_id")?.as_str()?).ok()?;
                let company_id = Uuid::parse_str(obj.get("company_id")?.as_str()?).ok()?;
                let score = obj.get("score")?.as_f64()? as f32;
                let chunk_text = obj.get("chunk_text")?.as_str()?.to_string();
                let chunk_index = obj.get("chunk_index")?.as_i64()? as usize;
                let chunk_position = obj.get("chunk_position")?.as_i64()? as usize;

                Some(SearchResult {
                    embedding_id,
                    document_id,
                    company_id,
                    deal_id: obj
                        .get("deal_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    score,
                    chunk_text,
                    chunk_index,
                    chunk_position,
                    page_number: obj
                        .get("page_number")
                        .and_then(|v| v.as_i64())
                        .map(|n| n as i32),
                })
            })
            .collect();

        debug!("Found {} search results", search_results.len());
        Ok(search_results)
    }

    /// Search within a specific document
    pub async fn search_by_document(
        &self,
        document_id: Uuid,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        debug!("Searching in document {} with limit {}", document_id, limit);

        // Don't use VALUE - need to ORDER BY score, which requires score in the result set
        // Explicitly cast UUID fields to strings using <string> syntax
        // Filter out records with null required fields
        let query = format!(
            "SELECT
             string::split(meta::id(id), ':')[1] AS embedding_id,
             document_id,
             company_id,
             deal_id,
             chunk_text,
             chunk_index,
             chunk_position,
             page_number,
             vector::similarity::cosine(embedding, $query_vector) AS score
             FROM document_embeddings
             WHERE document_id = $document_id
             AND document_id != NONE
             AND company_id != NONE
             AND chunk_text != NONE
             ORDER BY score DESC
             LIMIT {}",
            limit
        );

        debug!("Executing document search query: {}", query);
        debug!("Query vector dimensions: {}", query_vector.len());

        let mut result = self
            .db
            .query(&query)
            .bind(("query_vector", query_vector))
            .bind(("document_id", document_id.to_string()))
            .await
            .map_err(|e| {
                error!("Search query execution failed: {:#}", e);
                error!("Query was: {}", query);
                anyhow::anyhow!("Failed to execute search query: {:#}", e)
            })?;

        // Parse results directly as JSON values
        let raw_results: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!("Failed to parse search results: {:#}", e);
            anyhow::anyhow!("Failed to parse search results: {:#}", e)
        })?;

        debug!("Parsed {} raw search results", raw_results.len());

        let search_results: Vec<SearchResult> = raw_results
            .into_iter()
            .filter_map(|json_val| {
                let obj = json_val.as_object()?;

                // embedding_id might be null, use a random UUID as placeholder
                let embedding_id = obj
                    .get("embedding_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or_else(Uuid::new_v4);

                // Parse required fields - skip record if any are missing
                let document_id = Uuid::parse_str(obj.get("document_id")?.as_str()?).ok()?;
                let company_id = Uuid::parse_str(obj.get("company_id")?.as_str()?).ok()?;
                let score = obj.get("score")?.as_f64()? as f32;
                let chunk_text = obj.get("chunk_text")?.as_str()?.to_string();
                let chunk_index = obj.get("chunk_index")?.as_i64()? as usize;
                let chunk_position = obj.get("chunk_position")?.as_i64()? as usize;

                Some(SearchResult {
                    embedding_id,
                    document_id,
                    company_id,
                    deal_id: obj
                        .get("deal_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    score,
                    chunk_text,
                    chunk_index,
                    chunk_position,
                    page_number: obj
                        .get("page_number")
                        .and_then(|v| v.as_i64())
                        .map(|n| n as i32),
                })
            })
            .collect();

        debug!("Found {} search results", search_results.len());
        Ok(search_results)
    }

    /// Delete all documents and embeddings for a company
    pub async fn delete_company_data(&self, company_id: Uuid) -> Result<()> {
        info!("Deleting all data for company: {}", company_id);

        // Delete all embeddings
        self.db
            .query("DELETE document_embeddings WHERE company_id = $company_id")
            .bind(("company_id", company_id.to_string()))
            .await
            .context("Failed to delete company embeddings")?;

        // Delete all documents
        self.db
            .query("DELETE documents WHERE company_id = $company_id")
            .bind(("company_id", company_id.to_string()))
            .await
            .context("Failed to delete company documents")?;

        Ok(())
    }

    /// Health check - verify SurrealDB connection
    pub async fn health_check(&self) -> Result<bool> {
        debug!("Performing health check");

        // Try to query the database
        let result = self.db.query("SELECT * FROM documents LIMIT 1").await;

        match result {
            Ok(_) => {
                debug!("Health check passed");
                Ok(true)
            }
            Err(e) => {
                warn!("Health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_serialization() {
        let document = Document {
            document_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            deal_id: Some(Uuid::new_v4()),
            filename: "test.pdf".to_string(),
            document_type: "credit_agreement".to_string(),
            file_size: 1024,
            storage_path: Some("path/to/file.pdf".to_string()),
            user_id: Some(Uuid::new_v4()),
            upload_timestamp: Utc::now(),
            status: "completed".to_string(),
            content: "Test content".to_string(),
            page_count: Some(10),
            processing_progress: Some(1.0),
            error_message: None,
            retry_count: 0,
            processing_started_at: Some(Utc::now()),
            processing_completed_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&document).expect("Serialization failed");
        let deserialized: Document = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(document.document_id, deserialized.document_id);
        assert_eq!(document.filename, deserialized.filename);
    }

    #[test]
    fn test_embedding_serialization() {
        let embedding = DocumentEmbedding {
            embedding_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            deal_id: Some(Uuid::new_v4()),
            user_id: Some(Uuid::new_v4()),
            chunk_index: 0,
            chunk_text: "Test chunk".to_string(),
            chunk_position: 0,
            page_number: Some(1),
            embedding: vec![0.1, 0.2, 0.3],
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&embedding).expect("Serialization failed");
        let deserialized: DocumentEmbedding =
            serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(embedding.embedding_id, deserialized.embedding_id);
        assert_eq!(embedding.chunk_text, deserialized.chunk_text);
    }
}
